# Renogy BMS Library Examples

This document provides examples and best practices for using the Renogy BMS library.

## Overview

The Renogy BMS library provides a complete interface for monitoring and controlling Renogy Battery Management Systems via Modbus protocol. It supports:

- **49 total registers** (16 monitoring + 33 configuration)
- **Full read/write operations** with type safety
- **Complete Modbus exception handling**
- **Device control commands** (lock, unlock, test, factory reset)
- **Multi-sensor support** (environment, heater temperatures)
- **ACP (Advanced Communication Protocol)** support
- **Type-safe unit conversions** (volts, amperes, celsius)
- **BT-2 Bluetooth transport** via bluebus/BlueZ D-Bus API
- **Serial/RS-485 transport** via tokio-modbus
- **Physical-layer neutral design** with `Transport` trait

## BT-2 Bluetooth Transport

The library includes a transport layer for communicating with Renogy BMS devices via the BT-2 Bluetooth adapter.

### Discovering BT-2 Devices

```rust
use renogy_rs::discover_bt2_devices;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Find all BT-2 devices (names starting with "BT-TH-")
    let devices = discover_bt2_devices().await?;
    for device in devices {
        println!("Found: {} at {}",
            device.name.unwrap_or_default(),
            device.address);
    }
    Ok(())
}
```

### Connecting to a BT-2

```rust
use renogy_rs::{Bt2Transport, Transport, Register};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect by MAC address
    let mut transport = Bt2Transport::connect_by_address(
        "FD:86:6D:73:XX:XX",
        "hci0"
    ).await?;

    // Or connect by D-Bus path
    // let mut transport = Bt2Transport::connect(
    //     "/org/bluez/hci0/dev_FD_86_6D_73_XX_XX"
    // ).await?;

    // Read cell voltage (BMS address 0x30)
    let register = Register::CellVoltage(1);
    let regs = transport.read_holding_registers(
        0x30,
        register.address(),
        register.quantity()
    ).await?;

    let value = register.parse_registers(&regs);
    println!("Cell 1: {:?}", value);

    Ok(())
}
```

### Reading Multiple BMS Units

The BT-2 can communicate with multiple BMS units on the same RS-485 bus:

```rust
use renogy_rs::{Bt2Transport, Transport, Register, Value};

// BMS addresses (as seen in btsnoop capture)
const BMS_0: u8 = 0x30;  // Battery 0
const BMS_1: u8 = 0x31;  // Battery 1

async fn read_cell_voltage(
    transport: &mut Bt2Transport,
    bms_addr: u8,
    cell: u8
) -> renogy_rs::Result<renogy_rs::Value> {
    let register = Register::CellVoltage(cell);
    let regs = transport.read_holding_registers(
        bms_addr,
        register.address(),
        register.quantity()
    ).await?;

    Ok(register.parse_registers(&regs))
}
```

## Serial/RS-485 Transport

The library includes a serial transport for direct RS-485 Modbus RTU communication using `tokio-modbus`.

### Opening a Serial Connection

```rust
use renogy_rs::{SerialTransport, Transport, Register};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Open serial port with default baud rate (9600)
    let mut transport = SerialTransport::open("/dev/ttyUSB0", 0x01).await?;

    // Or specify baud rate explicitly
    // let mut transport = SerialTransport::new("/dev/ttyUSB0", 9600, 0x01).await?;

    // Read cell voltage
    let register = Register::CellVoltage(1);
    let regs = transport.read_holding_registers(
        0x01,
        register.address(),
        register.quantity()
    ).await?;

    let value = register.parse_registers(&regs);
    println!("Cell 1: {:?}", value);

    Ok(())
}
```

### Communicating with Multiple Devices

The serial transport can communicate with multiple devices on the same bus:

```rust
use renogy_rs::{SerialTransport, Transport, Register};

async fn read_from_multiple_devices(transport: &mut SerialTransport) -> renogy_rs::Result<()> {
    let register = Register::CellVoltage(1);

    // Read from device 0x01
    let regs1 = transport.read_holding_registers(0x01, register.address(), register.quantity()).await?;
    let value1 = register.parse_registers(&regs1);

    // Read from device 0x02
    let regs2 = transport.read_holding_registers(0x02, register.address(), register.quantity()).await?;
    let value2 = register.parse_registers(&regs2);

    println!("Device 1: {:?}", value1);
    println!("Device 2: {:?}", value2);

    Ok(())
}
```

## Basic Usage

### Reading BMS Data

```rust
use renogy_rs::{Register, Value};

// Create a register reference
let register = Register::CellVoltage(1);

// After reading from transport:
// let regs = transport.read_holding_registers(addr, register.address(), register.quantity()).await?;
// let value = register.parse_registers(&regs);

// Simulated register data
let regs = vec![33u16]; // 3.3V (raw value * 0.1)
let value = register.parse_registers(&regs);
println!("Cell 1 voltage: {:?}", value); // 3.3 volts
```

### Writing Configuration

