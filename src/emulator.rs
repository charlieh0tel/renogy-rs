//! In-memory battery emulator: a `Transport` backed by a register map.
//!
//! Used by tests (and, behind the `emulator` feature, a future standalone Modbus
//! server) to feed the real collector pipeline without hardware. Register words are
//! produced by `Register::encode_value`, so the emulator stays in lockstep with the
//! parser -- no hand-coded wire formats to drift.

use std::collections::BTreeMap;

use async_trait::async_trait;

use crate::error::RenogyError;
use crate::error::Result;
use crate::registers::Register;
use crate::registers::Value;
use crate::transport::Transport;
use crate::transport::TransportType;
use uom::si::electric_current::ampere;
use uom::si::electric_potential::volt;
use uom::si::f32::ElectricCurrent;
use uom::si::f32::ElectricPotential;
use uom::si::f32::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::degree_celsius;

/// A fake BMS that answers `read_holding_registers` from an in-memory word map.
pub struct EmulatedBattery {
    slave: u8,
    words: BTreeMap<u16, u16>,
}

impl EmulatedBattery {
    #[must_use]
    pub fn new(slave: u8) -> Self {
        Self {
            slave,
            words: BTreeMap::new(),
        }
    }

    /// Set a register to `value`, encoding via the register's own serializer.
    pub fn set(&mut self, register: Register, value: &Value) -> Result<()> {
        let bytes = register.encode_value(value)?;
        let base = register.address();
        for (i, chunk) in bytes.chunks(2).enumerate() {
            let lo = chunk.get(1).copied().unwrap_or(0);
            self.words
                .insert(base + i as u16, u16::from_be_bytes([chunk[0], lo]));
        }
        Ok(())
    }

    pub fn set_integer(&mut self, register: Register, value: u32) -> Result<()> {
        self.set(register, &Value::Integer(value))
    }

    pub fn set_string(&mut self, register: Register, value: &str) -> Result<()> {
        self.set(register, &Value::String(value.to_string()))
    }

    pub fn set_voltage(&mut self, register: Register, volts: f32) -> Result<()> {
        self.set(
            register,
            &Value::ElectricPotential(ElectricPotential::new::<volt>(volts)),
        )
    }

    pub fn set_current(&mut self, register: Register, amps: f32) -> Result<()> {
        self.set(
            register,
            &Value::ElectricCurrent(ElectricCurrent::new::<ampere>(amps)),
        )
    }

    pub fn set_temperature(&mut self, register: Register, celsius: f32) -> Result<()> {
        self.set(
            register,
            &Value::ThermodynamicTemperature(ThermodynamicTemperature::new::<degree_celsius>(
                celsius,
            )),
        )
    }
}

#[async_trait]
impl Transport for EmulatedBattery {
    async fn read_holding_registers(
        &mut self,
        slave: u8,
        addr: u16,
        quantity: u16,
    ) -> Result<Vec<u16>> {
        if slave != self.slave {
            return Err(RenogyError::InvalidData);
        }
        Ok((addr..addr + quantity)
            .map(|a| self.words.get(&a).copied().unwrap_or(0))
            .collect())
    }

    async fn write_single_register(&mut self, _slave: u8, addr: u16, value: u16) -> Result<()> {
        self.words.insert(addr, value);
        Ok(())
    }

    async fn write_multiple_registers(
        &mut self,
        _slave: u8,
        addr: u16,
        values: &[u16],
    ) -> Result<()> {
        for (i, value) in values.iter().enumerate() {
            self.words.insert(addr + i as u16, *value);
        }
        Ok(())
    }

    async fn send_custom(
        &mut self,
        _slave: u8,
        _function_code: u8,
        _data: &[u8],
    ) -> Result<Vec<u8>> {
        Err(RenogyError::UnsupportedOperation)
    }

    fn transport_type(&self) -> TransportType {
        TransportType::Serial
    }
}

#[cfg(test)]
mod tests {
    use super::EmulatedBattery;
    use crate::query::query_battery;
    use crate::registers::Register;
    use crate::registers::Value;
    use uom::si::electric_current::ampere;
    use uom::si::electric_potential::volt;
    use uom::si::f32::ElectricCurrent;
    use uom::si::f32::ElectricPotential;

    fn volts(v: f32) -> Value {
        Value::ElectricPotential(ElectricPotential::new::<volt>(v))
    }

    fn amps(a: f32) -> Value {
        Value::ElectricCurrent(ElectricCurrent::new::<ampere>(a))
    }

    #[tokio::test]
    async fn query_battery_reads_emulated_values() {
        let addr = 0x30;
        let mut bms = EmulatedBattery::new(addr);
        bms.set(Register::SnNumber, &Value::String("SN1234".to_string()))
            .unwrap();
        bms.set(Register::CellCount, &Value::Integer(4)).unwrap();
        for cell in 1..=4 {
            bms.set(Register::CellVoltage(cell), &volts(3.30)).unwrap();
        }
        bms.set(Register::ModuleVoltage, &volts(13.2)).unwrap();
        bms.set(Register::Current, &amps(-5.0)).unwrap();
        bms.set(Register::RemainingCapacity, &amps(50.0)).unwrap();
        bms.set(Register::TotalCapacity, &amps(100.0)).unwrap();

        let info = query_battery(&mut bms, addr).await.expect("battery info");

        assert_eq!(info.serial, "SN1234");
        assert_eq!(info.cell_count, 4);
        assert_eq!(info.cell_voltages.len(), 4);
        assert!((info.module_voltage - 13.2).abs() < 1e-2);
        assert!((info.current + 5.0).abs() < 1e-2);
        assert!((info.soc_percent - 50.0).abs() < 1e-2);
    }

    #[tokio::test]
    async fn query_battery_rejects_wrong_slave() {
        let mut bms = EmulatedBattery::new(0x30);
        bms.set(Register::SnNumber, &Value::String("SN1234".to_string()))
            .unwrap();
        assert!(query_battery(&mut bms, 0x31).await.is_none());
    }
}
