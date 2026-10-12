use crate::profile::{AiPoint, validation::ValidationErrors, values::TransmissionI32};

/// A single AI point: its DNP3 index paired with its raw transmitted integer value.
///
/// These are the values as transmitted over DNP3 (Group 30 Var 1, 32-bit signed integer).
/// They must be scaled using the profile's multiplier and offset before being used
/// for any functional control logic.
///
/// Primarily used as the value type for entries in an
/// [`IndexedEntryDatabase`](crate::profile::indexed_db::IndexedEntryDatabase).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AiValue {
    /// The DNP3 AI point index.
    pub index: u16,
    /// The transmitted value for this point.
    pub value: TransmissionI32,
}

impl AiValue {
    pub fn new(index: u16, value: i32) -> Self {
        Self {
            index,
            value: TransmissionI32(value),
        }
    }
}

impl TryFrom<&AiPoint> for AiValue {
    /// Convert an `AiPoint` from the profile into a runtime `AiValue`.
    /// Convert the engineering value to a raw transmitted integer using the profile's
    /// multiplier and offset.
    fn try_from(point: &AiPoint) -> Result<Self, ValidationErrors> {
        Ok(Self {
            index: point.point_index,
            value: TransmissionI32::try_from_engineering(
                point.value(),
                point.multiplier(),
                point.offset,
            )?,
        })
    }

    type Error = ValidationErrors;
}
