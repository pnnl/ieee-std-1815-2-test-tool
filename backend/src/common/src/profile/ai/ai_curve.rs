use serde::{Deserialize, Serialize};

use crate::profile::AiPoint;

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
