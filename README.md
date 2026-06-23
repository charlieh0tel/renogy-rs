# renogy-rs

Rust tools for monitoring Renogy BMS batteries via Bluetooth and serial, with APRS telemetry and a terminal UI.

## Binaries

- **renogy-bms-collector** -- Collects BMS data over Bluetooth and exports metrics to VictoriaMetrics
- **renogy-aprs** -- Beacons battery telemetry over APRS, via a TNC (Direwolf AGW), APRS-IS, or both
- **renogy-tui** -- Terminal UI for live battery monitoring
- **serial-query** -- Query BMS over serial/Modbus
- **bt2-query** -- Query BMS over Bluetooth

## Installing

### From .deb package

Download the appropriate .deb from the [releases page](https://github.com/charlieh0tel/renogy-rs/releases) and install:

```bash
sudo dpkg -i renogy-rs_*.deb
```

### From source

Requires Rust 1.89+ (see `rust-toolchain.toml`).

```bash
cargo install --path .
```

## Systemd Services

The repo includes systemd unit files for running `renogy-bms-collector` and `renogy-aprs` as system services. When installed from a .deb, the service files are placed in `/usr/lib/systemd/system/`.

### Configuration

Edit `/etc/default/renogy-rs` to configure the services (installed by the .deb):

```bash
sudo editor /etc/default/renogy-rs
```

- **SSID** -- APRS SSID, i.e. callsign-N (e.g. `Y0URS-12`). Defaults to `N0CALL`, which `renogy-aprs` will reject at startup.
- **COLLECTOR_ARGS** -- Arguments for `renogy-bms-collector`. Defaults to `bt2`. Examples: `bt2 --adapter hci1`, `serial --port /dev/ttyUSB0`.

`renogy-aprs` also reads these optional environment variables (see `/etc/default/renogy-aprs`):

- **APRS_TACTICAL** -- Optional tactical source callsign (e.g. `SOLAR1`). When set, beacons are sourced from it and the **SSID** operator callsign is appended to each telemetry packet as an identifying comment. **SSID** still drives the APRS-IS login and passcode.
- **APRS_TRANSPORT** -- `agw` (TNC, default), `aprs-is` (internet), or `both`.
- **APRSIS_HOST** / **APRSIS_PORT** -- APRS-IS server (default `rotate.aprs2.net:14580`). The passcode is computed from the callsign automatically.

### Enabling the Services

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now renogy-bms-collector
sudo systemctl enable --now renogy-aprs
```

### Managing the Services

```bash
systemctl status renogy-aprs
systemctl status renogy-bms-collector

journalctl -u renogy-aprs -f
journalctl -u renogy-bms-collector -f
```

## License

MIT
