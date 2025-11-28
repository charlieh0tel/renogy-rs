use crate::query::BatteryInfo;
use ratatui::widgets::ListState;
use std::time::Instant;

pub struct App {
    pub batteries: Vec<(u8, Option<BatteryInfo>)>,
    pub list_state: ListState,
    pub last_update: Option<Instant>,
    pub error: Option<String>,
    pub running: bool,
    pub refreshing: bool,
}

impl App {
    pub fn new(addresses: Vec<u8>) -> Self {
        let batteries: Vec<_> = addresses.into_iter().map(|addr| (addr, None)).collect();
        let mut list_state = ListState::default();
        if !batteries.is_empty() {
            list_state.select(Some(0));
        }
        Self {
            batteries,
            list_state,
            last_update: None,
            error: None,
            running: true,
            refreshing: false,
        }
    }

    pub fn select_next(&mut self) {
        if self.batteries.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => (i + 1) % self.batteries.len(),
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn select_previous(&mut self) {
        if self.batteries.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => i.checked_sub(1).unwrap_or(self.batteries.len() - 1),
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn selected(&self) -> usize {
        self.list_state.selected().unwrap_or(0)
    }

    pub fn selected_battery(&self) -> Option<&BatteryInfo> {
        self.batteries
            .get(self.selected())
            .and_then(|(_, b)| b.as_ref())
    }

    pub fn update_battery(&mut self, addr: u8, info: Option<BatteryInfo>) {
        if let Some((_, slot)) = self.batteries.iter_mut().find(|(a, _)| *a == addr) {
            *slot = info;
        }
        self.last_update = Some(Instant::now());
    }

    pub fn rollup(&self) -> RollUp {
        RollUp::from_batteries(&self.batteries)
    }
}

pub struct RollUp {
    pub battery_count: usize,
    pub responding_count: usize,
    pub total_current: f32,
    pub total_remaining_ah: f32,
    pub total_capacity_ah: f32,
    pub average_soc: f32,
    pub min_temperature: Option<f32>,
    pub max_temperature: Option<f32>,
}

impl RollUp {
    pub fn from_batteries(batteries: &[(u8, Option<BatteryInfo>)]) -> Self {
        let mut total_current = 0.0;
        let mut total_remaining_ah = 0.0;
        let mut total_capacity_ah = 0.0;
        let mut min_temp: Option<f32> = None;
        let mut max_temp: Option<f32> = None;
        let mut responding_count = 0;

        for (_, info) in batteries {
            if let Some(info) = info {
                responding_count += 1;
                total_current += info.current;
                total_remaining_ah += info.remaining_capacity;
                total_capacity_ah += info.total_capacity;

                for &temp in &info.cell_temperatures {
                    min_temp = Some(min_temp.map_or(temp, |m| m.min(temp)));
                    max_temp = Some(max_temp.map_or(temp, |m| m.max(temp)));
                }
            }
        }

        let average_soc = if total_capacity_ah > 0.0 {
            (total_remaining_ah / total_capacity_ah) * 100.0
        } else {
            0.0
        };

        Self {
            battery_count: batteries.len(),
            responding_count,
            total_current,
            total_remaining_ah,
            total_capacity_ah,
            average_soc,
            min_temperature: min_temp,
            max_temperature: max_temp,
        }
    }
}
