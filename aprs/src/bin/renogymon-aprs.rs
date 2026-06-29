use clap::Parser;
use clap::ValueEnum;
use renogy::system_summary::SystemSummary;
use renogy::vm_client::VmClient;
use renogymon_aprs::aprsis::passcode;
use renogymon_aprs::callsign::PLACEHOLDER;
use renogymon_aprs::callsign::Ssid;
use renogymon_aprs::position::format_position;
use renogymon_aprs::position::read_fix;
use renogymon_aprs::sink::Packet;
use renogymon_aprs::sink::SinkConfig;
use renogymon_aprs::sink::Transport;
use renogymon_aprs::sink::spawn_receivers;
use renogymon_aprs::telemetry::definition_packets;
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
/// Default gpsd port when `--gpsd` gives only a host.
const DEFAULT_GPSD_PORT: u16 = 2947;
/// Default seconds to wait for a gpsd fix at startup.
const DEFAULT_GPSD_FIX_TIMEOUT: u64 = 30;

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
#[command(name = "renogymon-aprs")]
#[command(about = "APRS telemetry beacon for Renogy BMS via Direwolf AGW and/or APRS-IS")]
struct Args {
    /// APRS SSID, i.e. callsign-N (e.g., W1AW-12). The licensed station: drives
    /// the APRS-IS login and passcode, and identifies the operator when --tactical
    /// is used.
    #[arg(long)]
    ssid: String,

    /// Tactical source callsign (e.g., SOLAR1). When set, beacons are sourced from
    /// it and the operator's base callsign (--ssid without the SSID suffix) is
    /// appended to each telemetry packet as an identifying comment.
    #[arg(long, env = "APRS_TACTICAL")]
    tactical: Option<String>,

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

    /// Static station latitude in decimal degrees (requires --longitude)
    #[arg(long, env = "APRS_LATITUDE", allow_hyphen_values = true)]
    latitude: Option<f64>,

    /// Static station longitude in decimal degrees (requires --latitude)
    #[arg(long, env = "APRS_LONGITUDE", allow_hyphen_values = true)]
    longitude: Option<f64>,

    /// Read the station position once from gpsd at HOST[:PORT] (e.g. localhost:2947)
    #[arg(long, env = "APRS_GPSD")]
    gpsd: Option<String>,

    /// Seconds to wait for a gpsd fix at startup before exiting (systemd then retries)
    #[arg(long, default_value_t = DEFAULT_GPSD_FIX_TIMEOUT, env = "APRS_GPSD_FIX_TIMEOUT")]
    gpsd_fix_timeout: u64,

    /// APRS symbol: table selector plus symbol code (e.g. /- for a house)
    #[arg(long, default_value = "/-", env = "APRS_SYMBOL")]
    symbol: String,

    /// Comment appended to the position beacon
    #[arg(long, env = "APRS_POSITION_COMMENT")]
    position_comment: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    let ssid: Ssid = args
        .ssid
        .parse()
        .map_err(|e| format!("Invalid --ssid: {e}"))?;

    if ssid.base_call().is_placeholder() {
        return Err(format!(
            "SSID is the placeholder {PLACEHOLDER}; set a real SSID via --ssid or SSID env var"
        )
        .into());
    }

    if args.latitude.is_some() != args.longitude.is_some() {
        return Err("--latitude and --longitude must be given together".into());
    }

    if args.symbol.chars().count() != 2 {
        return Err("--symbol must be exactly two characters (table selector + code)".into());
    }

    let transport: Transport = args.transport.into();
    let aprsis_passcode = passcode(&ssid.base_call());

    // Beacons are sourced from the tactical call when set, otherwise the operator
    // station. With a tactical call, the operator's base callsign (the licensed
    // call without the APRS SSID) rides along as a telemetry comment for station
    // identification.
    let source = args.tactical.as_deref().unwrap_or(ssid.as_str());
    let operator_call = args.tactical.as_ref().map(|_| ssid.base_call());
    let operator: Option<&str> = operator_call.as_ref().map(|call| call.as_str());

    info!(
        ssid = %ssid,
        source = %source,
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
        src: source,
        login: ssid.as_str(),
        dst: &args.tocall,
        agw_addr: &agw_addr,
        aprsis_host: &args.aprsis_host,
        aprsis_port: args.aprsis_port,
        aprsis_passcode,
    };

    // The station does not move, so resolve the position once at startup.
    let position = resolve_position(&args).await?;

    let (sender, _) = broadcast::channel::<Packet>(PIPE_DEPTH);
    let handles = spawn_receivers(&config, &sender)?;

    // Producer: queue definitions on startup then every 30 minutes, and a
    // position plus telemetry frame every interval. Receivers transmit
    // independently.
    let mut last_definitions = Instant::now() - Duration::from_secs(DEFINITION_INTERVAL);
    loop {
        if last_definitions.elapsed() >= Duration::from_secs(DEFINITION_INTERVAL) {
            queue(
                &sender,
                Packet::Definitions(definition_packets(source).to_vec()),
            );
            last_definitions = Instant::now();
        }

        // Position precedes telemetry so aprs.fi has a located station to attach
        // the telemetry to (it drops telemetry from position-less stations).
        if let Some(position) = &position {
            queue(&sender, Packet::Position(position.clone()));
        }

        match build_beacon_packet(&vm_client, operator).await {
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

/// Resolve the fixed station position once: static coordinates take precedence,
/// otherwise a single gpsd read. Returns `Ok(None)` (no position beacon) when
/// neither is configured. When gpsd is configured but no fix is obtained, returns
/// an error so the process exits and systemd restarts it to retry.
async fn resolve_position(args: &Args) -> Result<Option<String>, String> {
    let comment = args.position_comment.as_deref();

    if let (Some(lat), Some(lon)) = (args.latitude, args.longitude) {
        if args.gpsd.is_some() {
            warn!("Both static coordinates and --gpsd given; using static coordinates");
        }
        info!(lat, lon, "Using static station position");
        return Ok(Some(format_position(lat, lon, &args.symbol, comment)));
    }

    let Some(addr) = args.gpsd.as_deref() else {
        return Ok(None);
    };
    let (host, port) = parse_host_port(addr);
    let fix_wait = Duration::from_secs(args.gpsd_fix_timeout);
    let (lat, lon) = read_fix(host, port, fix_wait)
        .await
        .map_err(|e| format!("gpsd fix failed ({e}); exiting so systemd retries"))?;
    info!(lat, lon, "Read station position from gpsd");
    Ok(Some(format_position(lat, lon, &args.symbol, comment)))
}

/// Split `HOST[:PORT]`, defaulting the port to [`DEFAULT_GPSD_PORT`].
fn parse_host_port(addr: &str) -> (String, u16) {
    match addr.rsplit_once(':') {
        Some((host, port)) => (host.to_string(), port.parse().unwrap_or(DEFAULT_GPSD_PORT)),
        None => (addr.to_string(), DEFAULT_GPSD_PORT),
    }
}

async fn build_beacon_packet(
    vm_client: &VmClient,
    operator: Option<&str>,
) -> Result<String, String> {
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

    let packet = format_telemetry_packet(&summary, operator);
    debug!(packet = %packet, "Formatted telemetry packet");
    Ok(packet)
}

fn format_telemetry_packet(summary: &SystemSummary, operator: Option<&str>) -> String {
    static SEQ: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    renogymon_aprs::telemetry::format_telemetry_packet_seq(seq, summary, operator)
}
