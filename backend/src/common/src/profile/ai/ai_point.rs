use serde::{Deserialize, Serialize};

use crate::profile::{
    pics_profile::EventClass,
    validation::{ValidationError, ValidationErrors},
    values::{EngineeringF64, TransmissionI32},
};

/// An analog input (AI) point defined in the PICS profile. Do not construct it directly; instead, use AiPoint::new.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AiPoint {
    pub point_index: u16,
    pub name: String,
    pub event_class: EventClass,
    minimum: TransmissionI32,
    pub maximum: TransmissionI32,
    multiplier: f64,
    pub offset: EngineeringF64,
    pub units: String,
    pub iec_61850_uid: String,
    /// The default value of the point, in engineering units.
    value: EngineeringF64,
    pub assoc_ao: Option<String>,
    pub purpose: String,
    pub mandatory_1815: bool,
    pub mandatory_1547: bool,
}

impl AiPoint {
    pub fn eng_maximum(&self) -> EngineeringF64 {
        EngineeringF64::from_transmitted(self.maximum, self.multiplier, self.offset)
    }
    pub fn eng_minimum(&self) -> EngineeringF64 {
        EngineeringF64::from_transmitted(self.minimum, self.multiplier, self.offset)
    }
    pub fn value(&self) -> EngineeringF64 {
        self.value
    }

    pub fn minimum(&self) -> TransmissionI32 {
        self.minimum
    }

    pub fn maximum(&self) -> TransmissionI32 {
        self.maximum
    }

    pub fn full_index(&self) -> String {
        format!("AI{}", self.point_index)
    }

    pub fn set_value(&mut self, new_value: EngineeringF64) -> Result<(), ValidationErrors> {
        let old_value = self.value;
        self.value = new_value;
        match self.validate() {
            Ok(_) => Ok(()),
            Err(e) => {
                self.value = old_value; // rollback
                Err(e)
            }
        }
    }

    pub fn set_eng_bounds(
        &mut self,
        new_min: EngineeringF64,
        new_max: EngineeringF64,
    ) -> Result<(), ValidationErrors> {
        let old_min = self.minimum;
        let old_max = self.maximum;
        let new_transmission_min =
            TransmissionI32::try_from_engineering(new_min, self.multiplier, self.offset)?;
        let new_transmission_max =
            TransmissionI32::try_from_engineering(new_max, self.multiplier, self.offset)?;
        self.minimum = new_transmission_min;
        self.maximum = new_transmission_max;
        match self.validate() {
            Ok(_) => Ok(()),
            Err(e) => {
                self.minimum = old_min; // rollback
                self.maximum = old_max; // rollback
                Err(e)
            }
        }
    }

    /// Set the value without validation. Use with caution.
    pub fn set_value_unsafe(&mut self, new_value: EngineeringF64) {
        self.value = new_value;
    }

    /// Construct an [`AiPoint`], validating that `multiplier` is non-zero.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        point_index: u16,
        name: String,
        event_class: EventClass,
        minimum: TransmissionI32,
        maximum: TransmissionI32,
        multiplier: f64,
        offset: EngineeringF64,
        units: String,
        iec_61850_uid: String,
        value: EngineeringF64,
        assoc_ao: Option<String>,
        purpose: String,
        mandatory_1815: bool,
        mandatory_1547: bool,
    ) -> Result<Self, ValidationErrors> {
        let new_point = Self {
            point_index,
            name,
            event_class,
            minimum,
            maximum,
            multiplier,
            offset,
            units,
            iec_61850_uid,
            value,
            assoc_ao,
            purpose,
            mandatory_1815,
            mandatory_1547,
        };

        match new_point.validate() {
            Ok(_) => Ok(new_point),
            Err(e) => Err(e),
        }
    }

    pub fn multiplier(&self) -> f64 {
        self.multiplier
    }

    pub fn validate(&self) -> Result<(), ValidationErrors> {
        match self.collect_errors() {
            errors if errors.is_empty() => Ok(()),
            errors => Err(errors),
        }
    }

    pub(crate) fn collect_errors(&self) -> ValidationErrors {
        let mut errors = ValidationErrors::new();
        if self.minimum > self.maximum {
            errors.push(ValidationError {
                point: format!("AI{}", self.point_index),
                message: "Minimum value cannot be greater than maximum value".to_string(),
            });
        }

        if self.multiplier == 0.0 {
            errors.push(ValidationError {
                point: format!("AI{}", self.point_index),
                message: "Multiplier cannot be zero".to_string(),
            });
        }

        if self.value < self.eng_minimum() || self.value > self.eng_maximum() {
            errors.push(ValidationError {
                point: format!("AI{}", self.point_index),
                message: format!(
                    "Value must be between {} and {}, but is {}",
                    self.eng_minimum(),
                    self.eng_maximum(),
                    self.value
                ),
            });
        }
        errors
    }
}
