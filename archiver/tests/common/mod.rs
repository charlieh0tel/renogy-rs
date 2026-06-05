//! Shared in-process mock VictoriaMetrics for the archiver integration tests.
//!
//! Honors `match[]` (only `renogy_*` series are returned), serves per-time-range
//! `/api/v1/export` and date-aware `/api/v1/series`, ingests the collector's influx
//! `/write`, and can be told to fail `/api/v1/export` to exercise the error path.

// Different test binaries use different subsets of this helper.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;

use axum::Router;
use axum::extract::Query;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::routing::post;
use chrono::Duration;
use chrono::NaiveDate;
use chrono::TimeZone;
use chrono::Utc;

#[derive(Clone)]
struct Sample {
    metric: String,
    labels: BTreeMap<String, String>,
    ts_ms: i64,
    value: f64,
}

#[derive(Default)]
struct Inner {
    samples: Vec<Sample>,
    export_fails: bool,
}

type Store = Arc<Mutex<Inner>>;

/// Per-series accumulator keyed by (metric name, labels) -> (values, timestamps).
type SeriesMap = BTreeMap<(String, BTreeMap<String, String>), (Vec<f64>, Vec<i64>)>;

pub struct MockVm {
    pub base: String,
    store: Store,
}

impl MockVm {
    pub async fn start() -> Self {
        let store: Store = Arc::new(Mutex::new(Inner::default()));
        let app = Router::new()
            .route("/write", post(write_handler))
            .route("/api/v1/export", get(export_handler))
            .route("/api/v1/series", get(series_handler))
            .with_state(store.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self { base, store }
    }

    /// Add a sample directly (bypassing the influx `/write` path).
    pub fn add(&self, metric: &str, labels: &[(&str, &str)], ts_ms: i64, value: f64) {
        let labels = labels
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        self.store.lock().unwrap().samples.push(Sample {
            metric: metric.to_string(),
            labels,
            ts_ms,
            value,
        });
    }

    /// Make `/api/v1/export` return HTTP 500 (to exercise the no-advance path).
    pub fn set_export_fails(&self, fail: bool) {
        self.store.lock().unwrap().export_fails = fail;
    }
}

/// Noon-UTC epoch-ms for a calendar day -- a convenient in-range timestamp.
pub fn day_noon_ms(date: NaiveDate) -> i64 {
    Utc.from_utc_datetime(&date.and_hms_opt(12, 0, 0).unwrap())
        .timestamp_millis()
}

pub fn days_ago(n: i64) -> NaiveDate {
    (Utc::now() - Duration::days(n)).date_naive()
}

fn parse_line(line: &str) -> Option<Sample> {
    let mut parts = line.splitn(3, ' ');
    let key = parts.next()?;
    let field = parts.next()?;
    let ts_ns: i64 = parts.next()?.trim().parse().ok()?;
    let mut keys = key.split(',');
    let measurement = keys.next()?;
    let mut labels = BTreeMap::new();
    for tag in keys {
        let (k, v) = tag.split_once('=')?;
        labels.insert(k.to_string(), v.to_string());
    }
    let value: f64 = field.strip_prefix("value=")?.parse().ok()?;
    Some(Sample {
        metric: format!("{measurement}_value"),
        labels,
        ts_ms: ts_ns / 1_000_000,
        value,
    })
}

fn range_ms(params: &BTreeMap<String, String>) -> (i64, i64) {
    let secs = |k: &str, default: f64| {
        params
            .get(k)
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(default)
    };
    (
        (secs("start", 0.0) * 1000.0) as i64,
        (secs("end", f64::from(i32::MAX)) * 1000.0) as i64,
    )
}

async fn write_handler(State(store): State<Store>, body: String) {
    let mut inner = store.lock().unwrap();
    for line in body.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if let Some(sample) = parse_line(line) {
            inner.samples.push(sample);
        }
    }
}

async fn export_handler(
    State(store): State<Store>,
    Query(params): Query<BTreeMap<String, String>>,
) -> Result<String, StatusCode> {
    let inner = store.lock().unwrap();
    if inner.export_fails {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    let (start_ms, end_ms) = range_ms(&params);
    let mut series: SeriesMap = BTreeMap::new();
    for s in inner.samples.iter() {
        // Honor match[]={__name__=~"renogy_.*"}: foreign series are excluded.
        if s.metric.starts_with("renogy_") && s.ts_ms >= start_ms && s.ts_ms < end_ms {
            let entry = series
                .entry((s.metric.clone(), s.labels.clone()))
                .or_default();
            entry.0.push(s.value);
            entry.1.push(s.ts_ms);
        }
    }
    let mut lines = Vec::new();
    for ((metric, labels), (values, timestamps)) in series {
        let mut metric_obj = serde_json::Map::new();
        metric_obj.insert("__name__".into(), serde_json::json!(metric));
        for (k, v) in labels {
            metric_obj.insert(k, serde_json::json!(v));
        }
        lines.push(
            serde_json::json!({"metric": metric_obj, "values": values, "timestamps": timestamps})
                .to_string(),
        );
    }
    Ok(lines.join("\n"))
}

async fn series_handler(
    State(store): State<Store>,
    Query(params): Query<BTreeMap<String, String>>,
) -> String {
    let (start_ms, end_ms) = range_ms(&params);
    let inner = store.lock().unwrap();
    let any = inner
        .samples
        .iter()
        .any(|s| s.metric.starts_with("renogy_") && s.ts_ms >= start_ms && s.ts_ms < end_ms);
    let data: Vec<serde_json::Value> = if any {
        vec![serde_json::json!({"__name__": "renogy_module_voltage_value"})]
    } else {
        vec![]
    };
    serde_json::json!({"status": "success", "data": data}).to_string()
}
