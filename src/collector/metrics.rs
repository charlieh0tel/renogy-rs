use crate::BatteryInfo;
use influxdb_line_protocol::LineProtocolBuilder;
use prometheus_client::encoding::EncodeLabelSet;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::registry::Registry;
use std::sync::atomic::AtomicU64;

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct BatteryLabels {
    pub battery: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct CellLabels {
    pub battery: String,
    pub cell: String,
}

#[derive(Default)]
pub struct PrometheusMetrics {
    pub cell_voltage: Family<CellLabels, Gauge<f64, AtomicU64>>,
    pub cell_temperature: Family<CellLabels, Gauge<f64, AtomicU64>>,
    pub module_voltage: Family<BatteryLabels, Gauge<f64, AtomicU64>>,
    pub current: Family<BatteryLabels, Gauge<f64, AtomicU64>>,
    pub remaining_capacity_ah: Family<BatteryLabels, Gauge<f64, AtomicU64>>,
    pub total_capacity_ah: Family<BatteryLabels, Gauge<f64, AtomicU64>>,
    pub soc_percent: Family<BatteryLabels, Gauge<f64, AtomicU64>>,
    pub cycle_count: Family<BatteryLabels, Gauge<f64, AtomicU64>>,
}

impl PrometheusMetrics {
    pub fn register(&self, registry: &mut Registry) {
        registry.register(
            "renogy_cell_voltage",
            "Individual cell voltage in volts",
            self.cell_voltage.clone(),
        );
        registry.register(
            "renogy_cell_temperature",
            "Individual cell temperature in celsius",
            self.cell_temperature.clone(),
        );
        registry.register(
            "renogy_module_voltage",
            "Module/pack voltage in volts",
            self.module_voltage.clone(),
        );
        registry.register(
            "renogy_current",
            "Battery current in amps (positive=charging, negative=discharging)",
            self.current.clone(),
        );
        registry.register(
            "renogy_remaining_capacity_ah",
            "Remaining capacity in amp-hours",
            self.remaining_capacity_ah.clone(),
        );
        registry.register(
            "renogy_total_capacity_ah",
            "Total capacity in amp-hours",
            self.total_capacity_ah.clone(),
        );
        registry.register(
            "renogy_soc_percent",
            "State of charge percentage",
            self.soc_percent.clone(),
        );
        registry.register(
            "renogy_cycle_count",
            "Number of charge cycles",
            self.cycle_count.clone(),
        );
    }

    pub fn update(&self, info: &BatteryInfo) {
        let serial = &info.serial;
        let battery_labels = BatteryLabels {
            battery: serial.clone(),
        };

        for (i, &voltage) in info.cell_voltages.iter().enumerate() {
            let labels = CellLabels {
                battery: serial.clone(),
                cell: (i + 1).to_string(),
            };
            self.cell_voltage.get_or_create(&labels).set(voltage as f64);
        }

        for (i, &temp) in info.cell_temperatures.iter().enumerate() {
            let labels = CellLabels {
                battery: serial.clone(),
                cell: (i + 1).to_string(),
            };
            self.cell_temperature
                .get_or_create(&labels)
                .set(temp as f64);
        }

        self.module_voltage
            .get_or_create(&battery_labels)
            .set(info.module_voltage as f64);
        self.current
            .get_or_create(&battery_labels)
            .set(info.current as f64);
        self.remaining_capacity_ah
            .get_or_create(&battery_labels)
            .set(info.remaining_capacity as f64);
        self.total_capacity_ah
            .get_or_create(&battery_labels)
            .set(info.total_capacity as f64);
        self.soc_percent
            .get_or_create(&battery_labels)
            .set(info.soc_percent as f64);
        self.cycle_count
            .get_or_create(&battery_labels)
            .set(info.cycle_count as f64);
    }
}

pub fn batch_to_influx(samples: &[BatteryInfo]) -> String {
    macro_rules! measurement {
        ($b:expr, $name:expr, $serial:expr, $value:expr, $ts:expr) => {
            $b.measurement($name)
                .tag("battery", $serial)
                .field("value", $value)
                .timestamp($ts)
                .close_line()
        };
    }

    macro_rules! cell_measurement {
        ($b:expr, $name:expr, $serial:expr, $cell:expr, $value:expr, $ts:expr) => {
            $b.measurement($name)
                .tag("battery", $serial)
                .tag("cell", $cell)
                .field("value", $value)
                .timestamp($ts)
                .close_line()
        };
    }

    let mut builder = LineProtocolBuilder::new();

    for info in samples {
        let ts = info.timestamp.timestamp_nanos_opt().unwrap_or(0);
        let serial = &info.serial;

        for (i, &voltage) in info.cell_voltages.iter().enumerate() {
            let cell = (i + 1).to_string();
            builder = cell_measurement!(
                builder,
                "renogy_cell_voltage",
                serial,
                &cell,
                voltage as f64,
                ts
            );
        }

        for (i, &temp) in info.cell_temperatures.iter().enumerate() {
            let cell = (i + 1).to_string();
            builder = cell_measurement!(
                builder,
                "renogy_cell_temperature",
                serial,
                &cell,
                temp as f64,
                ts
            );
        }

        builder = measurement!(
            builder,
            "renogy_module_voltage",
            serial,
            info.module_voltage as f64,
            ts
        );
        builder = measurement!(builder, "renogy_current", serial, info.current as f64, ts);
        builder = measurement!(
            builder,
            "renogy_remaining_capacity_ah",
            serial,
            info.remaining_capacity as f64,
            ts
        );
        builder = measurement!(
            builder,
            "renogy_total_capacity_ah",
            serial,
            info.total_capacity as f64,
            ts
        );
        builder = measurement!(
            builder,
            "renogy_soc_percent",
            serial,
            info.soc_percent as f64,
            ts
        );
        builder = measurement!(
            builder,
            "renogy_cycle_count",
            serial,
            info.cycle_count as f64,
            ts
        );
    }

    String::from_utf8(builder.build()).expect("line protocol should be valid UTF-8")
}
