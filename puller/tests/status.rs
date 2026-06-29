//! The puller `status` gap audit reports calendar days missing from the archive dir.

use std::fs;
use std::process::Command;

#[test]
fn status_reports_missing_days() {
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join("archive");
    fs::create_dir_all(&dest).unwrap();
    // 06-01 and 06-03 present; 06-02 is the gap.
    for name in ["renogy_2026-06-01.parquet", "renogy_2026-06-03.parquet"] {
        fs::write(dest.join(name), b"x").unwrap();
    }

    let out = Command::new(env!("CARGO_BIN_EXE_renogymon-archiver-puller"))
        .args(["--dest", dest.to_str().unwrap(), "status"])
        .output()
        .unwrap();

    assert!(out.status.success(), "status exited with {}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("2026-06-02"),
        "missing day not reported; stdout was:\n{stdout}"
    );
}
