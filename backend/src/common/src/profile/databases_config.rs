use serde::{Deserialize, Serialize};

use super::pics_profile::PicsProfile;

pub const DEFAULT_MAX_DATABASE_ENTRIES: u16 = 100;

/// Configuration for in-memory curve and schedule databases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatabasesConfig {
    /// Maximum number of curves to support.
    pub max_curves: u16,
    /// Maximum number of backward-compatible schedules to support.
    pub max_schedules_bc: u16,
    /// Maximum number of IEEE 1815.2 schedules to support.
    pub max_schedules: u16,
}

impl Default for DatabasesConfig {
    fn default() -> Self {
        Self {
            max_curves: DEFAULT_MAX_DATABASE_ENTRIES,
            max_schedules_bc: DEFAULT_MAX_DATABASE_ENTRIES,
            max_schedules: DEFAULT_MAX_DATABASE_ENTRIES,
        }
    }
}

impl DatabasesConfig {
    /// Dynamically adjust capacities based on a profile's declared counts,
    /// capped at `DEFAULT_MAX_DATABASE_ENTRIES`.
    pub fn from_profile(profile: &PicsProfile) -> Self {
        Self {
            max_curves: (profile.ai.curves.len() as u16).min(DEFAULT_MAX_DATABASE_ENTRIES),
            max_schedules_bc: (profile.ai.schedules_bc.len() as u16)
                .min(DEFAULT_MAX_DATABASE_ENTRIES),
            max_schedules: (profile.ai.schedules.len() as u16).min(DEFAULT_MAX_DATABASE_ENTRIES),
        }
    }
}
