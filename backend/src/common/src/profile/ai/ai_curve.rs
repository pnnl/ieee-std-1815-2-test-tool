use serde::{Deserialize, Serialize};

use crate::profile::{
    AiPoint, PicsProfile,
    curves::scale_curve::{DependentVariableUnit, IndependentVariableUnit, ScalingEntry},
    validation::{ValidationError, ValidationErrors},
    values::{EngineeringF64, TransmissionI32},
};

/// AI points belonging to a Curve functional group.
/// The header fields capture the single per-curve metadata points.
/// The Vec fields hold one entry per curve point, parallel-indexed.
/// Note: the curve edit selector is stored in the base AiPoints array, not here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AiCurve {
    ///  Curve Type.  Enumeration:
    ///  <0> Curve is not defined
    ///  <1> Not applicable / Unknown
    ///  <2> Volt-Var
    ///  <3> Frequency-Watt
    ///  <4> Watt-Var
    ///  <5> Voltage-Watt
    ///  <6> Remain Connected
    ///  <7> Temperature mode
    ///  <8> Pricing signal mode
    /// High Voltage ride-through curves
    ///  <9> HVRT Must Trip
    ///  <10> HVRT Momentary Cessation
    /// Low Voltage ride-through curves
    ///  <11> LVRT Must Trip
    ///  <12> LVRT Momentary Cessation
    /// High Frequency ride-through curves
    ///  <13> HFRT Must Trip
    ///  <14> HFRT Momentary Cessation
    /// Low Frequency ride-through curves
    ///  <15> LFRT Must Trip
    ///  <16> LFRT Momentary Cessation"
    pub curve_type: AiPoint,
    pub number_of_points: AiPoint,
    pub x_units: AiPoint,
    pub y_units: AiPoint,
    pub x_values: Vec<AiPoint>,
    pub y_values: Vec<AiPoint>,
}

impl AiCurve {
    /// Resize the two axes atomically, validating only the count and new points.
    /// Retained points and unrelated in-progress edits are not revalidated.
    pub fn resize_points(&mut self, count: i64) -> Result<(), ValidationErrors> {
        if !(0..=100).contains(&count) {
            return Err(self.resize_error("Number of curve points must be between 0 and 100"));
        }
        let mut candidate = self.clone();
        candidate
            .number_of_points
            .set_value(EngineeringF64(count as f64))?;
        let count = count as usize;
        candidate.x_values.truncate(count);
        candidate.y_values.truncate(count);
        for is_x in [true, false] {
            let points = if is_x { &self.x_values } else { &self.y_values };
            for pair_index in points.len()..count {
                let point = self.create_curve_point(is_x, pair_index, points.last())?;
                // New identities must not alias headers, retained points, or other additions.
                if candidate.iter_points().iter().any(|existing| {
                    existing.point_index == point.point_index
                        || existing.name == point.name
                        || existing.iec_61850_uid == point.iec_61850_uid
                        || (point.assoc_ao.is_some() && existing.assoc_ao == point.assoc_ao)
                }) {
                    return Err(self
                        .resize_error("New curve point identity collides with an existing point"));
                }
                if is_x {
                    candidate.x_values.push(point);
                } else {
                    candidate.y_values.push(point);
                }
            }
        }
        let mut indexes = std::collections::HashSet::new();
        if candidate
            .iter_points()
            .iter()
            .any(|point| !indexes.insert(point.point_index))
        {
            return Err(self.resize_error("Curve point indexes must be distinct"));
        }
        if candidate.x_values.len() != count || candidate.y_values.len() != count {
            return Err(self.resize_error("Curve axes must match the requested point count"));
        }
        *self = candidate;
        Ok(())
    }

    fn resize_error(&self, message: &str) -> ValidationErrors {
        ValidationErrors::from_error(ValidationError {
            point: self.number_of_points.full_index(),
            message: message.to_string(),
        })
    }

