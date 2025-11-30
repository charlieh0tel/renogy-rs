use std::fmt;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RenogyError {
    #[error("invalid data")]
    InvalidData,
    #[error("CRC mismatch")]
    CrcMismatch,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Modbus exception: {0}")]
    ModbusException(ModbusExceptionCode),
    #[error("unsupported operation")]
    UnsupportedOperation,
    #[error("device control operation failed")]
    DeviceControlFailed,
    #[error("invalid register address or range")]
    InvalidRegisterRange,
    #[error("write operation failed")]
    WriteOperationFailed,
    #[error("Bluetooth error: {0}")]
    Bluetooth(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModbusExceptionCode {
    IllegalFunction = 0x01,
    IllegalDataAddress = 0x02,
    IllegalDataValue = 0x03,
    SlaveDeviceFailure = 0x04,
    Acknowledge = 0x05,
    SlaveDeviceBusy = 0x06,
    MemoryParityError = 0x08,
    GatewayPathUnavailable = 0x0A,
    GatewayTargetDeviceFailedToRespond = 0x0B,
}

impl fmt::Display for ModbusExceptionCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModbusExceptionCode::IllegalFunction => write!(f, "Illegal function (01h)"),
            ModbusExceptionCode::IllegalDataAddress => write!(f, "Illegal data address (02h)"),
            ModbusExceptionCode::IllegalDataValue => write!(f, "Illegal data value (03h)"),
            ModbusExceptionCode::SlaveDeviceFailure => write!(f, "Slave device failure (04h)"),
            ModbusExceptionCode::Acknowledge => write!(f, "Acknowledge (05h)"),
            ModbusExceptionCode::SlaveDeviceBusy => write!(f, "Slave device busy (06h)"),
            ModbusExceptionCode::MemoryParityError => write!(f, "Memory parity error (08h)"),
            ModbusExceptionCode::GatewayPathUnavailable => {
                write!(f, "Gateway path unavailable (0Ah)")
            }
            ModbusExceptionCode::GatewayTargetDeviceFailedToRespond => {
                write!(f, "Gateway target device failed to respond (0Bh)")
            }
        }
    }
}

impl ModbusExceptionCode {
    #[must_use]
    pub const fn from_u8(code: u8) -> Option<Self> {
        match code {
            0x01 => Some(ModbusExceptionCode::IllegalFunction),
            0x02 => Some(ModbusExceptionCode::IllegalDataAddress),
            0x03 => Some(ModbusExceptionCode::IllegalDataValue),
            0x04 => Some(ModbusExceptionCode::SlaveDeviceFailure),
            0x05 => Some(ModbusExceptionCode::Acknowledge),
            0x06 => Some(ModbusExceptionCode::SlaveDeviceBusy),
            0x08 => Some(ModbusExceptionCode::MemoryParityError),
            0x0A => Some(ModbusExceptionCode::GatewayPathUnavailable),
            0x0B => Some(ModbusExceptionCode::GatewayTargetDeviceFailedToRespond),
            _ => None,
        }
    }
}

impl From<zbus::Error> for RenogyError {
    fn from(err: zbus::Error) -> RenogyError {
        RenogyError::Bluetooth(err.to_string())
    }
}

impl From<zbus::fdo::Error> for RenogyError {
    fn from(err: zbus::fdo::Error) -> RenogyError {
        RenogyError::Bluetooth(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, RenogyError>;
