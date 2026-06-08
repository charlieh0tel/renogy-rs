//! Tier 2 system test (see SYSTEM_TEST.md): the puller copies staged files to the
//! archive dir and removes them from the source on success.
//!
//! Hermetic -- no SSH: rsync does a local-to-local copy (ignoring `-e ssh`) when the
//! source has no `host:`, so we point the puller at a local staging dir. Gated with
//! `#[ignore]`; run with `cargo test -p renogy-archiver-puller --test pull -- --ignored`.

use std::fs;
use std::process::Command;

#[test]
#[ignore = "system test; run explicitly with --ignored"]
fn pull_copies_then_removes_source() {
    let tmp = tempfile::tempdir().unwrap();
    let staging = tmp.path().join("staging");
    let dest = tmp.path().join("dest");
    fs::create_dir_all(&staging).unwrap();
    let name = "renogy_2026-06-03.parquet";
    fs::write(staging.join(name), b"PAR1 fake parquet").unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_renogy-archiver-puller"))
        .args([
            "--remote",
            &format!("{}/", staging.display()),
            "--dest",
            dest.to_str().unwrap(),
            "--ssh-key",
            "/nonexistent/key",
            "--lock-file",
            tmp.path().join("lock").to_str().unwrap(),
            "pull",
        ])
        .status()
        .unwrap();

    assert!(status.success(), "puller exited with {status}");
    assert!(dest.join(name).exists(), "file was not pulled to dest");
    assert!(
        !staging.join(name).exists(),
        "source file was not removed after a successful pull"
    );
}
