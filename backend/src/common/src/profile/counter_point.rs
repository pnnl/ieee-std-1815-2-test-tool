use serde::{Deserialize, Serialize};

use crate::profile::EventClass;

/// A counter (CTR) point defined in the PICS profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CtrPoint {
    pub point_index: u16,
    pub name: String,
    pub counter_event_class: EventClass,
    pub frozen_counter_exists: bool,
    pub frozen_counter_event_class: EventClass,
    pub iec_61850_uid: String,
    pub purpose: String,
    pub mandatory_1815: bool,
    pub mandatory_1547: bool,
}
