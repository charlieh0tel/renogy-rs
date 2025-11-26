use crate::alarm::{
    CellTemperatureAlarms, CellVoltageAlarms, CellVoltageErrors, ChargeDischargeStatus,
    OtherAlarmInfo, Status1, Status2, Status3,
};
use crate::error::{RenogyError, Result};
use byteorder::{BigEndian, ByteOrder};
use uom::si::electric_current::ampere;
use uom::si::electric_potential::volt;
use uom::si::f32::{ElectricCurrent, ElectricPotential, ThermodynamicTemperature};
use uom::si::thermodynamic_temperature::degree_celsius;

#[derive(Debug, PartialEq)]
pub enum Value {
    ElectricPotential(ElectricPotential),
    ElectricCurrent(ElectricCurrent),
    ThermodynamicTemperature(ThermodynamicTemperature),
    Integer(u32),
    CellVoltageAlarms(CellVoltageAlarms),
    CellTemperatureAlarms(CellTemperatureAlarms),
    OtherAlarmInfo(OtherAlarmInfo),
    Status1(Status1),
    Status2(Status2),
    Status3(Status3),
    CellVoltageErrors(CellVoltageErrors),
    ChargeDischargeStatus(ChargeDischargeStatus),
    String(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Register {
    CellCount,
    CellVoltage(u8),
    CellTemperatureCount,
    CellTemperature(u8),
    BmsTemperature,
    EnvironmentTemperatureCount,
    EnvironmentTemperature(u8),
    HeaterTemperatureCount,
    HeaterTemperature(u8),
    Current,
    ModuleVoltage,
    RemainingCapacity,
    TotalCapacity,
    CycleNumber,
    ChargeVoltageLimit,
    DischargeVoltageLimit,
    ChargeCurrentLimit,
    DischargeCurrentLimit,
    CellVoltageAlarmInfo,
    CellTemperatureAlarmInfo,
    OtherAlarmInfo,
    Status1,
    Status2,
    Status3,
    ChargeDischargeStatus,
    SnNumber,
    ManufactureVersion,
    MainlineVersion,
    CommunicationProtocolVersion,
    BatteryName,
    SoftwareVersion,
    ManufacturerName,
    // Configuration registers (5200-5229)
    CellOverVoltageLimit,
    CellHighVoltageLimit,
    CellLowVoltageLimit,
    CellUnderVoltageLimit,
    ChargeOverTemperatureLimit,
    ChargeHighTemperatureLimit,
    ChargeLowTemperatureLimit,
    ChargeUnderTemperatureLimit,
    ChargeOver2CurrentLimit,
    ChargeOver1CurrentLimit,
    ChargeHighCurrentLimit,
    ModuleOverVoltageLimit,
    ModuleHighVoltageLimit,
    ModuleLowVoltageLimit,
    ModuleUnderVoltageLimit,
    DischargeOverTemperatureLimit,
    DischargeHighTemperatureLimit,
    DischargeLowTemperatureLimit,
    DischargeUnderTemperatureLimit,
    DischargeOver2CurrentLimit,
    DischargeOver1CurrentLimit,
    DischargeHighCurrentLimit,
    ShutdownCommand,
    DeviceId,
    LockControl,
    TestReady,
    UniqueIdentificationCode,
    ChargePowerSetting,
    DischargePowerSetting,
    // ACP Protocol registers (61440-61442)
    AcpBroadcast,
    AcpConfigure,
    AcpShake,
}

impl Register {
    pub const fn address(&self) -> u16 {
        match self {
            Register::CellCount => 5000,
            Register::CellVoltage(n) => 5000 + *n as u16,
            Register::CellTemperatureCount => 5017,
            Register::CellTemperature(n) => 5017 + *n as u16,
            Register::BmsTemperature => 5035,
            Register::EnvironmentTemperatureCount => 5036,
            Register::EnvironmentTemperature(n) => 5036 + *n as u16,
            Register::HeaterTemperatureCount => 5039,
            Register::HeaterTemperature(n) => 5039 + *n as u16,
            Register::Current => 5042,
            Register::ModuleVoltage => 5043,
            Register::RemainingCapacity => 5044,
            Register::TotalCapacity => 5046,
            Register::CycleNumber => 5048,
            Register::ChargeVoltageLimit => 5049,
            Register::DischargeVoltageLimit => 5050,
            Register::ChargeCurrentLimit => 5051,
            Register::DischargeCurrentLimit => 5052,
            Register::CellVoltageAlarmInfo => 5100,
            Register::CellTemperatureAlarmInfo => 5102,
            Register::OtherAlarmInfo => 5104,
            Register::Status1 => 5106,
            Register::Status2 => 5107,
            Register::Status3 => 5108,
            Register::ChargeDischargeStatus => 5109,
            Register::SnNumber => 5110,
            Register::ManufactureVersion => 5118,
            Register::MainlineVersion => 5119,
            Register::CommunicationProtocolVersion => 5121,
            Register::BatteryName => 5122,
            Register::SoftwareVersion => 5130,
            Register::ManufacturerName => 5132,
            // Configuration registers
            Register::CellOverVoltageLimit => 5200,
            Register::CellHighVoltageLimit => 5201,
            Register::CellLowVoltageLimit => 5202,
            Register::CellUnderVoltageLimit => 5203,
            Register::ChargeOverTemperatureLimit => 5204,
            Register::ChargeHighTemperatureLimit => 5205,
            Register::ChargeLowTemperatureLimit => 5206,
            Register::ChargeUnderTemperatureLimit => 5207,
            Register::ChargeOver2CurrentLimit => 5208,
            Register::ChargeOver1CurrentLimit => 5209,
            Register::ChargeHighCurrentLimit => 5210,
            Register::ModuleOverVoltageLimit => 5211,
            Register::ModuleHighVoltageLimit => 5212,
            Register::ModuleLowVoltageLimit => 5213,
            Register::ModuleUnderVoltageLimit => 5214,
            Register::DischargeOverTemperatureLimit => 5215,
            Register::DischargeHighTemperatureLimit => 5216,
            Register::DischargeLowTemperatureLimit => 5217,
            Register::DischargeUnderTemperatureLimit => 5218,
            Register::DischargeOver2CurrentLimit => 5219,
            Register::DischargeOver1CurrentLimit => 5220,
            Register::DischargeHighCurrentLimit => 5221,
            Register::ShutdownCommand => 5222,
            Register::DeviceId => 5223,
            Register::LockControl => 5224,
            Register::TestReady => 5225,
            Register::UniqueIdentificationCode => 5226,
            Register::ChargePowerSetting => 5228,
            Register::DischargePowerSetting => 5229,
            // ACP Protocol registers
            Register::AcpBroadcast => 61440,
            Register::AcpConfigure => 61441,
            Register::AcpShake => 61442,
        }
    }

    pub const fn quantity(&self) -> u16 {
        match self {
            Register::RemainingCapacity
            | Register::TotalCapacity
            | Register::CellVoltageAlarmInfo
            | Register::CellTemperatureAlarmInfo
            | Register::OtherAlarmInfo
            | Register::MainlineVersion => 2,
            Register::SnNumber => 8,
            Register::BatteryName => 8,
            Register::SoftwareVersion => 2,
            Register::ManufacturerName => 10,
            Register::UniqueIdentificationCode => 2,
            _ => 1,
        }
    }

    pub fn parse_value(&self, data: &[u8]) -> Value {
        match self {
            Register::CellCount => Value::Integer(BigEndian::read_u16(data) as u32),
            Register::CellVoltage(_) => Value::ElectricPotential(ElectricPotential::new::<volt>(
                BigEndian::read_u16(data) as f32 * 0.1,
            )),
            Register::CellTemperatureCount => Value::Integer(BigEndian::read_u16(data) as u32),
            Register::CellTemperature(_) => {
                Value::ThermodynamicTemperature(ThermodynamicTemperature::new::<degree_celsius>(
                    BigEndian::read_u16(data) as f32 * 0.1,
                ))
            }
            Register::BmsTemperature => {
                Value::ThermodynamicTemperature(ThermodynamicTemperature::new::<degree_celsius>(
                    BigEndian::read_u16(data) as f32 * 0.1,
                ))
            }
            Register::EnvironmentTemperatureCount => {
                Value::Integer(BigEndian::read_u16(data) as u32)
            }
            Register::EnvironmentTemperature(_) => {
                Value::ThermodynamicTemperature(ThermodynamicTemperature::new::<degree_celsius>(
                    BigEndian::read_u16(data) as f32 * 0.1,
                ))
            }
            Register::HeaterTemperatureCount => Value::Integer(BigEndian::read_u16(data) as u32),
            Register::HeaterTemperature(_) => {
                Value::ThermodynamicTemperature(ThermodynamicTemperature::new::<degree_celsius>(
                    BigEndian::read_u16(data) as f32 * 0.1,
                ))
            }
            Register::Current => Value::ElectricCurrent(ElectricCurrent::new::<ampere>(
                BigEndian::read_i16(data) as f32 * 0.01,
            )),
            Register::ModuleVoltage => Value::ElectricPotential(ElectricPotential::new::<volt>(
                BigEndian::read_u16(data) as f32 * 0.1,
            )),
            Register::RemainingCapacity => Value::ElectricCurrent(ElectricCurrent::new::<ampere>(
                BigEndian::read_u32(data) as f32 * 0.001,
            )),
            Register::TotalCapacity => Value::ElectricCurrent(ElectricCurrent::new::<ampere>(
                BigEndian::read_u32(data) as f32 * 0.001,
            )),
            Register::CycleNumber => Value::Integer(BigEndian::read_u16(data) as u32),
            Register::ChargeVoltageLimit => Value::ElectricPotential(
                ElectricPotential::new::<volt>(BigEndian::read_u16(data) as f32 * 0.1),
            ),
            Register::DischargeVoltageLimit => {
                Value::ElectricPotential(ElectricPotential::new::<volt>(
                    BigEndian::read_u16(data) as f32 * 0.1,
                ))
            }
            Register::ChargeCurrentLimit => Value::ElectricCurrent(ElectricCurrent::new::<ampere>(
                BigEndian::read_u16(data) as f32 * 0.01,
            )),
            Register::DischargeCurrentLimit => Value::ElectricCurrent(
                ElectricCurrent::new::<ampere>(BigEndian::read_i16(data) as f32 * 0.01),
            ),
            Register::CellVoltageAlarmInfo => {
                Value::CellVoltageAlarms(CellVoltageAlarms::from_bits(BigEndian::read_u32(data)))
            }
            Register::CellTemperatureAlarmInfo => Value::CellTemperatureAlarms(
                CellTemperatureAlarms::from_bits(BigEndian::read_u32(data)),
            ),
            Register::OtherAlarmInfo => Value::OtherAlarmInfo(OtherAlarmInfo::from_bits_truncate(
                BigEndian::read_u32(data),
            )),
            Register::Status1 => {
                Value::Status1(Status1::from_bits_truncate(BigEndian::read_u16(data)))
            }
            Register::Status2 => {
                Value::Status2(Status2::from_bits_truncate(BigEndian::read_u16(data)))
            }
            Register::Status3 => {
                Value::Status3(Status3::from_bits_truncate(BigEndian::read_u16(data)))
            }
            Register::ChargeDischargeStatus => Value::ChargeDischargeStatus(
                ChargeDischargeStatus::from_bits_truncate(BigEndian::read_u16(data)),
            ),
            Register::SnNumber
            | Register::ManufactureVersion
            | Register::MainlineVersion
            | Register::CommunicationProtocolVersion
            | Register::BatteryName
            | Register::SoftwareVersion
            | Register::ManufacturerName => {
                Value::String(String::from_utf8_lossy(data).to_string())
            }
            // Configuration register parsing
            Register::CellOverVoltageLimit
            | Register::CellHighVoltageLimit
            | Register::CellLowVoltageLimit
            | Register::CellUnderVoltageLimit
            | Register::ModuleOverVoltageLimit
            | Register::ModuleHighVoltageLimit
            | Register::ModuleLowVoltageLimit
            | Register::ModuleUnderVoltageLimit => Value::ElectricPotential(
                ElectricPotential::new::<volt>(BigEndian::read_u16(data) as f32 * 0.1),
            ),
            Register::ChargeOverTemperatureLimit
            | Register::ChargeHighTemperatureLimit
            | Register::ChargeLowTemperatureLimit
            | Register::ChargeUnderTemperatureLimit
            | Register::DischargeOverTemperatureLimit
            | Register::DischargeHighTemperatureLimit
            | Register::DischargeLowTemperatureLimit
            | Register::DischargeUnderTemperatureLimit => {
                Value::ThermodynamicTemperature(ThermodynamicTemperature::new::<degree_celsius>(
                    BigEndian::read_i16(data) as f32 * 0.1,
                ))
            }
            Register::ChargeOver2CurrentLimit
            | Register::ChargeOver1CurrentLimit
            | Register::ChargeHighCurrentLimit
            | Register::DischargeOver2CurrentLimit
            | Register::DischargeOver1CurrentLimit
            | Register::DischargeHighCurrentLimit => Value::ElectricCurrent(
                ElectricCurrent::new::<ampere>(BigEndian::read_u16(data) as f32 * 0.01),
            ),
            Register::ShutdownCommand
            | Register::DeviceId
            | Register::LockControl
            | Register::TestReady
            | Register::ChargePowerSetting
            | Register::DischargePowerSetting => Value::Integer(BigEndian::read_u16(data) as u32),
            Register::UniqueIdentificationCode => Value::Integer(BigEndian::read_u32(data)),
            // ACP Protocol registers
            Register::AcpBroadcast | Register::AcpConfigure | Register::AcpShake => {
                Value::Integer(BigEndian::read_u16(data) as u32)
            }
        }
    }

    pub fn is_writable(&self) -> bool {
        matches!(
            self,
            Register::ChargeVoltageLimit
                | Register::DischargeVoltageLimit
                | Register::ChargeCurrentLimit
                | Register::DischargeCurrentLimit
                // Configuration registers are writable
                | Register::CellOverVoltageLimit
                | Register::CellHighVoltageLimit
                | Register::CellLowVoltageLimit
                | Register::CellUnderVoltageLimit
                | Register::ChargeOverTemperatureLimit
                | Register::ChargeHighTemperatureLimit
                | Register::ChargeLowTemperatureLimit
                | Register::ChargeUnderTemperatureLimit
                | Register::ChargeOver2CurrentLimit
                | Register::ChargeOver1CurrentLimit
                | Register::ChargeHighCurrentLimit
                | Register::ModuleOverVoltageLimit
                | Register::ModuleHighVoltageLimit
                | Register::ModuleLowVoltageLimit
                | Register::ModuleUnderVoltageLimit
                | Register::DischargeOverTemperatureLimit
                | Register::DischargeHighTemperatureLimit
                | Register::DischargeLowTemperatureLimit
                | Register::DischargeUnderTemperatureLimit
                | Register::DischargeOver2CurrentLimit
                | Register::DischargeOver1CurrentLimit
                | Register::DischargeHighCurrentLimit
                | Register::ShutdownCommand
                | Register::DeviceId
                | Register::LockControl
                | Register::TestReady
                | Register::UniqueIdentificationCode
                | Register::ChargePowerSetting
                | Register::DischargePowerSetting
                | Register::AcpBroadcast
                | Register::AcpConfigure
                | Register::AcpShake
        )
    }

    pub fn serialize_value(&self, value: &Value) -> Result<Vec<u8>> {
        let mut data = vec![0u8; (self.quantity() * 2) as usize];

        match (self, value) {
            // Original writable registers
            (
                Register::ChargeVoltageLimit | Register::DischargeVoltageLimit,
                Value::ElectricPotential(voltage),
            ) => {
                let raw_value = (voltage.get::<volt>() * 10.0) as u16;
                BigEndian::write_u16(&mut data, raw_value);
            }
            (
                Register::ChargeCurrentLimit | Register::DischargeCurrentLimit,
                Value::ElectricCurrent(current),
            ) => {
                let raw_value = (current.get::<ampere>() * 100.0) as u16;
                BigEndian::write_u16(&mut data, raw_value);
            }
            (Register::CycleNumber, Value::Integer(value)) => {
                BigEndian::write_u16(&mut data, *value as u16);
            }
            (
                Register::RemainingCapacity | Register::TotalCapacity,
                Value::ElectricCurrent(current),
            ) => {
                let raw_value = (current.get::<ampere>() * 1000.0) as u32;
                BigEndian::write_u32(&mut data, raw_value);
            }
            // Configuration voltage limits
            (
                Register::CellOverVoltageLimit
                | Register::CellHighVoltageLimit
                | Register::CellLowVoltageLimit
                | Register::CellUnderVoltageLimit
                | Register::ModuleOverVoltageLimit
                | Register::ModuleHighVoltageLimit
                | Register::ModuleLowVoltageLimit
                | Register::ModuleUnderVoltageLimit,
                Value::ElectricPotential(voltage),
            ) => {
                let raw_value = (voltage.get::<volt>() * 10.0) as u16;
                BigEndian::write_u16(&mut data, raw_value);
            }
            // Configuration temperature limits
            (
                Register::ChargeOverTemperatureLimit
                | Register::ChargeHighTemperatureLimit
                | Register::ChargeLowTemperatureLimit
                | Register::ChargeUnderTemperatureLimit
                | Register::DischargeOverTemperatureLimit
                | Register::DischargeHighTemperatureLimit
                | Register::DischargeLowTemperatureLimit
                | Register::DischargeUnderTemperatureLimit,
                Value::ThermodynamicTemperature(temp),
            ) => {
                let raw_value = (temp.get::<degree_celsius>() * 10.0) as i16;
                BigEndian::write_i16(&mut data, raw_value);
            }
            // Configuration current limits
            (
                Register::ChargeOver2CurrentLimit
                | Register::ChargeOver1CurrentLimit
                | Register::ChargeHighCurrentLimit
                | Register::DischargeOver2CurrentLimit
                | Register::DischargeOver1CurrentLimit
                | Register::DischargeHighCurrentLimit,
                Value::ElectricCurrent(current),
            ) => {
                let raw_value = (current.get::<ampere>() * 100.0) as u16;
                BigEndian::write_u16(&mut data, raw_value);
            }
            // Control and configuration registers
            (
                Register::ShutdownCommand
                | Register::DeviceId
                | Register::LockControl
                | Register::TestReady
                | Register::ChargePowerSetting
                | Register::DischargePowerSetting
                | Register::AcpBroadcast
                | Register::AcpConfigure
                | Register::AcpShake,
                Value::Integer(value),
            ) => {
                BigEndian::write_u16(&mut data, *value as u16);
            }
            (Register::UniqueIdentificationCode, Value::Integer(value)) => {
                BigEndian::write_u32(&mut data, *value);
            }
            _ => {
                return Err(RenogyError::UnsupportedOperation);
            }
        }

        Ok(data)
    }
}
