use serde::{Deserialize, Serialize};

use crate::profile::BoPoint;

/// All binary output points in the profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct BinaryOutputs {
    pub points: Vec<BoPoint>,
}
