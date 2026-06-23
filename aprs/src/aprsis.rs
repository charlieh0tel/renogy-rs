//! Direct APRS-IS client: TCP login plus TNC2-format packet injection.

use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::net::TcpStream;
use tokio::net::tcp::OwnedReadHalf;
use tokio::net::tcp::OwnedWriteHalf;
use tokio::time::Duration;
use tokio::time::timeout;
use tracing::debug;
use tracing::warn;

/// Errors arising from APRS-IS connection and transmission.
#[derive(Debug, thiserror::Error)]
pub enum AprsIsError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("timed out connecting to APRS-IS")]
    ConnectTimeout,
    #[error("APRS-IS server closed connection during login")]
    LoginClosed,
    #[error("APRS-IS server line exceeded {MAX_LINE_BYTES} bytes")]
    LineTooLong,
}

/// Maximum time to wait for the TCP connection to be established.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// Maximum time to wait for the server banner and login response.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);
/// Upper bound on a single server line, to bound memory against a server that
/// streams without a newline. APRS-IS lines are well under this.
const MAX_LINE_BYTES: u64 = 2048;
/// How long to wait for more drainable server data before giving up.
const DRAIN_POLL: Duration = Duration::from_millis(10);

/// Compute the APRS-IS passcode for a callsign.
///
/// The SSID (anything after `-`) is ignored and the base callsign is folded to
/// uppercase, matching the de-facto standard `aprspass` algorithm.
#[must_use]
pub fn passcode(callsign: &str) -> u16 {
    let base = callsign
        .split('-')
        .next()
        .unwrap_or(callsign)
        .to_ascii_uppercase();
    let bytes = base.as_bytes();
    let mut hash: u16 = 0x73e2;
    let mut i = 0;
    while i < bytes.len() {
        hash ^= u16::from(bytes[i]) << 8;
        if let Some(&next) = bytes.get(i + 1) {
            hash ^= u16::from(next);
        }
        i += 2;
    }
    hash & 0x7fff
}

/// A connected, logged-in APRS-IS session.
struct Session {
    reader: BufReader<OwnedReadHalf>,
    writer: OwnedWriteHalf,
}

/// Lazily-connected APRS-IS client that injects packets for a single source.
pub struct AprsIsClient {
    host: String,
    port: u16,
    /// Licensed operator callsign-SSID used for the APRS-IS login; its passcode
    /// must verify. May differ from `src` when a tactical call is in use.
    login: String,
    /// Source callsign placed in the TNC2 `from` field (the tactical call, or the
    /// operator call when no tactical call is configured).
    src: String,
    /// APRS destination/TOCALL placed in the TNC2 `to` field.
    dst: String,
    passcode: u16,
    session: Option<Session>,
}

impl AprsIsClient {
    #[must_use]
    pub fn new(
        host: String,
        port: u16,
        login: String,
        src: String,
        dst: String,
        passcode: u16,
    ) -> Self {
        Self {
            host,
            port,
            login,
            src,
            dst,
            passcode,
            session: None,
        }
    }

    /// Send APRS information fields, connecting and logging in on demand.
    ///
    /// On any I/O error the session is dropped so the next call reconnects.
    pub async fn send(&mut self, payloads: &[String]) -> Result<(), AprsIsError> {
        if self.session.is_none() {
            let session = self.connect().await?;
            self.session = Some(session);
        }

        let result = self.send_on_session(payloads).await;
        if result.is_err() {
            self.session = None;
        }
        result
    }

    async fn send_on_session(&mut self, payloads: &[String]) -> Result<(), AprsIsError> {
        let session = self
            .session
            .as_mut()
            .expect("session present after connect");
        // Discard any buffered server keepalive/comment lines to avoid backpressure.
        drain(&mut session.reader).await;
        for payload in payloads {
            let frame = format!("{}>{},TCPIP*:{}\r\n", self.src, self.dst, payload);
            debug!(frame = %frame.trim_end(), "Sending APRS-IS frame");
            session.writer.write_all(frame.as_bytes()).await?;
        }
        session.writer.flush().await?;
        Ok(())
    }

    async fn connect(&self) -> Result<Session, AprsIsError> {
        let addr = format!("{}:{}", self.host, self.port);
        debug!(addr = %addr, "Connecting to APRS-IS");
        let stream = timeout(CONNECT_TIMEOUT, TcpStream::connect(&addr))
            .await
            .map_err(|_| AprsIsError::ConnectTimeout)??;
        let (read_half, writer) = stream.into_split();
        let mut reader = BufReader::new(read_half);

        // Server greets with a comment line before login.
        read_line(&mut reader).await?;

        let login = format!(
            "user {} pass {} vers {} {}\r\n",
            self.login,
            self.passcode,
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
        );
        let mut writer = writer;
        writer.write_all(login.as_bytes()).await?;
        writer.flush().await?;

        // A successful login reply is `# logresp <call> verified, server ...`;
        // note "verified" is a substring of "unverified", so check that first.
        let response = read_line(&mut reader).await?;
        let response = response.trim_end();
        debug!(response = %response, "APRS-IS login response");
        if response.contains("unverified") {
            warn!(
                login = %self.login,
                "APRS-IS login unverified (telemetry will be dropped by the server); check that the callsign is valid"
            );
        } else if !response.contains("verified") {
            warn!(login = %self.login, response = %response, "Unexpected APRS-IS login response; proceeding");
        }

        Ok(Session { reader, writer })
    }
}

/// Read one line during the handshake, treating EOF as a closed connection and a
/// missing terminator within [`MAX_LINE_BYTES`] as an over-long line.
async fn read_line(reader: &mut BufReader<OwnedReadHalf>) -> Result<String, AprsIsError> {
    let mut line = String::new();
    let n = timeout(
        HANDSHAKE_TIMEOUT,
        (&mut *reader).take(MAX_LINE_BYTES).read_line(&mut line),
    )
    .await
    .map_err(|_| std::io::Error::from(std::io::ErrorKind::TimedOut))??;
    if n == 0 {
        return Err(AprsIsError::LoginClosed);
    }
    if !line.ends_with('\n') {
        return Err(AprsIsError::LineTooLong);
    }
    Ok(line)
}

/// Best-effort drain of any buffered server keepalive/comment traffic to avoid
/// backpressure. Uses a raw cancel-safe read (not `read_line`) so the per-poll
/// timeout never strands a partial line; the discarded bytes are never parsed.
async fn drain(reader: &mut BufReader<OwnedReadHalf>) {
    let mut buf = [0u8; 512];
    loop {
        match timeout(DRAIN_POLL, reader.read(&mut buf)).await {
            Ok(Ok(0)) | Err(_) => break,
            Ok(Ok(_)) => {}
            Ok(Err(e)) => {
                debug!(error = %e, "Error draining APRS-IS data");
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::passcode;

    #[test]
    fn passcode_ignores_ssid() {
        assert_eq!(passcode("W1AW-12"), passcode("W1AW"));
    }

    #[test]
    fn passcode_is_case_insensitive() {
        assert_eq!(passcode("w1aw"), passcode("W1AW"));
    }

    #[test]
    fn passcode_is_in_range() {
        assert!(passcode("W1AW") <= 0x7fff);
    }

    #[test]
    fn passcode_known_vector() {
        // Regression guard for the standard aprspass algorithm.
        assert_eq!(passcode("W1AW"), 25988);
    }
}
