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

## Basic Usage

### Reading BMS Data

```rust
use renogy_rs::{Register, Value, Pdu, FunctionCode};

// Create a read request for cell voltage
let register = Register::CellVoltage(1);
let mut payload = Vec::new();
payload.extend_from_slice(&register.address().to_be_bytes());
payload.extend_from_slice(&register.quantity().to_be_bytes());
let pdu = Pdu::new(1, FunctionCode::ReadHoldingRegisters, payload);

// Simulate response data (normally from BMS)
let response_data = 33u16.to_be_bytes().to_vec(); // 3.3V
let value = register.parse_value(&response_data);
println!("Cell 1 voltage: {:?}", value); // 3.3 volts
```

### Writing Configuration

```rust
use renogy_rs::{Register, Value};
use uom::si::{electric_potential::volt, f32::ElectricPotential};

// Set cell over voltage limit to 4.2V
let register = Register::CellOverVoltageLimit;
let voltage_limit = ElectricPotential::new::<volt>(4.2);
let value = Value::ElectricPotential(voltage_limit);

// Serialize for writing to BMS
let data = register.serialize_value(&value).unwrap();
// data = [0, 42] (4.2V * 10 = 42, encoded as big-endian u16)
```

## Device Control Commands

### Factory Reset

```rust
use renogy_rs::DeviceCommand;

let reset_cmd = DeviceCommand::RestoreFactoryDefault;
if reset_cmd.requires_unlock() {
    // First unlock the device
    let unlock_cmd = DeviceCommand::Unlock;
    let unlock_pdu = unlock_cmd.create_pdu(1);
    // Send unlock_pdu to BMS
}

// Then perform factory reset
let reset_pdu = reset_cmd.create_pdu(1);
// Send reset_pdu to BMS
```

### Device Lock/Unlock

```rust
use renogy_rs::DeviceCommand;

// Lock device (prevents configuration changes)
let lock_cmd = DeviceCommand::Lock;
let lock_pdu = lock_cmd.create_pdu(1);

// Unlock device (allows configuration changes)
let unlock_cmd = DeviceCommand::Unlock;
let unlock_pdu = unlock_cmd.create_pdu(1);
```

## Configuration Examples

### Setting Voltage Limits

```rust
use renogy_rs::{Register, Value};
use uom::si::{electric_potential::volt, f32::ElectricPotential};

// Configure all cell voltage limits
let limits = [
    (Register::CellOverVoltageLimit, 4.2),   // Over voltage protection
    (Register::CellHighVoltageLimit, 4.1),  // High voltage warning
    (Register::CellLowVoltageLimit, 3.2),   // Low voltage warning
    (Register::CellUnderVoltageLimit, 3.0), // Under voltage protection
];

for (register, voltage) in limits {
    let value = Value::ElectricPotential(ElectricPotential::new::<volt>(voltage));
    let data = register.serialize_value(&value).unwrap();
    // Create write PDU and send to BMS
}
```

### Setting Temperature Limits

```rust
use renogy_rs::{Register, Value};
use uom::si::{thermodynamic_temperature::degree_celsius, f32::ThermodynamicTemperature};

// Configure charge temperature limits
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
    // Create write PDU and send to BMS
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
use renogy_rs::{RenogyError, ModbusExceptionCode, Pdu};

// Parse PDU response
match Pdu::deserialize(&response_frame) {
    Ok(pdu) => {
        // Success - process PDU
        println!("Received: {:?}", pdu);
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

### 3. Handle Device Lock State

```rust
// For sensitive operations, ensure device is unlocked first
if sensitive_operation {
    let unlock_cmd = DeviceCommand::Unlock;
    // Send unlock command

    // Perform configuration changes

    let lock_cmd = DeviceCommand::Lock;
    // Re-lock device for safety
}
```

### 4. Validate Configuration Values

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

### 5. Use Appropriate Error Recovery

```rust
// Retry logic for transient errors
for attempt in 1..=3 {
    match send_command(&pdu) {
        Ok(response) => break,
        Err(RenogyError::ModbusException(ModbusExceptionCode::SlaveDeviceBusy)) => {
            if attempt < 3 {
                std::thread::sleep(std::time::Duration::from_millis(100));
                continue;
            }
        }
        Err(e) => {
            println!("Permanent error: {}", e);
            break;
        }
    }
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