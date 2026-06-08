use agw::AGW;
use agw::Call;
use clap::Parser;
use renogy_aprs::telemetry::definition_packets;
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
        // Ensure we have an AGW connection. AGW I/O is blocking, so the connect
        // and all sends run on a blocking thread to keep the reactor free.
        let conn = match agw.take() {
            Some(conn) => conn,
            None => {
                debug!(agw_addr = %agw_addr, "Connecting to AGW");
                let addr = agw_addr.clone();
                match tokio::task::spawn_blocking(move || AGW::new(&addr))
                    .await
                    .expect("AGW connect task panicked")
                {
                    Ok(conn) => {
                        info!("Connected to AGW");
                        conn
                    }
                    Err(e) => {
                        error!(error = %e, "Failed to connect to AGW at {}", agw_addr);
                        tokio::time::sleep(Duration::from_secs(args.interval)).await;
                        continue;
                    }
                }
            }
        };

        // Send definitions on startup and every 30 minutes.
        let conn = if last_definitions.elapsed() >= Duration::from_secs(DEFINITION_INTERVAL) {
            info!("Sending telemetry definitions");
            let packets = definition_packets(&args.ssid).to_vec();
            let (conn, result) = send_packets(conn, src.clone(), dst.clone(), packets).await;
            match result {
                Ok(()) => {
                    info!("Telemetry definitions sent");
                    last_definitions = Instant::now();
                    conn
                }
                Err(e) => {
                    error!(error = %e, "Failed to send definitions");
                    continue;
                }
            }
        } else {
            conn
        };

        let packet = match build_beacon_packet(&vm_client).await {
            Ok(packet) => packet,
            Err(e) => {
                error!(error = %e, "Failed to build beacon");
                agw = Some(conn);
                tokio::time::sleep(Duration::from_secs(args.interval)).await;
                continue;
            }
        };
        let (conn, result) = send_packets(conn, src.clone(), dst.clone(), vec![packet]).await;
        match result {
            Ok(()) => info!("Telemetry beacon sent"),
            Err(e) => {
                error!(error = %e, "Failed to send beacon");
                continue;
            }
        }
        agw = Some(conn);

        if args.once {
            break;
        }

        debug!(interval = args.interval, "Sleeping until next beacon");
        tokio::time::sleep(Duration::from_secs(args.interval)).await;
    }

    Ok(())
}

/// Send `packets` over the (blocking) AGW connection on a blocking thread,
/// returning the connection so the caller can reuse it.
async fn send_packets(
    mut agw: AGW,
    src: Call,
    dst: Call,
    packets: Vec<String>,
) -> (AGW, Result<(), String>) {
    tokio::task::spawn_blocking(move || {
        for packet in &packets {
            if let Err(e) = send_aprs_packet(&mut agw, &src, &dst, packet) {
                return (agw, Err(e));
            }
        }
        (agw, Ok(()))
    })
    .await
    .expect("AGW send task panicked")
}

async fn build_beacon_packet(vm_client: &VmClient) -> Result<String, String> {
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
    Ok(packet)
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
