use serde::{Deserialize, Serialize};

use crate::profile::{EquipmentPoints, SectionPoints};

/// Index and count metadata for all point groups in the profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct KeySheet {
    pub config: SectionPoints,
    pub functions: SectionPoints,
    pub curves: SectionPoints,
    pub system_meter: SectionPoints,
    pub extensions: SectionPoints,
    pub experimental: SectionPoints,
    pub vendor: SectionPoints,
    pub discovery: SectionPoints,
    pub schedules_bc: SectionPoints,
    pub schedules_bc_status: SectionPoints,
    pub schedules: SectionPoints,
    pub schedules_status: SectionPoints,
    pub max_points: u16,
    pub meter: EquipmentPoints,
    pub der: EquipmentPoints,
    pub inverter: EquipmentPoints,
    pub battery: EquipmentPoints,
}
