use renogy_rs::{FunctionCode, Pdu, Register, SerialTransport, Transport, Value};
use std::env;
use uom::si::electric_current::ampere;
use uom::si::electric_potential::volt;
use uom::si::thermodynamic_temperature::degree_celsius;

const DEFAULT_ADDRESSES: [u8; 4] = [0x01, 0x02, 0x03, 0x04];

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: serial-query <SERIAL_PORT> [BAUD_RATE]");
        eprintln!("  Example: serial-query /dev/ttyUSB0");
        eprintln!("  Example: serial-query /dev/ttyUSB0 9600");
        eprintln!();
        eprintln!("Environment variables:");
        eprintln!("  BMS_ADDRESSES - comma-separated list of addresses to scan (default: 1,2,3,4)");
        std::process::exit(1);
    }

    let port = &args[1];
    let baud_rate: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(9600);

    let addresses: Vec<u8> = env::var("BMS_ADDRESSES")
        .ok()
        .map(|s| {
            s.split(',')
                .filter_map(|addr| {
                    let addr = addr.trim();
                    if let Some(hex) = addr.strip_prefix("0x") {
                        u8::from_str_radix(hex, 16).ok()
                    } else {
                        addr.parse().ok()
                    }
                })
                .collect()
        })
        .unwrap_or_else(|| DEFAULT_ADDRESSES.to_vec());

    println!("Opening {} at {} baud...", port, baud_rate);
    let mut transport = SerialTransport::new(port, baud_rate, addresses[0]).await?;
    println!("Connected!\n");

    println!("Scanning for batteries at addresses: {:?}\n", addresses);

    for addr in addresses {
        if let Some(info) = query_battery(&mut transport, addr).await {
            print_battery_info(addr, &info);
        }
    }

    Ok(())
}

struct BatteryInfo {
    serial: String,
    cell_count: u32,
    cell_voltages: Vec<f32>,
    module_voltage: f32,
    current: f32,
    remaining_capacity: f32,
    total_capacity: f32,
    soc_percent: f32,
    cycle_count: u32,
    bms_temp: f32,
}

async fn query_battery(transport: &mut SerialTransport, addr: u8) -> Option<BatteryInfo> {
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

async fn read_register(
    transport: &mut SerialTransport,
    addr: u8,
    register: Register,
) -> Result<Value, renogy_rs::RenogyError> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&register.address().to_be_bytes());
    payload.extend_from_slice(&register.quantity().to_be_bytes());

    let pdu = Pdu::new(addr, FunctionCode::ReadHoldingRegisters, payload);
    let response = transport.send_receive(&pdu).await?;

    let data = if !response.payload.is_empty()
        && (response.payload[0] as usize) < response.payload.len()
    {
        &response.payload[1..]
    } else {
        &response.payload
    };

    Ok(register.parse_value(data))
}

fn print_battery_info(addr: u8, info: &BatteryInfo) {
    println!("═══════════════════════════════════════════════════════════");
    println!("Battery 0x{:02X} - {}", addr, info.serial);
    println!("═══════════════════════════════════════════════════════════");
    println!(
        "  Module Voltage: {:.1} V    Current: {:+.2} A",
        info.module_voltage, info.current
    );
    println!(
        "  Capacity: {:.1} / {:.1} Ah ({:.1}%)",
        info.remaining_capacity, info.total_capacity, info.soc_percent
    );
    println!(
        "  Cycles: {}    BMS Temp: {:.1} °C",
        info.cycle_count, info.bms_temp
    );
    println!();
    println!("  Cell Voltages ({} cells):", info.cell_count);
    for (i, voltage) in info.cell_voltages.iter().enumerate() {
        if i % 4 == 0 {
            print!("    ");
        }
        print!("C{:02}: {:.3}V  ", i + 1, voltage);
        if (i + 1) % 4 == 0 {
            println!();
        }
    }
    if !info.cell_voltages.len().is_multiple_of(4) {
        println!();
    }

    if !info.cell_voltages.is_empty() {
        let min = info
            .cell_voltages
            .iter()
            .cloned()
            .fold(f32::INFINITY, f32::min);
        let max = info
            .cell_voltages
            .iter()
            .cloned()
            .fold(f32::NEG_INFINITY, f32::max);
        println!(
            "    Min: {:.3}V  Max: {:.3}V  Delta: {:.0}mV",
            min,
            max,
            (max - min) * 1000.0
        );
    }
    println!();
}
