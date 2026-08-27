use std::collections::{HashMap, HashSet};

use crate::profile::{
    AiPoint, AoPoint, BiPoint, BoPoint, pics_profile::PicsProfile, validation::Validated,
};
// ---------------------------------------------------------------------------
// ProfileIndex — precomputed O(1) lookup maps derived from PicsProfile
// ---------------------------------------------------------------------------

/// Precomputed index derived from a `PicsProfile` for fast O(1) lookups.
///
/// Build once at startup with [`ProfileIndex::from_profile`] and share via `Arc`.
/// Eliminates per-request `Vec` allocations and linear scans in control handlers.
///
/// Also fixes a correctness issue: the curve/schedule selector AI points
/// (AI 328, AI 2000, AI 3000) live in sub-groups excluded by `base_ai_points()`.
/// The association maps here include those points, so AO 244 → AI 328 etc.
/// resolve correctly.
pub struct ProfileIndex {
    /// AO index → associated AI index (from `assoc_ai` field on each AO point).
    /// Includes curve/schedule selector associations.
    pub ao_to_ai: HashMap<u16, u16>,
    /// AI index → associated AO index (reverse of `ao_to_ai`).
    pub ai_to_ao: HashMap<u16, u16>,
    /// BO index → associated BI index (from `assoc_bi` field on each BO point).
    pub bo_to_bi: HashMap<u16, u16>,
    /// BI index → associated BO index (reverse of `bo_to_bi`).
    pub bi_to_bo: HashMap<u16, u16>,
    /// All AI points (base + curves + schedules + equipment) keyed by point index.
    pub ai_points: HashMap<u16, AiPoint>,
    /// All BI points keyed by point index.
    pub bi_points: HashMap<u16, BiPoint>,
    /// All AO points keyed by point index.
    pub ao_points: HashMap<u16, AoPoint>,
    /// All BO points keyed by point index.
    pub bo_points: HashMap<u16, BoPoint>,
    /// AI indices belonging to base/equipment sub-groups only (excludes curves and schedules).
    ///
    /// Used to distinguish plain AO→AI pairs (which the control loop syncs) from
    /// multiplexed curve/schedule AO→AI pairs (which the curve/schedule writers manage).
    pub base_ai_indices: HashSet<u16>,
}

impl ProfileIndex {
    /// Build a `ProfileIndex` from a `PicsProfile`.
    ///
    /// Iterates all point sub-groups once; all subsequent lookups are O(1).
    pub fn from_profile(profile: &Validated<PicsProfile>) -> Self {
        // --- Collect all AI points (including curves + schedules) ---
        let mut ai_points: HashMap<u16, AiPoint> = HashMap::new();
        for pt in profile.ai.all_ai_points_full() {
            ai_points.insert(pt.point_index, pt.clone());
        }

        // --- Collect all BI points ---
        let mut bi_points: HashMap<u16, BiPoint> = HashMap::new();
        for pt in profile.bi.all_bi_points() {
            bi_points.insert(pt.point_index, pt.clone());
        }

        // --- Collect all AO and BO points ---
        let mut ao_points: HashMap<u16, AoPoint> = HashMap::new();
        for pt in profile.ao.all_ao_points() {
            ao_points.insert(pt.point_index, pt.clone());
        }
        let mut bo_points: HashMap<u16, BoPoint> = HashMap::new();
        for pt in &profile.bo.points {
            bo_points.insert(pt.point_index, pt.clone());
        }

        // --- Build bidirectional association maps ---
        // assoc_ai and assoc_bi fields use "AI{n}" / "BI{n}" index strings (e.g. "AI277"),
        // not descriptive names, so parse the numeric index directly.
        let mut ao_to_ai: HashMap<u16, u16> = HashMap::new();
        let mut ai_to_ao: HashMap<u16, u16> = HashMap::new();
        for pt in profile.ao.all_ao_points() {
            if let Some(assoc) = &pt.assoc_ai {
                if let Some(idx_str) = assoc.strip_prefix("AI") {
                    if let Ok(ai_idx) = idx_str.parse::<u16>() {
                        ao_to_ai.insert(pt.point_index, ai_idx);
                        ai_to_ao.insert(ai_idx, pt.point_index);
                    }
                }
            }
        }

        let mut bo_to_bi: HashMap<u16, u16> = HashMap::new();
        let mut bi_to_bo: HashMap<u16, u16> = HashMap::new();
        for pt in &profile.bo.points {
            if let Some(assoc) = &pt.assoc_bi {
                if let Some(idx_str) = assoc.strip_prefix("BI") {
                    if let Ok(bi_idx) = idx_str.parse::<u16>() {
                        bo_to_bi.insert(pt.point_index, bi_idx);
                        bi_to_bo.insert(bi_idx, pt.point_index);
                    }
                }
            }
        }

        let base_ai_indices: HashSet<u16> = profile
            .ai
            .base_ai_points()
            .into_iter()
            .map(|pt| pt.point_index)
            .collect();

        Self {
            ao_to_ai,
            ai_to_ao,
            bo_to_bi,
            bi_to_bo,
            ai_points,
            bi_points,
            ao_points,
            bo_points,
            base_ai_indices,
        }
    }
}
