//! Data-loss guard: when VM reports no renogy series (e.g. mid-restart/replay),
//! `run_export` must NOT advance `last_exported_day` past real days.

use axum::Router;
use axum::routing::get;
use chrono::Duration;
use chrono::Utc;
use renogy_archiver::archiver::ExportConfig;
use renogy_archiver::archiver::run_export;
use renogy_archiver::archiver::state::State;

async fn empty_series() -> &'static str {
    r#"{"status":"success","data":[]}"#
}

async fn empty_export() -> &'static str {
    ""
}

#[tokio::test(flavor = "multi_thread")]
async fn empty_vm_does_not_advance_state() {
    let app = Router::new()
        .route("/api/v1/series", get(empty_series))
        .route("/api/v1/export", get(empty_export));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let tmp = tempfile::tempdir().unwrap();
    let staging = tmp.path().join("staging");
    let state_file = tmp.path().join("state.json");

    // We have exported before, through two days ago.
    let already = (Utc::now() - Duration::days(2)).date_naive();
    State {
        last_exported_day: Some(already),
    }
    .save(&state_file)
    .unwrap();

    let cfg = ExportConfig {
        vm_addr: base,
        staging_dir: staging.clone(),
        state_file: state_file.clone(),
        start_date: None,
        max_days: None,
    };
    run_export(&cfg).await.expect("run_export");

    // Guard tripped: state unchanged and nothing staged.
    assert_eq!(
        State::load(&state_file).unwrap().last_exported_day,
        Some(already)
    );
    let staged = std::fs::read_dir(&staging).map(|d| d.count()).unwrap_or(0);
    assert_eq!(
        staged, 0,
        "no files should be written when VM has no series"
    );
}
