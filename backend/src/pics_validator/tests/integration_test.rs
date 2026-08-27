use common::profile::{PicsProfile, validation::Validated};
use pics_validator::{load_json_profile, load_xlsx_profile};
use std::path::Path;

#[test]
fn test_load_full_xlsx() {
    let profile = PicsProfile::load_full_profile();

    // BO: all points in points (66 total)
    assert_eq!(profile.bo.points.len(), 66, "expected 66 BO points");
    assert_eq!(profile.bo.points[0].point_index, 0u16);

    // BI: base points — includes standard DNP3 range (< 5000) + experimental range (>= 30000)
    assert_eq!(profile.bi.points.len(), 134, "expected 134 base BI points");

    // BI: meters
    assert_eq!(
        profile.bi.meters[0].active_power_too_high.point_index,
        5000u16
    );
    assert_eq!(
        profile.bi.meters[0].communication_error.point_index,
        5012u16
    );

    assert_eq!(
        profile.bi.ders[0].maintenance_operational_state.point_index,
        10000u16
    );

    // BI: inverters — 2 configured inverters (Inverter #1 and Inverter #3), 35 points each
    assert_eq!(
        profile.bi.inverters.len(),
        2,
        "expected 2 BI inverter instances"
    );
    assert_eq!(
        profile.bi.inverters[0].active_power_too_high.point_index,
        15000u16
    );

    // BI: batteries — 2 standard instances (experimental sub-ranges are skipped)
    assert_eq!(
        profile.bi.batteries.len(),
        2,
        "expected 2 BI battery instances"
    );
    assert_eq!(
        profile.bi.batteries[0].status_of_storage.point_index,
        20000u16
    );
    assert_eq!(
        profile.bi.batteries[1].status_of_storage.point_index,
        20108u16
    );

    // AO: base points — standard DNP3 range (< 5000) + experimental range (>= 30000)
    assert_eq!(
        profile.ao.points.len(),
        1153,
        "expected 1153 AO base points"
    );

    // AO: meters
    assert_eq!(
        profile.ao.meters[0].active_power_high_threshold.point_index,
        5000u16
    );

    // AO: inverters — 2 configured inverters, 12 points each
    assert_eq!(
        profile.ao.inverters.len(),
        2,
        "expected 2 AO inverter instances"
    );
    assert_eq!(
        profile.ao.inverters[0]
            .active_power_high_threshold
            .point_index,
        15000u16
    );

    // AO: batteries — 2 standard instances (experimental sub-ranges are skipped)
    assert_eq!(
        profile.ao.batteries.len(),
        2,
        "expected 2 AO battery instances"
    );
    assert_eq!(
        profile.ao.batteries[0]
            .external_voltage_high_threshold
            .point_index,
        20000u16
    );
    assert_eq!(
        profile.ao.batteries[1]
            .external_voltage_high_threshold
            .point_index,
        20008u16
    );

    // AI: base points — standard DNP3 range ([0,328), [533,2000)) + experimental range (>= 30000)
    // + 3 edit selectors (curve at 328, schedule_bc at 2001, schedule at 3000)
    assert_eq!(
        profile.ai.points.len(),
        1105,
        "expected 1105 base AI points"
    );
    assert!(
        profile.ai.points.iter().any(|p| p.point_index == 328u16),
        "expected curve edit selector at point index 328 in base AI points"
    );
    assert!(
        profile.ai.points.iter().any(|p| p.point_index == 3000u16),
        "expected schedule edit selector at point index 3000 in base AI points"
    );

    // AI: curves
    assert_eq!(
        profile.ai.curves[0].x_values.len(),
        100,
        "expected 100 AI curve x-values"
    );
    assert_eq!(
        profile.ai.curves[0].y_values.len(),
        100,
        "expected 100 AI curve y-values"
    );

    // AI: meters
    assert_eq!(
        profile.ai.meters[0].type_of_connection_point.point_index,
        5000u16
    );
    assert_eq!(
        profile.ai.meters[0].phase_c_volts_low_threshold.point_index,
        5036u16
    );

    // CTR
    assert_eq!(profile.ctr[0].point_index, 0u16);

    // Key sheet basic checks
    assert!(profile.key.meter.bo.start > 0);
    assert!(profile.key.meter.bo.count > 0);
}

