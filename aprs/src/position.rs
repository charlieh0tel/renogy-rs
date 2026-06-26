//! APRS position-report formatting and a one-shot gpsd position source.

use gpsd_proto::Mode;
use gpsd_proto::ResponseData;
use std::io::BufReader;
use std::net::TcpStream;
use std::time::Duration;
use std::time::Instant;

/// Errors from reading a position fix from gpsd.
#[derive(Debug, thiserror::Error)]
pub enum PositionError {
    #[error("gpsd error: {0}")]
    Gpsd(String),
    #[error("no usable gpsd fix within {0:?}")]
    NoFix(Duration),
}

/// Per-read socket timeout while waiting for gpsd data.
const READ_TIMEOUT: Duration = Duration::from_secs(10);

/// Build an APRS position report (`!lat<table>lon<code><comment>`, no timestamp).
///
/// `symbol` is the two-character APRS symbol: table selector followed by symbol
/// code (e.g. `/-` for a house). `comment` is appended verbatim.
#[must_use]
pub fn format_position(lat: f64, lon: f64, symbol: &str, comment: Option<&str>) -> String {
    let mut symbols = symbol.chars();
    let table = symbols.next().unwrap_or('/');
    let code = symbols.next().unwrap_or('-');
    format!(
        "!{}{}{}{}{}",
        format_latitude(lat),
        table,
        format_longitude(lon),
        code,
        comment.unwrap_or("")
    )
}

/// Format a latitude as APRS `DDMM.hhN` / `DDMM.hhS`.
fn format_latitude(lat: f64) -> String {
    let hemisphere = if lat >= 0.0 { 'N' } else { 'S' };
    let abs = lat.abs();
    let degrees = abs.trunc() as u32;
    let minutes = (abs - f64::from(degrees)) * 60.0;
    format!("{degrees:02}{minutes:05.2}{hemisphere}")
}

/// Format a longitude as APRS `DDDMM.hhE` / `DDDMM.hhW`.
fn format_longitude(lon: f64) -> String {
    let hemisphere = if lon >= 0.0 { 'E' } else { 'W' };
    let abs = lon.abs();
    let degrees = abs.trunc() as u32;
    let minutes = (abs - f64::from(degrees)) * 60.0;
    format!("{degrees:03}{minutes:05.2}{hemisphere}")
}

/// Read a single position fix from gpsd, blocking until a 2D/3D fix or until
/// `fix_wait` elapses.
///
/// Runs the blocking gpsd I/O on a blocking thread to keep the reactor free.
pub async fn read_fix(
    host: String,
    port: u16,
    fix_wait: Duration,
) -> Result<(f64, f64), PositionError> {
    tokio::task::spawn_blocking(move || read_fix_blocking(&host, port, fix_wait))
        .await
        .map_err(|e| PositionError::Gpsd(format!("gpsd task panicked: {e}")))?
}

fn read_fix_blocking(
    host: &str,
    port: u16,
    fix_wait: Duration,
) -> Result<(f64, f64), PositionError> {
    let map_io = |e: std::io::Error| PositionError::Gpsd(e.to_string());
    let stream = TcpStream::connect((host, port)).map_err(map_io)?;
    stream
        .set_read_timeout(Some(READ_TIMEOUT))
        .map_err(map_io)?;
    let mut writer = stream.try_clone().map_err(map_io)?;
    let mut reader = BufReader::new(stream);

    gpsd_proto::handshake(&mut reader, &mut writer)
        .map_err(|e| PositionError::Gpsd(e.to_string()))?;

    let deadline = Instant::now() + fix_wait;
    while Instant::now() < deadline {
        let data =
            gpsd_proto::get_data(&mut reader).map_err(|e| PositionError::Gpsd(e.to_string()))?;
        if let ResponseData::Tpv(tpv) = data
            && matches!(tpv.mode, Mode::Fix2d | Mode::Fix3d)
            && let (Some(lat), Some(lon)) = (tpv.lat, tpv.lon)
        {
            return Ok((lat, lon));
        }
    }
    Err(PositionError::NoFix(fix_wait))
}

#[cfg(test)]
mod tests {
    use super::format_position;

    #[test]
    fn formats_northwest_position() {
        assert_eq!(
            format_position(40.7128, -74.006, "/-", Some("Solar site")),
            "!4042.77N/07400.36W-Solar site"
        );
    }

    #[test]
    fn formats_southeast_without_comment() {
        // -33.8568 -> 33 51.41 S; 151.2153 -> 151 12.92 E.
        assert_eq!(
            format_position(-33.8568, 151.2153, "/-", None),
            "!3351.41S/15112.92E-"
        );
    }

    #[test]
    fn uses_given_symbol_table_and_code() {
        let packet = format_position(40.7128, -74.006, "\\c", None);
        assert!(packet.starts_with("!4042.77N\\07400.36Wc"));
    }
}
