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
    Integer(i32),
}

#[derive(Debug, PartialEq)]
pub enum Register {
    CellCount,
    CellVoltage(u8),
    CellTemperatureCount,
    CellTemperature(u8),
    Current,
    ModuleVoltage,
    // Add other registers here
}

impl Register {
    pub fn address(&self) -> u16 {
        match self {
            Register::CellCount => 5000,
            Register::CellVoltage(n) => 5000 + *n as u16,
            Register::CellTemperatureCount => 5017,
            Register::CellTemperature(n) => 5017 + *n as u16,
            Register::Current => 5042,
            Register::ModuleVoltage => 5043,
        }
    }

    pub fn quantity(&self) -> u16 {
        match self {
            Register::CellCount => 1,
            Register::CellVoltage(_) => 1,
            Register::CellTemperatureCount => 1,
            Register::CellTemperature(_) => 1,
            Register::Current => 1,
            Register::ModuleVoltage => 1,
        }
    }

    pub fn parse_value(&self, data: &[u8]) -> Value {
        match self {
            Register::CellCount => {
                Value::Integer(BigEndian::read_u16(data) as i32)
            }
            Register::CellVoltage(_) => Value::ElectricPotential(ElectricPotential::new::<volt>(
                BigEndian::read_u16(data) as f32 * 0.1,
            )),
            Register::CellTemperatureCount => {
                Value::Integer(BigEndian::read_u16(data) as i32)
            }
            Register::CellTemperature(_) => Value::ThermodynamicTemperature(
                ThermodynamicTemperature::new::<degree_celsius>(
                    BigEndian::read_u16(data) as f32 * 0.1,
                ),
            ),
            Register::Current => Value::ElectricCurrent(ElectricCurrent::new::<ampere>(
                BigEndian::read_i16(data) as f32 * 0.01,
            )),
            Register::ModuleVoltage => Value::ElectricPotential(ElectricPotential::new::<volt>(
                BigEndian::read_u16(data) as f32 * 0.1,
            )),
        }
    }
}
