use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

#[allow(non_camel_case_types)]
#[derive(
    Display, EnumIter, EnumString, Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize,
)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub enum TestName {
    MON_001,
    SCHED_001,
    SCHED_002,
    ALARM_001,
    CONN_001,
    SERV_001,
    OP_001,
    CURVE_001,
    CURVE_002,
    CURVE_003,
    VRT_001,
    FRT_001,
    FW_001,
    DRCS_001,
    DVW_001,
    APL_001,
    CHG_001,
    CCM_001,
    APR1_001,
    APR2_001,
    APR3_001,
    AGC_001,
    APS_001,
    VW_001,
    FWC_001,
    CVAR_001,
    FPF_001,
    VV_001,
    WV_001,
    PFC_001,
    PSIG_001,
}

impl TestName {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::MON_001 => "Monitoring",
            Self::ALARM_001 => "Alarm Reporting",
            Self::CONN_001 => "Connect and Disconnect",
            Self::SERV_001 => "Cease to Energize and Return to Service",
            Self::OP_001 => "Operation States",
            Self::CURVE_001 => "Reference Indication",
            Self::CURVE_002 => "Curve Locking",
            Self::CURVE_003 => "Mode Type Mismatch",
            Self::VRT_001 => "Voltage Ride-Through",
            Self::FRT_001 => "Frequency Ride-Through",
            Self::FW_001 => "Frequency-Watt",
            Self::DRCS_001 => "Dynamic Reactive Current Support",
            Self::DVW_001 => "Dynamic Volt-Watt",
            Self::APL_001 => "Active Power Limit",
            Self::CHG_001 => "Charge/Discharge Storage",
            Self::CCM_001 => "Coordinates Charge/Discharge Management",
            Self::APR1_001 => "Active Power Response 1",
            Self::APR2_001 => "Active Power Response 2",
            Self::APR3_001 => "Active Power Response 3",
            Self::AGC_001 => "Automatic Generation Control",
            Self::APS_001 => "Active Power Smoothing",
            Self::VW_001 => "Volt-Watt",
            Self::FWC_001 => "Frequency-Watt",
            Self::CVAR_001 => "Constant VAR",
            Self::FPF_001 => "Fixed Power Factor",
            Self::VV_001 => "Volt-VAR Control",
            Self::WV_001 => "Watt-VAR Control",
            Self::PFC_001 => "Power Factor Correction",
            Self::PSIG_001 => "Pricing Signal Control",
            Self::SCHED_001 => "Backward-Compatible Scheduling",
            Self::SCHED_002 => "Scheduling",
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SunSpecComment {
    pub uid: String,
    /// AI27
    pub index: String,
    /// Current value of the data point​. String since this is only
    /// for display purposes.
    pub value: String,
    /// IEEE 1815.2 clause/statement that was failed​
    pub issue: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SunSpecMessage {
    pub test: TestName,
    pub scenario_id: String,
    pub timestamp: u64,
    pub pass: bool,
    pub comments: Vec<SunSpecComment>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TestEvent {
    pub pass: bool,
    pub comments: Vec<SunSpecComment>,
}

/// Identity of a point whose behavior is being checked.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CheckPoint {
    /// AI, BI, AO, or BO.
    pub point_type: String,
    pub index: u16,
    pub uid: String,
    pub name: Option<String>,
}

/// One completed check, streamed independently of the aggregate test result.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PointCheckResult {
    pub scenario_id: String,
    pub test: TestName,
    pub group_name: String,
    pub point: CheckPoint,
    /// Increasing emission order within the subprocess.
    pub sequence: u64,
    /// Unix time in milliseconds.
    pub timestamp: u64,
    pub check_id: String,
    pub expected_behavior: String,
    pub passed: bool,
    pub message: String,
}
