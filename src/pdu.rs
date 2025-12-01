use crate::error::{ModbusExceptionCode, RenogyError, Result};
use crc::{CRC_16_MODBUS, Crc};

pub const MODBUS_CRC: Crc<u16> = Crc::<u16>::new(&CRC_16_MODBUS);

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum FunctionCode {
    ReadHoldingRegisters = 0x03,
    WriteSingleRegister = 0x06,
    WriteMultipleRegisters = 0x10,
    RestoreFactoryDefault = 0x78,
    ClearHistory = 0x79,
}

impl FunctionCode {
    #[must_use]
    pub const fn from_u8(code: u8) -> Option<FunctionCode> {
        match code {
            0x03 => Some(FunctionCode::ReadHoldingRegisters),
            0x06 => Some(FunctionCode::WriteSingleRegister),
            0x10 => Some(FunctionCode::WriteMultipleRegisters),
            0x78 => Some(FunctionCode::RestoreFactoryDefault),
            0x79 => Some(FunctionCode::ClearHistory),
            _ => None,
        }
    }

    #[must_use]
    pub fn is_write_operation(self) -> bool {
        matches!(
            self,
            FunctionCode::WriteSingleRegister
                | FunctionCode::WriteMultipleRegisters
                | FunctionCode::RestoreFactoryDefault
                | FunctionCode::ClearHistory
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pdu {
    pub address: u8,
    pub function_code: FunctionCode,
    pub payload: Vec<u8>,
}

impl Pdu {
    #[must_use]
    pub const fn new(address: u8, function_code: FunctionCode, payload: Vec<u8>) -> Self {
        Self {
            address,
            function_code,
            payload,
        }
    }

    #[must_use]
    pub fn serialize(&self) -> Vec<u8> {
        let mut frame = Vec::with_capacity(2 + self.payload.len() + 2);
        frame.push(self.address);
        frame.push(self.function_code as u8);
        frame.extend(&self.payload);
        let crc = MODBUS_CRC.checksum(&frame);
        frame.extend(&crc.to_le_bytes());
        frame
    }

    #[must_use]
    pub fn is_write_operation(&self) -> bool {
        self.function_code.is_write_operation()
    }

    pub fn deserialize(frame: &[u8]) -> Result<Self> {
        if frame.len() < 4 {
            return Err(RenogyError::InvalidData);
        }

        let (data, crc_bytes) = frame.split_at(frame.len() - 2);
        let expected_crc = MODBUS_CRC.checksum(data);
        let actual_crc = u16::from_le_bytes([crc_bytes[0], crc_bytes[1]]);

        if expected_crc != actual_crc {
            return Err(RenogyError::CrcMismatch);
        }

        let address = data[0];
        let function_code_byte = data[1];

        if function_code_byte & 0x80 != 0 {
            if data.len() >= 3 {
                let exception_code = data[2];
                if let Some(modbus_exception) = ModbusExceptionCode::from_u8(exception_code) {
                    return Err(RenogyError::ModbusException(modbus_exception));
                }
            }
            return Err(RenogyError::InvalidData);
        }

        let function_code =
            FunctionCode::from_u8(function_code_byte).ok_or(RenogyError::InvalidData)?;

        let payload = data[2..].to_vec();

        Ok(Pdu {
            address,
            function_code,
            payload,
        })
    }
}
