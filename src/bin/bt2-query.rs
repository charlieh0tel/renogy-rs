use clap::Parser;
use renogy_rs::{Bt2Transport, Register, Transport, Value, discover_bt2_devices};
use uom::si::electric_current::ampere;
use uom::si::electric_potential::volt;
use uom::si::thermodynamic_temperature::degree_celsius;

#[derive(Parser)]
#[command(name = "bt2-query")]
#[command(about = "Query Renogy BMS batteries via BT-2 Bluetooth adapter")]
struct Args {
    /// BT-2 MAC address (e.g., FD:86:6D:73:XX:XX). If not specified, discovers and uses the first BT-2 found.
    #[arg(short, long)]
    mac: Option<String>,

    /// Bluetooth adapter name
    #[arg(short, long, default_value = "hci0")]
    adapter: String,

    /// BMS addresses to scan (hex values like 0x30 or decimal)
    #[arg(short = 'b', long, value_parser = parse_address, default_values_t = vec![0x30, 0x31, 0x32, 0x33])]
    bms_addresses: Vec<u8>,
}

fn parse_address(s: &str) -> Result<u8, String> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u8::from_str_radix(hex, 16).map_err(|e| e.to_string())
    } else {
        s.parse()
            .map_err(|e: std::num::ParseIntError| e.to_string())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let mac_address = if let Some(mac) = args.mac {
        mac
    } else {
        println!("Discovering BT-2 devices...");
        let devices = discover_bt2_devices().await?;
        if devices.is_empty() {
            eprintln!("No BT-2 devices found. Specify a MAC address with --mac");
            std::process::exit(1);
        }
        for device in &devices {
            println!(
                "  Found: {} ({})",
                device.name.as_deref().unwrap_or("unknown"),
                device.address
            );
        }
        devices[0].address.clone()
    };

    println!("Connecting to {} via {}...", mac_address, args.adapter);

    let mut transport = Bt2Transport::connect_by_address(&mac_address, &args.adapter).await?;
    println!("Connected!\n");

    println!("Scanning for batteries...\n");

    for addr in args.bms_addresses {
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

async fn query_battery(transport: &mut Bt2Transport, addr: u8) -> Option<BatteryInfo> {
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
    transport: &mut Bt2Transport,
    addr: u8,
    register: Register,
) -> Result<Value, renogy_rs::RenogyError> {
    let regs = transport
        .read_holding_registers(addr, register.address(), register.quantity())
        .await?;
    Ok(register.parse_registers(&regs))
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
