use crate::query::BatteryInfo;
use ratatui::widgets::ListState;
use serde::{Deserialize, Serialize};
use std::time::Instant;

use super::history::History;

#[derive(Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tab {
    #[default]
    Overview,
    Graphs,
}

pub const ZOOM_LEVELS: &[(u64, &str)] = &[
    (60, "1 min"),
    (300, "5 min"),
    (900, "15 min"),
    (3600, "1 hour"),
    (14400, "4 hours"),
    (43200, "12 hours"),
    (86400, "24 hours"),
    (172800, "48 hours"),
];

#[derive(Clone, Serialize, Deserialize)]
pub struct GraphViewState {
    pub zoom_level_idx: usize,
    pub scroll_offset_secs: u64,
}

impl Default for GraphViewState {
    fn default() -> Self {
        Self {
            zoom_level_idx: 2, // default to 15 min
            scroll_offset_secs: 0,
        }
    }
}

impl GraphViewState {
    pub fn zoom_window_secs(&self) -> u64 {
        ZOOM_LEVELS[self.zoom_level_idx].0
    }

    pub fn zoom_label(&self) -> &'static str {
        ZOOM_LEVELS[self.zoom_level_idx].1
    }

    pub fn zoom_in(&mut self) {
        if self.zoom_level_idx > 0 {
            self.zoom_level_idx -= 1;
        }
    }

    pub fn zoom_out(&mut self) {
        if self.zoom_level_idx < ZOOM_LEVELS.len() - 1 {
            self.zoom_level_idx += 1;
        }
    }

    pub fn scroll_back(&mut self, secs: u64, max_offset: u64) {
        self.scroll_offset_secs = (self.scroll_offset_secs + secs).min(max_offset);
    }

    pub fn scroll_forward(&mut self, secs: u64) {
        self.scroll_offset_secs = self.scroll_offset_secs.saturating_sub(secs);
    }

    pub fn jump_to_newest(&mut self) {
        self.scroll_offset_secs = 0;
    }

    pub fn jump_to_oldest(&mut self, history_duration: u64) {
        let window = self.zoom_window_secs();
        self.scroll_offset_secs = history_duration.saturating_sub(window);
    }
}

pub struct App {
    pub batteries: Vec<(u8, Option<BatteryInfo>)>,
    pub list_state: ListState,
    pub last_update: Option<Instant>,
    pub error: Option<String>,
    pub running: bool,
    pub refreshing: bool,
    pub active_tab: Tab,
    pub history: History,
    pub graph_view: GraphViewState,
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
            active_tab: Tab::default(),
            history: History::default(),
            graph_view: GraphViewState::default(),
        }
    }

    pub fn next_tab(&mut self) {
        self.active_tab = match self.active_tab {
            Tab::Overview => Tab::Graphs,
            Tab::Graphs => Tab::Overview,
        };
    }

    pub fn record_history(&mut self) {
        let rollup = self.rollup();
        self.history.push(&rollup);
    }

    pub fn history_duration(&self) -> u64 {
        self.history
            .time_range()
            .map(|(oldest, newest)| newest.saturating_sub(oldest))
            .unwrap_or(0)
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
