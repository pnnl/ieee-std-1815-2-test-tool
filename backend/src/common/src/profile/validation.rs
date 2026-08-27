use std::{error::Error, fmt::Debug, fmt::Display, ops::Deref};

use crate::profile::profile::AiPoint;
use crate::profile::values::EngineeringF64;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, PartialEq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ValidationError {
    pub point: String, // E.g. "AO253"
    pub message: String,
}

impl Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.point, self.message)
    }
}

impl Debug for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.point, self.message)
    }
}

#[derive(Clone, Serialize, PartialEq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ValidationErrors {
    pub errors: Vec<ValidationError>,
}

impl ValidationErrors {
    pub fn new() -> Self {
        ValidationErrors { errors: Vec::new() }
    }

    pub fn from_error(error: ValidationError) -> Self {
        ValidationErrors {
            errors: vec![error],
        }
    }

    pub fn push(&mut self, error: ValidationError) {
        self.errors.push(error);
    }

    pub fn extend(&mut self, other: ValidationErrors) {
        self.errors.extend(other.errors);
    }

    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn len(&self) -> usize {
        self.errors.len()
    }
}

impl Default for ValidationErrors {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for ValidationErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Print each on a separate line
        writeln!(f, "ValidationErrors:")?;
        let mut sorted_errors = self.errors.clone();
        sorted_errors.sort_by(|a, b| a.message.cmp(&b.message));
        for error in &sorted_errors {
            writeln!(f, "  {}", error)?;
        }
        Ok(())
    }
}

impl Debug for ValidationErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "ValidationErrors:")?;
        let mut sorted_errors = self.errors.clone();
        sorted_errors.sort_by(|a, b| a.message.cmp(&b.message));
        for error in &sorted_errors {
            writeln!(f, "  {:?}", error)?;
        }
        Ok(())
    }
}

impl Error for ValidationErrors {}

impl From<anyhow::Error> for ValidationErrors {
    fn from(err: anyhow::Error) -> Self {
        ValidationErrors::from_error(ValidationError {
            point: "N/A".to_string(),
            message: err.to_string(),
        })
    }
}

impl From<ValidationErrors> for String {
    fn from(errors: ValidationErrors) -> Self {
        errors.to_string()
    }
}

impl IntoIterator for &ValidationErrors {
    type Item = ValidationError;
    type IntoIter = std::vec::IntoIter<ValidationError>;

    fn into_iter(self) -> Self::IntoIter {
        self.errors.clone().into_iter()
    }
}

pub trait Validate {
    fn validate(&self) -> Result<(), ValidationErrors> {
        match self.collect_validation_errors() {
            errors if errors.is_empty() => Ok(()),
            errors => Err(errors),
        }
    }

    fn collect_validation_errors(&self) -> ValidationErrors;
}

/// Wrapper to indicated a value has been validated. Do not implement Deserialize
/// for this type since it would bypass validation.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(transparent)]
pub struct Validated<T>(T);

/// Implemented for ease of use.
///
/// DO NOT implement `DerefMut` for `Validated<T>`. This is intentional to prevent
/// accidental mutation of the inner value, which could invalidate the validation state.
impl<T> Deref for Validated<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> Validated<T> {
    /// Consumes the `Validated` wrapper and returns the inner value.
    /// Use to represent a profile with an unknown validation state.
    pub fn into_unvalidated(self) -> T {
        self.0
    }
}

impl<T: Validate> Validated<T> {
    pub fn try_new(value: T) -> Result<Self, ValidationErrors> {
        match value.collect_validation_errors() {
            errors if errors.is_empty() => Ok(Validated(value)),
            errors => Err(errors),
        }
    }
}

// Load errors ////////////////////////////////////////////////////////////////

/// A single validation error encountered while loading a PICS profile from an
/// xlsx file. The point's numeric field was missing, blank, or non-numeric.
/// The profile still loads with 0 as a placeholder so the frontend can display
/// the profile and highlight the fields that need to be corrected.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoadError {
    pub sheet: String,
    pub point_index: String,
    pub field: String,
    /// 1-indexed row in the spreadsheet, if known. None for post-assembly validation errors.
    pub row: Option<u32>,
    pub message: String,
}

impl LoadError {
    /// Create a new LoadError with a known spreadsheet row (1-indexed).
    pub fn at_row(sheet: &str, point_index: &str, field: &str, row: u32, message: String) -> Self {
        Self {
            sheet: sheet.to_string(),
            point_index: point_index.to_string(),
            field: field.to_string(),
            row: Some(row),
            message,
        }
    }

    /// Create a new LoadError for post-assembly validation (no specific row).
    pub fn post_assembly(sheet: &str, point_index: &str, field: &str, message: String) -> Self {
        Self {
            sheet: sheet.to_string(),
            point_index: point_index.to_string(),
            field: field.to_string(),
            row: None,
            message,
        }
    }
}

impl From<ValidationError> for LoadError {
    fn from(err: ValidationError) -> Self {
        Self {
            sheet: "Unknown".to_string(),
            point_index: err.point,
            field: "Unknown".to_string(),
            row: None,
            message: err.message,
        }
    }
}

impl From<Vec<LoadError>> for ValidationErrors {
    fn from(errors: Vec<LoadError>) -> Self {
        let validation_errors = errors
            .into_iter()
            .map(|e| ValidationError {
                point: e.point_index,
                message: e.message,
            })
            .collect();
        ValidationErrors {
            errors: validation_errors,
        }
    }
}

/// Check if a value is a whole number within a specified range (inclusive). Helpful
/// for validating enums.
pub fn is_whole_number_in_range(value: EngineeringF64, min: i64, max: i64) -> bool {
    let int_value = value.0 as i64;
    (int_value as f64) == value.0 && int_value >= min && int_value <= max
}

pub fn collect_low_high_threshold_errors(
    low_threshold: &AiPoint,
    high_threshold: &AiPoint,
) -> ValidationErrors {
    let mut errors = Vec::new();

    if low_threshold.value() > high_threshold.value() {
        errors.push(ValidationError {
            point: format!(
                "{} and {}",
                low_threshold.full_index(),
                high_threshold.full_index()
            ),
            message: format!(
                "Low threshold ({}) is greater than high threshold ({})",
                low_threshold.value(),
                high_threshold.value()
            ),
        });
    }

    ValidationErrors { errors }
}
