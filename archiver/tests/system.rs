//! Hermetic Tier 1 system test (see SYSTEM_TEST.md).
//!
//! Emulated battery -> real `query_battery` -> real collector influx encoding ->
//! in-process mock VictoriaMetrics -> real `renogy-archiver` export -> Parquet, then
//! assert the Parquet contents. No external binaries, no PTY. Gated with `#[ignore]`;
//! run with `cargo test -p renogy-archiver --test system -- --ignored`.

use std::collections::BTreeMap;
use std::fs::File;
use std::sync::Arc;
use std::sync::Mutex;

use arrow::array::Array;
use arrow::array::Float64Array;
use arrow::array::StringArray;
use axum::Router;
use axum::extract::Query;
use axum::extract::State;
use axum::routing::get;
use axum::routing::post;
use chrono::Duration;
use chrono::Utc;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use renogy_archiver::archiver::ExportConfig;
use renogy_archiver::archiver::run_export;
use renogy_rs::collector::metrics::batch_to_influx;
use renogy_rs::emulator::EmulatedBattery;
use renogy_rs::query::query_battery;
use renogy_rs::registers::Register;

/// One stored sample in the mock VM.
#[derive(Clone)]
struct Sample {
    metric: String,
    labels: BTreeMap<String, String>,
    ts_ms: i64,
    value: f64,
}

type Store = Arc<Mutex<Vec<Sample>>>;

/// Per-series accumulator keyed by (metric name, labels) -> (values, timestamps).
type SeriesMap = BTreeMap<(String, BTreeMap<String, String>), (Vec<f64>, Vec<i64>)>;

/// Parse one influx line: `measurement,tag=v,... value=<f> <ts_ns>`.
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

async fn write_handler(State(store): State<Store>, body: String) {
    let mut store = store.lock().unwrap();
    for line in body.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if let Some(sample) = parse_line(line) {
            store.push(sample);
        }
    }
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

/// Group in-range samples into VM `/api/v1/export` JSON-lines.
async fn export_handler(
    State(store): State<Store>,
    Query(params): Query<BTreeMap<String, String>>,
) -> String {
    let (start_ms, end_ms) = range_ms(&params);
    let store = store.lock().unwrap();

    let mut series: SeriesMap = BTreeMap::new();
    for s in store.iter() {
        if s.ts_ms >= start_ms && s.ts_ms < end_ms {
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
            serde_json::json!({
                "metric": metric_obj,
                "values": values,
                "timestamps": timestamps,
            })
            .to_string(),
        );
    }
    lines.join("\n")
}

async fn series_handler(
    State(store): State<Store>,
    Query(params): Query<BTreeMap<String, String>>,
) -> String {
    let (start_ms, end_ms) = range_ms(&params);
    let store = store.lock().unwrap();
    let any = store
        .iter()
        .any(|s| s.ts_ms >= start_ms && s.ts_ms < end_ms);
    let data: Vec<serde_json::Value> = if any {
        vec![serde_json::json!({"__name__": "renogy_module_voltage_value"})]
    } else {
        vec![]
    };
    serde_json::json!({"status": "success", "data": data}).to_string()
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "system test; run explicitly with --ignored"]
async fn battery_through_collector_to_parquet() {
    // Mock VM.
    let store: Store = Arc::new(Mutex::new(Vec::new()));
    let app = Router::new()
        .route("/write", post(write_handler))
        .route("/api/v1/export", get(export_handler))
        .route("/api/v1/series", get(series_handler))
        .with_state(store.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    // Emulated battery -> real query_battery.
    let addr = 0x30;
    let mut bms = EmulatedBattery::new(addr);
    bms.set_string(Register::SnNumber, "SN1234").unwrap();
    bms.set_integer(Register::CellCount, 4).unwrap();
    for cell in 1..=4 {
        bms.set_voltage(Register::CellVoltage(cell), 3.3).unwrap();
    }
    bms.set_voltage(Register::ModuleVoltage, 13.2).unwrap();
    bms.set_current(Register::Current, -5.0).unwrap();
    bms.set_current(Register::RemainingCapacity, 50.0).unwrap();
    bms.set_current(Register::TotalCapacity, 100.0).unwrap();

    let mut info = query_battery(&mut bms, addr).await.expect("battery info");

    // Stamp into yesterday so the archiver (which exports through today-1) picks it up.
    let yesterday = (Utc::now() - Duration::days(1)).date_naive();
    info.timestamp = yesterday.and_hms_opt(12, 0, 0).unwrap().and_utc();

    // Real collector encoding -> mock VM /write.
    let body = batch_to_influx(&[info]);
    reqwest::Client::new()
        .post(format!("{base}/write"))
        .body(body)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    // Real archiver export -> Parquet.
    let tmp = tempfile::tempdir().unwrap();
    let staging = tmp.path().join("staging");
    let cfg = ExportConfig {
        vm_addr: base.clone(),
        staging_dir: staging.clone(),
        state_file: tmp.path().join("state.json"),
        start_date: Some(yesterday),
        max_days: None,
    };
    run_export(&cfg).await.expect("export");

    // Assert the day's Parquet exists and round-tripped the cell voltage.
    let path = staging.join(format!("renogy_{}.parquet", yesterday.format("%Y-%m-%d")));
    assert!(path.exists(), "expected {}", path.display());

    let reader = ParquetRecordBatchReaderBuilder::try_new(File::open(&path).unwrap())
        .unwrap()
        .build()
        .unwrap();

    let mut rows = 0usize;
    let mut saw_cell_voltage_33 = false;
    for batch in reader {
        let batch = batch.unwrap();
        rows += batch.num_rows();
        let metric = batch
            .column(1)
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        let value = batch
            .column(2)
            .as_any()
            .downcast_ref::<Float64Array>()
            .unwrap();
        for i in 0..batch.num_rows() {
            if metric.value(i) == "renogy_cell_voltage_value" && (value.value(i) - 3.3).abs() < 1e-2
            {
                saw_cell_voltage_33 = true;
            }
        }
    }
    assert!(rows > 0, "no rows in parquet");
    assert!(saw_cell_voltage_33, "missing cell voltage 3.3 in parquet");
}
