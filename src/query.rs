use crate::error::Result;
use crate::registers::{Register, Value};
use crate::transport::Transport;
use uom::si::electric_current::ampere;
use uom::si::electric_potential::volt;
use uom::si::thermodynamic_temperature::degree_celsius;

pub struct BatteryInfo {
    pub serial: String,
    pub cell_count: u32,
    pub cell_voltages: Vec<f32>,
    pub module_voltage: f32,
    pub current: f32,
    pub remaining_capacity: f32,
    pub total_capacity: f32,
    pub soc_percent: f32,
    pub cycle_count: u32,
    pub bms_temp: f32,
}

pub async fn query_battery<T: Transport>(transport: &mut T, addr: u8) -> Option<BatteryInfo> {
    let serial = match read_register(transport, addr, Register::SnNumber).await {
        Ok(Value::String(s)) => s.trim_matches('\0').to_string(),
        _ => return None,
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

    let bms_temp = match read_register(transport, addr, Register::BmsTemperature).await {
        Ok(Value::ThermodynamicTemperature(t)) => t.get::<degree_celsius>(),
        _ => 0.0,
    };

    Some(BatteryInfo {
        serial,
        cell_count,
        cell_voltages,
        module_voltage,
        current,
        remaining_capacity,
        total_capacity,
        soc_percent,
        cycle_count,
        bms_temp,
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
