//! Serial/RS-485 transport for Modbus RTU communication.
//!
//! This module provides a serial transport that implements the `Transport` trait,
//! using `tokio-modbus` for the underlying Modbus RTU communication.

use crate::error::{RenogyError, Result};
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
/// use renogy_rs::{SerialTransport, Transport, Register};
///
/// let mut transport = SerialTransport::new("/dev/ttyUSB0", 9600, 0x01).await?;
///
/// let register = Register::CellVoltage(1);
/// let regs = transport.read_holding_registers(0x01, register.address(), register.quantity()).await?;
/// let value = register.parse_registers(&regs);
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

    fn ensure_slave(&mut self, slave: u8) {
        if slave != self.slave_id {
            self.set_slave(slave);
        }
    }
}

impl Transport for SerialTransport {
    async fn read_holding_registers(
        &mut self,
        slave: u8,
        addr: u16,
        quantity: u16,
    ) -> Result<Vec<u16>> {
        self.ensure_slave(slave);
        self.ctx
            .read_holding_registers(addr, quantity)
            .await
            .map_err(io_to_renogy_error)
    }

    async fn write_single_register(&mut self, slave: u8, addr: u16, value: u16) -> Result<()> {
        self.ensure_slave(slave);
        self.ctx
            .write_single_register(addr, value)
            .await
            .map_err(io_to_renogy_error)
    }

    async fn write_multiple_registers(
        &mut self,
        slave: u8,
        addr: u16,
        values: &[u16],
    ) -> Result<()> {
        self.ensure_slave(slave);
        self.ctx
            .write_multiple_registers(addr, values)
            .await
            .map_err(io_to_renogy_error)
    }

    async fn send_custom(&mut self, slave: u8, function_code: u8, data: &[u8]) -> Result<Vec<u8>> {
        use tokio_modbus::prelude::Request;

        self.ensure_slave(slave);
        let request = Request::Custom(function_code, data.to_vec());
        let response = self.ctx.call(request).await.map_err(io_to_renogy_error)?;

        match response {
            tokio_modbus::prelude::Response::Custom(_fc, response_data) => Ok(response_data),
            _ => Err(RenogyError::InvalidData),
        }
    }
}

fn io_to_renogy_error(e: IoError) -> RenogyError {
    match e.kind() {
        ErrorKind::InvalidData => RenogyError::InvalidData,
        _ => RenogyError::Io(e),
    }
}
