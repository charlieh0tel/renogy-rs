use agw::AGW;
use agw::Call;
use clap::Parser;
use renogy_rs::system_summary::SystemSummary;
use renogy_rs::vm_client::VmClient;
use std::time::Duration;
use std::time::Instant;
use tracing::debug;
use tracing::error;
use tracing::info;

const DEFAULT_BEACON_INTERVAL: u64 = 600; // 10 minutes
const DEFINITION_INTERVAL: u64 = 1800; // 30 minutes

#[derive(Parser)]
#[command(name = "renogy-aprs")]
#[command(about = "APRS telemetry beacon for Renogy BMS via Direwolf AGW interface")]
struct Args {
    /// APRS SSID, i.e. callsign-N (e.g., W1AW-12)
    #[arg(long)]
    ssid: String,

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

    if args.ssid.starts_with("N0CALL") {
        return Err("SSID starts with N0CALL; set a real SSID via --ssid or SSID env var".into());
    }

    info!(vm_url = %args.vm_url, agw = %format!("{}:{}", args.agw_host, args.agw_port), "Starting APRS beacon");

    let vm_client =
        VmClient::new(&args.vm_url).map_err(|e| format!("Failed to create VM client: {}", e))?;

    let src: Call = args
        .ssid
        .parse()
        .map_err(|e| format!("Invalid station ID: {}", e))?;
    let dst: Call = args
        .tocall
        .parse()
        .map_err(|e| format!("Invalid tocall: {}", e))?;
    let agw_addr = format!("{}:{}", args.agw_host, args.agw_port);

    info!(ssid = %args.ssid, interval = args.interval, "Configuration loaded");

    let mut last_definitions = Instant::now() - Duration::from_secs(DEFINITION_INTERVAL);
    let mut agw: Option<AGW> = None;

    loop {
        // Ensure we have an AGW connection
        if agw.is_none() {
            debug!(agw_addr = %agw_addr, "Connecting to AGW");
            match AGW::new(&agw_addr) {
                Ok(conn) => {
                    info!("Connected to AGW");
                    agw = Some(conn);
                }
                Err(e) => {
                    error!(error = %e, "Failed to connect to AGW at {}", agw_addr);
                    tokio::time::sleep(Duration::from_secs(args.interval)).await;
                    continue;
                }
            }
        }

        let agw_conn = agw.as_mut().unwrap();

        // Send definitions on startup and every 30 minutes
        if last_definitions.elapsed() >= Duration::from_secs(DEFINITION_INTERVAL) {
            match send_definitions(agw_conn, &src, &dst, &args.ssid) {
                Ok(()) => info!("Telemetry definitions sent"),
                Err(e) => {
                    error!(error = %e, "Failed to send definitions");
                    agw = None;
                    continue;
                }
            }
            last_definitions = Instant::now();
        }

        match query_and_beacon(&vm_client, agw_conn, &src, &dst).await {
            Ok(()) => info!("Telemetry beacon sent"),
            Err(e) => {
                error!(error = %e, "Failed to send beacon");
                agw = None;
                continue;
            }
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
    agw: &mut AGW,
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

    send_aprs_packet(agw, src, dst, &packet)?;

    Ok(())
}

fn format_telemetry_packet(summary: &SystemSummary) -> String {
    static SEQ: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    renogy_aprs::telemetry::format_telemetry_packet_seq(seq, summary)
}

fn send_aprs_packet(agw: &mut AGW, src: &Call, dst: &Call, data: &str) -> Result<(), String> {
    debug!(src = %src, dst = %dst, len = data.len(), "Sending unproto frame");
    agw.unproto(0, 0xF0, src, dst, data.as_bytes())
        .map_err(|e| format!("Failed to send packet: {}", e))?;

    debug!("Packet sent successfully");
    Ok(())
}

fn send_definitions(agw: &mut AGW, src: &Call, dst: &Call, callsign: &str) -> Result<(), String> {
    info!("Sending telemetry definitions");
    for packet in renogy_aprs::telemetry::definition_packets(callsign) {
        debug!(packet = %packet, "definition");
        send_aprs_packet(agw, src, dst, &packet)?;
    }
    info!("All definitions sent");
    Ok(())
}
