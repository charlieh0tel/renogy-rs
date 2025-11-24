use std::fmt;

#[derive(Debug)]
pub enum RenogyError {
    InvalidData,
    CrcMismatch,
    Io(std::io::Error),
}

impl fmt::Display for RenogyError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RenogyError::InvalidData => write!(f, "Invalid data"),
            RenogyError::CrcMismatch => write!(f, "CRC mismatch"),
            RenogyError::Io(e) => write!(f, "IO error: {}", e),
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
