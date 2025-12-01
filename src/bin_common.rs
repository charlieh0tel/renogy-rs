use renogy_rs::{BatteryInfo, Status1, Status2};

#[allow(dead_code)]
pub fn print_battery_info(addr: u8, info: &BatteryInfo) {
    println!("═══════════════════════════════════════════════════════════");
    println!("Battery 0x{:02X}", addr);
    println!("═══════════════════════════════════════════════════════════");
    println!("  Model: {}  Serial: {}", info.model, info.serial);
    println!(
        "  Manufacturer: {}  Version: {}",
        info.manufacturer, info.software_version
    );
    println!(
        "  Module Voltage: {:.1} V    Current: {:+.2} A",
        info.module_voltage, info.current
    );
    println!(
        "  Capacity: {:.1} / {:.1} Ah ({:.1}%)",
        info.remaining_capacity, info.total_capacity, info.soc_percent
    );
    if let (Some(min_temp), Some(max_temp)) = (
        info.cell_temperatures.iter().copied().reduce(f32::min),
        info.cell_temperatures.iter().copied().reduce(f32::max),
    ) {
        println!(
            "  Cycles: {}    Temp: {:.1}-{:.1} °C ({} sensors)",
            info.cycle_count,
            min_temp,
            max_temp,
            info.cell_temperatures.len()
        );
    } else {
        println!("  Cycles: {}", info.cycle_count);
    }

    print_temperatures(info);
    print_limits(info);
    println!();
    print_cell_voltages(info);
    print_status(info);
}

fn print_temperatures(info: &BatteryInfo) {
    let mut temps = Vec::new();
    if let Some(t) = info.bms_temperature {
        temps.push(format!("BMS: {:.1}°C", t));
    }
    for (i, t) in info.environment_temperatures.iter().enumerate() {
        temps.push(format!("Env{}: {:.1}°C", i + 1, t));
    }
    for (i, t) in info.heater_temperatures.iter().enumerate() {
        temps.push(format!("Heater{}: {:.1}°C", i + 1, t));
    }
    if !temps.is_empty() {
        println!("  Other Temps: {}", temps.join("  "));
    }
}

fn print_limits(info: &BatteryInfo) {
    if let (Some(cv), Some(dv)) = (info.charge_voltage_limit, info.discharge_voltage_limit) {
        println!(
            "  Limits: Voltage {:.1}-{:.1}V  Current {:.2}/{:.2}A",
            dv,
            cv,
            info.charge_current_limit.unwrap_or(0.0),
            info.discharge_current_limit.unwrap_or(0.0)
        );
    }
}

fn print_cell_voltages(info: &BatteryInfo) {
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

    if let (Some(min), Some(max)) = (
        info.cell_voltages.iter().copied().reduce(f32::min),
        info.cell_voltages.iter().copied().reduce(f32::max),
    ) {
        println!(
            "    Min: {:.3}V  Max: {:.3}V  Delta: {:.0}mV",
            min,
            max,
            (max - min) * 1000.0
        );
    }
    println!();
}

fn print_status(info: &BatteryInfo) {
    if let Some(status) = info.charge_discharge_status {
        let flags: Vec<_> = status.iter_names().map(|(name, _)| name).collect();
        if !flags.is_empty() {
            println!("  Charge/Discharge: {}", flags.join(", "));
        }
    }

    if let Some(s1) = info.status1 {
        println!(
            "  MOSFETs: Charge={} Discharge={}",
            if s1.contains(Status1::CHARGE_MOSFET) {
                "ON"
            } else {
                "OFF"
            },
            if s1.contains(Status1::DISCHARGE_MOSFET) {
                "ON"
            } else {
                "OFF"
            }
        );
    }

    if let Some(s2) = info.status2 {
        if s2.contains(Status2::FULLY_CHARGED) {
            println!("  State: FULLY CHARGED");
        }
        if s2.contains(Status2::HEATER_ON) {
            println!("  Heater: ON");
        }
    }

    print_alarms(info);
}

fn print_alarms(info: &BatteryInfo) {
    let alarms = info.active_alarms();
    if !alarms.is_empty() {
        println!();
        println!("  *** ALARMS ***");
        for alarm in alarms {
            println!("    - {}", alarm);
        }
    }
    println!();
}

pub fn parse_address(s: &str) -> Result<u8, String> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u8::from_str_radix(hex, 16).map_err(|e| e.to_string())
    } else {
        s.parse()
            .map_err(|e: std::num::ParseIntError| e.to_string())
    }
}
