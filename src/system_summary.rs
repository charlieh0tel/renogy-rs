use bitflags::bitflags;
use chrono::{DateTime, Utc};

use crate::alarm::{Status1, Status2};
use crate::query::BatteryInfo;

#[derive(Debug, Clone)]
pub struct SystemSummary {
    pub timestamp: DateTime<Utc>,
    pub battery_count: usize,
    pub total_current: f32,
    pub total_remaining_ah: f32,
    pub total_capacity_ah: f32,
    pub average_soc: f32,
    pub average_voltage: f32,
    pub average_temperature: Option<f32>,
    pub status1: Status1,
    pub status2: Status2,
}

impl SystemSummary {
    pub fn new(batteries: &[BatteryInfo]) -> Self {
        let mut total_current = 0.0;
        let mut total_remaining_ah = 0.0;
        let mut total_capacity_ah = 0.0;
        let mut voltage_sum = 0.0;
        let mut temp_sum = 0.0;
        let mut temp_count = 0usize;
        let mut status1 = Status1::empty();
        let mut status2 = Status2::empty();

        for info in batteries {
            total_current += info.current;
            total_remaining_ah += info.remaining_capacity;
            total_capacity_ah += info.total_capacity;
            voltage_sum += info.module_voltage;

            for &temp in &info.cell_temperatures {
                temp_sum += temp;
                temp_count += 1;
            }

            if let Some(s1) = info.status1 {
                status1 |= s1;
            }
            if let Some(s2) = info.status2 {
                status2 |= s2;
            }
        }

        let battery_count = batteries.len();
        let average_soc = if total_capacity_ah > 0.0 {
            (total_remaining_ah / total_capacity_ah) * 100.0
        } else {
            0.0
        };
        let average_voltage = if battery_count > 0 {
            voltage_sum / battery_count as f32
        } else {
            0.0
        };
        let average_temperature = if temp_count > 0 {
            Some(temp_sum / temp_count as f32)
        } else {
            None
        };

        Self {
            timestamp: Utc::now(),
            battery_count,
            total_current,
            total_remaining_ah,
            total_capacity_ah,
            average_soc,
            average_voltage,
            average_temperature,
            status1,
            status2,
        }
    }

    pub fn alarms(&self) -> SystemAlarms {
        SystemAlarms::from_status(self.status1, self.status2)
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub struct SystemAlarms: u8 {
        const OVER_VOLTAGE = 1 << 0;
        const UNDER_VOLTAGE = 1 << 1;
        const OVER_CURRENT = 1 << 2;
        const OVER_TEMP = 1 << 3;
        const UNDER_TEMP = 1 << 4;
        const SHORT_CIRCUIT = 1 << 5;
        const HEATER_ON = 1 << 6;
        const FULLY_CHARGED = 1 << 7;
    }
}

impl SystemAlarms {
    pub fn from_status(status1: Status1, status2: Status2) -> Self {
        let mut alarms = Self::empty();

        if status1.intersects(Status1::CELL_OVER_VOLTAGE | Status1::MODULE_OVER_VOLTAGE) {
            alarms |= Self::OVER_VOLTAGE;
        }
        if status1.intersects(Status1::CELL_UNDER_VOLTAGE | Status1::MODULE_UNDER_VOLTAGE) {
            alarms |= Self::UNDER_VOLTAGE;
        }
        if status1.intersects(
            Status1::CHARGE_OVER_CURRENT1
                | Status1::CHARGE_OVER_CURRENT2
                | Status1::DISCHARGE_OVER_CURRENT1
                | Status1::DISCHARGE_OVER_CURRENT2,
        ) {
            alarms |= Self::OVER_CURRENT;
        }
        if status1.intersects(Status1::CHARGE_OVER_TEMP | Status1::DISCHARGE_OVER_TEMP) {
            alarms |= Self::OVER_TEMP;
        }
        if status1.intersects(Status1::CHARGE_UNDER_TEMP | Status1::DISCHARGE_UNDER_TEMP) {
            alarms |= Self::UNDER_TEMP;
        }
        if status1.contains(Status1::SHORT_CIRCUIT) {
            alarms |= Self::SHORT_CIRCUIT;
        }
        if status2.contains(Status2::HEATER_ON) {
            alarms |= Self::HEATER_ON;
        }
        if status2.contains(Status2::FULLY_CHARGED) {
            alarms |= Self::FULLY_CHARGED;
        }

        alarms
    }

    pub fn to_aprs_binary_string(&self) -> String {
        let bits = self.bits();
        (0..8)
            .map(|i| if bits & (1 << i) != 0 { '1' } else { '0' })
            .collect()
    }
}
