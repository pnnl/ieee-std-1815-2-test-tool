use poem::web::Json;
use poem::{Route, get, handler};
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use utoipa::ToSchema;

use crate::routes::curve_types::{CurveTypeEntry, build_curve_types};
use common::compatibility_matrix::ModeType;

#[derive(Debug, Serialize, Clone, PartialEq, ToSchema)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub struct EnumEntry {
    pub value: u16,
    pub name: String,
    pub display_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, ToSchema)]
pub enum PowerModeCategory {
    ActivePower,
    ReactivePower,
    EmergencyMode,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, ToSchema)]
pub struct ModeTypeEntry {
    pub variant: ModeType,
    pub display_name: String,
    pub purposes: Vec<String>,
    pub category: PowerModeCategory,
}

#[derive(Debug, Serialize, Clone, PartialEq, ToSchema)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub struct EnumsResponse {
    pub curve_types: Vec<CurveTypeEntry>,
    pub curve_x_units: Vec<EnumEntry>,
    pub curve_y_units: Vec<EnumEntry>,
    pub action_types: Vec<EnumEntry>,
    pub schedule_interval_units: Vec<EnumEntry>,
    pub mode_types: Vec<ModeTypeEntry>,
}

fn build_enum_entries(variants: Vec<(u16, &str, &str)>) -> Vec<EnumEntry> {
    variants
        .into_iter()
        .map(|(value, name, display_name)| EnumEntry {
            value,
            name: name.to_string(),
            display_name: display_name.to_string(),
        })
        .collect()
}

// TODO: use IndependentVariableUnit instead
fn build_curve_x_units() -> Vec<EnumEntry> {
    let variants: Vec<(u16, &str, &str)> = vec![
        (0, "NotDefined", "Not Defined"),
        (1, "Unknown", "Unknown"),
        (4, "TimeMilliseconds", "Time (ms)"),
        (23, "CelsiusTemperature", "Celsius Temperature"),
        (29, "VoltageVolts", "Voltage (V)"),
        (33, "FrequencyHertz", "Frequency (Hz)"),
        (38, "Watts", "Watts (W)"),
        (
            100,
            "PriceHundredthsLocalCurrency",
            "Price (hundredths local currency)",
        ),
        (129, "PercentVoltage", "Percent Voltage (%)"),
        (133, "PercentFrequency", "Percent Frequency (%)"),
        (138, "PercentWatts", "Percent Watts (%)"),
        (233, "FrequencyDeviation", "Frequency Deviation"),
    ];
    build_enum_entries(variants)
}

// TODO: use DependentVariableUnit instead
fn build_curve_y_units() -> Vec<EnumEntry> {
    let variants: Vec<(u16, &str, &str)> = vec![
        (0, "NotDefined", "Not Defined"),
        (1, "Unknown", "Unknown"),
        (2, "VarsPercentMaxVars", "VARs (% Max VARs)"),
        (3, "VarsPercentVarAvailable", "VARs (% VAR Available)"),
        (4, "VarsPercentMaxWatts", "VARs (% Max Watts)"),
        (5, "WattsPercentMaxWatts", "Watts (% Max Watts)"),
        (
            6,
            "WattsPercentFrozenActivePower",
            "Watts (% Frozen Active Power)",
        ),
        (7, "PowerFactor", "Power Factor"),
        (8, "VoltsPercentVref", "Volts (% Vref)"),
        (9, "FrequencyPercentNominal", "Frequency (% Nominal)"),
        (29, "VoltageVolts", "Voltage (V)"),
        (33, "FrequencyHertz", "Frequency (Hz)"),
        (38, "Watts", "Watts (W)"),
    ];
    build_enum_entries(variants)
}

fn build_action_types() -> Vec<EnumEntry> {
    let variants: Vec<(u16, &str, &str)> = vec![
        (0, "Null", "Null"),
        (1, "SetAnalogOutput", "Set Analog Output"),
        (2, "SetBinaryOutput", "Set Binary Output"),
        (3, "StopSchedule", "Stop Schedule"),
    ];
    build_enum_entries(variants)
}

fn build_schedule_interval_units() -> Vec<EnumEntry> {
    let variants: Vec<(u16, &str, &str)> = vec![
        (0, "NoRepeat", "No Repeat"),
        (1, "Seconds", "Seconds"),
        (2, "Minutes", "Minutes"),
        (3, "Hours", "Hours"),
        (4, "Days", "Days"),
        (5, "Weeks", "Weeks"),
        (6, "Months", "Months"),
        (7, "MonthsOnSameDayOfWeek", "Months (Same Day of Week)"),
        (
            8,
            "MonthsOnSameDayOfWeekFromEnd",
            "Months (Same Day of Week from End)",
        ),
    ];
    build_enum_entries(variants)
}

