use std::collections::BTreeMap;
use std::collections::HashMap;

use chrono::Duration;
use chrono::NaiveDate;
use chrono::TimeZone;
use chrono::Utc;

use crate::archiver::ArchiverError;

/// One sample, the unit of a Parquet row.
pub struct Row {
    pub ts_ms: i64,
    pub metric: String,
    pub value: f64,
    pub labels: String,
}

/// A single time series as returned by VictoriaMetrics `/api/v1/export` (one JSON
/// object per line).
#[derive(serde::Deserialize)]
struct ExportLine {
    metric: HashMap<String, String>,
    values: Vec<f64>,
    timestamps: Vec<i64>,
}

const MATCH: &str = "{__name__=~\"renogy_.*\"}";
const DROP_LABELS: [&str; 3] = ["__name__", "job", "instance"];

/// JSON-encode the labels we keep (everything except `__name__`/`job`/`instance`),
/// sorted for deterministic output. Empty string when there are none.
fn labels_json(metric: &HashMap<String, String>) -> String {
    let kept: BTreeMap<&str, &str> = metric
        .iter()
        .filter(|(k, _)| !DROP_LABELS.contains(&k.as_str()))
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    if kept.is_empty() {
        String::new()
    } else {
        serde_json::to_string(&kept).unwrap_or_default()
    }
}

/// `[start, end)` of a UTC calendar day in epoch milliseconds.
pub fn day_bounds_ms(day: NaiveDate) -> (i64, i64) {
    let start = Utc.from_utc_datetime(&day.and_hms_opt(0, 0, 0).expect("valid midnight"));
    let end = start + Duration::days(1);
    (start.timestamp_millis(), end.timestamp_millis())
}

/// Export every renogy sample for `day` (UTC, midnight-to-midnight), sorted by time.
pub async fn export_day(
    client: &reqwest::Client,
    vm_addr: &str,
    day: NaiveDate,
) -> Result<Vec<Row>, ArchiverError> {
    let (start_ms, end_ms) = day_bounds_ms(day);
    let url = format!("{}/api/v1/export", vm_addr.trim_end_matches('/'));
    let body = client
        .get(&url)
        .query(&[
            ("match[]", MATCH.to_string()),
            ("start", format!("{}", start_ms as f64 / 1000.0)),
            ("end", format!("{}", end_ms as f64 / 1000.0)),
        ])
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    parse_export_body(&body, start_ms, end_ms)
}

/// Parse VM `/api/v1/export` newline-delimited JSON into rows, clipping to
/// `[start_ms, end_ms)` and sorting by time. Pure -- no I/O -- so it is unit-tested.
fn parse_export_body(body: &str, start_ms: i64, end_ms: i64) -> Result<Vec<Row>, ArchiverError> {
    let mut rows = Vec::new();
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parsed: ExportLine = serde_json::from_str(line)?;
        let Some(name) = parsed.metric.get("__name__") else {
            continue;
        };
        let labels = labels_json(&parsed.metric);
        for (ts, val) in parsed.timestamps.iter().zip(parsed.values.iter()) {
            // Clip to [start, end) so a sample exactly at the next midnight belongs to
            // the following day only -- no overlap between daily files.
            if *ts >= start_ms && *ts < end_ms {
                rows.push(Row {
                    ts_ms: *ts,
                    metric: name.clone(),
                    value: *val,
                    labels: labels.clone(),
                });
            }
        }
    }
    rows.sort_by_key(|r| r.ts_ms);
    Ok(rows)
}

/// True if any renogy series has a sample at or before the end of `day`.
async fn series_exists_through(
    client: &reqwest::Client,
    vm_addr: &str,
    day: NaiveDate,
) -> Result<bool, ArchiverError> {
    let (_, end_ms) = day_bounds_ms(day);
    let url = format!("{}/api/v1/series", vm_addr.trim_end_matches('/'));
    let json: serde_json::Value = client
        .get(&url)
        .query(&[
            ("match[]", MATCH.to_string()),
            ("start", "0".to_string()),
            ("end", format!("{}", end_ms as f64 / 1000.0)),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(json
        .get("data")
        .and_then(|d| d.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false))
}

/// Binary-search the earliest UTC day that has any renogy data, searching the window
/// `[search_from, today]`. Returns `None` if VM holds no renogy data at all.
pub async fn earliest_day(
    client: &reqwest::Client,
    vm_addr: &str,
    search_from: NaiveDate,
    today: NaiveDate,
) -> Result<Option<NaiveDate>, ArchiverError> {
    if !series_exists_through(client, vm_addr, today).await? {
        return Ok(None);
    }
    let mut lo = search_from;
    let mut hi = today;
    while lo < hi {
        let mid = lo + Duration::days((hi - lo).num_days() / 2);
        if series_exists_through(client, vm_addr, mid).await? {
            hi = mid;
        } else {
            lo = mid + Duration::days(1);
        }
    }
    Ok(Some(lo))
}

#[cfg(test)]
mod tests {
    use super::day_bounds_ms;
    use super::parse_export_body;
    use chrono::NaiveDate;

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn day_bounds_span_one_utc_day() {
        let (start, end) = day_bounds_ms(day(2026, 5, 31));
        assert_eq!(end - start, 86_400_000);
    }

    #[test]
    fn parse_extracts_rows_and_labels() {
        let (start, end) = day_bounds_ms(day(2026, 5, 31));
        let ts = start + 1000;
        let body = format!(
            r#"{{"metric":{{"__name__":"renogy_soc_percent_value","battery":"SN1","job":"x"}},"values":[55.0],"timestamps":[{ts}]}}"#
        );
        let rows = parse_export_body(&body, start, end).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].metric, "renogy_soc_percent_value");
        assert_eq!(rows[0].value, 55.0);
        // job is dropped; battery is kept.
        assert_eq!(rows[0].labels, r#"{"battery":"SN1"}"#);
    }

    #[test]
    fn parse_clips_to_day_bounds() {
        let (start, end) = day_bounds_ms(day(2026, 5, 31));
        let body = format!(
            r#"{{"metric":{{"__name__":"m"}},"values":[1.0,2.0,3.0],"timestamps":[{},{},{}]}}"#,
            start - 1,
            start,
            end
        );
        // start-1 (prev day) and end (next day's midnight) are excluded; only `start`.
        let rows = parse_export_body(&body, start, end).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].ts_ms, start);
    }

    #[test]
    fn parse_skips_blank_lines_and_empty_labels() {
        let body = r#"
{"metric":{"__name__":"m"},"values":[1.0],"timestamps":[10]}
"#;
        let rows = parse_export_body(body, 0, 100).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].labels, "");
    }
}
