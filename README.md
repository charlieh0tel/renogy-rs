# renogy-rs

Rust tools for monitoring Renogy BMS batteries via Bluetooth and serial, with APRS telemetry and a terminal UI.

## Binaries

- **renogy-bms-collector** -- Collects BMS data over Bluetooth and exports metrics to VictoriaMetrics
- **renogy-aprs** -- Beacons battery telemetry over APRS via Direwolf AGW interface
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

- **CALLSIGN** -- APRS station ID (e.g. `Y0URS-12`). Defaults to `N0CALL`, which `renogy-aprs` will reject at startup.
- **COLLECTOR_ARGS** -- Arguments for `renogy-bms-collector`. Defaults to `bt2`. Examples: `bt2 --adapter hci1`, `serial --port /dev/ttyUSB0`.

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
