use renogy_rs::alarm::{CellVoltageAlarm, CellVoltageAlarms, Status1};
use renogy_rs::pdu::{FunctionCode, Pdu};
use renogy_rs::registers::{Register, Value};
use uom::si::electric_current::ampere;
use uom::si::electric_potential::volt;
use uom::si::f32::ElectricPotential;

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
}
