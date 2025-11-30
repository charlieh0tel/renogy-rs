use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};

use super::RollUp;

const DEFAULT_MAX_POINTS: usize = 11_520; // 48 hours at 15s intervals

#[derive(Clone, Serialize, Deserialize)]
pub struct DataPoint {
    pub timestamp_secs: u64,
    pub current: f32,
    pub soc: f32,
    pub temp_avg: Option<f32>,
}

#[derive(Serialize, Deserialize)]
pub struct History {
    data: VecDeque<DataPoint>,
    max_points: usize,
}

impl Default for History {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_POINTS)
    }
}

impl History {
    pub fn new(max_points: usize) -> Self {
        Self {
            data: VecDeque::with_capacity(max_points.min(1024)),
            max_points,
        }
    }

    pub fn push(&mut self, rollup: &RollUp) {
        let timestamp_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let temp_avg = match (rollup.min_temperature, rollup.max_temperature) {
            (Some(min), Some(max)) => Some((min + max) / 2.0),
            (Some(t), None) | (None, Some(t)) => Some(t),
            (None, None) => None,
        };

        let point = DataPoint {
            timestamp_secs,
            current: rollup.total_current,
            soc: rollup.average_soc,
            temp_avg,
        };

        if self.data.len() >= self.max_points {
            self.data.pop_front();
        }
        self.data.push_back(point);
    }

    pub fn replace(&mut self, points: Vec<DataPoint>) {
        self.data.clear();
        for point in points {
            if self.data.len() >= self.max_points {
                break;
            }
            self.data.push_back(point);
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &DataPoint> {
        self.data.iter()
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn time_range(&self) -> Option<(u64, u64)> {
        let first = self.data.front()?.timestamp_secs;
        let last = self.data.back()?.timestamp_secs;
        Some((first, last))
    }

    pub fn newest_timestamp(&self) -> Option<u64> {
        self.data.back().map(|p| p.timestamp_secs)
    }

    pub fn oldest_timestamp(&self) -> Option<u64> {
        self.data.front().map(|p| p.timestamp_secs)
    }
}