    fn create_curve_point(
        &self,
        is_x: bool,
        pair_index: usize,
        template: Option<&AiPoint>,
    ) -> Result<AiPoint, ValidationErrors> {
        let delta = u16::try_from(pair_index)
            .ok()
            .and_then(|index| index.checked_mul(2))
            .and_then(|index| index.checked_add(if is_x { 1 } else { 2 }))
            .ok_or_else(|| self.resize_error("Curve point index arithmetic overflow"))?;
        let point_index = self
            .y_units
            .point_index
            .checked_add(delta)
            .ok_or_else(|| self.resize_error("Curve point index arithmetic overflow"))?;
        let ao_base = self
            .y_units
            .assoc_ao
            .as_deref()
            .and_then(|mapping| mapping.strip_prefix("AO"))
            .filter(|digits| {
                !digits.is_empty() && digits.bytes().all(|digit| digit.is_ascii_digit())
            })
            .map(|digits| {
                digits.parse::<u16>().map_err(|_| {
                    self.resize_error("Associated AO index is outside the supported range")
                })
            })
            .transpose()?;
        let assoc_ao = ao_base
            .map(|base| {
                base.checked_add(delta)
                    .map(|index| format!("AO{index}"))
                    .ok_or_else(|| self.resize_error("Associated AO index arithmetic overflow"))
            })
            .transpose()?;
        let number = pair_index + 1;
        let axis = if is_x { "X" } else { "Y" };
        let uid_axis = if is_x { "x" } else { "y" };
        let name = format!("Curve Point {number} {axis}-Value");
        let uid = format!("FMARn.PairArr.CrvPts.{number}.{uid_axis}Val");
        let mut point = if let Some(template) = template {
            let mut point = template.clone();
            point.point_index = point_index;
            point.name = name;
            point.iec_61850_uid = uid;
            point.assoc_ao = assoc_ao;
            point
        } else {
            let header = if is_x { &self.x_units } else { &self.y_units };
            let scaling = if is_x {
                IndependentVariableUnit::try_from(header.value())
                    .ok()
                    .and_then(|unit| unit.scaling())
            } else {
                DependentVariableUnit::try_from(header.value())
                    .ok()
                    .and_then(|unit| unit.scaling())
            }
            .unwrap_or(ScalingEntry {
                min: TransmissionI32(i32::MIN),
                max: TransmissionI32(i32::MAX),
                mult: 1.0,
            });
            AiPoint::new(
                point_index,
                name,
                header.event_class,
                scaling.min,
                scaling.max,
                scaling.mult,
                EngineeringF64(0.0),
                "Varies".to_string(),
                uid,
                EngineeringF64(0.0),
                assoc_ao,
                "Curve".to_string(),
                header.mandatory_1815,
                header.mandatory_1547,
            )?
        };
        let minimum = point.eng_minimum().0;
        let maximum = point.eng_maximum().0;
        if !point.multiplier().is_finite()
            || point.multiplier() == 0.0
            || !minimum.is_finite()
            || !maximum.is_finite()
            || minimum > maximum
        {
            return Err(
                self.resize_error("New curve point requires valid finite scaling and bounds")
            );
        }
        point.set_value(EngineeringF64(0.0_f64.clamp(minimum, maximum)))?;
        Ok(point)
    }

    pub fn iter_points(&self) -> Vec<&AiPoint> {
        let mut out = vec![
            &self.curve_type,
            &self.number_of_points,
            &self.x_units,
            &self.y_units,
        ];
        out.extend(self.x_values.iter().chain(self.y_values.iter()));
        out
    }

    pub fn iter_points_mut(&mut self) -> Vec<&mut AiPoint> {
        let AiCurve {
            curve_type,
            number_of_points,
            x_units,
            y_units,
            x_values,
            y_values,
        } = self;
        let mut out: Vec<&mut AiPoint> = vec![curve_type, number_of_points, x_units, y_units];
        out.extend(x_values.iter_mut());
        out.extend(y_values.iter_mut());
        out
    }
}

