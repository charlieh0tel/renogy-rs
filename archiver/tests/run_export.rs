//! run_export correctness: the no-data-loss branches (error-no-advance, max_days,
//! multi-day, empty-day, already-staged) and the first-run earliest-day search.

mod common;

use std::path::Path;

use chrono::NaiveDate;
use common::MockVm;
use common::day_noon_ms;
use common::days_ago;
use renogy_archiver::archiver::ExportConfig;
use renogy_archiver::archiver::parse_day_from_file;
use renogy_archiver::archiver::run_export;
use renogy_archiver::archiver::state::State;
use renogy_archiver::archiver::vm_export::earliest_day;

fn config(
    vm: &MockVm,
    dir: &Path,
    start: Option<NaiveDate>,
    max_days: Option<usize>,
) -> ExportConfig {
    ExportConfig {
        vm_addr: vm.base.clone(),
        staging_dir: dir.join("staging"),
        state_file: dir.join("state.json"),
        start_date: start,
        max_days,
    }
}

fn staged_days(dir: &Path) -> Vec<NaiveDate> {
    let mut days: Vec<NaiveDate> = std::fs::read_dir(dir.join("staging"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| parse_day_from_file(&e.file_name().to_string_lossy()))
        .collect();
    days.sort();
    days
}

fn seed_state(dir: &Path, day: NaiveDate) {
    State {
        last_exported_day: Some(day),
    }
    .save(&dir.join("state.json"))
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn error_aborts_without_advancing() {
    let vm = MockVm::start().await;
    vm.add(
        "renogy_module_voltage_value",
        &[],
        day_noon_ms(days_ago(1)),
        13.2,
    );
    vm.set_export_fails(true);
    let tmp = tempfile::tempdir().unwrap();
    seed_state(tmp.path(), days_ago(3));

    let cfg = config(&vm, tmp.path(), None, None);
    assert!(run_export(&cfg).await.is_err());

    // A failed export must not advance state or stage anything.
    assert_eq!(
        State::load(&cfg.state_file).unwrap().last_exported_day,
        Some(days_ago(3))
    );
    assert!(staged_days(tmp.path()).is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn multi_day_writes_each_day() {
    let vm = MockVm::start().await;
    for n in [3, 2, 1] {
        vm.add(
            "renogy_module_voltage_value",
            &[],
            day_noon_ms(days_ago(n)),
            13.0,
        );
    }
    let tmp = tempfile::tempdir().unwrap();

    let cfg = config(&vm, tmp.path(), Some(days_ago(3)), None);
    run_export(&cfg).await.unwrap();

    assert_eq!(
        staged_days(tmp.path()),
        vec![days_ago(3), days_ago(2), days_ago(1)]
    );
    assert_eq!(
        State::load(&cfg.state_file).unwrap().last_exported_day,
        Some(days_ago(1))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn max_days_caps_files_written() {
    let vm = MockVm::start().await;
    for n in [3, 2, 1] {
        vm.add("renogy_x_value", &[], day_noon_ms(days_ago(n)), 1.0);
    }
    let tmp = tempfile::tempdir().unwrap();

    let cfg = config(&vm, tmp.path(), Some(days_ago(3)), Some(2));
    run_export(&cfg).await.unwrap();

    assert_eq!(staged_days(tmp.path()), vec![days_ago(3), days_ago(2)]);
    assert_eq!(
        State::load(&cfg.state_file).unwrap().last_exported_day,
        Some(days_ago(2))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn empty_day_advances_without_file() {
    let vm = MockVm::start().await;
    // Data only on the most recent day; the day before is genuinely empty.
    vm.add("renogy_x_value", &[], day_noon_ms(days_ago(1)), 1.0);
    let tmp = tempfile::tempdir().unwrap();
    seed_state(tmp.path(), days_ago(3));

    let cfg = config(&vm, tmp.path(), None, None);
    run_export(&cfg).await.unwrap();

    assert_eq!(staged_days(tmp.path()), vec![days_ago(1)]);
    assert_eq!(
        State::load(&cfg.state_file).unwrap().last_exported_day,
        Some(days_ago(1))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn already_staged_day_is_not_re_exported() {
    let vm = MockVm::start().await;
    let day = days_ago(1);
    vm.add("renogy_x_value", &[], day_noon_ms(day), 1.0);
    let tmp = tempfile::tempdir().unwrap();
    let staging = tmp.path().join("staging");
    std::fs::create_dir_all(&staging).unwrap();
    let path = staging.join(format!("renogy_{}.parquet", day.format("%Y-%m-%d")));
    std::fs::write(&path, b"OLD").unwrap();

    let cfg = config(&vm, tmp.path(), Some(day), None);
    run_export(&cfg).await.unwrap();

    // Existing file left untouched, state still advances past it.
    assert_eq!(std::fs::read(&path).unwrap(), b"OLD");
    assert_eq!(
        State::load(&cfg.state_file).unwrap().last_exported_day,
        Some(day)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn earliest_day_finds_first_data() {
    let vm = MockVm::start().await;
    let target = days_ago(5);
    vm.add("renogy_x_value", &[], day_noon_ms(target), 1.0);

    let client = reqwest::Client::new();
    let from = NaiveDate::from_ymd_opt(2015, 1, 1).unwrap();
    let got = earliest_day(&client, &vm.base, from, days_ago(0))
        .await
        .unwrap();
    assert_eq!(got, Some(target));
}

#[tokio::test(flavor = "multi_thread")]
async fn earliest_day_none_when_empty() {
    let vm = MockVm::start().await;
    let client = reqwest::Client::new();
    let from = NaiveDate::from_ymd_opt(2015, 1, 1).unwrap();
    let got = earliest_day(&client, &vm.base, from, days_ago(0))
        .await
        .unwrap();
    assert!(got.is_none());
}
