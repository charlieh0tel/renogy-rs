use crate::error::Result;
use crate::pdu::Pdu;
use std::future::Future;

/// Transport trait for sending and receiving Modbus PDUs over any physical layer.
///
/// Implementations handle the physical layer details (serial, BLE, TCP, etc.)
/// while the PDU handles Modbus framing and CRC.
pub trait Transport {
    /// Send a PDU and wait for a response.
    fn send_receive(&mut self, pdu: &Pdu) -> impl Future<Output = Result<Pdu>> + Send;

    /// Send a PDU without waiting for a response.
    fn send(&mut self, pdu: &Pdu) -> impl Future<Output = Result<()>> + Send;
}
