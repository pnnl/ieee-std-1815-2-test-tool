use serde::{Deserialize, Serialize};

use crate::profile::{EventClass, Point};

/// A binary input (BI) point defined in the PICS profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct BiPoint {
    pub point_index: u16,
    pub name: String,
    pub event_class: EventClass,
    pub state_0: String,
    pub state_1: String,
    pub iec_61850_uid: String,
    pub assoc_bo: Option<String>,
    pub purpose: String,
    pub mandatory_1815: bool,
    pub mandatory_1547: bool,
}

impl Point for BiPoint {
    fn point_index(&self) -> u16 {
        self.point_index
    }
}
