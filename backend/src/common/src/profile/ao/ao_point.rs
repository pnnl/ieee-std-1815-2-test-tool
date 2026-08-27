use serde::{Deserialize, Serialize};

use crate::profile::{
    validation::{ValidationError, ValidationErrors},
    values::{EngineeringF64, TransmissionI32},
};

/// An analog output (AO) point defined in the PICS profile. Do not construct it directly; instead, use AoPoint::new.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AoPoint {
    pub point_index: u16,
    pub name: String,
    minimum: TransmissionI32,
    maximum: TransmissionI32,
    multiplier: f64,
    pub offset: EngineeringF64,
    pub units: String,
    pub iec_61850_uid: String,
    pub assoc_ai: Option<String>,
    pub purpose: String,
    pub mandatory_1815: bool,
    pub mandatory_1547: bool,
}

impl AoPoint {
    /// Construct an [`AoPoint`], validating that `multiplier` is non-zero.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        point_index: u16,
        name: String,
        minimum: TransmissionI32,
        maximum: TransmissionI32,
        multiplier: f64,
        offset: EngineeringF64,
        units: String,
        iec_61850_uid: String,
        assoc_ai: Option<String>,
        purpose: String,
        mandatory_1815: bool,
        mandatory_1547: bool,
    ) -> Result<Self, ValidationErrors> {
        let new_point = Self {
            point_index,
            name,
            minimum,
            maximum,
            multiplier,
            offset,
            units,
            iec_61850_uid,
            assoc_ai,
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

    pub fn eng_maximum(&self) -> EngineeringF64 {
        EngineeringF64::from_transmitted(self.maximum, self.multiplier, self.offset)
    }
    pub fn eng_minimum(&self) -> EngineeringF64 {
        EngineeringF64::from_transmitted(self.minimum, self.multiplier, self.offset)
    }

    pub fn minimum(&self) -> TransmissionI32 {
        self.minimum
    }

    pub fn maximum(&self) -> TransmissionI32 {
        self.maximum
    }

    pub fn set_transmission_bounds(
        &mut self,
        new_min: TransmissionI32,
        new_max: TransmissionI32,
    ) -> Result<(), ValidationErrors> {
        let old_min = self.minimum;
        let old_max = self.maximum;
        self.minimum = new_min;
        self.maximum = new_max;
        match self.validate() {
            Ok(_) => Ok(()),
            Err(e) => {
                self.minimum = old_min;
                self.maximum = old_max;
                Err(e)
            }
        }
    }

    pub fn validate(&self) -> Result<(), ValidationErrors> {
        let errors = self.collect_errors();

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn collect_errors(&self) -> ValidationErrors {
        let mut errors = ValidationErrors::new();

        if self.minimum > self.maximum {
            errors.push(ValidationError {
                point: format!("AO{}", self.point_index),
                message: "Minimum value cannot be greater than maximum value".to_string(),
            });
        }

        if self.multiplier == 0.0 {
            errors.push(ValidationError {
                point: format!("AO{}", self.point_index),
                message: "Multiplier cannot be zero".to_string(),
            });
        }

        errors
    }
}