#[test]
fn test_json_round_trip() {
    let profile = PicsProfile::load_full_profile();

    let json = serde_json::to_string_pretty(&profile.clone()).expect("should serialize to JSON");

    let parsed_profile: PicsProfile =
        serde_json::from_str(&json).expect("should deserialize from JSON");
    let round_tripped: Validated<PicsProfile> = parsed_profile
        .into_validated()
        .expect("should validate after round-trip");

    assert_eq!(round_tripped.bo.points.len(), profile.bo.points.len());
    assert_eq!(round_tripped.bi.meters.len(), profile.bi.meters.len());
    assert_eq!(round_tripped.ao.points.len(), profile.ao.points.len());
    assert_eq!(round_tripped.ai.inverters.len(), profile.ai.inverters.len());
    assert_eq!(round_tripped.ctr.len(), profile.ctr.len());
}

#[test]
fn test_load_full_profile() {
    // First write the profile JSON, then reload it via load_json_profile
    let profile = PicsProfile::load_full_profile();

    let tmp = std::env::temp_dir().join("pics_test_round_trip.json");
    let file = std::fs::File::create(&tmp).expect("should create temp file");
    serde_json::to_writer_pretty(file, &profile.clone()).expect("should write JSON");

    let loaded = load_json_profile(&tmp).expect("should load JSON profile");
    assert_eq!(loaded.bo.points.len(), profile.bo.points.len());

    std::fs::remove_file(&tmp).ok();
}

// ---------------------------------------------------------------------------
// Profile files in data/profiles/
//
// These are flat xlsx files (no section-header rows). The loader uses Key sheet
// boundaries to reclassify points into their correct structured groups.
// Experimental-range points (>= 30000) are kept in the base `points` list.
// ---------------------------------------------------------------------------

