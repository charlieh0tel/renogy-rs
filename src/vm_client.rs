use prometheus_http_query::{Client, Error as PromError};

use crate::BatteryInfo;
use crate::alarm::{Status1, Status2};

fn sort_and_extract(mut indexed: Vec<(u32, f32)>) -> Vec<f32> {
    indexed.sort_by_key(|(n, _)| *n);
    indexed.into_iter().map(|(_, v)| v).collect()
}

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
        let mut environment_temperatures: Vec<(u32, f32)> = Vec::new();
        let mut heater_temperatures: Vec<(u32, f32)> = Vec::new();
        let mut charge_voltage_limit = None;
        let mut discharge_voltage_limit = None;
        let mut charge_current_limit = None;
        let mut discharge_current_limit = None;
        let mut status1_raw = None;
        let mut status2_raw = None;

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
                Some("renogy_charge_voltage_limit_value") => charge_voltage_limit = Some(value),
                Some("renogy_discharge_voltage_limit_value") => {
                    discharge_voltage_limit = Some(value)
                }
                Some("renogy_charge_current_limit_value") => charge_current_limit = Some(value),
                Some("renogy_discharge_current_limit_value") => {
                    discharge_current_limit = Some(value)
                }
                Some("renogy_status1_value") => status1_raw = Some(value as u16),
                Some("renogy_status2_value") => status2_raw = Some(value as u16),
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
                Some("renogy_environment_temperature_value") => {
                    if let Some(sensor) = sample.metric().get("sensor").and_then(|c| c.parse().ok())
                    {
                        environment_temperatures.push((sensor, value));
                    }
                }
                Some("renogy_heater_temperature_value") => {
                    if let Some(sensor) = sample.metric().get("sensor").and_then(|c| c.parse().ok())
                    {
                        heater_temperatures.push((sensor, value));
                    }
                }
                _ => {}
            }
        }

        if module_voltage.is_none() && soc_percent.is_none() {
            return Ok(None);
        }

        let cell_voltages = sort_and_extract(cell_voltages);
        let cell_temperatures = sort_and_extract(cell_temperatures);
        let environment_temperatures = sort_and_extract(environment_temperatures);
        let heater_temperatures = sort_and_extract(heater_temperatures);

        Ok(Some(BatteryInfo {
            serial: battery.to_string(),
            model: String::new(),
            software_version: String::new(),
            manufacturer: String::new(),
            cell_count: cell_voltages.len() as u32,
            cell_voltages,
            cell_temperatures,
            bms_temperature: None,
            environment_temperatures,
            heater_temperatures,
            module_voltage: module_voltage.unwrap_or(0.0),
            current: current.unwrap_or(0.0),
            soc_percent: soc_percent.unwrap_or(0.0),
            remaining_capacity: remaining_capacity.unwrap_or(0.0),
            total_capacity: total_capacity.unwrap_or(0.0),
            cycle_count: cycle_count.unwrap_or(0.0) as u32,
            charge_voltage_limit,
            discharge_voltage_limit,
            charge_current_limit,
            discharge_current_limit,
            status1: status1_raw.map(Status1::from_bits_truncate),
            status2: status2_raw.map(Status2::from_bits_truncate),
            status3: None,
            other_alarm_info: None,
            cell_voltage_alarms: None,
            cell_temperature_alarms: None,
            charge_discharge_status: None,
            timestamp: chrono::Utc::now(),
        }))
    }

    pub async fn query_all_batteries(&self) -> Result<Vec<BatteryInfo>, String> {
        let batteries = self.discover_batteries().await?;
        let mut results = Vec::new();
        for battery in batteries {
            if let Some(info) = self.query_latest(&battery).await? {
                results.push(info);
            }
        }
        Ok(results)
    }

    pub async fn query_range_raw(
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
                data.push((sample.timestamp() as u64, sample.value() as f32));
            }
        }
        Ok(data)
    }
}
