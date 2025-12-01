use crate::error::Result;
use crate::transport::{Transport, TransportType};
use crate::{BatteryInfo, Bt2Transport, SerialTransport, query_battery};
use async_trait::async_trait;
use std::ops::RangeInclusive;

pub const BT2_SCAN_RANGE: RangeInclusive<u8> = 0x30..=0x3F;
pub const SERIAL_SCAN_RANGE: RangeInclusive<u8> = 0x01..=0x10;

pub struct AnyTransport(Box<dyn Transport + Send>);

impl AnyTransport {
    pub fn new(transport: impl Transport + Send + 'static) -> Self {
        Self(Box::new(transport))
    }

    pub async fn query_battery(&mut self, addr: u8) -> Option<BatteryInfo> {
        query_battery(self, addr).await
    }

    pub fn default_scan_range(&self) -> RangeInclusive<u8> {
        match self.0.transport_type() {
            TransportType::Bt2 => BT2_SCAN_RANGE,
            TransportType::Serial => SERIAL_SCAN_RANGE,
        }
    }

    pub async fn discover_batteries(&mut self, range: RangeInclusive<u8>) -> Vec<u8> {
        let mut found = Vec::new();
        for addr in range {
            if query_battery(self, addr).await.is_some() {
                found.push(addr);
            } else {
                break;
            }
        }
        found
    }
}

#[async_trait]
impl Transport for AnyTransport {
    async fn read_holding_registers(
        &mut self,
        slave: u8,
        addr: u16,
        quantity: u16,
    ) -> Result<Vec<u16>> {
        self.0.read_holding_registers(slave, addr, quantity).await
    }

    async fn write_single_register(&mut self, slave: u8, addr: u16, value: u16) -> Result<()> {
        self.0.write_single_register(slave, addr, value).await
    }

    async fn write_multiple_registers(
        &mut self,
        slave: u8,
        addr: u16,
        values: &[u16],
    ) -> Result<()> {
        self.0.write_multiple_registers(slave, addr, values).await
    }

    async fn send_custom(&mut self, slave: u8, function_code: u8, data: &[u8]) -> Result<Vec<u8>> {
        self.0.send_custom(slave, function_code, data).await
    }

    fn transport_type(&self) -> TransportType {
        self.0.transport_type()
    }
}

impl From<Bt2Transport> for AnyTransport {
    fn from(t: Bt2Transport) -> Self {
        AnyTransport::new(t)
    }
}

impl From<SerialTransport> for AnyTransport {
    fn from(t: SerialTransport) -> Self {
        AnyTransport::new(t)
    }
}
