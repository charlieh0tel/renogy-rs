use crate::error::Result;
use crate::pdu::{FunctionCode, Pdu};

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
    /// Create a PDU for executing this device command
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
                let mut payload = Vec::new();
                payload.extend_from_slice(&5222u16.to_be_bytes()); // ShutdownCommand register address
                payload.extend_from_slice(&1u16.to_be_bytes()); // Shutdown value
                Pdu::new(device_address, FunctionCode::WriteSingleRegister, payload)
            }
            DeviceCommand::Lock => {
                let mut payload = Vec::new();
                payload.extend_from_slice(&5224u16.to_be_bytes()); // LockControl register address
                payload.extend_from_slice(&0x5A5Au16.to_be_bytes()); // Lock value
                Pdu::new(device_address, FunctionCode::WriteSingleRegister, payload)
            }
            DeviceCommand::Unlock => {
                let mut payload = Vec::new();
                payload.extend_from_slice(&5224u16.to_be_bytes()); // LockControl register address
                payload.extend_from_slice(&0xA5A5u16.to_be_bytes()); // Unlock value
                Pdu::new(device_address, FunctionCode::WriteSingleRegister, payload)
            }
            DeviceCommand::TestBegin => {
                let mut payload = Vec::new();
                payload.extend_from_slice(&5225u16.to_be_bytes()); // TestReady register address
                payload.extend_from_slice(&0x5A5Au16.to_be_bytes()); // Test begin value
                Pdu::new(device_address, FunctionCode::WriteSingleRegister, payload)
            }
            DeviceCommand::TestEnd => {
                let mut payload = Vec::new();
                payload.extend_from_slice(&5225u16.to_be_bytes()); // TestReady register address
                payload.extend_from_slice(&0xA5A5u16.to_be_bytes()); // Test end value
                Pdu::new(device_address, FunctionCode::WriteSingleRegister, payload)
            }
        }
    }

    /// Check if this command requires device unlock first
    pub fn requires_unlock(&self) -> bool {
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
    /// Create new power settings with validation
    pub fn new(charge_power_percent: u8, discharge_power_percent: u8) -> Result<Self> {
        if charge_power_percent > 100 || discharge_power_percent > 100 {
            return Err(crate::error::RenogyError::InvalidRegisterRange);
        }
        Ok(PowerSettings {
            charge_power_percent,
            discharge_power_percent,
        })
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
    /// Create new ACP configuration with validation
    pub fn new(broadcast: u8, configure: u8, shake: u8) -> Result<Self> {
        if broadcast == 0
            || broadcast > 254
            || configure == 0
            || configure > 254
            || shake == 0
            || shake > 254
        {
            return Err(crate::error::RenogyError::InvalidRegisterRange);
        }
        Ok(AcpConfig {
            broadcast,
            configure,
            shake,
        })
    }
}
