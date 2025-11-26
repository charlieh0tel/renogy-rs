//! Serial/RS-485 transport for Modbus RTU communication.
//!
//! This module provides a serial transport that implements the `Transport` trait,
//! using `tokio-modbus` for the underlying Modbus RTU communication.

use crate::error::{RenogyError, Result};
use crate::pdu::{FunctionCode, Pdu};
use crate::transport::Transport;
use std::io::{Error as IoError, ErrorKind};
use tokio_modbus::client::{Client, Context, Reader, Writer};
use tokio_modbus::slave::{Slave, SlaveContext};
use tokio_serial::SerialPortBuilderExt;

/// Default baud rate for Renogy BMS communication
pub const DEFAULT_BAUD_RATE: u32 = 9600;

/// Serial transport for Modbus RTU communication.
///
/// Wraps `tokio-modbus` to implement our `Transport` trait.
///
/// # Example
///
/// ```ignore
/// use renogy_rs::{SerialTransport, Transport, Pdu, FunctionCode, Register};
///
/// let mut transport = SerialTransport::new("/dev/ttyUSB0", 9600, 0x01)?;
///
/// let register = Register::CellVoltage(1);
/// let mut payload = Vec::new();
/// payload.extend_from_slice(&register.address().to_be_bytes());
/// payload.extend_from_slice(&register.quantity().to_be_bytes());
///
/// let pdu = Pdu::new(0x01, FunctionCode::ReadHoldingRegisters, payload);
/// let response = transport.send_receive(&pdu).await?;
/// ```
pub struct SerialTransport {
    ctx: Context,
    slave_id: u8,
}

impl std::fmt::Debug for SerialTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SerialTransport")
            .field("slave_id", &self.slave_id)
            .finish_non_exhaustive()
    }
}

impl SerialTransport {
    /// Create a new serial transport.
    ///
    /// # Arguments
    /// * `path` - Serial port path (e.g., "/dev/ttyUSB0" or "COM3")
    /// * `baud_rate` - Baud rate (typically 9600 for Renogy BMS)
    /// * `slave_id` - Modbus slave address
    pub async fn new(path: &str, baud_rate: u32, slave_id: u8) -> Result<Self> {
        let port = tokio_serial::new(path, baud_rate)
            .open_native_async()
            .map_err(|e| RenogyError::Io(std::io::Error::other(e.to_string())))?;

        let ctx = tokio_modbus::client::rtu::attach_slave(port, Slave(slave_id));

        Ok(Self { ctx, slave_id })
    }

    /// Create a new serial transport with default baud rate (9600).
    pub async fn open(path: &str, slave_id: u8) -> Result<Self> {
        Self::new(path, DEFAULT_BAUD_RATE, slave_id).await
    }

    /// Change the slave address for subsequent requests.
    pub fn set_slave(&mut self, slave_id: u8) {
        self.slave_id = slave_id;
        self.ctx.set_slave(Slave(slave_id));
    }

    /// Get the current slave address.
    pub fn slave_id(&self) -> u8 {
        self.slave_id
    }
}

