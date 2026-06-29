//! Pure APRS telemetry formatting (data packet + definition messages).

use renogy::system_summary::SystemSummary;

/// Free-text project title shown on telemetry plots (the BITS field), suffixed
/// with the project URL so listeners can find the source.
const PROJECT_TITLE: &str = "Renogy BMS";

/// Build the APRS telemetry data packet `T#seq,A1..A5,bits` for a summary.
///
/// Analog channels are 0-255: SOC/capacity/voltage clamped, current offset by +128,
/// temperature offset by +40 (matching the `EQNS` coefficients in `definition_packets`).
///
/// `operator`, when set, is appended as a trailing comment to identify the licensed
/// operator behind a tactical source callsign. Strict telemetry parsers ignore it.
#[must_use]
pub fn format_telemetry_packet_seq(
    seq: u16,
    summary: &SystemSummary,
    operator: Option<&str>,
) -> String {
    let a1 = (summary.average_soc.round() as u16).min(255);
    let a2 = (summary.total_remaining_ah.round() as u16).min(255);
    let a3 = (summary.average_voltage.round() as u16).min(255);
    let a4 = ((summary.total_current + 128.0).round() as u16).clamp(0, 255);
    let a5 = summary
        .average_temperature
        .map(|t| ((t + 40.0).round() as u16).clamp(0, 255))
        .unwrap_or(0);
    let binary = summary.alarms().to_aprs_binary_string();

    let packet = format!(
        "T#{:03},{:03},{:03},{:03},{:03},{:03},{}",
        seq % 1000,
        a1,
        a2,
        a3,
        a4,
        a5,
        binary
    );
    match operator {
        Some(operator) => format!("{packet} {operator}"),
        None => packet,
    }
}

/// Build the four APRS telemetry-definition messages (PARM, UNIT, EQNS, BITS) for a
/// 9-char-padded message addressee.
#[must_use]
pub fn definition_packets(callsign: &str) -> [String; 4] {
    let padded = format!("{callsign:9}");
    [
        format!(":{padded}:PARM.SOC,Capacity,Voltage,Current,Temp,OV,UV,OC,OT,UT,SC,Htr,Full"),
        format!(":{padded}:UNIT.%,Ah,V,A,C"),
        format!(":{padded}:EQNS.0,1,0,0,1,0,0,1,0,0,1,-128,0,1,-40"),
        format!(
            ":{padded}:BITS.11111111,{PROJECT_TITLE} {}",
            env!("CARGO_PKG_REPOSITORY")
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::definition_packets;
    use super::format_telemetry_packet_seq;
    use chrono::Utc;
    use renogy::alarm::Status1;
    use renogy::alarm::Status2;
    use renogy::system_summary::SystemSummary;

    fn summary() -> SystemSummary {
        SystemSummary {
            timestamp: Utc::now(),
            battery_count: 1,
            total_current: -5.0,
            total_remaining_ah: 50.0,
            total_capacity_ah: 100.0,
            average_soc: 50.0,
            average_voltage: 13.0,
            average_temperature: Some(25.0),
            status1: Status1::empty(),
            status2: Status2::empty(),
        }
    }

    #[test]
    fn packet_encodes_offsets() {
        // current -5 -> +128 = 123; temp 25 -> +40 = 65; no alarms -> all zero bits.
        assert_eq!(
            format_telemetry_packet_seq(7, &summary(), None),
            "T#007,050,050,013,123,065,00000000"
        );
    }

    #[test]
    fn operator_is_appended_as_trailing_comment() {
        assert_eq!(
            format_telemetry_packet_seq(7, &summary(), Some("W1AW-12")),
            "T#007,050,050,013,123,065,00000000 W1AW-12"
        );
    }

    #[test]
    fn seq_wraps_and_missing_temp_is_zero() {
        let mut s = summary();
        s.average_temperature = None;
        let packet = format_telemetry_packet_seq(1000, &s, None);
        assert!(
            packet.starts_with("T#000,"),
            "seq should wrap at 1000: {packet}"
        );
        assert_eq!(packet.split(',').nth(5).unwrap(), "000", "temp should be 0");
    }

    #[test]
    fn out_of_range_channels_clamp_to_255() {
        let mut s = summary();
        s.average_soc = 999.0;
        s.total_current = 500.0;
        let fields: Vec<String> = format_telemetry_packet_seq(0, &s, None)
            .split(',')
            .map(str::to_string)
            .collect();
        assert_eq!(fields[1], "255");
        assert_eq!(fields[4], "255");
    }

    #[test]
    fn definitions_pad_callsign_and_fix_fields() {
        let d = definition_packets("W1AW-12");
        assert!(d[0].starts_with(":W1AW-12  :PARM."));
        assert_eq!(d[1], ":W1AW-12  :UNIT.%,Ah,V,A,C");
        assert!(d[2].ends_with("EQNS.0,1,0,0,1,0,0,1,0,0,1,-128,0,1,-40"));
        assert!(d[3].starts_with(":W1AW-12  :BITS.11111111,Renogy BMS "));
        assert!(d[3].ends_with(env!("CARGO_PKG_REPOSITORY")));
    }
}
