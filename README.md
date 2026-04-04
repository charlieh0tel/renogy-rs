# renogy-rs

Rust tools for monitoring Renogy BMS batteries via Bluetooth and serial, with APRS telemetry and a terminal UI.

## Binaries

- **renogy-bms-collector** -- Collects BMS data over Bluetooth and exports metrics to VictoriaMetrics
- **renogy-aprs** -- Beacons battery telemetry over APRS via Direwolf AGW interface
- **renogy-tui** -- Terminal UI for live battery monitoring
- **serial-query** -- Query BMS over serial/Modbus
- **bt2-query** -- Query BMS over Bluetooth

## Building

Requires Rust 1.89+ (see `rust-toolchain.toml`).

```bash
cargo build --release
```

## Systemd User Services

The repo includes systemd unit files for running `renogy-bms-collector` and `renogy-aprs` as user services.

### Callsign Configuration

`renogy-aprs` requires a valid amateur radio callsign. It defaults to `N0CALL` and will refuse to start until a real callsign is configured:

```bash
mkdir -p ~/.config/renogy
echo 'CALLSIGN=AI6KG-12' > ~/.config/renogy/env
```

### Installing the Services

```bash
mkdir -p ~/.config/systemd/user
ln -s ~/src/renogy-rs/renogy-aprs.service ~/.config/systemd/user/
ln -s ~/src/renogy-rs/renogy-bms-collector.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now renogy-bms-collector
systemctl --user enable --now renogy-aprs
```

### Managing the Services

```bash
# Check status
systemctl --user status renogy-aprs
systemctl --user status renogy-bms-collector

# View logs
journalctl --user -u renogy-aprs -f
journalctl --user -u renogy-bms-collector -f

# Restart after rebuilding
cargo build --release
systemctl --user restart renogy-aprs renogy-bms-collector
```

## License

MIT
