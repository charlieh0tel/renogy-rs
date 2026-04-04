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

## Systemd User Services

The repo includes systemd unit files for running `renogy-bms-collector` and `renogy-aprs` as user services. When installed from a .deb, the service files are placed in `/usr/lib/systemd/user/`.

### Configuration

Create `/etc/default/renogy-rs` to configure the services:

```bash
sudo tee /etc/default/renogy-rs << 'EOF'
CALLSIGN=Y0URS-12
COLLECTOR_ARGS=bt2
EOF
```

- **CALLSIGN** -- Required for `renogy-aprs`. Defaults to `N0CALL`, which the program will reject at startup.
- **COLLECTOR_ARGS** -- Arguments for `renogy-bms-collector`. Defaults to `bt2`. Examples: `bt2 --adapter hci1`, `serial --port /dev/ttyUSB0`.

### Enabling the Services

```bash
systemctl --user daemon-reload
systemctl --user enable --now renogy-bms-collector
systemctl --user enable --now renogy-aprs
```

### Managing the Services

```bash
systemctl --user status renogy-aprs
systemctl --user status renogy-bms-collector

journalctl --user -u renogy-aprs -f
journalctl --user -u renogy-bms-collector -f
```

## License

MIT
