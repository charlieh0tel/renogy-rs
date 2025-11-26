use renogy_rs::alarm::{CellVoltageAlarm, CellVoltageAlarms, Status1};
use renogy_rs::device::{DeviceCommand, PowerSettings};
use renogy_rs::pdu::{FunctionCode, Pdu};
use renogy_rs::registers::{Register, Value};
use uom::si::electric_current::ampere;
use uom::si::electric_potential::volt;
use uom::si::f32::{ElectricCurrent, ElectricPotential, ThermodynamicTemperature};
use uom::si::thermodynamic_temperature::degree_celsius;

fn main() {
    println!("--- Basic PDU Example ---");
    let register = Register::CellCount;
    let mut payload = Vec::new();
    payload.extend_from_slice(&register.address().to_be_bytes());
    payload.extend_from_slice(&register.quantity().to_be_bytes());
    let pdu = Pdu::new(1, FunctionCode::ReadHoldingRegisters, payload);
    let serialized = pdu.serialize();
    let deserialized = Pdu::deserialize(&serialized).unwrap();
    assert_eq!(pdu.address, deserialized.address);
    println!("PDU serialization and deserialization successful!");

    println!("\n--- Parsing Examples ---");

    // --- Voltage Example ---
    let register = Register::CellVoltage(1);
    let payload = 33u16.to_be_bytes().to_vec();
    let value = register.parse_value(&payload);
    println!("Parsed voltage value: {:?}", value);
    assert_eq!(
        value,
        Value::ElectricPotential(ElectricPotential::new::<volt>(3.3))
    );
    println!("Voltage parsing successful!");

    // --- Integer Example ---
    let register = Register::CellCount;
    let payload = 16u16.to_be_bytes().to_vec();
    let value = register.parse_value(&payload);
    println!("Parsed integer value: {:?}", value);
    assert_eq!(value, Value::Integer(16));
    println!("Integer parsing successful!");

    // --- Multi-word Example ---
    let register = Register::RemainingCapacity;
    let payload = 50000u32.to_be_bytes().to_vec();
    let value = register.parse_value(&payload);
    println!("Parsed multi-word value: {:?}", value);
    if let Value::ElectricCurrent(current) = value {
        assert!((current.get::<ampere>() - 50.0).abs() < 1e-5);
    } else {
        panic!("Wrong type");
    }
    println!("Multi-word parsing successful!");

    // --- Bit-mapped Status Example ---
    let register = Register::CellVoltageAlarmInfo;
    let payload = 0b00000000000000010000000000000001u32.to_be_bytes().to_vec();
    let value = register.parse_value(&payload);
    println!("Parsed bit-mapped value: {:?}", value);
    let expected = CellVoltageAlarms {
        alarms: [
            CellVoltageAlarm::OverVoltage, // cell 1
            CellVoltageAlarm::Normal,      // cell 2
            CellVoltageAlarm::Normal,      // cell 3
            CellVoltageAlarm::Normal,      // cell 4
            CellVoltageAlarm::Normal,      // cell 5
            CellVoltageAlarm::Normal,      // cell 6
            CellVoltageAlarm::Normal,      // cell 7
            CellVoltageAlarm::Normal,      // cell 8
            CellVoltageAlarm::Normal,      // cell 9
            CellVoltageAlarm::Normal,      // cell 10
            CellVoltageAlarm::Normal,      // cell 11
            CellVoltageAlarm::Normal,      // cell 12
            CellVoltageAlarm::Normal,      // cell 13
            CellVoltageAlarm::Normal,      // cell 14
            CellVoltageAlarm::Normal,      // cell 15
            CellVoltageAlarm::Normal,      // cell 16
        ],
    };
    assert_eq!(value, Value::CellVoltageAlarms(expected));
    println!("Bit-mapped status parsing successful!");

    // --- Status1 Example ---
    let register = Register::Status1;
    let payload = 0b1000000000000101u16.to_be_bytes().to_vec();
    let value = register.parse_value(&payload);
    println!("Parsed Status1 value: {:?}", value);
    let expected =
        Status1::MODULE_UNDER_VOLTAGE | Status1::DISCHARGE_MOSFET | Status1::SHORT_CIRCUIT;
    assert_eq!(value, Value::Status1(expected));
    println!("Status1 parsing successful!");

    // --- String Example ---
    let register = Register::SnNumber;
    let payload = "12345678".as_bytes().to_vec();
    let value = register.parse_value(&payload);
    println!("Parsed string value: {:?}", value);
    assert_eq!(value, Value::String("12345678".to_string()));
    println!("String parsing successful!");

    println!("\n--- Configuration Register Examples ---");

    // --- Voltage Limit Configuration Example ---
    let register = Register::CellOverVoltageLimit;
    let voltage_limit = ElectricPotential::new::<volt>(4.2);
    let value = Value::ElectricPotential(voltage_limit);
    let serialized = register.serialize_value(&value).unwrap();
    println!(
        "Serialized voltage limit: {:?} -> {:?}",
        voltage_limit, serialized
    );

    // --- Temperature Limit Configuration Example ---
    let register = Register::ChargeOverTemperatureLimit;
    let temp_limit = ThermodynamicTemperature::new::<degree_celsius>(60.0);
    let value = Value::ThermodynamicTemperature(temp_limit);
    let serialized = register.serialize_value(&value).unwrap();
    println!(
        "Serialized temperature limit: {:?} -> {:?}",
        temp_limit, serialized
    );

    // --- Current Limit Configuration Example ---
    let register = Register::ChargeOver1CurrentLimit;
    let current_limit = ElectricCurrent::new::<ampere>(100.0);
    let value = Value::ElectricCurrent(current_limit);
    let serialized = register.serialize_value(&value).unwrap();
    println!(
        "Serialized current limit: {:?} -> {:?}",
        current_limit, serialized
    );

    println!("\n--- Device Command Examples ---");

    // --- Factory Reset Command ---
    let factory_reset_cmd = DeviceCommand::RestoreFactoryDefault;
    let reset_pdu = factory_reset_cmd.create_pdu(1);
    println!("Factory reset PDU: {:?}", reset_pdu);
    println!(
        "Factory reset requires unlock: {}",
        factory_reset_cmd.requires_unlock()
    );

    // --- Device Lock/Unlock Commands ---
    let lock_cmd = DeviceCommand::Lock;
    let lock_pdu = lock_cmd.create_pdu(1);
    println!("Device lock PDU: {:?}", lock_pdu);

    let unlock_cmd = DeviceCommand::Unlock;
    let unlock_pdu = unlock_cmd.create_pdu(1);
    println!("Device unlock PDU: {:?}", unlock_pdu);

    // --- Test Mode Commands ---
    let test_begin_cmd = DeviceCommand::TestBegin;
    let test_begin_pdu = test_begin_cmd.create_pdu(1);
    println!("Test begin PDU: {:?}", test_begin_pdu);

    println!("\n--- Power Settings Example ---");

    // --- Power Configuration ---
    let power_settings = PowerSettings::new(80, 90).unwrap();
    println!("Power settings: {:?}", power_settings);
    println!(
        "Charge power: {}%, Discharge power: {}%",
        power_settings.charge_power_percent, power_settings.discharge_power_percent
    );

    println!("\n--- Write Operation Examples ---");

    // --- Write Voltage Limit Example ---
    let register = Register::CellHighVoltageLimit;
    let mut payload = Vec::new();
    payload.extend_from_slice(&register.address().to_be_bytes());
    let voltage_value = ElectricPotential::new::<volt>(4.0);
    let serialized_data = register
        .serialize_value(&Value::ElectricPotential(voltage_value))
        .unwrap();
    payload.extend_from_slice(&serialized_data);
    let write_pdu = Pdu::new(1, FunctionCode::WriteSingleRegister, payload);
    println!("Write voltage limit PDU: {:?}", write_pdu);
    println!("Is writable: {}", register.is_writable());

    println!("\n--- Multi-sensor Support Examples ---");

    // --- Environment Temperature Sensors ---
    for sensor_id in 1..=2 {
        let register = Register::EnvironmentTemperature(sensor_id);
        println!(
            "Environment sensor {} address: {}",
            sensor_id,
            register.address()
        );
    }

    // --- Heater Temperature Sensors ---
    for sensor_id in 1..=2 {
        let register = Register::HeaterTemperature(sensor_id);
        println!(
            "Heater sensor {} address: {}",
            sensor_id,
            register.address()
        );
    }

    println!("\n--- ACP Protocol Examples ---");

    // --- ACP Register Examples ---
    let acp_registers = [
        Register::AcpBroadcast,
        Register::AcpConfigure,
        Register::AcpShake,
    ];
    for register in &acp_registers {
        println!(
            "ACP register {:?} - Address: {}, Writable: {}",
            register,
            register.address(),
            register.is_writable()
        );
    }

    println!("\n--- Complete BMS System Example ---");
    println!("✅ Enhanced Renogy BMS Library Features:");
    println!("   • 49 total registers (16 monitoring + 33 configuration)");
    println!("   • Full read/write operation support");
    println!("   • Complete Modbus exception handling");
    println!("   • Device control commands (lock, unlock, test, reset)");
    println!("   • Multi-sensor support (environment, heater)");
    println!("   • ACP protocol support");
    println!("   • Type-safe unit conversions");
    println!("   • Configuration limits for voltage, current, temperature");
    println!("   • Power management settings");
    println!("   • Production-ready BMS control system");
}
