//! Packet fan-out: a producer pushes [`Packet`]s into a broadcast pipe and one
//! receiver task per enabled transport (a TNC via AGW, the internet via APRS-IS)
//! drains it and transmits, each owning its own connection and reconnect logic.

use crate::aprsis::AprsIsClient;
use agw::AGW;
use agw::Call;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::warn;

/// Errors arising while building or transmitting through a sink.
#[derive(Debug, thiserror::Error)]
pub enum SinkError {
    #[error("invalid callsign {call:?}: {message}")]
    InvalidCall { call: String, message: String },
    #[error("AGW error: {0}")]
    Agw(String),
    #[error("APRS-IS error: {0}")]
    AprsIs(#[from] crate::aprsis::AprsIsError),
}

/// A batch of APRS information fields to transmit as a unit.
#[derive(Clone, Debug)]
pub enum Packet {
    /// A station position report (`!...`).
    Position(String),
    /// A single telemetry data frame (`T#...`).
    Telemetry(String),
    /// The telemetry-definition messages (PARM/UNIT/EQNS/BITS).
    Definitions(Vec<String>),
}

impl Packet {
    /// The APRS information fields carried by this packet.
    fn payloads(&self) -> &[String] {
        match self {
            Packet::Position(field) | Packet::Telemetry(field) => std::slice::from_ref(field),
            Packet::Definitions(fields) => fields.as_slice(),
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            Packet::Position(_) => "position",
            Packet::Telemetry(_) => "telemetry",
            Packet::Definitions(_) => "definitions",
        }
    }
}

/// Which transports `renogymon-aprs` should beacon to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transport {
    /// TNC only, via the Direwolf AGW interface.
    Agw,
    /// Internet only, via APRS-IS.
    AprsIs,
    /// Both the AGW TNC and APRS-IS.
    Both,
}

impl Transport {
    fn has_agw(self) -> bool {
        matches!(self, Transport::Agw | Transport::Both)
    }

    fn has_aprsis(self) -> bool {
        matches!(self, Transport::AprsIs | Transport::Both)
    }
}

/// Connection parameters shared by all transmitters.
pub struct SinkConfig<'a> {
    pub transport: Transport,
    /// Source callsign placed in every transmitted frame: the tactical call when
    /// configured, otherwise the operator station (e.g. `W1AW-12`).
    pub src: &'a str,
    /// Licensed operator callsign-SSID used for the APRS-IS login; its passcode
    /// must verify. Equals `src` when no tactical call is configured.
    pub login: &'a str,
    /// APRS destination/TOCALL.
    pub dst: &'a str,
    pub agw_addr: &'a str,
    pub aprsis_host: &'a str,
    pub aprsis_port: u16,
    pub aprsis_passcode: u16,
}

/// A connection over which APRS information fields are transmitted.
///
/// Implementations connect lazily and drop a broken connection on error, so a
/// failed `send` only affects the current batch; the next call reconnects.
#[async_trait::async_trait]
trait Transmitter: Send {
    async fn send(&mut self, payloads: &[String]) -> Result<(), SinkError>;
    fn name(&self) -> &'static str;
}

/// Subscribe a receiver task per enabled transport to `sender`'s pipe.
///
/// Returns the spawned task handles; await them after dropping the sender to let
/// each transmitter flush its final packets.
pub fn spawn_receivers(
    config: &SinkConfig,
    sender: &broadcast::Sender<Packet>,
) -> Result<Vec<JoinHandle<()>>, SinkError> {
    let mut handles = Vec::new();
    if config.transport.has_agw() {
        let tx = AgwTransmitter::new(config.src, config.dst, config.agw_addr.to_string())?;
        handles.push(spawn_receiver(Box::new(tx), sender.subscribe()));
    }
    if config.transport.has_aprsis() {
        let tx = AprsIsTransmitter::new(AprsIsClient::new(
            config.aprsis_host.to_string(),
            config.aprsis_port,
            config.login.to_string(),
            config.src.to_string(),
            config.dst.to_string(),
            config.aprsis_passcode,
        ));
        handles.push(spawn_receiver(Box::new(tx), sender.subscribe()));
    }
    Ok(handles)
}

fn spawn_receiver(
    mut transmitter: Box<dyn Transmitter>,
    mut receiver: broadcast::Receiver<Packet>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            match receiver.recv().await {
                Ok(packet) => match transmitter.send(packet.payloads()).await {
                    Ok(()) => info!(transport = transmitter.name(), kind = packet.kind(), "Sent"),
                    Err(e) => error!(transport = transmitter.name(), error = %e, "Send failed"),
                },
                Err(broadcast::error::RecvError::Closed) => break,
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    warn!(transport = transmitter.name(), skipped, "Receiver lagged");
                }
            }
        }
    })
}

fn parse_call(call: &str) -> Result<Call, SinkError> {
    call.parse().map_err(|e| SinkError::InvalidCall {
        call: call.to_string(),
        message: format!("{e}"),
    })
}

/// TNC transmitter: unproto frames over the (blocking) Direwolf AGW interface.
struct AgwTransmitter {
    src: Call,
    dst: Call,
    addr: String,
    conn: Option<AGW>,
}

impl AgwTransmitter {
    fn new(src: &str, dst: &str, addr: String) -> Result<Self, SinkError> {
        Ok(Self {
            src: parse_call(src)?,
            dst: parse_call(dst)?,
            addr,
            conn: None,
        })
    }
}

#[async_trait::async_trait]
impl Transmitter for AgwTransmitter {
    async fn send(&mut self, payloads: &[String]) -> Result<(), SinkError> {
        let conn = match self.conn.take() {
            Some(conn) => conn,
            None => {
                debug!(addr = %self.addr, "Connecting to AGW");
                let addr = self.addr.clone();
                let conn = tokio::task::spawn_blocking(move || AGW::new(&addr))
                    .await
                    .map_err(|e| SinkError::Agw(format!("connect task panicked: {e}")))?
                    .map_err(|e| SinkError::Agw(e.to_string()))?;
                info!("Connected to AGW");
                conn
            }
        };

        // AGW I/O is blocking; run sends on a blocking thread and return the
        // connection so it can be reused across beacons.
        let src = self.src.clone();
        let dst = self.dst.clone();
        let payloads = payloads.to_vec();
        let (conn, result) = tokio::task::spawn_blocking(move || {
            let mut conn = conn;
            for payload in &payloads {
                if let Err(e) = conn.unproto(0, 0xF0, &src, &dst, payload.as_bytes()) {
                    return (conn, Err(SinkError::Agw(e.to_string())));
                }
            }
            (conn, Ok(()))
        })
        .await
        .map_err(|e| SinkError::Agw(format!("send task panicked: {e}")))?;

        if result.is_ok() {
            self.conn = Some(conn);
        }
        result
    }

    fn name(&self) -> &'static str {
        "agw"
    }
}

/// Internet transmitter: TNC2 frames injected over APRS-IS.
struct AprsIsTransmitter {
    client: AprsIsClient,
}

impl AprsIsTransmitter {
    fn new(client: AprsIsClient) -> Self {
        Self { client }
    }
}

#[async_trait::async_trait]
impl Transmitter for AprsIsTransmitter {
    async fn send(&mut self, payloads: &[String]) -> Result<(), SinkError> {
        self.client.send(payloads).await.map_err(SinkError::from)
    }

    fn name(&self) -> &'static str {
        "aprs-is"
    }
}
