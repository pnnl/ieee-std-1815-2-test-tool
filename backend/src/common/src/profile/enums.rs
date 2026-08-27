/// To implement a more convenient enum from an AiPoint:
/// 1. Define the enum with #[repr(i64)] and derive strum::FromRepr.
/// 2. Implement AiEnum for the enum, using strum::FromRepr to implement from_int().
/// 3. Add a getter to the parent struct that calls AiEnum::from_value() on the AiPoint value
/// 4. Add validation in the parent struct's collect_errors() method using AiEnum::from_value_errors().
use crate::profile::{validation::ValidationErrors, values::EngineeringF64};

/// To assist with validation and parsing of enum values. Enums should only define
/// from_int().
pub trait AiEnum: Sized + Copy {
    fn from_value_errors(value: EngineeringF64) -> ValidationErrors {
        match Self::try_from_value(value) {
            Some(_v) => ValidationErrors::new(),
            None => ValidationErrors::from_error(crate::profile::validation::ValidationError {
                point: "AiEnum".to_string(),
                message: format!("Invalid value for enum: {}", value.0),
            }),
        }
    }

    fn try_from_value(value: EngineeringF64) -> Option<Self> {
        if value.0 == value.0 as i64 as f64 {
            Self::from_int(value.0 as i64)
        } else {
            None
        }
    }

    fn from_value(value: EngineeringF64) -> Self {
        Self::try_from_value(value).expect("Invalid value for enum")
    }

    /// Convert an integer representation to the enum variant; can be
    /// derived with strum_macros::FromRepr.
    fn from_int(value: i64) -> Option<Self>;
}
