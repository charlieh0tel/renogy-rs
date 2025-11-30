use prometheus_http_query::{Client, Error as PromError};

use super::history::DataPoint;
use crate::BatteryInfo;

pub struct VmClient {
    client: Client,
}

impl VmClient {
    pub fn new(base_url: &str) -> Result<Self, PromError> {
        let client = Client::try_from(base_url)?;
        Ok(Self { client })
    }

    pub async fn discover_batteries(&self) -> Result<Vec<String>, String> {
        let response = self
            .client
            .query("group by (battery) (renogy_soc_percent_value)")
            .get()
            .await
            .map_err(|e| format!("Query failed: {}", e))?;

        let mut batteries = Vec::new();
        if let Some(instant) = response.data().as_vector() {
            for sample in instant {
                if let Some(battery) = sample.metric().get("battery") {
                    batteries.push(battery.to_string());
                }
            }
        } else {
            return Err(format!("Unexpected response type: {:?}", response.data()));
        }

        Ok(batteries)
    }

    pub async fn query_latest(&self, battery: &str) -> Result<Option<BatteryInfo>, String> {
        let query = format!("{{battery=\"{}\",__name__=~\"renogy_.*_value\"}}", battery);
        let response = self
            .client
            .query(query)
            .get()
            .await
            .map_err(|e| e.to_string())?;

        let Some(samples) = response.data().as_vector() else {
            return Ok(None);
        };

        if samples.is_empty() {
            return Ok(None);
        }

        let mut module_voltage = None;
        let mut current = None;
        let mut soc_percent = None;
        let mut remaining_capacity = None;
        let mut total_capacity = None;
        let mut cycle_count = None;
        let mut cell_voltages: Vec<(u32, f32)> = Vec::new();
        let mut cell_temperatures: Vec<(u32, f32)> = Vec::new();

        for sample in samples {
            let value = sample.sample().value() as f32;
            let metric_name = sample.metric().get("__name__").map(String::as_str);

            match metric_name {
                Some("renogy_module_voltage_value") => module_voltage = Some(value),
                Some("renogy_current_value") => current = Some(value),
                Some("renogy_soc_percent_value") => soc_percent = Some(value),
                Some("renogy_remaining_capacity_ah_value") => remaining_capacity = Some(value),
                Some("renogy_total_capacity_ah_value") => total_capacity = Some(value),
                Some("renogy_cycle_count_value") => cycle_count = Some(value),
                Some("renogy_cell_voltage_value") => {
                    if let Some(cell) = sample.metric().get("cell").and_then(|c| c.parse().ok()) {
                        cell_voltages.push((cell, value));
                    }
                }
                Some("renogy_cell_temperature_value") => {
                    if let Some(cell) = sample.metric().get("cell").and_then(|c| c.parse().ok()) {
                        cell_temperatures.push((cell, value));
                    }
                }
                _ => {}
            }
        }

        if module_voltage.is_none() && soc_percent.is_none() {
            return Ok(None);
        }

        cell_voltages.sort_by_key(|(n, _)| *n);
        cell_temperatures.sort_by_key(|(n, _)| *n);

        let cell_voltages: Vec<f32> = cell_voltages.into_iter().map(|(_, v)| v).collect();
        let cell_temperatures: Vec<f32> = cell_temperatures.into_iter().map(|(_, v)| v).collect();

        Ok(Some(BatteryInfo {
            serial: battery.to_string(),
            model: String::new(),
            software_version: String::new(),
            manufacturer: String::new(),
            cell_count: cell_voltages.len() as u32,
            cell_voltages,
            cell_temperatures,
            bms_temperature: None,
            environment_temperatures: Vec::new(),
            heater_temperatures: Vec::new(),
            module_voltage: module_voltage.unwrap_or(0.0),
            current: current.unwrap_or(0.0),
            soc_percent: soc_percent.unwrap_or(0.0),
            remaining_capacity: remaining_capacity.unwrap_or(0.0),
            total_capacity: total_capacity.unwrap_or(0.0),
            cycle_count: cycle_count.unwrap_or(0.0) as u32,
            charge_voltage_limit: None,
            discharge_voltage_limit: None,
            charge_current_limit: None,
            discharge_current_limit: None,
            status1: None,
            status2: None,
            status3: None,
            other_alarm_info: None,
            cell_voltage_alarms: None,
            cell_temperature_alarms: None,
            charge_discharge_status: None,
            timestamp: chrono::Utc::now(),
        }))
    }

    pub async fn query_range(
        &self,
        start_secs: u64,
        end_secs: u64,
        step_secs: u64,
    ) -> Result<Vec<DataPoint>, String> {
        let start = start_secs as i64;
        let end = end_secs as i64;
        let step = step_secs as f64;

        let agg_window = format!("{}s", step_secs);

        let current_query = format!("avg_over_time(sum(renogy_current_value)[{}])", agg_window);
        let soc_query = format!(
            "avg_over_time((sum(renogy_remaining_capacity_ah_value) / sum(renogy_total_capacity_ah_value) * 100)[{}])",
            agg_window
        );
        let temp_query = format!(
            "avg_over_time(avg(renogy_cell_temperature_value)[{}])",
            agg_window
        );

        let current_data = self
            .query_range_single(&current_query, start, end, step)
            .await?;
        let soc_data = self
            .query_range_single(&soc_query, start, end, step)
            .await?;
        let temp_data = self
            .query_range_single(&temp_query, start, end, step)
            .await?;

        let mut all_timestamps: Vec<u64> = current_data
            .iter()
            .chain(soc_data.iter())
            .chain(temp_data.iter())
            .map(|(ts, _)| *ts)
            .collect();
        all_timestamps.sort();
        all_timestamps.dedup();

        let current_map: std::collections::HashMap<u64, f32> = current_data.into_iter().collect();
        let soc_map: std::collections::HashMap<u64, f32> = soc_data.into_iter().collect();
        let temp_map: std::collections::HashMap<u64, f32> = temp_data.into_iter().collect();

        let points: Vec<DataPoint> = all_timestamps
            .into_iter()
            .map(|ts| DataPoint {
                timestamp_secs: ts,
                current: current_map.get(&ts).copied().unwrap_or(0.0),
                soc: soc_map.get(&ts).copied().unwrap_or(0.0),
                temp_avg: temp_map.get(&ts).copied(),
            })
            .collect();

        Ok(points)
    }

    async fn query_range_single(
        &self,
        query: &str,
        start: i64,
        end: i64,
        step: f64,
    ) -> Result<Vec<(u64, f32)>, String> {
        let response = self
            .client
            .query_range(query, start, end, step)
            .get()
            .await
            .map_err(|e| e.to_string())?;

        let mut data = Vec::new();

        if let Some(matrix) = response.data().as_matrix()
            && let Some(range_vec) = matrix.first()
        {
            for sample in range_vec.samples() {
                let ts = sample.timestamp();
                let val = sample.value() as f32;
                data.push((ts as u64, val));
            }
        }

        Ok(data)
    }
}

pub fn calculate_step_for_duration(duration_secs: u64) -> u64 {
    match duration_secs {
        0..=3600 => 15,       // 1 hour: 15s step
        3601..=21600 => 60,   // 6 hours: 1m step
        21601..=86400 => 300, // 24 hours: 5m step
        _ => 1800,            // 7 days+: 30m step
    }
}