impl Transport for SerialTransport {
    async fn send_receive(&mut self, pdu: &Pdu) -> Result<Pdu> {
        // Update slave if PDU has different address
        if pdu.address != self.slave_id {
            self.set_slave(pdu.address);
        }

        // Convert our Pdu to tokio-modbus Request and call
        let response = match pdu.function_code {
            FunctionCode::ReadHoldingRegisters => {
                if pdu.payload.len() < 4 {
                    return Err(RenogyError::InvalidData);
                }
                let address = u16::from_be_bytes([pdu.payload[0], pdu.payload[1]]);
                let quantity = u16::from_be_bytes([pdu.payload[2], pdu.payload[3]]);

                let registers = self
                    .ctx
                    .read_holding_registers(address, quantity)
                    .await
                    .map_err(io_to_renogy_error)?;

                // Convert registers back to bytes (big-endian)
                let mut response_payload = Vec::with_capacity(1 + registers.len() * 2);
                response_payload.push((registers.len() * 2) as u8);
                for reg in registers {
                    response_payload.extend_from_slice(&reg.to_be_bytes());
                }

                Pdu::new(
                    pdu.address,
                    FunctionCode::ReadHoldingRegisters,
                    response_payload,
                )
            }

            FunctionCode::WriteSingleRegister => {
                if pdu.payload.len() < 4 {
                    return Err(RenogyError::InvalidData);
                }
                let address = u16::from_be_bytes([pdu.payload[0], pdu.payload[1]]);
                let value = u16::from_be_bytes([pdu.payload[2], pdu.payload[3]]);

                self.ctx
                    .write_single_register(address, value)
                    .await
                    .map_err(io_to_renogy_error)?;

                // Echo back the request as response
                Pdu::new(
                    pdu.address,
                    FunctionCode::WriteSingleRegister,
                    pdu.payload.clone(),
                )
            }

            FunctionCode::WriteMultipleRegisters => {
                if pdu.payload.len() < 5 {
                    return Err(RenogyError::InvalidData);
                }
                let address = u16::from_be_bytes([pdu.payload[0], pdu.payload[1]]);
                let quantity = u16::from_be_bytes([pdu.payload[2], pdu.payload[3]]);
                let byte_count = pdu.payload[4] as usize;

                if pdu.payload.len() < 5 + byte_count {
                    return Err(RenogyError::InvalidData);
                }

                // Parse register values from payload
                let mut values = Vec::with_capacity(quantity as usize);
                for i in 0..quantity as usize {
                    let offset = 5 + i * 2;
                    values.push(u16::from_be_bytes([
                        pdu.payload[offset],
                        pdu.payload[offset + 1],
                    ]));
                }

                self.ctx
                    .write_multiple_registers(address, &values)
                    .await
                    .map_err(io_to_renogy_error)?;

                // Response is address + quantity
                let mut response_payload = Vec::with_capacity(4);
                response_payload.extend_from_slice(&address.to_be_bytes());
                response_payload.extend_from_slice(&quantity.to_be_bytes());

                Pdu::new(
                    pdu.address,
                    FunctionCode::WriteMultipleRegisters,
                    response_payload,
                )
            }

            // For custom function codes (like RestoreFactoryDefault, ClearHistory),
            // use tokio-modbus Custom request
            FunctionCode::RestoreFactoryDefault | FunctionCode::ClearHistory => {
                use tokio_modbus::prelude::Request;

                let request = Request::Custom(pdu.function_code as u8, pdu.payload.clone());
                let response = self.ctx.call(request).await.map_err(io_to_renogy_error)?;

                match response {
                    tokio_modbus::prelude::Response::Custom(fc, data) => {
                        let function_code =
                            FunctionCode::from_u8(fc).ok_or(RenogyError::InvalidData)?;
                        Pdu::new(pdu.address, function_code, data)
                    }
                    _ => return Err(RenogyError::InvalidData),
                }
            }

            // Error response codes shouldn't be sent as requests
            FunctionCode::ReadHoldingRegistersError
            | FunctionCode::WriteSingleRegisterError
            | FunctionCode::WriteMultipleRegistersError
            | FunctionCode::RestoreFactoryDefaultError
            | FunctionCode::ClearHistoryError => {
                return Err(RenogyError::InvalidData);
            }
        };

        Ok(response)
    }

    async fn send(&mut self, pdu: &Pdu) -> Result<()> {
        // For broadcast/no-response operations
        self.send_receive(pdu).await?;
        Ok(())
    }
}

fn io_to_renogy_error(e: IoError) -> RenogyError {
    match e.kind() {
        ErrorKind::InvalidData => RenogyError::InvalidData,
        _ => RenogyError::Io(e),
    }
}
