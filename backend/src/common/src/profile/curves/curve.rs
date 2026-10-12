use tracing::error;

use crate::profile::{
    AiCurve, AiPoint, CurveType,
    scale_curve::{DependentVariableUnit, IndependentVariableUnit, ScalingEntry},
    validation::{
        ValidationError, ValidationErrors, collect_duplicate_ai_errors, is_whole_number_in_range,
    },
};

/// Convert a finite, whole-number f64 to a u8 curve code.
/// Returns None if the value is NaN, infinite, fractional, negative, or > 255.
pub fn float_to_curve_code(v: f64) -> Option<u8> {
    if v.is_finite() && v.fract() == 0.0 && (0.0..=255.0).contains(&v) {
        Some(v as u8)
    } else {
        None
    }
}

/// Check that an AiPoint's minimum, maximum, and multiplier match the expected
/// scaling entry from the curve scaling table (Tables 16 or 17).
fn check_ai_scaling_match(point: &AiPoint, scaling: &ScalingEntry) -> ValidationErrors {
    let mut errors = ValidationErrors::new();

    let expected_min = scaling.min;
    let expected_max = scaling.max;
    let point_index_str = point.point_index.to_string();
    if point.minimum() != expected_min {
        errors.push(ValidationError {
            point: format!("AI{}", point_index_str),
            message: format!(
                "Expected minimum {} per curve scaling table, got {:?}",
                expected_min,
                point.minimum()
            ),
        })
    }
    if point.maximum != expected_max {
        errors.push(ValidationError {
            point: format!("AI{}", point_index_str),
            message: format!(
                "Expected maximum {} per curve scaling table, got {:?}",
                expected_max, point.maximum
            ),
        });
    }
    if (point.multiplier() - scaling.mult).abs() > 1e-9 {
        errors.push(ValidationError {
            point: format!("AI{}", point_index_str),
            message: format!(
                "Expected multiplier {} per curve scaling table, got {}",
                scaling.mult,
                point.multiplier()
            ),
        });
    }
    errors
}

impl AiCurve {
    fn display_name(&self) -> String {
        format!(
            "<Curve of type '{}' ({} points, x_units: {}, y_units: {})>",
            self.curve_type_enum(),
            self.number_of_points.value(),
            self.x_units_enum(),
            self.y_units_enum()
        )
    }

    fn curve_type_enum(&self) -> CurveType {
        CurveType::try_from(self.curve_type.value()).unwrap_or_else(|_| {
            error!("Invalid curve type {}", self.curve_type.value().0,);
            CurveType::Unknown // Should be impossible due to validation
        })
    }

    fn x_units_enum(&self) -> IndependentVariableUnit {
        IndependentVariableUnit::try_from(self.x_units.value()).unwrap_or_else(|_| {
            error!("Invalid x unit '{}'", self.x_units.value());
            IndependentVariableUnit::NotDefined
        })
    }

    fn y_units_enum(&self) -> DependentVariableUnit {
        DependentVariableUnit::try_from(self.y_units.value()).unwrap_or_else(|_| {
            error!("Invalid y unit '{}'", self.y_units.value());
            DependentVariableUnit::NotDefined
        })
    }

