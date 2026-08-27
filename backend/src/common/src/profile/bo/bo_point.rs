use serde::{Deserialize, Serialize};

/// A binary output (BO) point defined in the PICS profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct BoPoint {
    pub point_index: u16,
    pub name: String,
    pub state_0: String,
    pub state_1: String,
    pub iec_61850_uid: String,
    pub assoc_bi: Option<String>,
    pub purpose: String,
    pub mandatory_1815: bool,
    pub mandatory_1547: bool,
}
