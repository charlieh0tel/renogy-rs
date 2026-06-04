# System-Level Test Plan

End-to-end test of the data pipeline: an emulated battery feeds the collector, which
records to VictoriaMetrics, which the archiver exports to Parquet, which the puller
transfers. The goal is one automated test that exercises the real wiring, not mocks.

## Scope

Test the **serial (Modbus RTU)** path, not Bluetooth.

- Serial and BT-2 converge immediately above the transport (`query_battery` ->
  `BatteryInfo`), so the serial path covers nearly all the logic.
- Emulating BlueZ (a mock D-Bus service + BLE GATT peripheral) is a project in itself
  and is explicitly out of scope.

## Data flow under test

```
Modbus-RTU battery emulator
    |  pseudo-terminal (PTY) pair  ==  a virtual serial port
    v
renogy-bms-collector --serial --port <pty>
    |  influx /write
    v
VictoriaMetrics (real single binary, ephemeral port + tmpdir)
    |  /api/v1/export, /api/v1/series
    v
renogy-archiver export   ->   staging/renogy_YYYY-MM-DD.parquet
    |  (Tier 2) rsync --remove-source-files
    v
local archive dir   (assert files present + removed from source)
```

## Components to build

1. **Modbus-RTU battery emulator** -- a slave that answers `ReadHoldingRegisters`
   for the Renogy register map with canned values. Reuse `registers.rs`
   (`Register::address()` + `serialize_value`) to encode responses so the emulator
   and the parser cannot drift. `tokio-modbus` provides an RTU server (enable its
   `server` feature).
2. **Virtual serial** -- a PTY pair (`nix::pty::openpty`, or `socat`); the emulator
   opens one end, the collector's `--port` is the other. No change to
   `SerialTransport` -- it sees a real tty. Works on GitHub runners (`/dev/ptmx`).
3. **VictoriaMetrics** -- download and cache the static binary (or a testcontainer),
   run on an ephemeral port + tmpdir, tear down after.
4. **Harness** -- start VM, start emulator, run the collector for a bounded window
   (then SIGINT; graceful shutdown already exists), run `archiver export`, read the
   Parquet back (arrow) and assert the values match what the emulator served.

## Code seams this exposes

- **Export's `today-1` boundary.** Export only writes whole days through yesterday,
  so a few-seconds run produces no file. Two options:
  - push **past-dated** samples straight into VM via `/api/v1/import` instead of
    relying on the collector's live timestamps (no production-code change); or
  - add a `--now` / `--end-date` test override to the archiver.
  Prefer the import approach.
- **Collector is a daemon.** The harness runs it for a bounded window then signals
  it; it already handles graceful shutdown via the cancellation token.

## Where it runs

- Not in `cargo test` -- it needs external binaries (VM) and a PTY.
- Make it a **gated integration test** (`#[ignore]`, or behind `RUN_SYSTEM_TESTS=1`)
  plus a **dedicated CI job** that provisions VictoriaMetrics. The normal build stays
  fast.

## Scope tiers

- **Tier 1 (recommended first):** emulator -> collector -> real VM -> `export` ->
  assert Parquet. The full "battery in, Parquet out" spine.
- **Tier 2:** add the puller (rsync to a local dest; assert files copied and removed
  from the source).

## Lighter hermetic alternative

A mock-based integration test gets ~70% of the value with no external deps: a tiny
in-process HTTP server (`axum` / `wiremock`) standing in for VM's `/write` and
`/api/v1/export`, plus a PTY (or an in-process mock `Transport`) for the battery.
Faster and self-contained, but it tests against our model of VM, not real VM.

## Effort

Tier 1 with real VM: roughly 1-2 days. The Modbus emulator (~half a day, reusing
`registers.rs`) is the reusable centerpiece; once it exists, asserting at each stage
is cheap.

## Open questions

1. Real VM in CI vs. the hermetic mock alternative (or both)?
2. Solve the `today-1` boundary via `/api/v1/import` of past-dated data, or add a
   test-only clock override to the archiver?
3. Tier 1 only, or include the puller (Tier 2)?
