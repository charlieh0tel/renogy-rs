use crate::registers::{Register, Value};
use crate::transport::Transport;
use chrono::{DateTime, Utc};
use uom::si::electric_current::ampere;
use uom::si::electric_potential::volt;
use uom::si::thermodynamic_temperature::degree_celsius;

#[derive(Clone, Debug)]
pub struct BatteryInfo {
    pub timestamp: DateTime<Utc>,
    pub serial: String,
    pub model: String,
    pub software_version: String,
    pub manufacturer: String,
    pub cell_count: u32,
    pub cell_voltages: Vec<f32>,
    pub cell_temperatures: Vec<f32>,
    pub module_voltage: f32,
    pub current: f32,
    pub remaining_capacity: f32,
    pub total_capacity: f32,
    pub soc_percent: f32,
    pub cycle_count: u32,
}

pub async fn query_battery<T: Transport>(transport: &mut T, addr: u8) -> Option<BatteryInfo> {
    let serial = read_string(transport, addr, Register::SnNumber).await?;
    let model = read_string(transport, addr, Register::BatteryName)
        .await
        .unwrap_or_default();
    let software_version = read_string(transport, addr, Register::SoftwareVersion)
        .await
        .unwrap_or_default();
    let manufacturer = read_string(transport, addr, Register::ManufacturerName)
        .await
        .unwrap_or_default();

    let cell_count = read_integer(transport, addr, Register::CellCount).await?;

    let mut cell_voltages = Vec::with_capacity(cell_count.min(16) as usize);
    for i in 1..=cell_count.min(16) {
        if let Some(v) = read_voltage(transport, addr, Register::CellVoltage(i as u8)).await {
            cell_voltages.push(v);
        }
    }

    let module_voltage = read_voltage(transport, addr, Register::ModuleVoltage)
        .await
        .unwrap_or(0.0);
    let current = read_current(transport, addr, Register::Current)
        .await
        .unwrap_or(0.0);
    let remaining_capacity = read_current(transport, addr, Register::RemainingCapacity)
        .await
        .unwrap_or(0.0);
    let total_capacity = read_current(transport, addr, Register::TotalCapacity)
        .await
        .unwrap_or(0.0);

    let soc_percent = if total_capacity > 0.0 {
        (remaining_capacity / total_capacity) * 100.0
    } else {
        0.0
    };

    let cycle_count = read_integer(transport, addr, Register::CycleNumber)
        .await
        .unwrap_or(0);

    let cell_temp_count = read_integer(transport, addr, Register::CellTemperatureCount)
        .await
        .unwrap_or(0);

    let mut cell_temperatures = Vec::with_capacity(cell_temp_count.min(16) as usize);
    for i in 1..=cell_temp_count.min(16) {
        if let Some(t) = read_temperature(transport, addr, Register::CellTemperature(i as u8)).await
        {
            cell_temperatures.push(t);
        }
    }

    Some(BatteryInfo {
        timestamp: Utc::now(),
        serial,
        model,
        software_version,
        manufacturer,
        cell_count,
        cell_voltages,
        cell_temperatures,
        module_voltage,
        current,
        remaining_capacity,
        total_capacity,
        soc_percent,
        cycle_count,
    })
}

async fn read_register<T: Transport>(
    transport: &mut T,
    addr: u8,
    register: Register,
) -> Option<Value> {
    let regs = transport
        .read_holding_registers(addr, register.address(), register.quantity())
        .await
        .ok()?;
    Some(register.parse_registers(&regs))
}

async fn read_string<T: Transport>(
    transport: &mut T,
    addr: u8,
    register: Register,
) -> Option<String> {
    read_register(transport, addr, register)
        .await?
        .as_string()
        .map(|s| s.trim_matches('\0').to_string())
}

async fn read_integer<T: Transport>(
    transport: &mut T,
    addr: u8,
    register: Register,
) -> Option<u32> {
    read_register(transport, addr, register).await?.as_integer()
}

async fn read_voltage<T: Transport>(
    transport: &mut T,
    addr: u8,
    register: Register,
) -> Option<f32> {
    read_register(transport, addr, register)
        .await?
        .as_voltage()
        .map(|v| v.get::<volt>())
}

async fn read_current<T: Transport>(
    transport: &mut T,
    addr: u8,
    register: Register,
) -> Option<f32> {
    read_register(transport, addr, register)
        .await?
        .as_current()
        .map(|c| c.get::<ampere>())
}

async fn read_temperature<T: Transport>(
    transport: &mut T,
    addr: u8,
    register: Register,
) -> Option<f32> {
    read_register(transport, addr, register)
        .await?
        .as_temperature()
        .map(|t| t.get::<degree_celsius>())
}
