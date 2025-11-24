use std::fmt;

#[derive(Debug)]
pub enum RenogyError {
    InvalidData,
    CrcMismatch,
    Io(std::io::Error),
    ModbusException(ModbusExceptionCode),
    UnsupportedOperation,
    DeviceControlFailed,
    InvalidRegisterRange,
    WriteOperationFailed,
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

impl fmt::Display for RenogyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenogyError::InvalidData => write!(f, "Invalid data"),
            RenogyError::CrcMismatch => write!(f, "CRC mismatch"),
            RenogyError::Io(e) => write!(f, "IO error: {}", e),
            RenogyError::ModbusException(code) => write!(f, "Modbus exception: {}", code),
            RenogyError::UnsupportedOperation => write!(f, "Unsupported operation"),
            RenogyError::DeviceControlFailed => write!(f, "Device control operation failed"),
            RenogyError::InvalidRegisterRange => write!(f, "Invalid register address or range"),
            RenogyError::WriteOperationFailed => write!(f, "Write operation failed"),
        }
    }
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

impl std::error::Error for RenogyError {}

impl From<std::io::Error> for RenogyError {
    fn from(err: std::io::Error) -> RenogyError {
        RenogyError::Io(err)
    }
}

pub type Result<T> = std::result::Result<T, RenogyError>;