```rust
use renogy_rs::{Register, Value, Transport};
use uom::si::{electric_potential::volt, f32::ElectricPotential};

// Set cell over voltage limit to 4.2V
let register = Register::CellOverVoltageLimit;
let voltage_limit = ElectricPotential::new::<volt>(4.2);
let value = Value::ElectricPotential(voltage_limit);

// Serialize to register value
let data = register.serialize_value(&value).unwrap();
let reg_value = u16::from_be_bytes([data[0], data[1]]);

// Write to device
// transport.write_single_register(addr, register.address(), reg_value).await?;
```

## Device Control Commands

### Factory Reset

```rust
use renogy_rs::{DeviceCommand, Transport};

async fn factory_reset(transport: &mut impl Transport, addr: u8) -> renogy_rs::Result<()> {
    let reset_cmd = DeviceCommand::RestoreFactoryDefault;

    if reset_cmd.requires_unlock() {
        // First unlock the device
        transport.write_single_register(addr, 5224, 0xA5A5).await?;
    }

    // Perform factory reset using custom function code
    transport.send_custom(addr, 0x78, &[0x00, 0x00, 0x00, 0x01]).await?;

    Ok(())
}
```

### Device Lock/Unlock

```rust
use renogy_rs::Transport;

async fn lock_device(transport: &mut impl Transport, addr: u8) -> renogy_rs::Result<()> {
    transport.write_single_register(addr, 5224, 0x5A5A).await
}

async fn unlock_device(transport: &mut impl Transport, addr: u8) -> renogy_rs::Result<()> {
    transport.write_single_register(addr, 5224, 0xA5A5).await
}
```

## Configuration Examples

### Setting Voltage Limits

```rust
use renogy_rs::{Register, Value, Transport};
use uom::si::{electric_potential::volt, f32::ElectricPotential};

async fn set_voltage_limits(transport: &mut impl Transport, addr: u8) -> renogy_rs::Result<()> {
    let limits = [
        (Register::CellOverVoltageLimit, 4.2),   // Over voltage protection
        (Register::CellHighVoltageLimit, 4.1),  // High voltage warning
        (Register::CellLowVoltageLimit, 3.2),   // Low voltage warning
        (Register::CellUnderVoltageLimit, 3.0), // Under voltage protection
    ];

    for (register, voltage) in limits {
        let value = Value::ElectricPotential(ElectricPotential::new::<volt>(voltage));
        let data = register.serialize_value(&value).unwrap();
        let reg_value = u16::from_be_bytes([data[0], data[1]]);
        transport.write_single_register(addr, register.address(), reg_value).await?;
    }

    Ok(())
}
```

### Setting Temperature Limits

```rust
use renogy_rs::{Register, Value, Transport};
use uom::si::{thermodynamic_temperature::degree_celsius, f32::ThermodynamicTemperature};

async fn set_temp_limits(transport: &mut impl Transport, addr: u8) -> renogy_rs::Result<()> {
    let temp_limits = [
        (Register::ChargeOverTemperatureLimit, 60.0),   // Max charge temp
        (Register::ChargeHighTemperatureLimit, 50.0),  // High temp warning
        (Register::ChargeLowTemperatureLimit, 5.0),    // Low temp warning
        (Register::ChargeUnderTemperatureLimit, 0.0),  // Min charge temp
    ];

    for (register, temp) in temp_limits {
        let value = Value::ThermodynamicTemperature(
            ThermodynamicTemperature::new::<degree_celsius>(temp)
        );
        let data = register.serialize_value(&value).unwrap();
        let reg_value = i16::from_be_bytes([data[0], data[1]]) as u16;
        transport.write_single_register(addr, register.address(), reg_value).await?;
    }

    Ok(())
}
```

### Power Management

```rust
use renogy_rs::PowerSettings;

// Set charge power to 80%, discharge power to 90%
let power_settings = PowerSettings::new(80, 90).unwrap();
println!("Charge: {}%, Discharge: {}%",
    power_settings.charge_power_percent,
    power_settings.discharge_power_percent
);
```

## Multi-sensor Support

```rust
use renogy_rs::Register;

// Read environment temperature sensors
for sensor_id in 1..=2 {
    let register = Register::EnvironmentTemperature(sensor_id);
    println!("Env sensor {} at address {}", sensor_id, register.address());
}

// Read heater temperature sensors
for sensor_id in 1..=2 {
    let register = Register::HeaterTemperature(sensor_id);
    println!("Heater sensor {} at address {}", sensor_id, register.address());
}
```

## Error Handling

```rust
use renogy_rs::{RenogyError, ModbusExceptionCode, Transport};

async fn read_with_error_handling(transport: &mut impl Transport, addr: u8) {
    match transport.read_holding_registers(addr, 5000, 1).await {
        Ok(regs) => {
            println!("Read {} registers", regs.len());
        }
        Err(RenogyError::ModbusException(code)) => {
            match code {
                ModbusExceptionCode::IllegalFunction => {
                    println!("Unsupported function code");
                }
                ModbusExceptionCode::IllegalDataAddress => {
                    println!("Invalid register address");
                }
                ModbusExceptionCode::IllegalDataValue => {
                    println!("Invalid data value");
                }
                _ => println!("Modbus error: {}", code),
            }
        }
        Err(RenogyError::CrcMismatch) => {
            println!("Communication error - invalid CRC");
        }
        Err(e) => println!("Error: {}", e),
    }
}
```

