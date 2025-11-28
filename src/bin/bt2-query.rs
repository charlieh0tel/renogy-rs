#[path = "../bin_common.rs"]
mod common;

use clap::Parser;
use common::{parse_address, print_battery_info};
use renogy_rs::{Bt2Transport, discover_bt2_devices, query_battery};

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
