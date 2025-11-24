use crate::error::{RenogyError, Result};
use crc::{CRC_16_MODBUS, Crc};

pub const MODBUS_CRC: Crc<u16> = Crc::<u16>::new(&CRC_16_MODBUS);

#[derive(Debug, PartialEq, Clone)]
pub enum FunctionCode {
    ReadHoldingRegisters = 0x03,
    WriteSingleRegister = 0x06,
    WriteMultipleRegisters = 0x10,
    RestoreFactoryDefault = 0x78,
    ClearHistory = 0x79,
    ReadHoldingRegistersError = 0x83,
    WriteSingleRegisterError = 0x86,
    WriteMultipleRegistersError = 0x90,
    RestoreFactoryDefaultError = 0xF8,
    ClearHistoryError = 0xF9,
}

impl FunctionCode {
    pub fn from_u8(code: u8) -> Option<FunctionCode> {
        match code {
            0x03 => Some(FunctionCode::ReadHoldingRegisters),
            0x06 => Some(FunctionCode::WriteSingleRegister),
            0x10 => Some(FunctionCode::WriteMultipleRegisters),
            0x78 => Some(FunctionCode::RestoreFactoryDefault),
            0x79 => Some(FunctionCode::ClearHistory),
            0x83 => Some(FunctionCode::ReadHoldingRegistersError),
            0x86 => Some(FunctionCode::WriteSingleRegisterError),
            0x90 => Some(FunctionCode::WriteMultipleRegistersError),
            0xF8 => Some(FunctionCode::RestoreFactoryDefaultError),
            0xF9 => Some(FunctionCode::ClearHistoryError),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct Pdu {
    pub address: u8,
    pub function_code: FunctionCode,
    pub payload: Vec<u8>,
}

impl Pdu {
    pub fn new(address: u8, function_code: FunctionCode, payload: Vec<u8>) -> Self {
        Pdu {
            address,
            function_code,
            payload,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut frame = Vec::new();
        frame.push(self.address);
        frame.push(self.function_code.clone() as u8);
        frame.extend(&self.payload);

        let crc = MODBUS_CRC.checksum(&frame);
        frame.extend(&crc.to_le_bytes());
        frame
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
        let function_code = FunctionCode::from_u8(data[1]).ok_or(RenogyError::InvalidData)?;
        let payload = data[2..].to_vec();

        Ok(Pdu {
            address,
            function_code,
            payload,
        })
    }
}
