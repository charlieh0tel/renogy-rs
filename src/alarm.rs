use bitflags::bitflags;

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub enum CellVoltageAlarm {
    #[default]
    Normal,
    OverVoltage,
    UnderVoltage,
}

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub enum CellTemperatureAlarm {
    #[default]
    Normal,
    OverTemperature,
    UnderTemperature,
}

macro_rules! define_cell_alarms {
    ($name:ident, $alarm_type:ty, $over:expr, $under:expr) => {
        #[derive(Debug, PartialEq, Eq, Clone, Copy)]
        pub struct $name {
            pub alarms: [$alarm_type; 16],
        }

        impl $name {
            #[must_use]
            pub fn from_bits(value: u32) -> Self {
                let mut alarms = [<$alarm_type>::default(); 16];
                for (i, alarm) in alarms.iter_mut().enumerate() {
                    if (value >> (i + 16)) & 1 == 1 {
                        *alarm = $over;
                    } else if (value >> i) & 1 == 1 {
                        *alarm = $under;
                    }
                }
                Self { alarms }
            }
        }
    };
}

define_cell_alarms!(
    CellVoltageAlarms,
    CellVoltageAlarm,
    CellVoltageAlarm::OverVoltage,
    CellVoltageAlarm::UnderVoltage
);

define_cell_alarms!(
    CellTemperatureAlarms,
    CellTemperatureAlarm,
    CellTemperatureAlarm::OverTemperature,
    CellTemperatureAlarm::UnderTemperature
);

bitflags! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    pub struct OtherAlarmInfo: u32 {
        const BMS_OVER_TEMPERATURE = 1 << 31;
        const BMS_UNDER_TEMPERATURE = 1 << 30;
        const ENV_OVER_TEMPERATURE = 1 << 29;
        const ENV_UNDER_TEMPERATURE = 1 << 28;
        const HEATER_OVER_TEMPERATURE = 1 << 27;
        const HEATER_UNDER_TEMPERATURE = 1 << 26;
        const CHARGE_OVER_CURRENT = 1 << 21;
        const DISCHARGE_OVER_CURRENT = 1 << 19;
    }
}

bitflags! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    pub struct Status1: u16 {
        const MODULE_UNDER_VOLTAGE = 1 << 15;
        const CHARGE_OVER_TEMP = 1 << 14;
        const CHARGE_UNDER_TEMP = 1 << 13;
        const DISCHARGE_OVER_TEMP = 1 << 12;
        const DISCHARGE_UNDER_TEMP = 1 << 11;
        const DISCHARGE_OVER_CURRENT1 = 1 << 10;
        const CHARGE_OVER_CURRENT1 = 1 << 9;
        const CELL_OVER_VOLTAGE = 1 << 8;
        const CELL_UNDER_VOLTAGE = 1 << 7;
        const MODULE_OVER_VOLTAGE = 1 << 6;
        const DISCHARGE_OVER_CURRENT2 = 1 << 5;
        const CHARGE_OVER_CURRENT2 = 1 << 4;
        const USING_BATTERY_MODULE_POWER = 1 << 3;
        const DISCHARGE_MOSFET = 1 << 2;
        const CHARGE_MOSFET = 1 << 1;
        const SHORT_CIRCUIT = 1 << 0;
    }
}

bitflags! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    pub struct Status2: u16 {
        const EFFECTIVE_CHARGE_CURRENT = 1 << 15;
        const EFFECTIVE_DISCHARGE_CURRENT = 1 << 14;
        const HEATER_ON = 1 << 13;
        const FULLY_CHARGED = 1 << 11;
        const BUZZER = 1 << 8;
        const DISCHARGE_HIGH_TEMP_WARN = 1 << 7;
        const DISCHARGE_LOW_TEMP_WARN = 1 << 6;
        const CHARGE_HIGH_TEMP_WARN = 1 << 5;
        const CHARGE_LOW_TEMP_WARN = 1 << 4;
        const MODULE_HIGH_VOLTAGE_WARN = 1 << 3;
        const MODULE_LOW_VOLTAGE_WARN = 1 << 2;
        const CELL_HIGH_VOLTAGE_WARN = 1 << 1;
        const CELL_LOW_VOLTAGE_WARN = 1 << 0;
    }
}

bitflags! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    pub struct Status3: u16 {
        const CELL_1_VOLTAGE_ERROR = 1 << 0;
        const CELL_2_VOLTAGE_ERROR = 1 << 1;
        const CELL_3_VOLTAGE_ERROR = 1 << 2;
        const CELL_4_VOLTAGE_ERROR = 1 << 3;
        const CELL_5_VOLTAGE_ERROR = 1 << 4;
        const CELL_6_VOLTAGE_ERROR = 1 << 5;
        const CELL_7_VOLTAGE_ERROR = 1 << 6;
        const CELL_8_VOLTAGE_ERROR = 1 << 7;
        const CELL_9_VOLTAGE_ERROR = 1 << 8;
        const CELL_10_VOLTAGE_ERROR = 1 << 9;
        const CELL_11_VOLTAGE_ERROR = 1 << 10;
        const CELL_12_VOLTAGE_ERROR = 1 << 11;
        const CELL_13_VOLTAGE_ERROR = 1 << 12;
        const CELL_14_VOLTAGE_ERROR = 1 << 13;
        const CELL_15_VOLTAGE_ERROR = 1 << 14;
        const CELL_16_VOLTAGE_ERROR = 1 << 15;
    }
}

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub enum CellVoltageError {
    #[default]
    Normal,
    Error,
}

macro_rules! define_cell_errors {
    ($name:ident, $error_type:ty, $error_variant:expr) => {
        #[derive(Debug, PartialEq, Eq, Clone, Copy)]
        pub struct $name {
            pub errors: [$error_type; 16],
        }

        impl $name {
            #[must_use]
            pub fn from_bits(value: u16) -> Self {
                let mut errors = [<$error_type>::default(); 16];
                for (i, error) in errors.iter_mut().enumerate() {
                    if (value >> i) & 1 == 1 {
                        *error = $error_variant;
                    }
                }
                Self { errors }
            }
        }
    };
}

define_cell_errors!(CellVoltageErrors, CellVoltageError, CellVoltageError::Error);

bitflags! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    pub struct ChargeDischargeStatus: u16 {
        const CHARGE_ENABLE = 1 << 7;
        const DISCHARGE_ENABLE = 1 << 6;
        const CHARGE_IMMEDIATE = 1 << 5;
        const CHARGE_IMMEDIATE2 = 1 << 4;
        const FULL_CHARGE_REQUEST = 1 << 3;
    }
}
