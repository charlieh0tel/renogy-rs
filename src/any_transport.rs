use crate::error::Result;
use crate::transport::Transport;
use crate::{BatteryInfo, Bt2Transport, SerialTransport, query_battery};
use std::ops::RangeInclusive;

pub const BT2_SCAN_RANGE: RangeInclusive<u8> = 0x30..=0x3F;
pub const SERIAL_SCAN_RANGE: RangeInclusive<u8> = 0x01..=0x10;

pub enum AnyTransport {
    Bt2(Bt2Transport),
    Serial(SerialTransport),
}

impl AnyTransport {
    pub async fn query_battery(&mut self, addr: u8) -> Option<BatteryInfo> {
        query_battery(self, addr).await
    }

    pub fn default_scan_range(&self) -> RangeInclusive<u8> {
        match self {
            AnyTransport::Bt2(_) => BT2_SCAN_RANGE,
            AnyTransport::Serial(_) => SERIAL_SCAN_RANGE,
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

impl Transport for AnyTransport {
    async fn read_holding_registers(
        &mut self,
        slave: u8,
        addr: u16,
        quantity: u16,
    ) -> Result<Vec<u16>> {
        match self {
            AnyTransport::Bt2(t) => t.read_holding_registers(slave, addr, quantity).await,
            AnyTransport::Serial(t) => t.read_holding_registers(slave, addr, quantity).await,
        }
    }

    async fn write_single_register(&mut self, slave: u8, addr: u16, value: u16) -> Result<()> {
        match self {
            AnyTransport::Bt2(t) => t.write_single_register(slave, addr, value).await,
            AnyTransport::Serial(t) => t.write_single_register(slave, addr, value).await,
        }
    }

    async fn write_multiple_registers(
        &mut self,
        slave: u8,
        addr: u16,
        values: &[u16],
    ) -> Result<()> {
        match self {
            AnyTransport::Bt2(t) => t.write_multiple_registers(slave, addr, values).await,
            AnyTransport::Serial(t) => t.write_multiple_registers(slave, addr, values).await,
        }
    }

    async fn send_custom(&mut self, slave: u8, function_code: u8, data: &[u8]) -> Result<Vec<u8>> {
        match self {
            AnyTransport::Bt2(t) => t.send_custom(slave, function_code, data).await,
            AnyTransport::Serial(t) => t.send_custom(slave, function_code, data).await,
        }
    }
}

impl From<Bt2Transport> for AnyTransport {
    fn from(t: Bt2Transport) -> Self {
        AnyTransport::Bt2(t)
    }
}

impl From<SerialTransport> for AnyTransport {
    fn from(t: SerialTransport) -> Self {
        AnyTransport::Serial(t)
    }
}
