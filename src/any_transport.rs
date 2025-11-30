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
        match self {
            AnyTransport::Bt2(t) => query_battery(t, addr).await,
            AnyTransport::Serial(t) => query_battery(t, addr).await,
        }
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
            if let Some(_info) = self.query_battery(addr).await {
                found.push(addr);
            } else {
                break;
            }
        }
        found
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
