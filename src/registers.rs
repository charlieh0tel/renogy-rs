use crate::alarm::{
    CellTemperatureAlarms, CellVoltageAlarms, CellVoltageErrors, ChargeDischargeStatus,
    OtherAlarmInfo, Status1, Status2, Status3,
};
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

#[derive(Debug, PartialEq)]
pub enum Register {
    CellCount,
    CellVoltage(u8),
    CellTemperatureCount,
    CellTemperature(u8),
    BmsTemperature,
    EnvironmentTemperature(u8),
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
}

impl Register {
    pub fn address(&self) -> u16 {
        match self {
            Register::CellCount => 5000,
            Register::CellVoltage(n) => 5000 + *n as u16,
            Register::CellTemperatureCount => 5017,
            Register::CellTemperature(n) => 5017 + *n as u16,
            Register::BmsTemperature => 5035,
            Register::EnvironmentTemperature(n) => 5036 + *n as u16,
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
        }
    }

    pub fn quantity(&self) -> u16 {
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
            Register::EnvironmentTemperature(_) => {
                Value::ThermodynamicTemperature(ThermodynamicTemperature::new::<degree_celsius>(
                    BigEndian::read_u16(data) as f32 * 0.1,
                ))
            }
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
        }
    }
}