#[test]
fn test_load_profile_full() {
    let profile = load_xlsx_profile(Path::new("../../../data/profiles/full.xlsx"))
        .expect("should load full.xlsx profile");

    assert_eq!(profile.bo.points.len(), 66, "expected 66 BO points");
    assert_eq!(
        profile.bo.points[0].point_index, 0u16,
        "expected first BO point at index 0"
    );
    assert_eq!(
        profile.bo.points[65].point_index, 30010u16,
        "expected last BO point at index 30010"
    );

    // BI: base (<5000) + experimental (>=30000)
    assert_eq!(profile.bi.points.len(), 134, "expected 134 base BI points");
    assert_eq!(profile.bi.points[0].point_index, 0u16);
    assert_eq!(
        profile.bi.points[133].point_index, 30011u16,
        "expected last BI point at index 30011"
    );
    assert_eq!(profile.bi.meters.len(), 1, "expected 1 BI meter instance");
    assert_eq!(
        profile.bi.meters[0].active_power_too_high.point_index,
        5000u16
    );
    assert_eq!(profile.bi.ders.len(), 1, "expected 1 BI DER instance");
    assert_eq!(
        profile.bi.ders[0].maintenance_operational_state.point_index,
        10000u16
    );
    assert_eq!(
        profile.bi.inverters.len(),
        2,
        "expected 2 BI inverter instances"
    );
    assert_eq!(
        profile.bi.inverters[0].active_power_too_high.point_index,
        15000u16
    );
    assert_eq!(
        profile.bi.batteries.len(),
        2,
        "expected 2 BI battery instances"
    );
    assert_eq!(
        profile.bi.batteries[0].status_of_storage.point_index,
        20000u16
    );

    // AO: base (<5000) + experimental (>=30000)
    assert_eq!(
        profile.ao.points.len(),
        1153,
        "expected 1153 base AO points"
    );
    assert_eq!(profile.ao.points[0].point_index, 0u16);
    assert_eq!(
        profile.ao.points[1152].point_index, 30061u16,
        "expected last AO point at index 30061"
    );
    assert_eq!(profile.ao.meters.len(), 1, "expected 1 AO meter instance");
    assert_eq!(
        profile.ao.meters[0].active_power_high_threshold.point_index,
        5000u16
    );
    assert_eq!(
        profile.ao.inverters.len(),
        2,
        "expected 2 AO inverter instances"
    );
    assert_eq!(
        profile.ao.inverters[0]
            .active_power_high_threshold
            .point_index,
        15000u16
    );
    assert_eq!(
        profile.ao.batteries.len(),
        2,
        "expected 2 AO battery instances"
    );
    assert_eq!(
        profile.ao.batteries[0]
            .external_voltage_high_threshold
            .point_index,
        20000u16
    );

    // AI: base ([0,328), [533,2000), >=30000) + structured groups + 3 edit selectors
    assert_eq!(
        profile.ai.points.len(),
        1105,
        "expected 1105 base AI points"
    );
    assert_eq!(profile.ai.points[0].point_index, 0u16);

    assert_eq!(profile.ai.curves.len(), 4, "expected 4 AI curves");
    assert!(
        profile.ai.points.iter().any(|p| p.point_index == 328u16),
        "expected curve edit selector at point index 328 in base AI points"
    );
    assert_eq!(
        profile.ai.curves[0].x_values.len(),
        100,
        "expected 100 AI curve x-values"
    );
    assert_eq!(
        profile.ai.curves[0].y_values.len(),
        100,
        "expected 100 AI curve y-values"
    );
    assert_eq!(
        profile.ai.schedules_bc.len(),
        4,
        "expected 4 AI schedule_bc entries"
    );
    assert_eq!(profile.ai.schedules.len(), 4, "expected 4 AI schedules");
    assert_eq!(profile.ai.meters.len(), 1, "expected 1 AI meter instance");
    assert_eq!(
        profile.ai.meters[0].type_of_connection_point.point_index,
        5000u16
    );
    assert!(
        !profile.ai.ders.is_empty(),
        "expected at least 1 AI DER instance"
    );
    assert_eq!(profile.ai.ders[0].unit_type.point_index, 10000u16);
    assert!(
        !profile.ai.inverters.is_empty(),
        "expected at least 1 AI inverter instance"
    );
    assert_eq!(
        profile.ai.inverters[0]
            .apparent_power_calc_method
            .point_index,
        15000u16
    );
    assert!(
        !profile.ai.batteries.is_empty(),
        "expected at least 1 AI battery instance"
    );
    assert_eq!(
        profile.ai.batteries[0].type_of_storage.point_index,
        20000u16
    );

    assert_eq!(profile.ctr.len(), 8, "expected 8 CTR points");
    assert_eq!(profile.ctr[0].point_index, 0u16);
    assert_eq!(
        profile.ctr[7].point_index, 5003u16,
        "expected last CTR point at index 5003"
    );
}

