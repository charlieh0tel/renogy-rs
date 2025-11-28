use crate::error::Result;
use crate::registers::{Register, Value};
use crate::transport::Transport;
use uom::si::electric_current::ampere;
use uom::si::electric_potential::volt;
use uom::si::thermodynamic_temperature::degree_celsius;

pub struct BatteryInfo {
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
    let serial = match read_register(transport, addr, Register::SnNumber).await {
        Ok(Value::String(s)) => s.trim_matches('\0').to_string(),
        _ => return None,
    };

    let model = match read_register(transport, addr, Register::BatteryName).await {
        Ok(Value::String(s)) => s.trim_matches('\0').to_string(),
        _ => String::new(),
    };

    let software_version = match read_register(transport, addr, Register::SoftwareVersion).await {
        Ok(Value::String(s)) => s.trim_matches('\0').to_string(),
        _ => String::new(),
    };

    let manufacturer = match read_register(transport, addr, Register::ManufacturerName).await {
        Ok(Value::String(s)) => s.trim_matches('\0').to_string(),
        _ => String::new(),
    };

    let cell_count = match read_register(transport, addr, Register::CellCount).await {
        Ok(Value::Integer(n)) => n,
        _ => return None,
    };

    let mut cell_voltages = Vec::new();
    for i in 1..=cell_count.min(16) {
        if let Ok(Value::ElectricPotential(v)) =
            read_register(transport, addr, Register::CellVoltage(i as u8)).await
        {
            cell_voltages.push(v.get::<volt>());
        }
    }

    let module_voltage = match read_register(transport, addr, Register::ModuleVoltage).await {
        Ok(Value::ElectricPotential(v)) => v.get::<volt>(),
        _ => 0.0,
    };

    let current = match read_register(transport, addr, Register::Current).await {
        Ok(Value::ElectricCurrent(c)) => c.get::<ampere>(),
        _ => 0.0,
    };

    let remaining_capacity = match read_register(transport, addr, Register::RemainingCapacity).await
    {
        Ok(Value::ElectricCurrent(c)) => c.get::<ampere>(),
        _ => 0.0,
    };

    let total_capacity = match read_register(transport, addr, Register::TotalCapacity).await {
        Ok(Value::ElectricCurrent(c)) => c.get::<ampere>(),
        _ => 0.0,
    };

    let soc_percent = if total_capacity > 0.0 {
        (remaining_capacity / total_capacity) * 100.0
    } else {
        0.0
    };

    let cycle_count = match read_register(transport, addr, Register::CycleNumber).await {
        Ok(Value::Integer(n)) => n,
        _ => 0,
    };

    let cell_temp_count = match read_register(transport, addr, Register::CellTemperatureCount).await
    {
        Ok(Value::Integer(n)) => n,
        _ => 0,
    };

    let mut cell_temperatures = Vec::new();
    for i in 1..=cell_temp_count.min(16) {
        if let Ok(Value::ThermodynamicTemperature(t)) =
            read_register(transport, addr, Register::CellTemperature(i as u8)).await
        {
            cell_temperatures.push(t.get::<degree_celsius>());
        }
    }

    Some(BatteryInfo {
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
) -> Result<Value> {
    let regs = transport
        .read_holding_registers(addr, register.address(), register.quantity())
        .await?;
    Ok(register.parse_registers(&regs))
}