fn build_mode_types() -> Vec<ModeTypeEntry> {
    ModeType::iter()
        .map(|mode| {
            let variant = mode;
            let display_name = mode_display_name(&mode);
            let purposes = mode.to_purposes().iter().map(|s| s.to_string()).collect();
            ModeTypeEntry {
                variant,
                display_name,
                purposes,
                category: power_mode_to_category(&mode),
            }
        })
        .collect()
}

fn mode_display_name(mode: &ModeType) -> String {
    match mode {
        ModeType::VoltageRideThrough => "Voltage Ride-Through",
        ModeType::FrequencyRideThrough => "Frequency Ride-Through",
        ModeType::SetActivePower => "Set Active Power",
        ModeType::ActivePowerLimiting => "Active Power Limiting",
        ModeType::FrequencyWatt => "Frequency-Watt",
        ModeType::VoltWatt => "Volt-Watt",
        ModeType::CoordinatedChargeDischarge => "Coordinated Charge/Discharge",
        ModeType::ActivePowerFollowing => "Active Power Following",
        ModeType::AutomaticGenerationControl => "Automatic Generation Control",
        ModeType::ActivePowerSmoothing => "Active Power Smoothing",
        ModeType::FrequencyWattCurve => "Frequency-Watt Curve",
        ModeType::DynamicVoltWatt => "Dynamic Volt-Watt",
        ModeType::ConstantVars => "Constant VARs",
        ModeType::ConstantPowerFactor => "Constant Power Factor",
        ModeType::VoltVar => "Volt-VAR",
        ModeType::WattVar => "Watt-VAR",
        ModeType::PowerFactorCorrection => "Power Factor Correction",
        ModeType::DynamicReactiveCurrent => "Dynamic Reactive Current",
        ModeType::Price => "Pricing Signal",
        ModeType::PeakPowerLimiting => "Peak Power Limiting",
    }
    .to_string()
}

fn power_mode_to_category(mode: &ModeType) -> PowerModeCategory {
    match mode {
        ModeType::SetActivePower
        | ModeType::CoordinatedChargeDischarge
        | ModeType::ActivePowerLimiting
        | ModeType::AutomaticGenerationControl
        | ModeType::ActivePowerSmoothing
        | ModeType::ActivePowerFollowing
        | ModeType::VoltWatt
        | ModeType::FrequencyWattCurve
        | ModeType::PeakPowerLimiting
        | ModeType::Price => PowerModeCategory::ActivePower,
        ModeType::ConstantVars
        | ModeType::ConstantPowerFactor
        | ModeType::VoltVar
        | ModeType::WattVar
        | ModeType::PowerFactorCorrection => PowerModeCategory::ReactivePower,
        ModeType::VoltageRideThrough
        | ModeType::FrequencyRideThrough
        | ModeType::DynamicReactiveCurrent
        | ModeType::DynamicVoltWatt
        | ModeType::FrequencyWatt => PowerModeCategory::EmergencyMode,
    }
}

#[utoipa::path(
    get,
    path = "/api/enums",
    tag = "enums",
    responses(
        (status = 200, description = "Enum lookup tables for the frontend", body = EnumsResponse)
    )
)]
#[handler]
pub async fn get_enums() -> Json<EnumsResponse> {
    Json(EnumsResponse {
        curve_types: build_curve_types(),
        curve_x_units: build_curve_x_units(),
        curve_y_units: build_curve_y_units(),
        action_types: build_action_types(),
        schedule_interval_units: build_schedule_interval_units(),
        mode_types: build_mode_types(),
    })
}

pub fn routes() -> Route {
    Route::new().at("/", get(get_enums))
}

#[cfg(test)]
mod tests {
    use super::*;
    use poem::test::TestClient;

    #[tokio::test]
    async fn test_get_enums_returns_ok() {
        let app = routes();
        let client = TestClient::new(app);
        let resp = client.get("/").send().await;
        resp.assert_status_is_ok();
    }

