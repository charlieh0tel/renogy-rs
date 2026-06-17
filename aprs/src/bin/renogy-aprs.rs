use clap::Parser;
use clap::ValueEnum;
use renogy_aprs::aprsis::passcode;
use renogy_aprs::sink::Packet;
use renogy_aprs::sink::SinkConfig;
use renogy_aprs::sink::Transport;
use renogy_aprs::sink::spawn_receivers;
use renogy_aprs::telemetry::definition_packets;
use renogy_rs::system_summary::SystemSummary;
use renogy_rs::vm_client::VmClient;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::broadcast;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::warn;

const DEFAULT_BEACON_INTERVAL: u64 = 600; // 10 minutes
const DEFINITION_INTERVAL: u64 = 1800; // 30 minutes
/// Broadcast pipe depth; far more than a receiver can fall behind between beacons.
const PIPE_DEPTH: usize = 16;

/// Output transports, selectable on the command line.
#[derive(Clone, Copy, Debug, ValueEnum)]
enum TransportArg {
    /// TNC only, via the Direwolf AGW interface.
    Agw,
    /// Internet only, via APRS-IS.
    AprsIs,
    /// Both the AGW TNC and APRS-IS.
    Both,
}

impl From<TransportArg> for Transport {
    fn from(arg: TransportArg) -> Self {
        match arg {
            TransportArg::Agw => Transport::Agw,
            TransportArg::AprsIs => Transport::AprsIs,
            TransportArg::Both => Transport::Both,
        }
    }
}

#[derive(Parser)]
#[command(name = "renogy-aprs")]
#[command(about = "APRS telemetry beacon for Renogy BMS via Direwolf AGW and/or APRS-IS")]
struct Args {
    /// APRS SSID, i.e. callsign-N (e.g., W1AW-12)
    #[arg(long)]
    ssid: String,

    /// VictoriaMetrics URL
    #[arg(long, default_value = "http://localhost:8428")]
    vm_url: String,

    /// Output transport(s)
    #[arg(long, value_enum, default_value_t = TransportArg::Agw, env = "APRS_TRANSPORT")]
    transport: TransportArg,

    /// Direwolf AGW host
    #[arg(long, default_value = "localhost")]
    agw_host: String,

    /// Direwolf AGW port
    #[arg(long, default_value = "8000")]
    agw_port: u16,

    /// APRS-IS server host
    #[arg(long, default_value = "rotate.aprs2.net", env = "APRSIS_HOST")]
    aprsis_host: String,

    /// APRS-IS server port
    #[arg(long, default_value = "14580", env = "APRSIS_PORT")]
    aprsis_port: u16,

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

    let transport: Transport = args.transport.into();
    let aprsis_passcode = passcode(&args.ssid);

    info!(
        ssid = %args.ssid,
        transport = ?transport,
        vm_url = %args.vm_url,
        interval = args.interval,
        "Starting APRS beacon"
    );

    let vm_client =
        VmClient::new(&args.vm_url).map_err(|e| format!("Failed to create VM client: {}", e))?;

    let agw_addr = format!("{}:{}", args.agw_host, args.agw_port);
    let config = SinkConfig {
        transport,
        src: &args.ssid,
        dst: &args.tocall,
        agw_addr: &agw_addr,
        aprsis_host: &args.aprsis_host,
        aprsis_port: args.aprsis_port,
        aprsis_passcode,
    };

    let (sender, _) = broadcast::channel::<Packet>(PIPE_DEPTH);
    let handles = spawn_receivers(&config, &sender)?;

    // Producer: queue definitions on startup then every 30 minutes, and a
    // telemetry frame every interval. Receivers transmit independently.
    let mut last_definitions = Instant::now() - Duration::from_secs(DEFINITION_INTERVAL);
    loop {
        if last_definitions.elapsed() >= Duration::from_secs(DEFINITION_INTERVAL) {
            queue(
                &sender,
                Packet::Definitions(definition_packets(&args.ssid).to_vec()),
            );
            last_definitions = Instant::now();
        }

        match build_beacon_packet(&vm_client).await {
            Ok(packet) => queue(&sender, Packet::Telemetry(packet)),
            Err(e) => error!(error = %e, "Failed to build beacon"),
        }

        if args.once {
            break;
        }

        debug!(interval = args.interval, "Sleeping until next beacon");
        tokio::time::sleep(Duration::from_secs(args.interval)).await;
    }

    // Close the pipe and let each receiver flush its queued packets.
    drop(sender);
    for handle in handles {
        let _ = handle.await;
    }

    Ok(())
}

fn queue(sender: &broadcast::Sender<Packet>, packet: Packet) {
    if let Err(e) = sender.send(packet) {
        warn!(error = %e, "No receivers for packet");
    }
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