impl Default for AiCurve {
    fn default() -> Self {
        PicsProfile::full_profile()
            .ai
            .curves
            .first()
            .expect("full.json must contain a default curve")
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_empty_axes_and_regrow() {
        let mut curve = AiCurve::default();
        curve.resize_points(0).unwrap();
        curve.resize_points(3).unwrap();
        assert_eq!(curve.number_of_points.value(), EngineeringF64(3.0));
        assert_eq!(curve.x_values.len(), 3);
        assert_eq!(curve.y_values.len(), 3);
        for (k, (x, y)) in curve.x_values.iter().zip(&curve.y_values).enumerate() {
            assert_eq!(x.point_index, curve.y_units.point_index + 1 + 2 * k as u16);
            assert_eq!(y.point_index, x.point_index + 1);
            assert_eq!(x.value(), EngineeringF64(0.0));
            assert_eq!(y.value(), EngineeringF64(0.0));
            assert_eq!(x.name, format!("Curve Point {} X-Value", k + 1));
            assert_eq!(
                y.iec_61850_uid,
                format!("FMARn.PairArr.CrvPts.{}.yVal", k + 1)
            );
        }
        let expected = curve.clone();
        curve.resize_points(0).unwrap();
        curve.resize_points(3).unwrap();
        assert_eq!(curve, expected);
    }

    #[test]
    fn resize_preserves_retained_points_and_axis_metadata() {
        let mut curve = AiCurve::default();
        curve.resize_points(1).unwrap();
        curve.x_values[0].name = "Edited name".into();
        curve.x_values[0].units = "custom units".into();
        curve.x_values[0].offset = EngineeringF64(5.0);
        // An existing invalid value must be preserved, not revalidated.
        curve.x_values[0].set_value_unsafe(EngineeringF64(-999.0));
        let x = curve.x_values[0].clone();
        let y = curve.y_values[0].clone();
        curve.resize_points(4).unwrap();
        assert_eq!(curve.x_values[0], x);
        assert_eq!(curve.y_values[0], y);
        assert_eq!(curve.x_values[1].units, x.units);
        assert_eq!(curve.x_values[1].offset, x.offset);
        assert_eq!(curve.x_values[1].value(), EngineeringF64(5.0));
        assert_eq!(curve.x_values[1].event_class, x.event_class);
        let points = curve.iter_points();
        let indexes: std::collections::HashSet<_> =
            points.iter().map(|point| point.point_index).collect();
        let names: std::collections::HashSet<_> = points.iter().map(|point| &point.name).collect();
        let uids: std::collections::HashSet<_> =
            points.iter().map(|point| &point.iec_61850_uid).collect();
        let associations: std::collections::HashSet<_> = points
            .iter()
            .filter_map(|point| point.assoc_ao.as_ref())
            .collect();
        assert_eq!(indexes.len(), points.len());
        assert_eq!(names.len(), points.len());
        assert_eq!(uids.len(), points.len());
        assert_eq!(associations.len(), points.len());
        curve.resize_points(1).unwrap();
        assert_eq!(curve.x_values, vec![x]);
        assert_eq!(curve.y_values, vec![y]);
    }

    #[test]
    fn resize_one_empty_axis_uses_current_units_and_headers() {
        for is_x in [true, false] {
            let mut curve = AiCurve::default();
            curve.resize_points(1).unwrap();
            let retained = if is_x {
                curve.y_values[0].clone()
            } else {
                curve.x_values[0].clone()
            };
            if is_x {
                curve.x_values.clear();
                curve.x_units.set_value_unsafe(EngineeringF64(23.0));
            } else {
                curve.y_values.clear();
                curve.y_units.set_value_unsafe(EngineeringF64(7.0));
            }
            curve.resize_points(1).unwrap();
            let created = if is_x {
                &curve.x_values[0]
            } else {
                &curve.y_values[0]
            };
            assert_eq!(created.multiplier(), if is_x { 0.1 } else { 0.01 });
            assert_eq!(created.minimum().0, if is_x { -5000 } else { -100 });
            assert_eq!(created.units, "Varies");
            assert_eq!(created.purpose, "Curve");
            assert_eq!(
                if is_x {
                    &curve.y_values[0]
                } else {
                    &curve.x_values[0]
                },
                &retained
            );
        }
    }

    #[test]
    fn resize_custom_indexes_associations_and_neutral_scaling() {
        for mapping in [Some("AO700"), None, Some("unknown")] {
            for code in [0.0, 1.0, 255.0, 2.5] {
                let mut curve = AiCurve::default();
                curve.resize_points(0).unwrap();
                curve.y_units.point_index = 900;
                curve.y_units.assoc_ao = mapping.map(String::from);
                curve.x_units.set_value_unsafe(EngineeringF64(code));
                curve.y_units.set_value_unsafe(EngineeringF64(code));
                curve.x_units.event_class = crate::profile::EventClass::None;
                curve.x_units.mandatory_1815 = false;
                curve.resize_points(2).unwrap();
                assert_eq!(curve.x_values[1].point_index, 903);
                assert_eq!(curve.y_values[1].point_index, 904);
                assert_eq!(
                    curve.x_values[1].assoc_ao,
                    mapping
                        .filter(|value| *value == "AO700")
                        .map(|_| "AO703".into())
                );
                for point in curve.x_values.iter().chain(&curve.y_values) {
                    assert_eq!(point.minimum().0, i32::MIN);
                    assert_eq!(point.maximum().0, i32::MAX);
                    assert_eq!(point.multiplier(), 1.0);
                    assert_eq!(point.value(), EngineeringF64(0.0));
                }
                assert_eq!(curve.x_values[0].event_class, curve.x_units.event_class);
                assert_eq!(
                    curve.x_values[0].mandatory_1815,
                    curve.x_units.mandatory_1815
                );
            }
        }
    }

    #[test]
    fn resize_rejections_are_atomic() {
        for case in 0..11 {
            let mut curve = AiCurve::default();
            curve.resize_points(1).unwrap();
            let count = match case {
                0 => -1,
                1 => 101,
                2 => {
                    curve.number_of_points.maximum = TransmissionI32(1);
                    2
                }
                3 => {
                    curve.y_units.point_index = u16::MAX - 2;
                    2
                }
                4 => {
                    curve.y_units.assoc_ao = Some("AO65533".into());
                    2
                }
                5 => {
                    curve.x_values[0].point_index = curve.y_units.point_index + 3;
                    2
                }
                6 => {
                    curve.x_values[0].name = "Curve Point 2 X-Value".into();
                    2
                }
                7 => {
                    curve.x_values[0].iec_61850_uid = "FMARn.PairArr.CrvPts.2.xVal".into();
                    2
                }
                8 => {
                    curve.x_values[0].assoc_ao = curve
                        .y_units
                        .assoc_ao
                        .as_deref()
                        .and_then(|value| value.strip_prefix("AO"))
                        .and_then(|value| value.parse::<u16>().ok())
                        .map(|value| format!("AO{}", value + 3));
                    2
                }
                9 => {
                    curve.x_values[0].maximum = TransmissionI32(-1);
                    2
                }
                _ => {
                    curve.y_units.assoc_ao = Some("AO65536".into());
                    2
                }
            };
            let original = curve.clone();
            assert!(curve.resize_points(count).is_err(), "case {case}");
            assert_eq!(curve, original, "case {case}");
        }
    }

    #[test]
    fn resize_accepts_maximum_count() {
        let mut curve = AiCurve::default();
        curve.resize_points(100).unwrap();
        assert_eq!(curve.x_values.len(), 100);
        assert_eq!(curve.y_values.len(), 100);
    }
}
