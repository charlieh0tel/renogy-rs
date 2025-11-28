use renogy_rs::BatteryInfo;

pub fn print_battery_info(addr: u8, info: &BatteryInfo) {
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

pub fn parse_address(s: &str) -> Result<u8, String> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u8::from_str_radix(hex, 16).map_err(|e| e.to_string())
    } else {
        s.parse()
            .map_err(|e: std::num::ParseIntError| e.to_string())
    }
}
