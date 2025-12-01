use agw::{AGW, Call};
use clap::Parser;
use renogy_rs::{SystemSummary, VmClient};
use std::time::{Duration, Instant};
use tracing::{debug, error, info};

const DEFAULT_BEACON_INTERVAL: u64 = 600; // 10 minutes
const DEFINITION_INTERVAL: u64 = 1800; // 30 minutes

#[derive(Parser)]
#[command(name = "renogy-aprs")]
#[command(about = "APRS telemetry beacon for Renogy BMS via Direwolf AGW interface")]
struct Args {
    /// APRS callsign with SSID (e.g., N0CALL-13)
    #[arg(long)]
    callsign: String,

    /// VictoriaMetrics URL
    #[arg(long, default_value = "http://localhost:8428")]
    vm_url: String,

    /// Direwolf AGW host
    #[arg(long, default_value = "localhost")]
    agw_host: String,

    /// Direwolf AGW port
    #[arg(long, default_value = "8000")]
    agw_port: u16,

    /// Beacon interval in seconds
    #[arg(long, default_value_t = DEFAULT_BEACON_INTERVAL)]
    interval: u64,

    /// Send once and exit (for testing)
    #[arg(long)]
    once: bool,

    /// APRS destination/TOCALL (default: APREN0)
    #[arg(long, default_value = "APREN0")]
    tocall: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    info!(vm_url = %args.vm_url, agw = %format!("{}:{}", args.agw_host, args.agw_port), "Starting APRS beacon");

    let vm_client =
        VmClient::new(&args.vm_url).map_err(|e| format!("Failed to create VM client: {}", e))?;

    let src: Call = args
        .callsign
        .parse()
        .map_err(|e| format!("Invalid callsign: {}", e))?;
    let dst: Call = args
        .tocall
        .parse()
        .map_err(|e| format!("Invalid tocall: {}", e))?;
    let agw_addr = format!("{}:{}", args.agw_host, args.agw_port);

    info!(callsign = %args.callsign, interval = args.interval, "Configuration loaded");

    let mut last_definitions = Instant::now() - Duration::from_secs(DEFINITION_INTERVAL);

    loop {
        // Send definitions on startup and every 30 minutes
        if last_definitions.elapsed() >= Duration::from_secs(DEFINITION_INTERVAL) {
            match send_definitions(&agw_addr, &src, &dst, &args.callsign) {
                Ok(()) => info!("Telemetry definitions sent"),
                Err(e) => error!(error = %e, "Failed to send definitions"),
            }
            last_definitions = Instant::now();
        }

        match query_and_beacon(&vm_client, &agw_addr, &src, &dst).await {
            Ok(()) => info!("Telemetry beacon sent"),
            Err(e) => error!(error = %e, "Failed to send beacon"),
        }

        if args.once {
            break;
        }

        debug!(interval = args.interval, "Sleeping until next beacon");
        tokio::time::sleep(Duration::from_secs(args.interval)).await;
    }

    Ok(())
}

async fn query_and_beacon(
    vm_client: &VmClient,
    agw_addr: &str,
    src: &Call,
    dst: &Call,
) -> Result<(), String> {
    debug!("Querying batteries from VictoriaMetrics");
    let batteries = vm_client
        .query_all_batteries()
        .await
        .map_err(|e| e.to_string())?;
    if batteries.is_empty() {
        return Err("No batteries found".to_string());
    }
    debug!(count = batteries.len(), "Found batteries");

    let summary = SystemSummary::new(&batteries);
    debug!(
        soc = summary.average_soc,
        voltage = summary.average_voltage,
        current = summary.total_current,
        temp = ?summary.average_temperature,
        "System summary computed"
    );

    let packet = format_telemetry_packet(&summary);
    debug!(packet = %packet, "Formatted telemetry packet");

    send_aprs_packet(agw_addr, src, dst, &packet)?;

    Ok(())
}

fn format_telemetry_packet(summary: &SystemSummary) -> String {
    static SEQ: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % 1000;

    // A1: SOC % (0-100)
    let a1 = (summary.average_soc.round() as u16).min(255);
    // A2: Remaining capacity in Ah (0-255)
    let a2 = (summary.total_remaining_ah.round() as u16).min(255);
    // A3: Voltage (0-255V)
    let a3 = (summary.average_voltage.round() as u16).min(255);
    // A4: Current + 128 offset (0-255 = -128 to +127 A)
    let a4 = ((summary.total_current + 128.0).round() as u16).clamp(0, 255);
    // A5: Temperature + 40 offset (0-255 = -40 to +215 C)
    let a5 = summary
        .average_temperature
        .map(|t| ((t + 40.0).round() as u16).clamp(0, 255))
        .unwrap_or(0);

    let binary = summary.alarms().to_aprs_binary_string();

    format!(
        "T#{:03},{:03},{:03},{:03},{:03},{:03},{}",
        seq, a1, a2, a3, a4, a5, binary
    )
}

fn send_aprs_packet(agw_addr: &str, src: &Call, dst: &Call, data: &str) -> Result<(), String> {
    debug!(agw_addr = %agw_addr, "Connecting to AGW");
    let mut agw = AGW::new(agw_addr)
        .map_err(|e| format!("Failed to connect to AGW at {}: {}", agw_addr, e))?;

    debug!(src = %src, dst = %dst, len = data.len(), "Sending unproto frame");
    agw.unproto(0, 0xF0, src, dst, data.as_bytes())
        .map_err(|e| format!("Failed to send packet: {}", e))?;

    debug!("Packet sent successfully");
    Ok(())
}

fn send_definitions(agw_addr: &str, src: &Call, dst: &Call, callsign: &str) -> Result<(), String> {
    info!("Sending telemetry definitions");

    // Pad callsign to 9 chars for message addressee
    let padded = format!("{:9}", callsign);

    // PARM - parameter names
    let parm = format!(
        ":{}:PARM.SOC,Capacity,Voltage,Current,Temp,OV,UV,OC,OT,UT,SC,Htr,Full",
        padded
    );
    debug!(packet = %parm, "PARM");
    send_aprs_packet(agw_addr, src, dst, &parm)?;

    // UNIT - units for each parameter
    let unit = format!(":{}:UNIT.%,Ah,V,A,C", padded);
    debug!(packet = %unit, "UNIT");
    send_aprs_packet(agw_addr, src, dst, &unit)?;

    // EQNS - coefficients: a*x^2 + b*x + c for each analog channel
    // A1: SOC (0-100, no transform) -> 0,1,0
    // A2: Capacity (0-255 Ah, no transform) -> 0,1,0
    // A3: Voltage (0-255V, no transform) -> 0,1,0
    // A4: Current (offset by 128) -> 0,1,-128
    // A5: Temp (offset by 40) -> 0,1,-40
    let eqns = format!(":{}:EQNS.0,1,0,0,1,0,0,1,0,0,1,-128,0,1,-40", padded);
    debug!(packet = %eqns, "EQNS");
    send_aprs_packet(agw_addr, src, dst, &eqns)?;

    // BITS - bit sense (all active high) + project title
    let bits = format!(":{}:BITS.11111111,Renogy BMS", padded);
    debug!(packet = %bits, "BITS");
    send_aprs_packet(agw_addr, src, dst, &bits)?;

    info!("All definitions sent");
    Ok(())
}
