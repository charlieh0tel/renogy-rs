#[path = "../bin_common.rs"]
mod common;

use clap::Parser;
use common::{parse_address, print_battery_info};
use renogy_rs::{SerialTransport, query_battery};

#[derive(Parser)]
#[command(name = "serial-query")]
#[command(about = "Query Renogy BMS batteries via serial/RS-485")]
struct Args {
    /// Serial port path (e.g., /dev/ttyUSB0 or COM3)
    #[arg(short, long)]
    port: String,

    /// Baud rate
    #[arg(short = 'r', long, default_value_t = 9600)]
    baud_rate: u32,

    /// BMS addresses to scan (hex values like 0x01 or decimal)
    #[arg(short, long, value_parser = parse_address, default_values_t = vec![0x01, 0x02, 0x03, 0x04])]
    bms_addresses: Vec<u8>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    println!("Opening {} at {} baud...", args.port, args.baud_rate);
    let mut transport =
        SerialTransport::new(&args.port, args.baud_rate, args.bms_addresses[0]).await?;
    println!("Connected!\n");

    println!(
        "Scanning for batteries at addresses: {:02X?}\n",
        args.bms_addresses
    );

    for addr in args.bms_addresses {
        if let Some(info) = query_battery(&mut transport, addr).await {
            print_battery_info(addr, &info);
        }
    }

    Ok(())
}
