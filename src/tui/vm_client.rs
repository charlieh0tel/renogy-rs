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
            return Err(format!(
                "Unexpected response type: {:?}",
                response.data()
            ));
        }

        Ok(batteries)
    }

    pub async fn query_latest(&self, battery: &str) -> Result<Option<BatteryInfo>, String> {
        let module_voltage = self
            .query_instant_value(&format!(
                "renogy_module_voltage_value{{battery=\"{}\"}}",
                battery
            ))
            .await?;
        let current = self
            .query_instant_value(&format!("renogy_current_value{{battery=\"{}\"}}", battery))
            .await?;
        let soc_percent = self
            .query_instant_value(&format!(
                "renogy_soc_percent_value{{battery=\"{}\"}}",
                battery
            ))
            .await?;
        let remaining_capacity = self
            .query_instant_value(&format!(
                "renogy_remaining_capacity_ah_value{{battery=\"{}\"}}",
                battery
            ))
            .await?;
        let total_capacity = self
            .query_instant_value(&format!(
                "renogy_total_capacity_ah_value{{battery=\"{}\"}}",
                battery
            ))
            .await?;
        let cycle_count = self
            .query_instant_value(&format!(
                "renogy_cycle_count_value{{battery=\"{}\"}}",
                battery
            ))
            .await?;

        if module_voltage.is_none() && soc_percent.is_none() {
            return Ok(None);
        }

        let cell_voltages = self
            .query_cell_values(battery, "renogy_cell_voltage_value")
            .await?;
        let cell_temperatures = self
            .query_cell_values(battery, "renogy_cell_temperature_value")
            .await?;

        let info = BatteryInfo {
            serial: battery.to_string(),
            model: String::new(),
            software_version: String::new(),
            manufacturer: String::new(),
            cell_count: cell_voltages.len() as u32,
            cell_voltages,
            cell_temperatures,
            module_voltage: module_voltage.unwrap_or(0.0),
            current: current.unwrap_or(0.0),
            soc_percent: soc_percent.unwrap_or(0.0),
            remaining_capacity: remaining_capacity.unwrap_or(0.0),
            total_capacity: total_capacity.unwrap_or(0.0),
            cycle_count: cycle_count.unwrap_or(0.0) as u32,
            timestamp: chrono::Utc::now(),
        };

        Ok(Some(info))
    }

    async fn query_instant_value(&self, query: &str) -> Result<Option<f32>, String> {
        let response = self
            .client
            .query(query)
            .get()
            .await
            .map_err(|e| e.to_string())?;

        if let Some(instant) = response.data().as_vector()
            && let Some(sample) = instant.first()
        {
            return Ok(Some(sample.sample().value() as f32));
        }

        Ok(None)
    }

    async fn query_cell_values(&self, battery: &str, metric: &str) -> Result<Vec<f32>, String> {
        let query = format!("{}{{battery=\"{}\"}}", metric, battery);
        let response = self
            .client
            .query(query)
            .get()
            .await
            .map_err(|e| e.to_string())?;

        let mut cells: Vec<(u32, f32)> = Vec::new();

        if let Some(instant) = response.data().as_vector() {
            for sample in instant {
                if let Some(cell_str) = sample.metric().get("cell")
                    && let Ok(cell_num) = cell_str.parse::<u32>()
                {
                    cells.push((cell_num, sample.sample().value() as f32));
                }
            }
        }

        cells.sort_by_key(|(n, _)| *n);
        Ok(cells.into_iter().map(|(_, v)| v).collect())
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