## Best Practices

### 1. Always Check if Register is Writable

```rust
let register = Register::CellOverVoltageLimit;
if register.is_writable() {
    // Safe to write configuration
} else {
    println!("Register is read-only");
}
```

### 2. Use Type-Safe Units

```rust
// Good: Type-safe with units
use uom::si::{electric_potential::volt, f32::ElectricPotential};
let voltage = ElectricPotential::new::<volt>(4.2);

// Avoid: Raw numbers without context
let voltage = 42u16; // What unit? What scale?
```

### 3. Validate Configuration Values

```rust
// PowerSettings automatically validates range
match PowerSettings::new(150, 90) { // Invalid: >100%
    Ok(settings) => println!("Valid settings"),
    Err(RenogyError::InvalidRegisterRange) => {
        println!("Power percentage must be 0-100%");
    }
    Err(e) => println!("Other error: {}", e),
}
```

### 4. Use Appropriate Error Recovery

```rust
use std::time::Duration;
use tokio::time::sleep;

async fn read_with_retry(transport: &mut impl Transport, addr: u8) -> renogy_rs::Result<Vec<u16>> {
    for attempt in 1..=3 {
        match transport.read_holding_registers(addr, 5000, 1).await {
            Ok(regs) => return Ok(regs),
            Err(RenogyError::ModbusException(ModbusExceptionCode::SlaveDeviceBusy)) => {
                if attempt < 3 {
                    sleep(Duration::from_millis(100)).await;
                    continue;
                }
            }
            Err(e) => return Err(e),
        }
    }
    Err(RenogyError::DeviceControlFailed)
}
```

## Register Reference

### Monitoring Registers (Read-Only)
- `CellVoltage(1-16)` - Individual cell voltages
- `CellTemperature(1-16)` - Individual cell temperatures
- `Current` - Battery current (charge/discharge)
- `ModuleVoltage` - Total module voltage
- `Status1`, `Status2`, `Status3` - System status flags
- `CellVoltageAlarmInfo` - Cell voltage alarm status
- `RemainingCapacity` - Current capacity
- `CycleNumber` - Charge/discharge cycles

### Configuration Registers (Read/Write)
- `CellOverVoltageLimit` - Cell over voltage protection
- `ChargeCurrentLimit` - Maximum charge current
- `DischargeCurrentLimit` - Maximum discharge current
- `ChargeOverTemperatureLimit` - Charge temperature limit
- `ModuleOverVoltageLimit` - Module voltage protection
- `DeviceId` - Device identifier
- `ChargePowerSetting` - Charge power percentage
- `DischargePowerSetting` - Discharge power percentage

### Control Registers
- `ShutdownCommand` - Device shutdown control
- `LockControl` - Device lock/unlock
- `TestReady` - Test mode control

### ACP Protocol Registers
- `AcpBroadcast` - ACP broadcast setting
- `AcpConfigure` - ACP configuration setting
- `AcpShake` - ACP handshake setting

For complete register specifications, refer to the Renogy BMS Modbus Protocol V1.7 documentation.

## Transport Layer Architecture

The library uses a physical-layer neutral design with the `Transport` trait:

```rust
use renogy_rs::Result;

pub trait Transport {
    async fn read_holding_registers(&mut self, slave: u8, addr: u16, quantity: u16) -> Result<Vec<u16>>;
    async fn write_single_register(&mut self, slave: u8, addr: u16, value: u16) -> Result<()>;
    async fn write_multiple_registers(&mut self, slave: u8, addr: u16, values: &[u16]) -> Result<()>;
    async fn send_custom(&mut self, slave: u8, function_code: u8, data: &[u8]) -> Result<Vec<u8>>;
}
```

### Available Transports

- **`Bt2Transport`** - Bluetooth Low Energy via Renogy BT-2 adapter
- **`SerialTransport`** - Serial/RS-485 via tokio-modbus

### Implementing Custom Transports

You can implement the `Transport` trait for other physical layers:

```rust
use renogy_rs::{Result, RenogyError};

struct MyCustomTransport {
    // Your implementation
}

impl Transport for MyCustomTransport {
    async fn read_holding_registers(&mut self, slave: u8, addr: u16, quantity: u16) -> Result<Vec<u16>> {
        // Implement reading...
        todo!()
    }

    async fn write_single_register(&mut self, slave: u8, addr: u16, value: u16) -> Result<()> {
        // Implement writing...
        todo!()
    }

    async fn write_multiple_registers(&mut self, slave: u8, addr: u16, values: &[u16]) -> Result<()> {
        // Implement writing...
        todo!()
    }

    async fn send_custom(&mut self, slave: u8, function_code: u8, data: &[u8]) -> Result<Vec<u8>> {
        // Implement custom function codes...
        todo!()
    }
}
```

This allows the same high-level code to work with different physical connections (BLE, RS-485, USB, etc.).
