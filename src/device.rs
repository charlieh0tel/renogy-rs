use crate::error::Result;
use crate::pdu::{FunctionCode, Pdu};
use crate::registers::Register;

const SHUTDOWN_VALUE: u16 = 1;
const LOCK_VALUE: u16 = 0x5A5A;
const UNLOCK_VALUE: u16 = 0xA5A5;
const TEST_BEGIN_VALUE: u16 = 0x5A5A;
const TEST_END_VALUE: u16 = 0xA5A5;

/// BMS device operation commands
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceCommand {
    /// Restore factory default settings
    RestoreFactoryDefault,
    /// Clear stored history data
    ClearHistory,
    /// Shutdown device
    Shutdown,
    /// Lock device (prevent configuration changes)
    Lock,
    /// Unlock device (allow configuration changes)
    Unlock,
    /// Begin test mode
    TestBegin,
    /// End test mode
    TestEnd,
}

impl DeviceCommand {
    #[must_use]
    pub fn create_pdu(&self, device_address: u8) -> Pdu {
        match self {
            DeviceCommand::RestoreFactoryDefault => Pdu::new(
                device_address,
                FunctionCode::RestoreFactoryDefault,
                vec![0x00, 0x00, 0x00, 0x01], // Supplement data as per PDF
            ),
            DeviceCommand::ClearHistory => Pdu::new(
                device_address,
                FunctionCode::ClearHistory,
                vec![0x00, 0x00, 0x00, 0x01], // Supplement data as per PDF
            ),
            DeviceCommand::Shutdown => {
                Self::create_write_pdu(device_address, Register::ShutdownCommand, SHUTDOWN_VALUE)
            }
            DeviceCommand::Lock => {
                Self::create_write_pdu(device_address, Register::LockControl, LOCK_VALUE)
            }
            DeviceCommand::Unlock => {
                Self::create_write_pdu(device_address, Register::LockControl, UNLOCK_VALUE)
            }
            DeviceCommand::TestBegin => {
                Self::create_write_pdu(device_address, Register::TestReady, TEST_BEGIN_VALUE)
            }
            DeviceCommand::TestEnd => {
                Self::create_write_pdu(device_address, Register::TestReady, TEST_END_VALUE)
            }
        }
    }

    fn create_write_pdu(device_address: u8, register: Register, value: u16) -> Pdu {
        let payload = [register.address().to_be_bytes(), value.to_be_bytes()].concat();
        Pdu::new(device_address, FunctionCode::WriteSingleRegister, payload)
    }

    #[must_use]
    pub const fn requires_unlock(&self) -> bool {
        matches!(
            self,
            DeviceCommand::RestoreFactoryDefault | DeviceCommand::ClearHistory
        )
    }
}

/// Device identification and configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub serial_number: String,
    pub manufacture_version: String,
    pub mainline_version: String,
    pub communication_protocol_version: String,
    pub battery_name: String,
    pub software_version: String,
    pub manufacturer_name: String,
    pub unique_identification_code: u32,
}

/// Power configuration settings
#[derive(Debug, Clone, PartialEq)]
pub struct PowerSettings {
    /// Charging power setting (percentage, 0-100)
    pub charge_power_percent: u8,
    /// Discharge power setting (percentage, 0-100)
    pub discharge_power_percent: u8,
}

impl PowerSettings {
    const MAX_POWER_PERCENT: u8 = 100;

    /// Create new power settings with validation
    pub fn new(charge_power_percent: u8, discharge_power_percent: u8) -> Result<Self> {
        if charge_power_percent > Self::MAX_POWER_PERCENT
            || discharge_power_percent > Self::MAX_POWER_PERCENT
        {
            return Err(crate::error::RenogyError::InvalidRegisterRange);
        }
        Ok(Self {
            charge_power_percent,
            discharge_power_percent,
        })
    }

    #[must_use]
    pub const fn is_valid_percent(percent: u8) -> bool {
        percent <= Self::MAX_POWER_PERCENT
    }
}

/// ACP (Advanced Communication Protocol) configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpConfig {
    /// ACP broadcast setting (range: 1-254)
    pub broadcast: u8,
    /// ACP configure setting (range: 1-254)
    pub configure: u8,
    /// ACP shake setting (range: 1-254)
    pub shake: u8,
}

impl AcpConfig {
    const MIN_ACP_VALUE: u8 = 1;
    const MAX_ACP_VALUE: u8 = 254;

    /// Create new ACP configuration with validation
    pub fn new(broadcast: u8, configure: u8, shake: u8) -> Result<Self> {
        if !Self::is_valid_acp_value(broadcast)
            || !Self::is_valid_acp_value(configure)
            || !Self::is_valid_acp_value(shake)
        {
            return Err(crate::error::RenogyError::InvalidRegisterRange);
        }
        Ok(Self {
            broadcast,
            configure,
            shake,
        })
    }

    #[must_use]
    pub const fn is_valid_acp_value(value: u8) -> bool {
        value >= Self::MIN_ACP_VALUE && value <= Self::MAX_ACP_VALUE
    }
}
