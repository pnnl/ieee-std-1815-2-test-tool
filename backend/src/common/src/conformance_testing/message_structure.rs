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
