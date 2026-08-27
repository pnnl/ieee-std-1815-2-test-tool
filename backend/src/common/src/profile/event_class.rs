use serde::{Deserialize, Serialize};

/// DNP3 event class assignment for a point (Class1, Class2, Class3, or None).
/// Event class represents priority: Class1 > Class2 > Class3.
/// None means you are not expected to generate events for those points
#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub enum EventClass {
    Class1,
    Class2,
    Class3,
    #[default]
    None,
}
