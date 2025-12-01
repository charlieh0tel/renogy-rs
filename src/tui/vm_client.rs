use std::collections::HashMap;

pub use crate::vm_client::{VmClient, VmError};

use super::history::DataPoint;

pub async fn query_range(
    client: &VmClient,
    start_secs: u64,
    end_secs: u64,
    step_secs: u64,
) -> Result<Vec<DataPoint>, VmError> {
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

    let current_data = client
        .query_range_raw(&current_query, start, end, step)
        .await?;
    let soc_data = client.query_range_raw(&soc_query, start, end, step).await?;
    let temp_data = client
        .query_range_raw(&temp_query, start, end, step)
        .await?;

    let mut all_timestamps: Vec<u64> = current_data
        .iter()
        .chain(soc_data.iter())
        .chain(temp_data.iter())
        .map(|(ts, _)| *ts)
        .collect();
    all_timestamps.sort();
    all_timestamps.dedup();

    let current_map: HashMap<u64, f32> = current_data.into_iter().collect();
    let soc_map: HashMap<u64, f32> = soc_data.into_iter().collect();
    let temp_map: HashMap<u64, f32> = temp_data.into_iter().collect();

    Ok(all_timestamps
        .into_iter()
        .map(|ts| DataPoint {
            timestamp_secs: ts,
            current: current_map.get(&ts).copied().unwrap_or(0.0),
            soc: soc_map.get(&ts).copied().unwrap_or(0.0),
            temp_avg: temp_map.get(&ts).copied(),
        })
        .collect())
}

pub fn calculate_step_for_duration(duration_secs: u64) -> u64 {
    match duration_secs {
        0..=3600 => 15,       // 1 hour: 15s step
        3601..=21600 => 60,   // 6 hours: 1m step
        21601..=86400 => 300, // 24 hours: 5m step
        _ => 1800,            // 7 days+: 30m step
    }
}