#[test]
fn test_load_profile_mandatory_1547() {
    let profile = load_xlsx_profile(Path::new("../../../data/profiles/mandatory_1547.xlsx"))
        .expect("should load mandatory_1547.xlsx profile");

    assert_eq!(profile.bo.points.len(), 7, "expected 7 BO points");
    assert_eq!(
        profile.bo.points[0].point_index, 3u16,
        "expected first BO point at index 3"
    );
    assert_eq!(
        profile.bo.points[6].point_index, 41u16,
        "expected last BO point at index 41"
    );

    // BI: base (<5000) only; this profile has no experimental-range BI points
    assert_eq!(profile.bi.points.len(), 19, "expected 19 base BI points");
    assert_eq!(profile.bi.points[0].point_index, 0u16);
    assert_eq!(
        profile.bi.points[18].point_index, 82u16,
        "expected last BI point at index 82"
    );
    assert_eq!(profile.bi.meters.len(), 1, "expected 1 BI meter instance");
    assert_eq!(
        profile.bi.meters[0].active_power_too_high.point_index,
        5000u16
    );
    assert_eq!(profile.bi.ders.len(), 1, "expected 1 BI DER instance");
    assert_eq!(
        profile.bi.ders[0].maintenance_operational_state.point_index,
        10000u16
    );
    assert_eq!(
        profile.bi.inverters.len(),
        1,
        "expected 1 BI inverter instance"
    );
    assert_eq!(
        profile.bi.inverters[0].active_power_too_high.point_index,
        15000u16
    );
    assert_eq!(
        profile.bi.batteries.len(),
        1,
        "expected 1 BI battery instance"
    );
    assert_eq!(
        profile.bi.batteries[0].status_of_storage.point_index,
        20000u16
    );

    assert_eq!(profile.ao.points.len(), 42, "expected 42 base AO points");
    assert_eq!(
        profile.ao.points[0].point_index, 23u16,
        "expected first AO point at index 23"
    );
    assert_eq!(
        profile.ao.points[41].point_index, 511u16,
        "expected last AO point at index 511"
    );
    assert_eq!(profile.ao.meters.len(), 1, "expected 1 AO meter instance");
    assert_eq!(
        profile.ao.meters[0].active_power_high_threshold.point_index,
        5000u16
    );
    assert_eq!(
        profile.ao.inverters.len(),
        1,
        "expected 1 AO inverter instance"
    );
    assert_eq!(
        profile.ao.inverters[0]
            .active_power_high_threshold
            .point_index,
        15000u16
    );
    assert_eq!(
        profile.ao.batteries.len(),
        1,
        "expected 1 AO battery instance"
    );
    assert_eq!(
        profile.ao.batteries[0]
            .external_voltage_high_threshold
            .point_index,
        20000u16
    );

    // AI: base ([0,328), [533,2000)); curves populated but no schedules or equipment groups
    // + 1 curve edit selector moved to base points
    assert_eq!(profile.ai.points.len(), 55, "expected 55 base AI points");
    assert_eq!(
        profile.ai.points[0].point_index, 2u16,
        "expected first AI point at index 2"
    );
    assert_eq!(
        profile.ai.points[53].point_index, 621u16,
        "expected last base AI point at index 621"
    );
    assert_eq!(profile.ai.curves.len(), 4, "expected 4 AI curves");
    assert!(
        profile.ai.points.iter().any(|p| p.point_index == 328u16),
        "expected curve edit selector at point index 328 in base AI points"
    );
    assert_eq!(
        profile.ai.curves[0].x_values.len(),
        4,
        "expected 4 AI curve x-values"
    );
    assert_eq!(
        profile.ai.curves[0].y_values.len(),
        4,
        "expected 4 AI curve y-values"
    );
    assert_eq!(
        profile.ai.schedules_bc.len(),
        0,
        "expected 0 AI schedules_bc"
    );
    assert_eq!(profile.ai.schedules.len(), 0, "expected 0 AI schedules");
    assert_eq!(profile.ai.meters.len(), 1, "expected 1 AI meter instance");
    assert_eq!(
        profile.ai.meters[0].type_of_connection_point.point_index,
        5000u16
    );
    assert_eq!(profile.ai.ders.len(), 1, "expected 1 AI DER instance");
    assert_eq!(profile.ai.ders[0].unit_type.point_index, 10000u16);
    assert_eq!(
        profile.ai.inverters.len(),
        1,
        "expected 1 AI inverter instance"
    );
    assert_eq!(
        profile.ai.inverters[0]
            .apparent_power_calc_method
            .point_index,
        15000u16
    );
    assert_eq!(
        profile.ai.batteries.len(),
        1,
        "expected 1 AI battery instance"
    );
    assert_eq!(
        profile.ai.batteries[0].type_of_storage.point_index,
        20000u16
    );

    assert_eq!(profile.ctr.len(), 4, "expected 4 CTR points");
    assert_eq!(
        profile.ctr[0].point_index, 5000u16,
        "expected first CTR point at index 5000"
    );
    assert_eq!(
        profile.ctr[3].point_index, 5003u16,
        "expected last CTR point at index 5003"
    );
}

