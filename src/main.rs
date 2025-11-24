use renogy_rs::pdu::{FunctionCode, Pdu};
use renogy_rs::registers::{Register, Value};
use uom::si::electric_potential::volt;
use uom::si::f32::ElectricPotential;

fn main() {
    // 1. Create a PDU to read the cell count (register 5000).
    let register = Register::CellCount;
    let mut payload = Vec::new();
    payload.extend_from_slice(&register.address().to_be_bytes());
    payload.extend_from_slice(&register.quantity().to_be_bytes());

    let pdu = Pdu::new(1, FunctionCode::ReadHoldingRegisters, payload);

    println!("Original PDU: {:?}", pdu);

    // 2. Serialize the PDU to a byte vector.
    let serialized = pdu.serialize();
    println!("Serialized: {:?}", serialized);

    // 3. Deserialize the byte vector back into a PDU.
    let deserialized = Pdu::deserialize(&serialized).unwrap();
    println!("Deserialized PDU: {:?}", deserialized);

    // 4. Verify that the deserialized PDU is the same as the original.
    assert_eq!(pdu.address, deserialized.address);
    assert_eq!(pdu.function_code, deserialized.function_code);
    assert_eq!(pdu.payload, deserialized.payload);

    println!("Serialization and deserialization successful!");

    println!("\n--- Parsing Example ---");

    // --- Voltage Example ---
    let cell_voltage_register = Register::CellVoltage(1);
    let response_payload = 33u16.to_be_bytes().to_vec();
    let parsed_value = cell_voltage_register.parse_value(&response_payload);
    println!("Parsed voltage value: {:?}", parsed_value);
    assert_eq!(
        parsed_value,
        Value::ElectricPotential(ElectricPotential::new::<volt>(3.3))
    );
    println!("Voltage parsing successful!");

    // --- Integer Example ---
    let cell_count_register = Register::CellCount;
    let response_payload = 16u16.to_be_bytes().to_vec();
    let parsed_value = cell_count_register.parse_value(&response_payload);
    println!("Parsed integer value: {:?}", parsed_value);
    assert_eq!(
        parsed_value,
        Value::Integer(16)
    );
    println!("Integer parsing successful!");
}