    #[tokio::test]
    async fn test_get_enums_returns_all_sections() {
        // Build expected response directly and verify all sections non-empty
        let response = EnumsResponse {
            curve_types: build_curve_types(),
            curve_x_units: build_curve_x_units(),
            curve_y_units: build_curve_y_units(),
            action_types: build_action_types(),
            schedule_interval_units: build_schedule_interval_units(),
            mode_types: build_mode_types(),
        };
        assert!(!response.curve_types.is_empty());
        assert!(!response.curve_x_units.is_empty());
        assert!(!response.curve_y_units.is_empty());
        assert!(!response.action_types.is_empty());
        assert!(!response.schedule_interval_units.is_empty());
        assert!(!response.mode_types.is_empty());

        // Verify it round-trips through JSON
        let json = serde_json::to_value(&response).unwrap();
        assert!(json["curve_types"].is_array());
        assert!(json["curve_x_units"].is_array());
        assert!(json["curve_y_units"].is_array());
        assert!(json["action_types"].is_array());
        assert!(json["schedule_interval_units"].is_array());
        assert!(json["mode_types"].is_array());
    }

    #[tokio::test]
    async fn test_curve_x_units_has_12_entries() {
        let entries = build_curve_x_units();
        assert_eq!(entries.len(), 12);
        assert_eq!(entries[0].value, 0);
        assert_eq!(entries[11].name, "FrequencyDeviation");
        assert_eq!(entries[11].value, 233);
    }

    #[tokio::test]
    async fn test_curve_y_units_has_13_entries() {
        let entries = build_curve_y_units();
        assert_eq!(entries.len(), 13);
        assert_eq!(entries[0].value, 0);
        assert_eq!(entries[12].name, "Watts");
        assert_eq!(entries[12].value, 38);
    }

    #[tokio::test]
    async fn test_action_types_has_4_entries() {
        let entries = build_action_types();
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0].name, "Null");
        assert_eq!(entries[0].display_name, "Null");
        assert_eq!(entries[1].display_name, "Set Analog Output");
        assert_eq!(entries[3].display_name, "Stop Schedule");
    }

    #[tokio::test]
    async fn test_schedule_interval_units_has_9_entries() {
        let entries = build_schedule_interval_units();
        assert_eq!(entries.len(), 9);
        assert_eq!(entries[0].name, "NoRepeat");
        assert_eq!(entries[8].name, "MonthsOnSameDayOfWeekFromEnd");
    }

    #[tokio::test]
    async fn test_mode_types_include_purposes() {
        let entries = build_mode_types();
        // VoltVar should have "volt-var" purpose
        let volt_var = entries
            .iter()
            .find(|e| e.variant == ModeType::VoltVar)
            .unwrap();
        assert_eq!(volt_var.purposes, vec!["volt-var"]);

        // ConstantVars should have two purposes
        let const_vars = entries
            .iter()
            .find(|e| e.variant == ModeType::ConstantVars)
            .unwrap();
        assert_eq!(const_vars.purposes, vec!["const vars", "constant vars"]);
    }

    #[tokio::test]
    async fn test_curve_types_display_names() {
        let entries = build_curve_types();
        assert_eq!(entries[2].display_name, "Volt-VAR");
        assert_eq!(entries[3].display_name, "Frequency-Watt");
        assert_eq!(entries[6].display_name, "Remain Connected");
    }

    #[tokio::test]
    async fn test_mode_types_display_names() {
        let entries = build_mode_types();
        let vrt = entries
            .iter()
            .find(|e| e.variant == ModeType::VoltageRideThrough)
            .unwrap();
        assert_eq!(vrt.display_name, "Voltage Ride-Through");

        let agc = entries
            .iter()
            .find(|e| e.variant == ModeType::AutomaticGenerationControl)
            .unwrap();
        assert_eq!(agc.display_name, "Automatic Generation Control");
    }

    #[test]
    fn guard_mode_types_matches_enum_count() {
        let endpoint_count = build_mode_types().len();
        let enum_count = ModeType::iter().count();
        assert_eq!(
            endpoint_count, enum_count,
            "ModeType enum has {enum_count} variants but endpoint serves {endpoint_count}. Update build_mode_types()."
        );
    }

    #[test]
    fn guard_action_types_matches_enum_count() {
        // ActionType has 4 variants (0..=3). No EnumIter available.
        let endpoint_count = build_action_types().len();
        assert_eq!(
            endpoint_count, 4,
            "Expected 4 ActionType variants but endpoint serves {endpoint_count}. Update build_action_types()."
        );
    }

    #[test]
    fn guard_schedule_interval_units_matches_enum_count() {
        // ScheduleIntervalUnit has 9 variants (0..=8). No EnumIter available.
        let endpoint_count = build_schedule_interval_units().len();
        assert_eq!(
            endpoint_count, 9,
            "Expected 9 ScheduleIntervalUnit variants but endpoint serves {endpoint_count}. Update build_schedule_interval_units()."
        );
    }
}