#[test]
fn test_load_profile_mandatory_1815() {
    let profile = load_xlsx_profile(Path::new("../../../data/profiles/mandatory_1815.xlsx"))
        .expect("should load mandatory_1815.xlsx profile");

    assert_eq!(profile.bo.points.len(), 58, "expected 58 BO points");
    assert_eq!(
        profile.bo.points[0].point_index, 0u16,
        "expected first BO point at index 0"
    );
    assert_eq!(
        profile.bo.points[57].point_index, 30010u16,
        "expected last BO point at index 30010"
    );

    // BI: base (<5000) + experimental (>=30000)
    assert_eq!(profile.bi.points.len(), 107, "expected 107 base BI points");
    assert_eq!(profile.bi.points[0].point_index, 0u16);
    assert_eq!(
        profile.bi.points[106].point_index, 30011u16,
        "expected last base BI point at index 30011"
    );
    assert_eq!(profile.bi.meters.len(), 1, "expected 1 BI meter instance");
    assert_eq!(
        profile.bi.meters[0].active_power_too_high.point_index,
        5000u16
    );
    assert_eq!(profile.bi.ders.len(), 1, "expected 1 BI DER instance");
    assert_eq!(
        profile.bi.ders[0].maintenance_operational_state.point_index,
        10000u16
    );
    assert_eq!(
        profile.bi.inverters.len(),
        1,
        "expected 1 BI inverter instance"
    );
    assert_eq!(
        profile.bi.inverters[0].active_power_too_high.point_index,
        15000u16
    );
    assert_eq!(
        profile.bi.batteries.len(),
        1,
        "expected 1 BI battery instance"
    );
    assert_eq!(
        profile.bi.batteries[0].status_of_storage.point_index,
        20000u16
    );

    assert_eq!(profile.ao.points.len(), 357, "expected 357 base AO points");
    assert_eq!(
        profile.ao.points[0].point_index, 22u16,
        "expected first AO point at index 22"
    );
    assert_eq!(
        profile.ao.points[356].point_index, 30020u16,
        "expected last AO point at index 30020"
    );
    assert_eq!(profile.ao.meters.len(), 1, "expected 1 AO meter instance");
    assert_eq!(
        profile.ao.inverters.len(),
        1,
        "expected 1 AO inverter instance"
    );
    assert_eq!(
        profile.ao.batteries.len(),
        1,
        "expected 1 AO battery instance"
    );

    // AI: base ([0,328), [533,2000)); curves populated but no schedules or equipment groups
    // + 1 curve edit selector moved to base points
    assert_eq!(profile.ai.points.len(), 434, "expected 434 base AI points");
    assert_eq!(
        profile.ai.points[0].point_index, 0u16,
        "expected first AI point at index 0"
    );
    assert_eq!(
        profile.ai.points[288].point_index, 621u16,
        "expected base AI point at index 621"
    );
    assert_eq!(profile.ai.curves.len(), 4, "expected 4 AI curves");
    assert!(
        profile.ai.points.iter().any(|p| p.point_index == 328u16),
        "expected curve edit selector at point index 328 in base AI points"
    );
    assert_eq!(
        profile.ai.curves[0].x_values.len(),
        10,
        "expected 10 AI curve x-values"
    );
    assert_eq!(
        profile.ai.curves[0].y_values.len(),
        10,
        "expected 10 AI curve y-values"
    );
    assert_eq!(
        profile.ai.schedules_bc.len(),
        4,
        "expected 4 AI schedules_bc"
    );
    assert_eq!(profile.ai.schedules.len(), 4, "expected 4 AI schedules");
    assert_eq!(profile.ai.meters.len(), 1, "expected 1 AI meter instance");
    assert_eq!(
        profile.ai.meters[0].type_of_connection_point.point_index,
        5000u16
    );
    assert_eq!(profile.ai.ders.len(), 1, "expected 1 AI DER instance");
    assert_eq!(profile.ai.ders[0].unit_type.point_index, 10000u16);
    assert_eq!(
        profile.ai.inverters.len(),
        1,
        "expected 1 AI inverter instance"
    );
    assert_eq!(
        profile.ai.inverters[0]
            .apparent_power_calc_method
            .point_index,
        15000u16
    );
    assert_eq!(
        profile.ai.batteries.len(),
        1,
        "expected 1 AI battery instance"
    );
    assert_eq!(
        profile.ai.batteries[0].type_of_storage.point_index,
        20000u16
    );

    assert_eq!(profile.ctr.len(), 4, "expected 4 CTR points");
    assert_eq!(
        profile.ctr[0].point_index, 5000u16,
        "expected first CTR point at index 5000"
    );
    assert_eq!(
        profile.ctr[3].point_index, 5003u16,
        "expected last CTR point at index 5003"
    );
}