    pub fn collect_errors(&self) -> ValidationErrors {
        let mut errors = ValidationErrors::new();

        errors.extend(collect_duplicate_ai_errors(self.iter_points()));

        let _curve_type_enum = match CurveType::try_from(self.curve_type.value()) {
            Ok(curve_type) => curve_type,
            Err(_) => {
                errors.push(ValidationError {
                    point: format!(
                        "{}: curve type ({})",
                        self.display_name(),
                        self.curve_type.full_index()
                    ),
                    message: format!(
                        "Invalid curve type {}, expected a whole number between 0 and {}",
                        self.curve_type.value().0,
                        CurveType::max_value()
                    ),
                });
                CurveType::Unknown // Allow validation to continue
            }
        };

        if !is_whole_number_in_range(self.number_of_points.value(), 0, 100) {
            errors.push(ValidationError {
                point: format!(
                    "{}: number of points ({})",
                    self.display_name(),
                    self.number_of_points.full_index()
                ),
                message: format!(
                    "Number of points must be a whole number between 0 and 100, got {}",
                    self.number_of_points.value().0
                ),
            });
        }

        // Check curve type and unit compatibility
        let curve_type = self.curve_type_enum();
        let x_unit = self.x_units_enum();
        let y_unit = self.y_units_enum();

        if !curve_type.is_compatible_with_x_unit(x_unit) {
            errors.push(ValidationError {
                point: format!(
                    "{}: x_units ({})",
                    self.display_name(),
                    self.x_units.full_index()
                ),
                message: format!(
                    "Curve type {} is not compatible with x_units {:?}. Compatible units: {:?}",
                    curve_type,
                    x_unit,
                    curve_type.compatible_x_units()
                ),
            });
        }

        if !curve_type.is_compatible_with_y_unit(y_unit) {
            errors.push(ValidationError {
                point: format!(
                    "{}: y_units ({})",
                    self.display_name(),
                    self.y_units.full_index()
                ),
                message: format!(
                    "Curve type {} is not compatible with y_units {:?}. Compatible units: {:?}",
                    curve_type,
                    y_unit,
                    curve_type.compatible_y_units()
                ),
            });
        }

        errors.extend(self.collect_errors_ai_curve_scaling());

        // Check consistency within each axis independently.
        for (axis, values) in [("x", &self.x_values), ("y", &self.y_values)] {
            let Some((first, rest)) = values.split_first() else {
                continue;
            };
            let point = format!("{}: {axis}_values", self.display_name());

            for current in rest {
                let error_count = errors.len();

                // Accept either a field (offset) or a getter (minimum()).
                macro_rules! check_consistency {
                    ($field_or_method:ident $($call_parens:tt)*) => {{
                        let a = &first.$field_or_method $($call_parens)*;
                        let b = &current.$field_or_method $($call_parens)*;
                        if a != b {
                            errors.push(ValidationError {
                                point: point.clone(),
                                message: format!(
                                    "Inconsistency found in field '{}' among {axis}_values. First {axis} value:\n{:?},\nCurrent {axis} value:\n{:?}",
                                    stringify!($field_or_method),
                                    a,
                                    b,
                                ),
                            });
                        }
                    }};
                }

                check_consistency!(minimum());
                check_consistency!(maximum());
                check_consistency!(multiplier());
                check_consistency!(offset);
                check_consistency!(units);
                check_consistency!(event_class);
                check_consistency!(purpose);

                if errors.len() > error_count {
                    break;
                }
            }
        }

        errors
    }

    /// Validate that x_values and y_values of an AI curve have min/max/multiplier
    /// consistent with the scaling tables in scale_curve.rs.
    /// Uses x_units.value and y_units.value to determine the active variable types.
    fn collect_errors_ai_curve_scaling(&self) -> ValidationErrors {
        let mut errors = ValidationErrors::new();

        let x_var: Option<IndependentVariableUnit> = float_to_curve_code(self.x_units.value().0)
            .and_then(IndependentVariableUnit::from_code);

        let y_var: Option<DependentVariableUnit> =
            float_to_curve_code(self.y_units.value().0).and_then(DependentVariableUnit::from_code);

        if let Some(var) = x_var {
            if let Some(scaling) = var.scaling() {
                for point in self.x_values.iter() {
                    errors.extend(check_ai_scaling_match(point, &scaling));
                }
            }
        }

        if let Some(var) = y_var {
            if let Some(scaling) = var.scaling() {
                for point in &self.y_values {
                    errors.extend(check_ai_scaling_match(point, &scaling));
                }
            }
        }
        errors
    }
}
