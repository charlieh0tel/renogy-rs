#[path = "../bin_common.rs"]
mod common;

use clap::{Parser, Subcommand};
use common::parse_address;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use renogy_rs::{
    Bt2Transport, SerialTransport, discover_bt2_devices, query_battery,
    tui::{App, Event, EventHandler, draw},
};
use std::io::stdout;
use std::time::{Duration, Instant};

const REFRESH_INTERVAL: Duration = Duration::from_secs(15);
const TICK_RATE: Duration = Duration::from_millis(250);

#[derive(Parser)]
#[command(name = "renogy-tui")]
#[command(about = "TUI monitor for Renogy BMS batteries")]
struct Args {
    #[command(subcommand)]
    transport: TransportCmd,
}

#[derive(Subcommand)]
enum TransportCmd {
    /// Connect via BT-2 Bluetooth adapter
    Bt2 {
        /// BT-2 MAC address. If not specified, discovers and uses the first BT-2 found.
        #[arg(short, long)]
        mac: Option<String>,

        /// Bluetooth adapter name
        #[arg(short, long, default_value = "hci0")]
        adapter: String,

        /// BMS addresses to monitor. If not specified, scans 0x30-0x3F to discover batteries.
        #[arg(short = 'b', long, value_parser = parse_address)]
        bms_addresses: Vec<u8>,
    },
    /// Connect via serial/RS-485
    Serial {
        /// Serial port path
        #[arg(short, long)]
        port: String,

        /// Baud rate
        #[arg(short = 'r', long, default_value_t = 9600)]
        baud_rate: u32,

        /// BMS addresses to monitor. If not specified, scans 0x01-0x10 to discover batteries.
        #[arg(short, long, value_parser = parse_address)]
        bms_addresses: Vec<u8>,
    },
}

const BT2_SCAN_RANGE: std::ops::RangeInclusive<u8> = 0x30..=0x3F;
const SERIAL_SCAN_RANGE: std::ops::RangeInclusive<u8> = 0x01..=0x10;

enum AnyTransport {
    Bt2(Bt2Transport),
    Serial(SerialTransport),
}

impl AnyTransport {
    async fn query_battery(&mut self, addr: u8) -> Option<renogy_rs::BatteryInfo> {
        match self {
            AnyTransport::Bt2(t) => query_battery(t, addr).await,
            AnyTransport::Serial(t) => query_battery(t, addr).await,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    match args.transport {
        TransportCmd::Bt2 {
            mac,
            adapter,
            bms_addresses,
        } => {
            let mac_address = if let Some(mac) = mac {
                mac
            } else {
                eprintln!("Discovering BT-2 devices...");
                let devices = discover_bt2_devices().await?;
                if devices.is_empty() {
                    eprintln!("No BT-2 devices found. Specify a MAC address with --mac");
                    std::process::exit(1);
                }
                for device in &devices {
                    eprintln!(
                        "  Found: {} ({})",
                        device.name.as_deref().unwrap_or("unknown"),
                        device.address
                    );
                }
                devices[0].address.clone()
            };

            eprintln!("Connecting to {} via {}...", mac_address, adapter);
            let mut transport =
                AnyTransport::Bt2(Bt2Transport::connect_by_address(&mac_address, &adapter).await?);

            let addresses = if bms_addresses.is_empty() {
                discover_batteries(&mut transport, BT2_SCAN_RANGE).await
            } else {
                bms_addresses
            };

            if addresses.is_empty() {
                eprintln!("No batteries found!");
                std::process::exit(1);
            }

            run_tui(transport, addresses).await
        }
        TransportCmd::Serial {
            port,
            baud_rate,
            bms_addresses,
        } => {
            eprintln!("Opening {} at {} baud...", port, baud_rate);
            let first_addr = bms_addresses.first().copied().unwrap_or(0x01);
            let mut transport =
                AnyTransport::Serial(SerialTransport::new(&port, baud_rate, first_addr).await?);

            let addresses = if bms_addresses.is_empty() {
                discover_batteries(&mut transport, SERIAL_SCAN_RANGE).await
            } else {
                bms_addresses
            };

            if addresses.is_empty() {
                eprintln!("No batteries found!");
                std::process::exit(1);
            }

            run_tui(transport, addresses).await
        }
    }
}

async fn discover_batteries(
    transport: &mut AnyTransport,
    range: std::ops::RangeInclusive<u8>,
) -> Vec<u8> {
    eprintln!(
        "Scanning for batteries at 0x{:02X}-0x{:02X}...",
        range.start(),
        range.end()
    );

    let mut found = Vec::new();
    for addr in range {
        eprint!("  0x{:02X}... ", addr);
        if let Some(info) = transport.query_battery(addr).await {
            eprintln!("found: {}", info.model);
            found.push(addr);
        } else {
            eprintln!("-");
            break;
        }
    }

    eprintln!("Found {} battery(s)", found.len());
    found
}

async fn run_tui(
    mut transport: AnyTransport,
    addresses: Vec<u8>,
) -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(addresses.clone());
    let mut events = EventHandler::new(TICK_RATE);
    let mut last_refresh = Instant::now() - REFRESH_INTERVAL;

    let result = run_event_loop(
        &mut terminal,
        &mut app,
        &mut events,
        &mut transport,
        &mut last_refresh,
        &addresses,
    )
    .await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

async fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
    events: &mut EventHandler,
    transport: &mut AnyTransport,
    last_refresh: &mut Instant,
    addresses: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    while app.running {
        terminal.draw(|f| draw(f, app))?;

        let should_refresh = last_refresh.elapsed() >= REFRESH_INTERVAL;

        if let Some(event) = events.next().await {
            match event {
                Event::Quit => app.running = false,
                Event::Refresh => {
                    refresh_batteries(app, transport, addresses).await;
                    *last_refresh = Instant::now();
                }
                Event::Tick if should_refresh => {
                    refresh_batteries(app, transport, addresses).await;
                    *last_refresh = Instant::now();
                }
                Event::Key(key) => {
                    use crossterm::event::KeyCode;
                    match key.code {
                        KeyCode::Up | KeyCode::Char('k') => app.select_previous(),
                        KeyCode::Down | KeyCode::Char('j') => app.select_next(),
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    Ok(())
}

async fn refresh_batteries(app: &mut App, transport: &mut AnyTransport, addresses: &[u8]) {
    app.refreshing = true;
    app.error = None;

    for &addr in addresses {
        let info = transport.query_battery(addr).await;
        app.update_battery(addr, info);
    }

    app.refreshing = false;
}