#[test]
fn test_load_profile_minimal_1547() {
    let profile = load_xlsx_profile(Path::new("../../../data/profiles/minimal_1547.xlsx"))
        .expect("should load minimal_1547.xlsx profile");

    assert_eq!(profile.bo.points.len(), 7, "expected 7 BO points");
    assert_eq!(
        profile.bo.points[0].point_index, 3u16,
        "expected first BO point at index 3"
    );
    assert_eq!(
        profile.bo.points[6].point_index, 41u16,
        "expected last BO point at index 41"
    );

    assert_eq!(profile.bi.points.len(), 19, "expected 19 base BI points");
    assert_eq!(profile.bi.points[0].point_index, 0u16);
    assert_eq!(
        profile.bi.points[18].point_index, 82u16,
        "expected last BI point at index 82"
    );
    assert_eq!(profile.bi.meters.len(), 0);
    assert_eq!(profile.bi.ders.len(), 0);
    assert_eq!(profile.bi.inverters.len(), 0);
    assert_eq!(profile.bi.batteries.len(), 0);

    assert_eq!(profile.ao.points.len(), 23, "expected 23 base AO points");
    assert_eq!(
        profile.ao.points[0].point_index, 23u16,
        "expected first AO point at index 23"
    );
    assert_eq!(
        profile.ao.points[22].point_index, 244u16,
        "expected last AO point at index 244"
    );
    assert_eq!(profile.ao.meters.len(), 0);
    assert_eq!(profile.ao.inverters.len(), 0);
    assert_eq!(profile.ao.batteries.len(), 0);

    // AI: base ([0,328), [533,2000)); curves populated with 1 curve of 10 x/y pairs
    // + 1 curve edit selector moved to base points
    assert_eq!(profile.ai.points.len(), 55, "expected 55 base AI points");
    assert_eq!(
        profile.ai.points[0].point_index, 2u16,
        "expected first AI point at index 2"
    );
    assert_eq!(
        profile.ai.points[53].point_index, 621u16,
        "expected last base AI point at index 621"
    );
    assert_eq!(profile.ai.curves.len(), 4, "expected 4 AI curves");
    assert!(
        profile.ai.points.iter().any(|p| p.point_index == 328u16),
        "expected curve edit selector at point index 328 in base AI points"
    );
    assert_eq!(
        profile.ai.curves[0].x_values.len(),
        4,
        "expected 4 AI curve x-values"
    );
    assert_eq!(
        profile.ai.curves[0].y_values.len(),
        4,
        "expected 4 AI curve y-values"
    );
    assert_eq!(profile.ai.schedules_bc.len(), 0);
    assert_eq!(profile.ai.schedules.len(), 0);
    assert_eq!(profile.ai.meters.len(), 0);
    assert_eq!(profile.ai.ders.len(), 0);
    assert_eq!(profile.ai.inverters.len(), 0);
    assert_eq!(profile.ai.batteries.len(), 0);

    assert_eq!(
        profile.ctr.len(),
        0,
        "expected 0 CTR points in minimal profile"
    );
}
