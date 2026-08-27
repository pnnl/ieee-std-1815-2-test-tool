use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;

use anyhow::{Context, Result};
use calamine::{Data, Range, Reader, Xlsx, open_workbook};
use common::profile::curve::float_to_curve_code;
use common::profile::validation::{LoadError, Validated, ValidationError, ValidationErrors};
use common::profile::values::{EngineeringF64, TransmissionI32};
use common::profile::{ActionType, BiPoint, BoPoint, CurveType, ProfileIndex};
use common::uids::bo_uid::BoUid;

use crate::models::{
    AiBattery, AiCurve, AiDer, AiInverter, AiMeter, AiPoint, AiSchedule, AiScheduleBC,
    AnalogInputs, AnalogOutputs, AoBattery, AoInverter, AoMeter, AoPoint, BiBattery, BiDer,
    BiInverter, BiMeter, BinaryInputs, BinaryOutputs, CtrPoint, EquipmentInfo, EquipmentPoints,
    EventClass, KeySheet, PicsProfile, SectionInfo, SectionPoints, evaluate_conditional_mandatory,
};
use crate::schema::{self, AiCol, AoCol, BiCol, BoCol, CtrCol, Sheet};
use common::profile::scale_curve::{DependentVariableUnit, IndependentVariableUnit};

// ---------------------------------------------------------------------------
// Workbook loading
// ---------------------------------------------------------------------------

pub struct Workbook {
    pub sheets: HashMap<String, Range<Data>>,
    pub sheet_names: Vec<String>,
}

pub fn load_workbook(path: &Path) -> Result<Workbook> {
    let mut xl: Xlsx<_> = open_workbook(path)
        .with_context(|| format!("Could not open workbook: {}", path.display()))?;

    let sheet_names = xl.sheet_names().to_vec();
    let mut sheets = HashMap::new();

    for name in &sheet_names {
        if let Ok(range) = xl.worksheet_range(name) {
            sheets.insert(name.clone(), range);
        }
    }

    Ok(Workbook {
        sheets,
        sheet_names,
    })
}

pub fn load_workbook_from_bytes(bytes: &[u8]) -> Result<Workbook> {
    let cursor = Cursor::new(bytes);
    let mut xl: Xlsx<_> =
        Xlsx::new(cursor).with_context(|| "Could not open workbook from uploaded bytes")?;

    let sheet_names = xl.sheet_names().to_vec();
    let mut sheets = HashMap::new();

    for name in &sheet_names {
        if let Ok(range) = xl.worksheet_range(name) {
            sheets.insert(name.clone(), range);
        }
    }

    Ok(Workbook {
        sheets,
        sheet_names,
    })
}

pub fn get_worksheet<'a>(wb: &'a Workbook, name: &str) -> Option<&'a Range<Data>> {
    if let Some(r) = wb.sheets.get(name) {
        return Some(r);
    }
    let trimmed_name = name.trim();
    for (sheet_name, range) in &wb.sheets {
        if sheet_name.trim() == trimmed_name {
            return Some(range);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Cell value helpers
// ---------------------------------------------------------------------------

fn cell_str(range: &Range<Data>, row: u32, col: u32) -> Option<String> {
    let val = range.get((row as usize, col as usize))?;
    match val {
        Data::Empty => None,
        Data::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Data::Float(f) => Some(f.to_string()),
        Data::Int(i) => Some(i.to_string()),
        Data::Bool(b) => Some(b.to_string()),
        Data::Error(e) => Some(format!("{:?}", e)),
        Data::DateTime(dt) => Some(dt.to_string()),
        Data::DurationIso(s) | Data::DateTimeIso(s) => Some(s.to_string()),
    }
}

fn cell_str_required(
    range: &Range<Data>,
    row: u32,
    col: u32,
    field: &str,
    point_index: &str,
) -> Result<String, ValidationErrors> {
    cell_str(range, row, col).ok_or_else(|| {
        ValidationErrors::from_error(ValidationError {
            point: point_index.to_string(),
            message: format!(
                "Missing required field '{}' for point '{}' (row {})",
                field,
                point_index,
                row + 1
            ),
        })
    })
}

fn cell_float_required(
    range: &Range<Data>,
    row: u32,
    col: u32,
    field: &str,
    point_index: &str,
    sheet: &str,
    errors: &mut Vec<LoadError>,
) -> f64 {
    match range.get((row as usize, col as usize)) {
        Some(Data::Float(f)) => *f,
        Some(Data::Int(i)) => *i as f64, // i64 → f64 is lossless for spreadsheet-range integers
        Some(Data::String(s)) => {
            let trimmed = s.trim();
            if let Ok(v) = trimmed.parse::<f64>() {
                v
            } else {
                errors.push(LoadError::at_row(
                    sheet,
                    point_index,
                    field,
                    row + 1,
                    format!(
                        "Expected numeric value for '{}' of point '{}' (row {}), got {:?}",
                        field,
                        point_index,
                        row + 1,
                        trimmed
                    ),
                ));
                0.0
            }
        }
        _ => {
            errors.push(LoadError::at_row(
                sheet,
                point_index,
                field,
                row + 1,
                format!(
                    "Missing required field '{}' for point '{}' (row {})",
                    field,
                    point_index,
                    row + 1
                ),
            ));
            0.0
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn cell_i32_required(
    range: &Range<Data>,
    row: u32,
    col: u32,
    field: &str,
    point_index: &str,
    sheet: &str,
    errors: &mut Vec<LoadError>,
    default: i32,
) -> i32 {
    match range.get((row as usize, col as usize)) {
        Some(Data::Float(f)) => {
            let rounded = f.round();
            if rounded < i32::MIN as f64 || rounded > i32::MAX as f64 {
                errors.push(LoadError::at_row(
                    sheet,
                    point_index,
                    field,
                    row + 1,
                    format!(
                        "Value {} for '{}' of point '{}' (row {}) is outside i32 range",
                        f,
                        field,
                        point_index,
                        row + 1
                    ),
                ));
                default
            } else {
                rounded as i32
            }
        }
        Some(Data::Int(i)) => match i32::try_from(*i) {
            Ok(value) => value,
            Err(_) => {
                errors.push(LoadError::at_row(
                    sheet,
                    point_index,
                    field,
                    row + 1,
                    format!(
                        "Value {} for '{}' of point '{}' (row {}) is outside i32 range",
                        i,
                        field,
                        point_index,
                        row + 1
                    ),
                ));
                default
            }
        },
        Some(Data::String(s)) => {
            let trimmed = s.trim();
            match trimmed.parse::<f64>() {
                Ok(v) => {
                    let rounded = v.round();
                    if rounded < i32::MIN as f64 || rounded > i32::MAX as f64 {
                        errors.push(LoadError::at_row(
                            sheet,
                            point_index,
                            field,
                            row + 1,
                            format!(
                                "Value {} for '{}' of point '{}' (row {}) is outside i32 range",
                                v,
                                field,
                                point_index,
                                row + 1
                            ),
                        ));
                        default
                    } else {
                        rounded as i32
                    }
                }
                Err(_) => {
                    errors.push(LoadError::at_row(
                        sheet,
                        point_index,
                        field,
                        row + 1,
                        format!(
                            "Expected numeric value for '{}' of point '{}' (row {}), got {:?}",
                            field,
                            point_index,
                            row + 1,
                            trimmed
                        ),
                    ));
                    default
                }
            }
        }
        _ => {
            errors.push(LoadError::at_row(
                sheet,
                point_index,
                field,
                row + 1,
                format!(
                    "Missing required field '{}' for point '{}' (row {})",
                    field,
                    point_index,
                    row + 1
                ),
            ));
            default
        }
    }
}

fn cell_event_class(range: &Range<Data>, row: u32, col: u32) -> EventClass {
    match range.get((row as usize, col as usize)) {
        Some(Data::Int(i)) => match i {
            1 => EventClass::Class1,
            2 => EventClass::Class2,
            3 => EventClass::Class3,
            _ => EventClass::None,
        },
        Some(Data::Float(f)) => match *f as i64 {
            1 => EventClass::Class1,
            2 => EventClass::Class2,
            3 => EventClass::Class3,
            _ => EventClass::None,
        },
        Some(Data::String(s)) => match s.trim() {
            "1" => EventClass::Class1,
            "2" => EventClass::Class2,
            "3" => EventClass::Class3,
            _ => EventClass::None,
        },
        _ => EventClass::None,
    }
}

fn parse_mandatory(range: &Range<Data>, row: u32, col: u32) -> bool {
    match cell_str(range, row, col).as_deref() {
        Some("M") => true,
        Some(s) if s.starts_with("C:") || s.starts_with("C ") => evaluate_conditional_mandatory(s),
        _ => false,
    }
}

fn parse_frozen_counter_exists(range: &Range<Data>, row: u32, col: u32) -> bool {
    cell_str(range, row, col)
        .map(|s| s.eq_ignore_ascii_case("yes"))
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Section detection helpers
// ---------------------------------------------------------------------------

fn is_standard_section_header(range: &Range<Data>, row: u32) -> bool {
    let col_a = range.get((row as usize, 0));
    let col_b = range.get((row as usize, 1));
    let a_empty = matches!(col_a, None | Some(Data::Empty));
    let b_has_value = matches!(col_b, Some(Data::String(s)) if !s.trim().is_empty());
    a_empty && b_has_value
}

/// Returns true if the point index string contains an integer portion (e.g. "BI 50" or "50").
/// Returns false for section header labels like "BI VP" or "BI Vendor Points" that pass
/// `is_data_row` but have no numeric index.
fn has_integer_index(point_index_str: &str) -> bool {
    !point_index_str
        .trim_start_matches(|c: char| !c.is_ascii_digit())
        .is_empty()
}

fn is_data_row(range: &Range<Data>, row: u32, sheet_type: &str) -> bool {
    match range.get((row as usize, 0)) {
        Some(Data::String(s)) => s.trim().starts_with(sheet_type),
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Key sheet loading
// ---------------------------------------------------------------------------

pub fn load_key_sheet(range: &Range<Data>) -> Option<KeySheet> {
    fn cell_int(range: &Range<Data>, row: u32, col: u32) -> Option<u16> {
        let val = range.get((row as usize, col as usize))?;
        match val {
            Data::Float(f) => {
                let rounded = f.round();
                if rounded >= 0.0 && rounded <= u16::MAX as f64 {
                    Some(rounded as u16)
                } else {
                    None // TODO: propagate errors for out-of-range values
                }
            }
            Data::Int(i) => u16::try_from(*i).ok(),
            Data::String(s) => {
                let trimmed = s.trim();
                if trimmed.is_empty() || trimmed == "-" || trimmed == "x" {
                    None
                } else {
                    trimmed.parse::<f64>().ok().and_then(|f| {
                        let rounded = f.round();
                        if rounded >= 0.0 && rounded <= u16::MAX as f64 {
                            Some(rounded as u16)
                        } else {
                            None // TODO: propagate errors for out-of-range values
                        }
                    })
                }
            }
            _ => None,
        }
    }

    // Validate the Key sheet has the expected column layout; fail fast on unknown structure.
    match range.get((
        schema::KEY_HEADER_ROW as usize,
        schema::KEY_START_COL as usize,
    )) {
        Some(Data::String(s)) if s.trim() == "Start Index" => {}
        _ => return None,
    }

    fn load_section_points(range: &Range<Data>, row: u32) -> SectionPoints {
        SectionPoints {
            bo: SectionInfo {
                start: cell_int(range, row, schema::KEY_BO_COL).unwrap_or(0),
            },
            bi: SectionInfo {
                start: cell_int(range, row, schema::KEY_BI_COL).unwrap_or(0),
            },
            ao: SectionInfo {
                start: cell_int(range, row, schema::KEY_AO_COL).unwrap_or(0),
            },
            ai: SectionInfo {
                start: cell_int(range, row, schema::KEY_AI_COL).unwrap_or(0),
            },
            ctr: SectionInfo {
                start: cell_int(range, row, schema::KEY_CTR_COL).unwrap_or(0),
            },
        }
    }

    fn load_single_section_points(range: &Range<Data>, row: u32, default: u16) -> SectionPoints {
        let start = cell_int(range, row, schema::KEY_START_COL).unwrap_or(default);
        let info = SectionInfo { start };
        SectionPoints {
            bo: info.clone(),
            bi: info.clone(),
            ao: info.clone(),
            ai: info.clone(),
            ctr: info,
        }
    }

    fn load_equipment_points(
        range: &Range<Data>,
        desc_row: u32,
        per_row: u32,
        default_start: u16,
    ) -> EquipmentPoints {
        let count = cell_int(range, desc_row, schema::KEY_COUNT_COL).unwrap_or(0);
        let start = cell_int(range, desc_row, schema::KEY_START_COL).unwrap_or(default_start);
        EquipmentPoints {
            bo: EquipmentInfo {
                count,
                start,
                points_per: cell_int(range, per_row, schema::KEY_BO_COL).unwrap_or(0),
            },
            bi: EquipmentInfo {
                count,
                start,
                points_per: cell_int(range, per_row, schema::KEY_BI_COL).unwrap_or(0),
            },
            ao: EquipmentInfo {
                count,
                start,
                points_per: cell_int(range, per_row, schema::KEY_AO_COL).unwrap_or(0),
            },
            ai: EquipmentInfo {
                count,
                start,
                points_per: cell_int(range, per_row, schema::KEY_AI_COL).unwrap_or(0),
            },
            ctr: EquipmentInfo {
                count,
                start,
                points_per: cell_int(range, per_row, schema::KEY_CTR_COL).unwrap_or(0),
            },
        }
    }

    Some(KeySheet {
        config: load_section_points(range, schema::KEY_SECTION_ROWS[0].0),
        functions: load_section_points(range, schema::KEY_SECTION_ROWS[1].0),
        curves: load_section_points(range, schema::KEY_SECTION_ROWS[2].0),
        system_meter: load_section_points(range, schema::KEY_SECTION_ROWS[3].0),
        extensions: load_section_points(range, schema::KEY_SECTION_ROWS[4].0),
        experimental: load_single_section_points(range, schema::KEY_EXPERIMENTAL_ROW, 30000),
        vendor: load_single_section_points(range, schema::KEY_VENDOR_ROW, 50000),
        discovery: load_single_section_points(range, schema::KEY_AUTO_DISC_ROW, 60000),
        schedules_bc: load_single_section_points(range, schema::KEY_SCHEDULE_BC_ROW, 2000),
        schedules_bc_status: load_single_section_points(
            range,
            schema::KEY_SCHEDULE_BC_STATUS_ROW,
            2300,
        ),
        schedules: load_single_section_points(range, schema::KEY_SCHEDULE_ROW, 3000),
        schedules_status: load_single_section_points(range, schema::KEY_SCHEDULE_STATUS_ROW, 3500),
        max_points: cell_int(range, schema::KEY_MAX_POINTS_ROW, schema::KEY_START_COL)
            .unwrap_or(65535),
        meter: load_equipment_points(
            range,
            schema::KEY_METER_ROW,
            schema::KEY_METER_PER_ROW,
            5000,
        ),
        der: load_equipment_points(
            range,
            schema::KEY_DER_UNIT_ROW,
            schema::KEY_DER_UNIT_PER_ROW,
            10000,
        ),
        inverter: load_equipment_points(
            range,
            schema::KEY_INVERTER_ROW,
            schema::KEY_INVERTER_PER_ROW,
            15000,
        ),
        battery: load_equipment_points(
            range,
            schema::KEY_BATTERY_ROW,
            schema::KEY_BATTERY_PER_ROW,
            20000,
        ),
    })
}

// ---------------------------------------------------------------------------
// Instance grouping helpers
// ---------------------------------------------------------------------------

/// Parses the numeric suffix from a point index string (e.g., "BI5000" → 5000).
fn parse_point_index_u16(s: &str) -> Result<u16, ValidationErrors> {
    let digits = s.trim_start_matches(|c: char| !c.is_ascii_digit());
    digits.parse::<u16>().map_err(|_| {
        ValidationErrors::from_error(ValidationError {
            point: s.to_string(),
            message: format!(
                "Invalid point index '{}'. Could not parse {} as u16",
                s, digits
            ),
        })
    })
}

/// Groups a flat list of points into contiguous runs separated by index gaps.
/// Each run where consecutive point indices differ by more than 1 starts a new group.
/// Used for equipment sections (meters, inverters, batteries, DER units) where
/// each gap indicates a missing/unconfigured equipment instance.
fn group_by_index_gap<P, F>(points: Vec<P>, get_index: F) -> Vec<Vec<P>>
where
    F: Fn(&P) -> Option<i64>,
{
    let mut groups: Vec<Vec<P>> = Vec::new();
    let mut current: Vec<P> = Vec::new();
    let mut last_num: Option<i64> = None;

    for point in points {
        let num = get_index(&point);
        if let (Some(last), Some(current_num)) = (last_num, num) {
            if current_num > last + 1 && !current.is_empty() {
                groups.push(std::mem::take(&mut current));
            }
        }
        last_num = num;
        current.push(point);
    }
    if !current.is_empty() {
        groups.push(current);
    }
    groups
}

// ---------------------------------------------------------------------------
// BO sheet loading
// ---------------------------------------------------------------------------

pub fn load_bo_sheet(range: &Range<Data>) -> Result<BinaryOutputs, ValidationErrors> {
    let mut points = Vec::new();
    let (row_count, _) = range.get_size();

    for row_idx in schema::DATA_ROW_START..row_count {
        let row = row_idx as u32;
        if !is_data_row(range, row, Sheet::Bo.as_str()) {
            continue;
        }
        let Some(point_index_str) = cell_str(range, row, BoCol::PointIndex as u32) else {
            continue;
        };
        if !has_integer_index(&point_index_str) {
            continue;
        }
        let point_index = parse_point_index_u16(&point_index_str)?;
        if point_index == u16::MAX {
            continue;
        }
        points.push(BoPoint {
            name: cell_str_required(range, row, BoCol::Name as u32, "name", &point_index_str)?,
            state_0: cell_str_required(
                range,
                row,
                BoCol::State0 as u32,
                "state_0",
                &point_index_str,
            )?,
            state_1: cell_str_required(
                range,
                row,
                BoCol::State1 as u32,
                "state_1",
                &point_index_str,
            )?,
            iec_61850_uid: cell_str_required(
                range,
                row,
                BoCol::Iec61850 as u32,
                "iec61850",
                &point_index_str,
            )?,
            assoc_bi: cell_str(range, row, BoCol::AssocBi as u32).map(|s| s.replace(' ', "")),
            purpose: cell_str_required(
                range,
                row,
                BoCol::Purpose as u32,
                "purpose",
                &point_index_str,
            )?,
            mandatory_1815: parse_mandatory(range, row, BoCol::Mandatory1815 as u32),
            mandatory_1547: parse_mandatory(range, row, BoCol::Mandatory1547 as u32),
            point_index,
        });
    }

    Ok(BinaryOutputs { points })
}

// ---------------------------------------------------------------------------
// BI sheet loading - flat (reclassification is done by reclassify_by_key)
// ---------------------------------------------------------------------------

pub fn load_bi_sheet(range: &Range<Data>) -> Result<BinaryInputs> {
    let mut points = Vec::new();
    let (row_count, _) = range.get_size();

    for row_idx in schema::DATA_ROW_START..row_count {
        let row = row_idx as u32;
        if is_standard_section_header(range, row) {
            continue;
        }
        if !is_data_row(range, row, Sheet::Bi.as_str()) {
            continue;
        }
        let Some(point_index_str) = cell_str(range, row, BiCol::PointIndex as u32) else {
            continue;
        };
        if !has_integer_index(&point_index_str) {
            continue;
        }
        let point_index = parse_point_index_u16(&point_index_str)?;
        if point_index == u16::MAX {
            continue;
        }
        points.push(BiPoint {
            name: cell_str_required(range, row, BiCol::Name as u32, "name", &point_index_str)?,
            event_class: cell_event_class(range, row, BiCol::EventClass as u32),
            state_0: cell_str_required(
                range,
                row,
                BiCol::State0 as u32,
                "state_0",
                &point_index_str,
            )?,
            state_1: cell_str_required(
                range,
                row,
                BiCol::State1 as u32,
                "state_1",
                &point_index_str,
            )?,
            iec_61850_uid: cell_str_required(
                range,
                row,
                BiCol::Iec61850 as u32,
                "iec61850",
                &point_index_str,
            )?,
            assoc_bo: cell_str(range, row, BiCol::AssocBo as u32).map(|s| s.replace(' ', "")),
            purpose: cell_str_required(
                range,
                row,
                BiCol::Purpose as u32,
                "purpose",
                &point_index_str,
            )?,
            mandatory_1815: parse_mandatory(range, row, BiCol::Mandatory1815 as u32),
            mandatory_1547: parse_mandatory(range, row, BiCol::Mandatory1547 as u32),
            point_index,
        });
    }

    Ok(BinaryInputs {
        points,
        meters: vec![],
        ders: vec![],
        inverters: vec![],
        batteries: vec![],
    })
}

// ---------------------------------------------------------------------------
// AO sheet loading - flat (reclassification is done by reclassify_by_key)
// ---------------------------------------------------------------------------

pub fn load_ao_sheet(range: &Range<Data>, errors: &mut Vec<LoadError>) -> Result<AnalogOutputs> {
    let mut ao_points = Vec::new();
    let (row_count, _) = range.get_size();

    for row_idx in schema::DATA_ROW_START..row_count {
        let row = row_idx as u32;
        if is_standard_section_header(range, row) {
            continue;
        }
        if !is_data_row(range, row, Sheet::Ao.as_str()) {
            continue;
        }
        let Some(point_index_str) = cell_str(range, row, AoCol::PointIndex as u32) else {
            continue;
        };
        if !has_integer_index(&point_index_str) {
            continue;
        }
        let point_index = parse_point_index_u16(&point_index_str)?;
        if point_index == u16::MAX {
            continue;
        }
        let raw_multiplier = cell_float_required(
            range,
            row,
            AoCol::Multiplier as u32,
            "multiplier",
            &point_index_str,
            "AO",
            errors,
        );

        let resolved_multiplier = if raw_multiplier == 0.0 {
            errors.push(LoadError::at_row(
                "AO",
                &point_index_str,
                "multiplier",
                row + 1,
                "Multiplier cannot be zero; defaulting to 1.0".to_string(),
            ));
            1.0
        } else {
            raw_multiplier
        };

        let point = AoPoint::new(
            point_index,
            cell_str_required(range, row, AoCol::Name as u32, "name", &point_index_str)?,
            TransmissionI32(cell_i32_required(
                range,
                row,
                AoCol::Minimum as u32,
                "minimum",
                &point_index_str,
                "AO",
                errors,
                i32::MIN,
            )),
            TransmissionI32(cell_i32_required(
                range,
                row,
                AoCol::Maximum as u32,
                "maximum",
                &point_index_str,
                "AO",
                errors,
                i32::MAX,
            )),
            resolved_multiplier,
            EngineeringF64(cell_float_required(
                range,
                row,
                AoCol::Offset as u32,
                "offset",
                &point_index_str,
                "AO",
                errors,
            )),
            cell_str(range, row, AoCol::Units as u32).unwrap_or_default(),
            cell_str_required(
                range,
                row,
                AoCol::Iec61850 as u32,
                "iec61850",
                &point_index_str,
            )?,
            cell_str(range, row, AoCol::AssocAi as u32).map(|s| s.replace(' ', "")),
            cell_str_required(
                range,
                row,
                AoCol::Purpose as u32,
                "purpose",
                &point_index_str,
            )?,
            parse_mandatory(range, row, AoCol::Mandatory1815 as u32),
            parse_mandatory(range, row, AoCol::Mandatory1547 as u32),
        )?;
        ao_points.push(point);
    }

    Ok(AnalogOutputs {
        points: ao_points,
        meters: vec![],
        inverters: vec![],
        batteries: vec![],
    })
}

// ---------------------------------------------------------------------------
// AI sheet loading - flat (reclassification is done by reclassify_by_key)
// ---------------------------------------------------------------------------

pub fn load_ai_sheet(
    range: &Range<Data>,
    errors: &mut Vec<LoadError>,
) -> Result<AnalogInputs, ValidationErrors> {
    let mut ai_points = Vec::new();
    let (row_count, _) = range.get_size();

    for row_idx in schema::DATA_ROW_START..row_count {
        let row = row_idx as u32;
        if is_standard_section_header(range, row) {
            continue;
        }
        if !is_data_row(range, row, Sheet::Ai.as_str()) {
            continue;
        }
        let Some(point_index_str) = cell_str(range, row, AiCol::PointIndex as u32) else {
            continue;
        };
        if !has_integer_index(&point_index_str) {
            continue;
        }
        let point_index = parse_point_index_u16(&point_index_str)?;
        if point_index == u16::MAX {
            continue;
        }

        let raw_multiplier = cell_float_required(
            range,
            row,
            AiCol::Multiplier as u32,
            "multiplier",
            &point_index_str,
            "AI",
            errors,
        );

        let resolved_multiplier = if raw_multiplier == 0.0 {
            errors.push(LoadError::at_row(
                "AI",
                &point_index_str,
                "multiplier",
                row + 1,
                "Multiplier cannot be zero; defaulting to 1.0".to_string(),
            ));
            1.0
        } else {
            raw_multiplier
        };

        let original_value = cell_float_required(
            range,
            row,
            AiCol::Value as u32,
            "value",
            &point_index_str,
            "AI",
            errors,
        );

        let point = AiPoint::new(
            point_index,
            cell_str_required(range, row, AiCol::Name as u32, "name", &point_index_str)?,
            cell_event_class(range, row, AiCol::EventClass as u32),
            TransmissionI32(cell_i32_required(
                range,
                row,
                AiCol::Minimum as u32,
                "minimum",
                &point_index_str,
                "AI",
                errors,
                i32::MIN,
            )),
            TransmissionI32(cell_i32_required(
                range,
                row,
                AiCol::Maximum as u32,
                "maximum",
                &point_index_str,
                "AI",
                errors,
                i32::MAX,
            )),
            resolved_multiplier,
            EngineeringF64(cell_float_required(
                range,
                row,
                AiCol::Offset as u32,
                "offset",
                &point_index_str,
                "AI",
                errors,
            )),
            cell_str(range, row, AiCol::Units as u32).unwrap_or_default(),
            cell_str_required(
                range,
                row,
                AiCol::Iec61850 as u32,
                "iec61850",
                &point_index_str,
            )?,
            EngineeringF64(original_value),
            cell_str(range, row, AiCol::AssocAo as u32).map(|s| s.replace(' ', "")),
            cell_str_required(
                range,
                row,
                AiCol::Purpose as u32,
                "purpose",
                &point_index_str,
            )?,
            parse_mandatory(range, row, AiCol::Mandatory1815 as u32),
            parse_mandatory(range, row, AiCol::Mandatory1547 as u32),
        )?;

        let eng_min = point.eng_minimum().0;
        let eng_max = point.eng_maximum().0;
        if original_value < eng_min || original_value > eng_max {
            errors.push(LoadError::at_row(
                "AI",
                &point_index_str,
                "value",
                row + 1,
                format!(
                    "AI point value out of range: value={}, eng_minimum={}, eng_maximum={}. Clamped to valid range.",
                    original_value,
                    eng_min,
                    eng_max
                ),
            ));
        }

        ai_points.push(point);
    }

    Ok(AnalogInputs {
        points: ai_points,
        curves: vec![],
        schedules_bc: vec![],
        schedules: vec![],
        meters: vec![],
        ders: vec![],
        inverters: vec![],
        batteries: vec![],
    })
}

const RIDE_THROUGH_PROFILE_NAMES: &[&str] =
    &["full", "mandatory_1815", "mandatory_1547", "minimal_1547"];

/// Clone an AiPoint but replace its engineering value.
fn clone_point_with_value(template: &AiPoint, value: f64, errors: &mut Vec<LoadError>) -> AiPoint {
    let mut new_point = template.clone();
    new_point
        .set_value(EngineeringF64(value))
        .unwrap_or_else(|e| {
            errors.push(LoadError::post_assembly(
                "AI",
                &template.point_index.to_string(),
                "value",
                format!(
                    "Failed to set value {} on cloned point: {}. Using original value {} instead.",
                    value,
                    e,
                    template.value()
                ),
            ));
        });
    new_point
}

fn find_curve_selector(
    profile_index: &ProfileIndex,
    curves_start: u16,
) -> Option<(ActionType, u16)> {
    if let Some(&ao_idx) = profile_index.ai_to_ao.get(&curves_start) {
        return Some((ActionType::AO, ao_idx));
    }
    if let Some(&bo_idx) = profile_index.bi_to_bo.get(&curves_start) {
        return Some((ActionType::BO, bo_idx));
    }
    None
}

fn find_associated_ao(profile_index: &ProfileIndex, ai_point: &AiPoint) -> Option<u16> {
    profile_index.ai_to_ao.get(&ai_point.point_index).copied()
}

fn find_bo_by_index(profile_index: &ProfileIndex, index: u16) -> Option<u16> {
    profile_index
        .bo_points
        .contains_key(&index)
        .then_some(index)
}

#[allow(clippy::too_many_arguments)]
struct SynthesizeCtx<'a> {
    profile_index: &'a ProfileIndex,
    curve_template: Option<&'a AiCurve>,
    schedule_bc_template: Option<&'a AiScheduleBC>,
    schedule_template: Option<&'a AiSchedule>,
    ao_points_by_index: &'a mut HashMap<u16, AoPoint>,
    errors: &'a mut Vec<LoadError>,
}

#[allow(clippy::too_many_arguments)]
fn synthesize_ride_through(
    ctx: &mut SynthesizeCtx<'_>,
    points: &[(f64, f64)],
    curve_type: CurveType,
    x_units: IndependentVariableUnit,
    y_units: DependentVariableUnit,
    identity_val: f64,
    schedule_type_val: f64,
    bo_uid: BoUid,
    curves_start: u16,
    should_expand: bool,
) -> (Option<AiCurve>, Option<AiScheduleBC>, Option<AiSchedule>) {
    let curve = ctx.curve_template.map(|template| {
        let x_scaling = x_units.scaling();
        let y_scaling = y_units.scaling();

        let curve_type_pt =
            clone_point_with_value(&template.curve_type, curve_type as u8 as f64, ctx.errors);
        let number_of_points_pt =
            clone_point_with_value(&template.number_of_points, points.len() as f64, ctx.errors);
        let x_units_pt =
            clone_point_with_value(&template.x_units, x_units as u8 as f64, ctx.errors);
        let y_units_pt =
            clone_point_with_value(&template.y_units, y_units as u8 as f64, ctx.errors);

        let target_len = if should_expand {
            template.x_values.len()
        } else {
            points.len()
        };
        let mut x_values = Vec::with_capacity(target_len);
        let mut y_values = Vec::with_capacity(target_len);
        for idx in 0..target_len {
            let (x_eng, y_eng) = if idx < points.len() {
                points[idx]
            } else {
                points[points.len() - 1]
            };

            let x_template = match template.x_values.get(idx) {
                Some(p) => p,
                None => {
                    ctx.errors.push(LoadError::post_assembly(
                        "AI",
                        "curve",
                        "x_values",
                        format!("Missing template x_value at index {}", idx),
                    ));
                    continue;
                }
            };
            let mut x_point = x_template.clone();
            if let Some(ref scaling) = x_scaling {
                x_point = with_generated_scaling(
                    &x_point,
                    scaling.min,
                    scaling.max,
                    scaling.mult,
                    EngineeringF64(0.0),
                );
            }
            if let Err(e) = x_point.set_value(EngineeringF64(x_eng)) {
                ctx.errors.extend(e.errors.into_iter().map(LoadError::from));
            }
            x_values.push(x_point);

            let y_template = match template.y_values.get(idx) {
                Some(p) => p,
                None => {
                    ctx.errors.push(LoadError::post_assembly(
                        "AI",
                        "curve",
                        "y_values",
                        format!("Missing template y_value at index {}", idx),
                    ));
                    continue;
                }
            };
            let mut y_point = y_template.clone();
            if let Some(ref scaling) = y_scaling {
                y_point = with_generated_scaling(
                    &y_point,
                    scaling.min,
                    scaling.max,
                    scaling.mult,
                    EngineeringF64(0.0),
                );
            }
            if let Err(e) = y_point.set_value(EngineeringF64(y_eng)) {
                ctx.errors.extend(e.errors.into_iter().map(LoadError::from));
            }
            y_values.push(y_point);
        }

        let mut curve = AiCurve {
            curve_type: curve_type_pt,
            number_of_points: number_of_points_pt,
            x_units: x_units_pt,
            y_units: y_units_pt,
            x_values,
            y_values,
        };
        apply_curve_point_scaling(&mut curve, ctx.errors);

        for x_pt in &curve.x_values {
            if let Some(ao_idx) = ctx.profile_index.ai_to_ao.get(&x_pt.point_index).copied() {
                if let Some(ao_pt) = ctx.ao_points_by_index.get_mut(&ao_idx) {
                    *ao_pt = with_generated_ao_scaling(
                        ao_pt,
                        x_pt.minimum(),
                        x_pt.maximum(),
                        x_pt.multiplier(),
                        x_pt.offset,
                    );
                }
            }
        }
        for y_pt in &curve.y_values {
            if let Some(ao_idx) = ctx.profile_index.ai_to_ao.get(&y_pt.point_index).copied() {
                if let Some(ao_pt) = ctx.ao_points_by_index.get_mut(&ao_idx) {
                    *ao_pt = with_generated_ao_scaling(
                        ao_pt,
                        y_pt.minimum(),
                        y_pt.maximum(),
                        y_pt.multiplier(),
                        y_pt.offset,
                    );
                }
            }
        }

        curve
    });

    let schedule_bc = ctx.schedule_bc_template.map(|template| {
        let identity = clone_point_with_value(&template.identity, identity_val, ctx.errors);
        let priority = clone_point_with_value(&template.priority, 1.0, ctx.errors);
        let schedule_type =
            clone_point_with_value(&template.schedule_type, schedule_type_val, ctx.errors);
        let start_date = clone_point_with_value(&template.start_date, 0.0, ctx.errors);
        let start_time = clone_point_with_value(&template.start_time, 0.0, ctx.errors);
        let repeat_interval = clone_point_with_value(&template.repeat_interval, 0.0, ctx.errors);
        let repeat_interval_units =
            clone_point_with_value(&template.repeat_interval_units, 0.0, ctx.errors);
        let validation_status =
            clone_point_with_value(&template.validation_status, 2.0, ctx.errors);
        let status = clone_point_with_value(&template.status, 2.0, ctx.errors);
        let number_of_points = clone_point_with_value(&template.number_of_points, 1.0, ctx.errors);

        let time_template = template
            .time_offsets
            .first()
            .expect("xlsx schedule_bc has no time_offset");
        let value_template = template
            .values
            .first()
            .expect("xlsx schedule_bc has no value");

        let mut time_point = clone_point_with_value(time_template, 0.0, ctx.errors);
        time_point = with_generated_scaling(
            &time_point,
            time_template.minimum(),
            time_template.maximum(),
            time_template.multiplier(),
            time_template.offset,
        );

        let mut value_point = clone_point_with_value(value_template, 1.0, ctx.errors);
        value_point = with_generated_scaling(
            &value_point,
            value_template.minimum(),
            value_template.maximum(),
            value_template.multiplier(),
            value_template.offset,
        );

        AiScheduleBC {
            identity,
            priority,
            schedule_type,
            start_date,
            start_time,
            repeat_interval,
            repeat_interval_units,
            validation_status,
            status,
            number_of_points,
            time_offsets: vec![time_point],
            values: vec![value_point],
        }
    });

    let schedule = match (ctx.schedule_template, &curve) {
        (Some(template), Some(c)) => {
            let identity = clone_point_with_value(&template.identity, identity_val, ctx.errors);
            let priority = clone_point_with_value(&template.priority, 1.0, ctx.errors);
            let start_date = clone_point_with_value(&template.start_date, 0.0, ctx.errors);
            let start_time = clone_point_with_value(&template.start_time, 0.0, ctx.errors);
            let stop_date = clone_point_with_value(&template.stop_date, 0.0, ctx.errors);
            let stop_time = clone_point_with_value(&template.stop_time, 0.0, ctx.errors);
            let repeat_interval =
                clone_point_with_value(&template.repeat_interval, 0.0, ctx.errors);
            let repeat_interval_units =
                clone_point_with_value(&template.repeat_interval_units, 0.0, ctx.errors);
            let validation_state =
                clone_point_with_value(&template.validation_state, 2.0, ctx.errors);
            let status = clone_point_with_value(&template.status, 2.0, ctx.errors);

            let mut time_offsets = Vec::new();
            let mut action_types = Vec::new();
            let mut action_indexes = Vec::new();
            let mut values = Vec::new();

            let mut push_step = |action_type_val: f64, action_index_val: f64, value_val: f64| {
                let step_idx = time_offsets.len();
                if step_idx >= template.time_offsets.len() {
                    return;
                }
                let time_tpl = &template.time_offsets[step_idx];
                let action_type_tpl = &template.action_types[step_idx];
                let action_index_tpl = &template.action_indexes[step_idx];
                let value_tpl = &template.values[step_idx];

                let mut time_point = clone_point_with_value(time_tpl, 0.0, ctx.errors);
                time_point = with_generated_scaling(
                    &time_point,
                    time_tpl.minimum(),
                    time_tpl.maximum(),
                    time_tpl.multiplier(),
                    time_tpl.offset,
                );
                time_offsets.push(time_point);

                action_types.push(clone_point_with_value(
                    action_type_tpl,
                    action_type_val,
                    ctx.errors,
                ));
                action_indexes.push(clone_point_with_value(
                    action_index_tpl,
                    action_index_val,
                    ctx.errors,
                ));

                let mut value_point = value_tpl.clone();
                if action_type_val == ActionType::AO as u8 as f64 {
                    let ao_idx = action_index_val as u16;
                    if let Some(ao_point) = ctx.ao_points_by_index.get(&ao_idx) {
                        value_point = with_generated_scaling(
                            &value_point,
                            ao_point.minimum(),
                            ao_point.maximum(),
                            ao_point.multiplier(),
                            ao_point.offset,
                        );
                    }
                }
                if let Err(e) = value_point.set_value(EngineeringF64(value_val)) {
                    ctx.errors.extend(e.errors.into_iter().map(LoadError::from));
                }
                values.push(value_point);
            };

            // 1. Selector write
            let (selector_action_type, selector_ao_index) =
                find_curve_selector(ctx.profile_index, curves_start)
                    .map(|(action_type, point_idx)| (action_type as u8 as f64, point_idx as f64))
                    .expect("no AI-to-AO or BI-to-BO mapping exists for curves_start");
            push_step(selector_action_type, selector_ao_index, identity_val);

            // 2. Curve Type write
            if let Some(ao_idx) = find_associated_ao(ctx.profile_index, &c.curve_type) {
                push_step(
                    ActionType::AO as u8 as f64,
                    f64::from(ao_idx),
                    curve_type as u8 as f64,
                );
            }

            // 3. Number of Points write
            if let Some(ao_idx) = find_associated_ao(ctx.profile_index, &c.number_of_points) {
                push_step(
                    ActionType::AO as u8 as f64,
                    f64::from(ao_idx),
                    points.len() as f64,
                );
            }

            // 4. X Units write
            if let Some(ao_idx) = find_associated_ao(ctx.profile_index, &c.x_units) {
                push_step(
                    ActionType::AO as u8 as f64,
                    f64::from(ao_idx),
                    x_units as u8 as f64,
                );
            }

            // 5. Y Units write
            if let Some(ao_idx) = find_associated_ao(ctx.profile_index, &c.y_units) {
                push_step(
                    ActionType::AO as u8 as f64,
                    f64::from(ao_idx),
                    y_units as u8 as f64,
                );
            }

            // Calculate K (number of coordinate slots to write)
            let k = if should_expand {
                let initial_meta_count = 1 // Selector
                    + if find_associated_ao(ctx.profile_index, &c.curve_type).is_some() { 1 } else { 0 }
                    + if find_associated_ao(ctx.profile_index, &c.number_of_points).is_some() { 1 } else { 0 }
                    + if find_associated_ao(ctx.profile_index, &c.x_units).is_some() { 1 } else { 0 }
                    + if find_associated_ao(ctx.profile_index, &c.y_units).is_some() { 1 } else { 0 };

                let final_meta_count =
                    if find_bo_by_index(ctx.profile_index, bo_uid as u16).is_some() {
                        1
                    } else {
                        0
                    };

                let meta_total = initial_meta_count + final_meta_count;
                let max_coords =
                    (template.time_offsets.len() as isize - meta_total as isize).max(0) as usize;
                (max_coords / 2).min(c.x_values.len())
            } else {
                points.len()
            };

            // 6. X values write
            for idx in 0..k {
                let x_eng = if idx < points.len() {
                    points[idx].0
                } else {
                    points[points.len() - 1].0
                };
                if let Some(ao_idx) = find_associated_ao(ctx.profile_index, &c.x_values[idx]) {
                    push_step(ActionType::AO as u8 as f64, f64::from(ao_idx), x_eng);
                }
            }

            // 7. Y values write
            for idx in 0..k {
                let y_eng = if idx < points.len() {
                    points[idx].1
                } else {
                    points[points.len() - 1].1
                };
                if let Some(ao_idx) = find_associated_ao(ctx.profile_index, &c.y_values[idx]) {
                    push_step(ActionType::AO as u8 as f64, f64::from(ao_idx), y_eng);
                }
            }

            // 8. Enable control mode write
            if let Some(bo_idx) = find_bo_by_index(ctx.profile_index, bo_uid as u16) {
                push_step(ActionType::BO as u8 as f64, f64::from(bo_idx), 1.0);
            }

            let number_of_points = clone_point_with_value(
                &template.number_of_points,
                time_offsets.len() as f64,
                ctx.errors,
            );

            let mut schedule = AiSchedule {
                identity,
                priority,
                start_date,
                start_time,
                stop_date,
                stop_time,
                repeat_interval,
                repeat_interval_units,
                validation_state,
                status,
                number_of_points,
                time_offsets,
                action_types,
                action_indexes,
                values,
            };
            apply_schedule_value_scaling(&mut schedule, ctx.ao_points_by_index);
            Some(schedule)
        }
        _ => None,
    };

    (curve, schedule_bc, schedule)
}

/// Generate ride-through curves and schedules by cloning the xlsx-loaded
/// templates and populating them with NERC guideline values and scaling.
#[allow(clippy::too_many_arguments)]
fn synthesize_ride_through_profile(
    validated_profile: Validated<PicsProfile>,
    profile_index: &ProfileIndex,
    profile_name: Option<&str>,
    errors: &mut Vec<LoadError>,
) -> Result<Validated<PicsProfile>, ValidationErrors> {
    let Some(profile_name) = profile_name else {
        return Ok(validated_profile);
    };

    if !RIDE_THROUGH_PROFILE_NAMES.contains(&profile_name) {
        return Ok(validated_profile);
    }

    let mut profile = validated_profile.into_unvalidated();

    // Extract templates before clearing them
    let curve_template = profile.ai.curves.first().cloned();
    profile.ai.curves.clear();

    let schedule_bc_template = profile.ai.schedules_bc.first().cloned();
    profile.ai.schedules_bc.clear();

    let schedule_template = profile.ai.schedules.first().cloned();
    profile.ai.schedules.clear();

    let mut ao_points_by_index: HashMap<u16, AoPoint> = profile
        .ao
        .all_ao_points()
        .into_iter()
        .map(|point| (point.point_index, point.clone()))
        .collect();

    let curves_start = profile.key.curves.ai.start;

    let should_expand = profile_name == "full" || profile_name == "mandatory_1815";

    let mut ctx = SynthesizeCtx {
        profile_index,
        curve_template: curve_template.as_ref(),
        schedule_bc_template: schedule_bc_template.as_ref(),
        schedule_template: schedule_template.as_ref(),
        ao_points_by_index: &mut ao_points_by_index,
        errors,
    };

    // 1. Synthesize HVRT
    let hvrt_points = &[
        (160.0, 120.0),
        (2000.0, 120.0),
        (2000.0, 110.0),
        (100000.0, 110.0),
    ];
    let (hvrt_curve, hvrt_bc, hvrt_sched) = synthesize_ride_through(
        &mut ctx,
        hvrt_points,
        CurveType::HVRTMustTrip,
        IndependentVariableUnit::TimeMs,
        DependentVariableUnit::VoltsPctVRef,
        1.0, // identity_val
        1.0, // schedule_type_val
        BoUid::Volt_Ride_Through_DHVT_Mod,
        curves_start,
        should_expand,
    );
    if let Some(c) = hvrt_curve {
        profile.ai.curves.push(c);
    }
    if let Some(bc) = hvrt_bc {
        profile.ai.schedules_bc.push(bc);
    }
    if let Some(s) = hvrt_sched {
        profile.ai.schedules.push(s);
    }

    // 2. Synthesize LVRT
    let lvrt_points = &[
        (100000.0, 70.0),
        (2000.0, 70.0),
        (2000.0, 45.0),
        (160.0, 45.0),
    ];
    let (lvrt_curve, lvrt_bc, lvrt_sched) = synthesize_ride_through(
        &mut ctx,
        lvrt_points,
        CurveType::LVRTMustTrip,
        IndependentVariableUnit::TimeMs,
        DependentVariableUnit::VoltsPctVRef,
        2.0, // identity_val
        2.0, // schedule_type_val
        BoUid::Volt_Ride_Through_DHVT_Mod,
        curves_start,
        should_expand,
    );
    if let Some(c) = lvrt_curve {
        profile.ai.curves.push(c);
    }
    if let Some(bc) = lvrt_bc {
        profile.ai.schedules_bc.push(bc);
    }
    if let Some(s) = lvrt_sched {
        profile.ai.schedules.push(s);
    }

    // 3. Synthesize HFRT
    let hfrt_points = &[
        (160.0, 62.0),
        (300000.0, 62.0),
        (300000.0, 61.2),
        (3000000.0, 61.2),
    ];
    let (hfrt_curve, hfrt_bc, hfrt_sched) = synthesize_ride_through(
        &mut ctx,
        hfrt_points,
        CurveType::HFRTMustTrip,
        IndependentVariableUnit::TimeMs,
        DependentVariableUnit::FrequencyPctNominal,
        3.0, // identity_val
        5.0, // schedule_type_val
        BoUid::Freq_Ride_Through_DHFT_Mod,
        curves_start,
        should_expand,
    );
    if let Some(c) = hfrt_curve {
        profile.ai.curves.push(c);
    }
    if let Some(bc) = hfrt_bc {
        profile.ai.schedules_bc.push(bc);
    }
    if let Some(s) = hfrt_sched {
        profile.ai.schedules.push(s);
    }

    // 4. Synthesize LFRT
    let lfrt_points = &[
        (3000000.0, 58.5),
        (300000.0, 58.5),
        (300000.0, 56.5),
        (160.0, 56.5),
    ];
    let (lfrt_curve, lfrt_bc, lfrt_sched) = synthesize_ride_through(
        &mut ctx,
        lfrt_points,
        CurveType::LFRTMustTrip,
        IndependentVariableUnit::TimeMs,
        DependentVariableUnit::FrequencyPctNominal,
        4.0, // identity_val
        6.0, // schedule_type_val
        BoUid::Freq_Ride_Through_DHFT_Mod,
        curves_start,
        should_expand,
    );
    if let Some(c) = lfrt_curve {
        profile.ai.curves.push(c);
    }
    if let Some(bc) = lfrt_bc {
        profile.ai.schedules_bc.push(bc);
    }
    if let Some(s) = lfrt_sched {
        profile.ai.schedules.push(s);
    }

    // Write back the updated AO points with the new scaling
    for ao_pt in &mut profile.ao.points {
        if let Some(updated) = ao_points_by_index.get(&ao_pt.point_index) {
            *ao_pt = updated.clone();
        }
    }

    profile.into_validated()
}

fn float_to_u16_code(v: f64) -> Option<u16> {
    if v.is_finite() && v.fract() == 0.0 && (0.0..=f64::from(u16::MAX)).contains(&v) {
        Some(v as u16)
    } else {
        None
    }
}

/// Rebuild an AiPoint with updated scaling (min/max/multiplier/offset) while
/// preserving the existing engineering value. The value field in the xlsx is
/// already an engineering value and must not be recomputed.
fn with_generated_scaling(
    point: &AiPoint,
    minimum: TransmissionI32,
    maximum: TransmissionI32,
    multiplier: f64,
    offset: EngineeringF64,
) -> AiPoint {
    AiPoint::new(
        point.point_index,
        point.name.clone(),
        point.event_class,
        minimum,
        maximum,
        multiplier,
        offset,
        point.units.clone(),
        point.iec_61850_uid.clone(),
        point.value(),
        point.assoc_ao.clone(),
        point.purpose.clone(),
        point.mandatory_1815,
        point.mandatory_1547,
    )
    .expect("generated curve/schedule scaling must use non-zero multipliers")
}

fn with_generated_ao_scaling(
    point: &AoPoint,
    minimum: TransmissionI32,
    maximum: TransmissionI32,
    multiplier: f64,
    offset: EngineeringF64,
) -> AoPoint {
    AoPoint::new(
        point.point_index,
        point.name.clone(),
        minimum,
        maximum,
        multiplier,
        offset,
        point.units.clone(),
        point.iec_61850_uid.clone(),
        point.assoc_ai.clone(),
        point.purpose.clone(),
        point.mandatory_1815,
        point.mandatory_1547,
    )
    .expect("generated curve/schedule scaling must use non-zero multipliers")
}

fn apply_curve_point_scaling(curve: &mut AiCurve, errors: &mut Vec<LoadError>) {
    if let Some(x_var) =
        float_to_curve_code(curve.x_units.value().0).and_then(IndependentVariableUnit::from_code)
    {
        if let Some(scaling) = x_var.scaling() {
            for point in &mut curve.x_values {
                *point = with_generated_scaling(
                    point,
                    scaling.min,
                    scaling.max,
                    scaling.mult,
                    EngineeringF64(0.0),
                );
            }
        }
    }

    if let Some(y_var) =
        float_to_curve_code(curve.y_units.value().0).and_then(DependentVariableUnit::from_code)
    {
        if let Some(scaling) = y_var.scaling() {
            for point in &mut curve.y_values {
                *point = with_generated_scaling(
                    point,
                    scaling.min,
                    scaling.max,
                    scaling.mult,
                    EngineeringF64(0.0),
                );
            }
        }
    }

    curve
        .collect_errors()
        .errors
        .into_iter()
        .for_each(|e| errors.push(LoadError::from(e)));
}

fn apply_schedule_value_scaling(
    schedule: &mut AiSchedule,
    ao_points_by_index: &HashMap<u16, AoPoint>,
) {
    let slot_count = schedule
        .time_offsets
        .len()
        .min(schedule.action_types.len())
        .min(schedule.action_indexes.len())
        .min(schedule.values.len());

    for slot_index in 0..slot_count {
        let action_type = float_to_u16_code(schedule.action_types[slot_index].value().0);
        if action_type != Some(ActionType::AO as u16) {
            continue;
        }

        let ao_point_index = match float_to_u16_code(schedule.action_indexes[slot_index].value().0)
        {
            Some(index) => index,
            None => continue,
        };

        let ao_point = match ao_points_by_index.get(&ao_point_index) {
            Some(point) => point,
            None => continue,
        };

        schedule.values[slot_index] = with_generated_scaling(
            &schedule.values[slot_index],
            ao_point.minimum(),
            ao_point.maximum(),
            ao_point.multiplier(),
            ao_point.offset,
        );
    }
}

pub fn load_ctr_sheet(range: &Range<Data>) -> Result<Vec<CtrPoint>> {
    let mut points = Vec::new();
    let (row_count, _) = range.get_size();

    for row_idx in schema::DATA_ROW_START..row_count {
        let row = row_idx as u32;
        if !is_data_row(range, row, Sheet::Ctr.as_str()) {
            continue;
        }
        let Some(point_index_str) = cell_str(range, row, CtrCol::PointIndex as u32) else {
            continue;
        };
        if !has_integer_index(&point_index_str) {
            continue;
        }
        let point_index = parse_point_index_u16(&point_index_str)?;
        if point_index == u16::MAX {
            continue;
        }
        points.push(CtrPoint {
            name: cell_str_required(range, row, CtrCol::Name as u32, "name", &point_index_str)?,
            counter_event_class: cell_event_class(range, row, CtrCol::CounterEventClass as u32),
            frozen_counter_exists: parse_frozen_counter_exists(
                range,
                row,
                CtrCol::FrozenCounterExists as u32,
            ),
            frozen_counter_event_class: cell_event_class(
                range,
                row,
                CtrCol::FrozenCounterEventClass as u32,
            ),
            iec_61850_uid: cell_str_required(
                range,
                row,
                CtrCol::Iec61850 as u32,
                "iec61850",
                &point_index_str,
            )?,
            purpose: cell_str(range, row, CtrCol::Purpose as u32).unwrap_or_default(),
            mandatory_1815: parse_mandatory(range, row, CtrCol::Mandatory1815 as u32),
            mandatory_1547: parse_mandatory(range, row, CtrCol::Mandatory1547 as u32),
            point_index,
        });
    }

    Ok(points)
}

// ---------------------------------------------------------------------------
// Key-based point reclassification
// ---------------------------------------------------------------------------

/// Log a warning when an equipment group has an unexpected number of points.
fn warn_equipment_group_size(
    errors: &mut Vec<LoadError>,
    sheet: &str,
    equipment_type: &str,
    group_index: usize,
    actual: usize,
    expected: usize,
) {
    if actual != expected {
        errors.push(LoadError::post_assembly(
            sheet,
            &format!("{} #{}", equipment_type, group_index + 1),
            "point_count",
            format!(
                "Expected {} points per {} instance, got {}; group dropped",
                expected, equipment_type, actual
            ),
        ));
    }
}

/// Reclassify BI points from a flat list into equipment-specific groups.
fn reclassify_bi(
    key: &KeySheet,
    points: Vec<BiPoint>,
    errors: &mut Vec<LoadError>,
) -> BinaryInputs {
    let experimental_start = key.experimental.bi.start as i64;
    let meter_start = key.meter.bi.start as i64;
    let der_start = key.der.bi.start as i64;
    let inv_start = key.inverter.bi.start as i64;
    let bat_start = key.battery.bi.start as i64;

    let mut base: Vec<BiPoint> = Vec::new();
    let mut meters_flat: Vec<BiPoint> = Vec::new();
    let mut ders_flat: Vec<BiPoint> = Vec::new();
    let mut inverters_flat: Vec<BiPoint> = Vec::new();
    let mut batteries_flat: Vec<BiPoint> = Vec::new();

    for point in points {
        let idx = point.point_index as i64;
        if idx >= experimental_start {
            base.push(point);
        } else if idx >= bat_start {
            batteries_flat.push(point);
        } else if idx >= inv_start {
            inverters_flat.push(point);
        } else if idx >= der_start {
            ders_flat.push(point);
        } else if idx >= meter_start {
            meters_flat.push(point);
        } else {
            base.push(point);
        }
    }

    let get_num = |p: &BiPoint| Some(p.point_index as i64);

    let meters = group_by_index_gap(meters_flat, get_num)
        .into_iter()
        .enumerate()
        .filter_map(|(group_idx, pts)| {
            if pts.len() != 13 {
                warn_equipment_group_size(errors, "BI", "meter", group_idx, pts.len(), 13);
                return None;
            }
            let mut it = pts.into_iter();
            Some(BiMeter {
                active_power_too_high: it.next()?,
                active_power_too_low: it.next()?,
                reactive_power_too_high: it.next()?,
                reactive_power_too_low: it.next()?,
                power_factor_too_high: it.next()?,
                power_factor_too_low: it.next()?,
                phase_a_voltage_too_high: it.next()?,
                phase_a_voltage_too_low: it.next()?,
                phase_b_voltage_too_high: it.next()?,
                phase_b_voltage_too_low: it.next()?,
                phase_c_voltage_too_high: it.next()?,
                phase_c_voltage_too_low: it.next()?,
                communication_error: it.next()?,
            })
        })
        .collect();

    let ders = group_by_index_gap(ders_flat, get_num)
        .into_iter()
        .enumerate()
        .filter_map(|(group_idx, pts)| {
            if pts.len() != 4 {
                warn_equipment_group_size(errors, "BI", "DER unit", group_idx, pts.len(), 4);
                return None;
            }
            let mut it = pts.into_iter();
            Some(BiDer {
                maintenance_operational_state: it.next()?,
                has_p1_alarms: it.next()?,
                has_p2_alarms: it.next()?,
                has_p3_alarms: it.next()?,
            })
        })
        .collect();

    let inverters = group_by_index_gap(inverters_flat, get_num)
        .into_iter()
        .enumerate()
        .filter_map(|(group_idx, pts)| {
            if pts.len() != 35 {
                warn_equipment_group_size(errors, "BI", "inverter", group_idx, pts.len(), 35);
                return None;
            }
            let mut it = pts.into_iter();
            Some(BiInverter {
                active_power_too_high: it.next()?,
                active_power_too_low: it.next()?,
                reactive_power_too_high: it.next()?,
                reactive_power_too_low: it.next()?,
                frequency_too_high: it.next()?,
                frequency_too_low: it.next()?,
                dc_input_power_too_high: it.next()?,
                dc_input_power_too_low: it.next()?,
                dc_current_too_high: it.next()?,
                dc_current_too_low: it.next()?,
                dc_voltage_too_high: it.next()?,
                dc_voltage_too_low: it.next()?,
                power_factor_excitation: it.next()?,
                communication_error: it.next()?,
                local_control_mode: it.next()?,
                dc_contactor_closed: it.next()?,
                ground_fault_alarm: it.next()?,
                dc_over_voltage_alarm: it.next()?,
                dc_under_voltage_alarm: it.next()?,
                ac_disconnect_warning: it.next()?,
                dc_disconnect_warning: it.next()?,
                grid_disconnect_warning: it.next()?,
                cabinet_open_warning: it.next()?,
                manual_shutdown_warning: it.next()?,
                over_temperature_alarm: it.next()?,
                under_temperature_alarm: it.next()?,
                over_frequency_alarm: it.next()?,
                under_frequency_alarm: it.next()?,
                ac_over_voltage_alarm: it.next()?,
                ac_under_voltage_alarm: it.next()?,
                blown_string_fuse_alarm: it.next()?,
                memory_loss_alarm: it.next()?,
                hardware_test_failure: it.next()?,
                other_alarm: it.next()?,
                other_warning: it.next()?,
            })
        })
        .collect();

    let batteries = group_by_index_gap(batteries_flat, get_num)
        .into_iter()
        .enumerate()
        .filter_map(|(group_idx, pts)| {
            if pts.len() != 54 {
                warn_equipment_group_size(errors, "BI", "battery", group_idx, pts.len(), 54);
                return None;
            }
            let mut it = pts.into_iter();
            Some(BiBattery {
                status_of_storage: it.next()?,
                communication_error: it.next()?,
                local_control_mode: it.next()?,
                dc_contactor_closed: it.next()?,
                is_charging: it.next()?,
                is_discharging: it.next()?,
                external_voltage_too_high: it.next()?,
                external_voltage_too_low: it.next()?,
                internal_voltage_too_high: it.next()?,
                internal_voltage_too_low: it.next()?,
                over_temperature_alarm: it.next()?,
                under_temperature_alarm: it.next()?,
                temperature_imbalance_alarm: it.next()?,
                over_temperature_warning: it.next()?,
                under_temperature_warning: it.next()?,
                temperature_imbalance_warning: it.next()?,
                over_charge_current_alarm: it.next()?,
                over_discharge_current_alarm: it.next()?,
                over_charge_current_warning: it.next()?,
                over_discharge_current_warning: it.next()?,
                voltage_imbalance_warning: it.next()?,
                current_imbalance_warning: it.next()?,
                over_voltage_alarm: it.next()?,
                under_voltage_alarm: it.next()?,
                over_voltage_warning: it.next()?,
                under_voltage_warning: it.next()?,
                over_soc_max_alarm: it.next()?,
                under_soc_min_alarm: it.next()?,
                over_soc_max_warning: it.next()?,
                under_soc_min_warning: it.next()?,
                contactor_failure: it.next()?,
                fan_error: it.next()?,
                ground_fault: it.next()?,
                door_open_alarm: it.next()?,
                configuration_error: it.next()?,
                configuration_warning: it.next()?,
                other_alarm: it.next()?,
                other_warning: it.next()?,
                fire_alarm: it.next()?,
                fire_supervisory_warning: it.next()?,
                fire_trouble_warning: it.next()?,
                fire_power_fault_warning: it.next()?,
                chiller_alarm: it.next()?,
                chiller_warning: it.next()?,
                air_handler_alarm: it.next()?,
                air_handler_warning: it.next()?,
                fluid_alarm: it.next()?,
                fluid_warning: it.next()?,
                gas_alarm: it.next()?,
                gas_warning: it.next()?,
                electrolyte_alarm: it.next()?,
                electrolyte_warning: it.next()?,
                electrical_alarm: it.next()?,
                electrical_warning: it.next()?,
            })
        })
        .collect();

    BinaryInputs {
        points: base,
        meters,
        ders,
        inverters,
        batteries,
    }
}

/// Reclassify AO points from a flat list into equipment-specific groups.
fn reclassify_ao(
    key: &KeySheet,
    points: Vec<AoPoint>,
    errors: &mut Vec<LoadError>,
) -> AnalogOutputs {
    let experimental_start = key.experimental.ao.start as i64;
    let meter_start = key.meter.ao.start as i64;
    let inv_start = key.inverter.ao.start as i64;
    let bat_start = key.battery.ao.start as i64;

    let mut base: Vec<AoPoint> = Vec::new();
    let mut meters_flat: Vec<AoPoint> = Vec::new();
    let mut inverters_flat: Vec<AoPoint> = Vec::new();
    let mut batteries_flat: Vec<AoPoint> = Vec::new();

    for point in points {
        let idx = point.point_index as i64;
        if idx >= experimental_start {
            base.push(point);
        } else if idx >= bat_start {
            batteries_flat.push(point);
        } else if idx >= inv_start {
            inverters_flat.push(point);
        } else if idx >= meter_start {
            meters_flat.push(point);
        } else {
            base.push(point);
        }
    }

    let get_num = |p: &AoPoint| Some(p.point_index as i64);

    let meters = group_by_index_gap(meters_flat, get_num)
        .into_iter()
        .enumerate()
        .filter_map(|(group_idx, pts)| {
            if pts.len() != 12 {
                warn_equipment_group_size(errors, "AO", "meter", group_idx, pts.len(), 12);
                return None;
            }
            let mut it = pts.into_iter();
            Some(AoMeter {
                active_power_high_threshold: it.next()?,
                active_power_low_threshold: it.next()?,
                reactive_power_high_threshold: it.next()?,
                reactive_power_low_threshold: it.next()?,
                power_factor_high_threshold: it.next()?,
                power_factor_low_threshold: it.next()?,
                phase_a_volts_high_threshold: it.next()?,
                phase_a_volts_low_threshold: it.next()?,
                phase_b_volts_high_threshold: it.next()?,
                phase_b_volts_low_threshold: it.next()?,
                phase_c_volts_high_threshold: it.next()?,
                phase_c_volts_low_threshold: it.next()?,
            })
        })
        .collect();

    let inverters = group_by_index_gap(inverters_flat, get_num)
        .into_iter()
        .enumerate()
        .filter_map(|(group_idx, pts)| {
            if pts.len() != 12 {
                warn_equipment_group_size(errors, "AO", "inverter", group_idx, pts.len(), 12);
                return None;
            }
            let mut it = pts.into_iter();
            Some(AoInverter {
                active_power_high_threshold: it.next()?,
                active_power_low_threshold: it.next()?,
                reactive_power_high_threshold: it.next()?,
                reactive_power_low_threshold: it.next()?,
                frequency_high_threshold: it.next()?,
                frequency_low_threshold: it.next()?,
                dc_input_power_high_threshold: it.next()?,
                dc_input_power_low_threshold: it.next()?,
                dc_current_high_threshold: it.next()?,
                dc_current_low_threshold: it.next()?,
                dc_voltage_high_threshold: it.next()?,
                dc_voltage_low_threshold: it.next()?,
            })
        })
        .collect();

    let batteries = group_by_index_gap(batteries_flat, get_num)
        .into_iter()
        .enumerate()
        .filter_map(|(group_idx, pts)| {
            if pts.len() != 4 {
                warn_equipment_group_size(errors, "AO", "battery", group_idx, pts.len(), 4);
                return None;
            }
            let mut it = pts.into_iter();
            Some(AoBattery {
                external_voltage_high_threshold: it.next()?,
                external_voltage_low_threshold: it.next()?,
                internal_voltage_high_threshold: it.next()?,
                internal_voltage_low_threshold: it.next()?,
            })
        })
        .collect();

    AnalogOutputs {
        points: base,
        meters,
        inverters,
        batteries,
    }
}

/// Reclassify AI points from a flat list into equipment-specific groups,
/// curves, and schedules.
fn reclassify_ai(
    key: &KeySheet,
    points: Vec<AiPoint>,
    errors: &mut Vec<LoadError>,
    ao_points_by_index: &HashMap<u16, AoPoint>,
) -> AnalogInputs {
    let experimental_start = key.experimental.ai.start as i64;
    let curves_start = key.curves.ai.start as i64;
    let system_meter_start = key.system_meter.ai.start as i64;
    let schedule_bc_start = key.schedules_bc.ai.start as i64;
    let schedule_bc_status_start = key.schedules_bc_status.ai.start as i64;
    let schedule_start = key.schedules.ai.start as i64;
    let schedule_status_start = key.schedules_status.ai.start as i64;
    let meter_start = key.meter.ai.start as i64;
    let der_start = key.der.ai.start as i64;
    let inv_start = key.inverter.ai.start as i64;
    let bat_start = key.battery.ai.start as i64;

    let mut base: Vec<AiPoint> = Vec::new();
    let mut curves_flat: Vec<AiPoint> = Vec::new();
    let mut schedules_bc_flat: Vec<AiPoint> = Vec::new();
    let mut schedules_flat: Vec<AiPoint> = Vec::new();
    let mut meters_flat: Vec<AiPoint> = Vec::new();
    let mut ders_flat: Vec<AiPoint> = Vec::new();
    let mut inverters_flat: Vec<AiPoint> = Vec::new();
    let mut batteries_flat: Vec<AiPoint> = Vec::new();

    for point in points {
        let idx = point.point_index as i64;
        if idx >= experimental_start {
            base.push(point);
        } else if idx >= curves_start && idx < system_meter_start {
            curves_flat.push(point);
        } else if idx >= schedule_bc_start && idx < schedule_bc_status_start {
            schedules_bc_flat.push(point);
        } else if idx >= schedule_bc_status_start && idx < schedule_start {
            base.push(point);
        } else if idx >= schedule_start && idx < schedule_status_start {
            schedules_flat.push(point);
        } else if idx >= schedule_status_start && idx < meter_start {
            base.push(point);
        } else if idx >= bat_start {
            batteries_flat.push(point);
        } else if idx >= inv_start {
            inverters_flat.push(point);
        } else if idx >= der_start {
            ders_flat.push(point);
        } else if idx >= meter_start {
            meters_flat.push(point);
        } else {
            base.push(point);
        }
    }

    for point in &mut base {
        let idx = point.point_index as i64;
        if idx >= schedule_bc_status_start && idx < schedule_start {
            if (idx - schedule_bc_status_start) % 3 == 0 {
                let sched_num = (idx - schedule_bc_status_start) / 3 + 1;
                let default_status = if (1..=4).contains(&sched_num) {
                    2.0
                } else {
                    1.0
                };
                let _ = point.set_value(EngineeringF64(default_status));
            }
        } else if idx >= schedule_status_start && idx < meter_start {
            if (idx - schedule_status_start) % 3 == 0 {
                let sched_num = (idx - schedule_status_start) / 3 + 1;
                let default_status = if (1..=4).contains(&sched_num) {
                    2.0
                } else {
                    1.0
                };
                let _ = point.set_value(EngineeringF64(default_status));
            }
        }
    }

    let get_num = |p: &AiPoint| Some(p.point_index as i64);

    let meters = group_by_index_gap(meters_flat, get_num)
        .into_iter()
        .enumerate()
        .filter_map(|(group_idx, pts)| {
            if pts.len() != 37 {
                warn_equipment_group_size(errors, "AI", "meter", group_idx, pts.len(), 37);
                return None;
            }
            let mut it = pts.into_iter();
            Some(AiMeter {
                type_of_connection_point: it.next()?,
                der_input_output_included: it.next()?,
                type_of_circuit_phases: it.next()?,
                apparent_power_calc_method: it.next()?,
                frequency: it.next()?,
                active_power: it.next()?,
                active_power_a: it.next()?,
                active_power_b: it.next()?,
                active_power_c: it.next()?,
                reactive_power: it.next()?,
                reactive_power_a: it.next()?,
                reactive_power_b: it.next()?,
                reactive_power_c: it.next()?,
                power_factor: it.next()?,
                apparent_power: it.next()?,
                phase_a_volts: it.next()?,
                phase_a_angle: it.next()?,
                phase_b_volts: it.next()?,
                phase_b_angle: it.next()?,
                phase_c_volts: it.next()?,
                phase_c_angle: it.next()?,
                avg_line_to_line_voltage: it.next()?,
                current_a: it.next()?,
                current_b: it.next()?,
                current_c: it.next()?,
                active_power_high_threshold: it.next()?,
                active_power_low_threshold: it.next()?,
                reactive_power_high_threshold: it.next()?,
                reactive_power_low_threshold: it.next()?,
                power_factor_high_threshold: it.next()?,
                power_factor_low_threshold: it.next()?,
                phase_a_volts_high_threshold: it.next()?,
                phase_a_volts_low_threshold: it.next()?,
                phase_b_volts_high_threshold: it.next()?,
                phase_b_volts_low_threshold: it.next()?,
                phase_c_volts_high_threshold: it.next()?,
                phase_c_volts_low_threshold: it.next()?,
            })
        })
        .collect();

    let ders = group_by_index_gap(ders_flat, get_num)
        .into_iter()
        .enumerate()
        .filter_map(|(group_idx, pts)| {
            if pts.len() != 15 {
                warn_equipment_group_size(errors, "AI", "DER unit", group_idx, pts.len(), 15);
                return None;
            }
            let mut it = pts.into_iter();
            Some(AiDer {
                unit_type: it.next()?,
                nameplate_energy_capacity: it.next()?,
                normal_operating_performance_category: it.next()?,
                abnormal_operating_performance_category: it.next()?,
                max_apparent_generation_power: it.next()?,
                max_apparent_charging_power: it.next()?,
                operational_time: it.next()?,
                connection_time: it.next()?,
                available_active_generation_power: it.next()?,
                available_active_charging_power: it.next()?,
                available_reactive_injection_power: it.next()?,
                available_reactive_absorption_power: it.next()?,
                non_impacting_injection_vars: it.next()?,
                non_impacting_absorption_vars: it.next()?,
                link_to_meter: it.next()?,
            })
        })
        .collect();

    let inverters = group_by_index_gap(inverters_flat, get_num)
        .into_iter()
        .enumerate()
        .filter_map(|(group_idx, pts)| {
            if pts.len() != 34 {
                warn_equipment_group_size(errors, "AI", "inverter", group_idx, pts.len(), 34);
                return None;
            }
            let mut it = pts.into_iter();
            Some(AiInverter {
                apparent_power_calc_method: it.next()?,
                active_power_target: it.next()?,
                reactive_power_target: it.next()?,
                active_power: it.next()?,
                reactive_power: it.next()?,
                power_factor: it.next()?,
                apparent_power: it.next()?,
                dc_input_power: it.next()?,
                dc_voltage: it.next()?,
                dc_current: it.next()?,
                avg_line_to_neutral_voltage: it.next()?,
                voltage_phase_a_to_b: it.next()?,
                voltage_phase_b_to_c: it.next()?,
                voltage_phase_c_to_a: it.next()?,
                ac_current: it.next()?,
                current_phase_a: it.next()?,
                current_phase_b: it.next()?,
                current_phase_c: it.next()?,
                internal_temperature: it.next()?,
                heat_sink_temperature: it.next()?,
                transformer_temperature: it.next()?,
                active_power_high_threshold: it.next()?,
                active_power_low_threshold: it.next()?,
                reactive_power_high_threshold: it.next()?,
                reactive_power_low_threshold: it.next()?,
                frequency_high_threshold: it.next()?,
                frequency_low_threshold: it.next()?,
                dc_input_power_high_threshold: it.next()?,
                dc_input_power_low_threshold: it.next()?,
                dc_current_high_threshold: it.next()?,
                dc_current_low_threshold: it.next()?,
                dc_voltage_high_threshold: it.next()?,
                dc_voltage_low_threshold: it.next()?,
                link_to_der_unit: it.next()?,
            })
        })
        .collect();

    let batteries = group_by_index_gap(batteries_flat, get_num)
        .into_iter()
        .enumerate()
        .filter_map(|(group_idx, pts)| {
            if pts.len() != 28 {
                warn_equipment_group_size(errors, "AI", "battery", group_idx, pts.len(), 28);
                return None;
            }
            let mut it = pts.into_iter();
            Some(AiBattery {
                type_of_storage: it.next()?,
                nameplate_actual_capacity: it.next()?,
                effective_capacity: it.next()?,
                minimum_reserve: it.next()?,
                maximum_reserve: it.next()?,
                battery_state: it.next()?,
                actual_state_of_charge: it.next()?,
                state_of_health: it.next()?,
                external_voltage: it.next()?,
                internal_voltage: it.next()?,
                current: it.next()?,
                power: it.next()?,
                min_cell_voltage: it.next()?,
                max_cell_voltage: it.next()?,
                min_temperature: it.next()?,
                max_temperature: it.next()?,
                external_ambient_temperature: it.next()?,
                internal_ambient_temperature: it.next()?,
                charge_current_limit: it.next()?,
                discharge_current_limit: it.next()?,
                min_voltage_limit: it.next()?,
                max_voltage_limit: it.next()?,
                connected_string_count: it.next()?,
                external_voltage_high_threshold: it.next()?,
                external_voltage_low_threshold: it.next()?,
                internal_voltage_high_threshold: it.next()?,
                internal_voltage_low_threshold: it.next()?,
                link_to_inverter: it.next()?,
            })
        })
        .collect();

    let schedules_bc: Vec<AiScheduleBC> = if schedules_bc_flat.len() >= 12 {
        let mut it = schedules_bc_flat.into_iter();
        let running_schedule_index = it.next().expect("len >= 12 verified");
        base.push(running_schedule_index);
        let schedule_bc_edit_selector = it.next().expect("len >= 12 verified");
        base.push(schedule_bc_edit_selector);
        let identity = it.next().expect("len >= 12 verified");
        let priority = it.next().expect("len >= 12 verified");
        let schedule_type = it.next().expect("len >= 12 verified");
        let start_date = it.next().expect("len >= 12 verified");
        let start_time = it.next().expect("len >= 12 verified");
        let repeat_interval = it.next().expect("len >= 12 verified");
        let repeat_interval_units = it.next().expect("len >= 12 verified");
        let mut validation_status = it.next().expect("len >= 12 verified");
        let _ = validation_status.set_value(EngineeringF64(1.0));
        let mut status = it.next().expect("len >= 12 verified");
        let _ = status.set_value(EngineeringF64(1.0));
        let number_of_points = it.next().expect("len >= 12 verified");
        let mut time_offsets = Vec::new();
        let mut values = Vec::new();
        while let (Some(t), Some(v)) = (it.next(), it.next()) {
            time_offsets.push(t);
            values.push(v);
        }
        vec![AiScheduleBC {
            identity,
            priority,
            schedule_type,
            start_date,
            start_time,
            repeat_interval,
            repeat_interval_units,
            validation_status,
            status,
            number_of_points,
            time_offsets,
            values,
        }]
    } else {
        vec![]
    };

    let schedules: Vec<AiSchedule> = if schedules_flat.len() >= 12 {
        let mut it = schedules_flat.into_iter();
        let schedule_edit_selector = it.next().expect("len >= 12 verified");
        base.push(schedule_edit_selector);
        let identity = it.next().expect("len >= 12 verified");
        let priority = it.next().expect("len >= 12 verified");
        let start_date = it.next().expect("len >= 12 verified");
        let start_time = it.next().expect("len >= 12 verified");
        let stop_date = it.next().expect("len >= 12 verified");
        let stop_time = it.next().expect("len >= 12 verified");
        let repeat_interval = it.next().expect("len >= 12 verified");
        let repeat_interval_units = it.next().expect("len >= 12 verified");
        let mut validation_state = it.next().expect("len >= 12 verified");
        let _ = validation_state.set_value(EngineeringF64(1.0));
        let mut status = it.next().expect("len >= 12 verified");
        let _ = status.set_value(EngineeringF64(1.0));
        let number_of_points = it.next().expect("len >= 12 verified");
        let mut time_offsets = Vec::new();
        let mut action_types = Vec::new();
        let mut action_indexes = Vec::new();
        let mut values = Vec::new();
        while let (Some(t), Some(at), Some(ai_idx), Some(v)) =
            (it.next(), it.next(), it.next(), it.next())
        {
            time_offsets.push(t);
            action_types.push(at);
            action_indexes.push(ai_idx);
            values.push(v);
        }
        let mut schedule = AiSchedule {
            identity,
            priority,
            start_date,
            start_time,
            stop_date,
            stop_time,
            repeat_interval,
            repeat_interval_units,
            validation_state,
            status,
            number_of_points,
            time_offsets,
            action_types,
            action_indexes,
            values,
        };
        apply_schedule_value_scaling(&mut schedule, ao_points_by_index);
        vec![schedule]
    } else {
        vec![]
    };

    let curves: Vec<AiCurve> = if curves_flat.len() >= 5 {
        let mut it = curves_flat.into_iter();
        let curve_edit_selector = it.next().expect("len >= 5 verified");
        base.push(curve_edit_selector);
        let curve_type = it.next().expect("len >= 5 verified");
        let number_of_points = it.next().expect("len >= 5 verified");
        let x_units = it.next().expect("len >= 5 verified");
        let y_units = it.next().expect("len >= 5 verified");
        let mut x_values = Vec::new();
        let mut y_values = Vec::new();
        while let (Some(x), Some(y)) = (it.next(), it.next()) {
            x_values.push(x);
            y_values.push(y);
        }
        let mut curve = AiCurve {
            curve_type,
            number_of_points,
            x_units,
            y_units,
            x_values,
            y_values,
        };
        apply_curve_point_scaling(&mut curve, errors);
        vec![curve]
    } else {
        vec![]
    };

    AnalogInputs {
        points: base,
        curves,
        schedules_bc,
        schedules,
        meters,
        ders,
        inverters,
        batteries,
    }
}

/// Reclassify all points from flat `points` lists into their correct structured groups
/// using the Key sheet boundaries. Called after all sheets are loaded flat.
fn reclassify_by_key(
    profile: Validated<PicsProfile>,
    errors: &mut Vec<LoadError>,
    profile_name: Option<&str>,
) -> Result<Validated<PicsProfile>, ValidationErrors> {
    let key = profile.key.clone();
    let mut profile = profile.into_unvalidated();

    profile.bi = reclassify_bi(&key, profile.bi.points, errors);
    profile.ao = reclassify_ao(&key, profile.ao.points, errors);
    let mut ao_points_by_index: HashMap<u16, AoPoint> = profile
        .ao
        .all_ao_points()
        .into_iter()
        .map(|point| (point.point_index, point.clone()))
        .collect();

    profile.ai = reclassify_ai(&key, profile.ai.points, errors, &ao_points_by_index);

    // Propagate scaling from AI curves to their associated AO points
    let temp_validated = profile.clone().into_validated()?;
    let profile_index = ProfileIndex::from_profile(&temp_validated);
    for curve in &profile.ai.curves {
        for x_pt in &curve.x_values {
            if let Some(ao_idx) = profile_index.ai_to_ao.get(&x_pt.point_index).copied() {
                if let Some(ao_pt) = ao_points_by_index.get_mut(&ao_idx) {
                    *ao_pt = with_generated_ao_scaling(
                        ao_pt,
                        x_pt.minimum(),
                        x_pt.maximum(),
                        x_pt.multiplier(),
                        x_pt.offset,
                    );
                }
            }
        }
        for y_pt in &curve.y_values {
            if let Some(ao_idx) = profile_index.ai_to_ao.get(&y_pt.point_index).copied() {
                if let Some(ao_pt) = ao_points_by_index.get_mut(&ao_idx) {
                    *ao_pt = with_generated_ao_scaling(
                        ao_pt,
                        y_pt.minimum(),
                        y_pt.maximum(),
                        y_pt.multiplier(),
                        y_pt.offset,
                    );
                }
            }
        }
    }

    // Update profile.ao.points with the new scaling
    for ao_pt in &mut profile.ao.points {
        if let Some(updated) = ao_points_by_index.get(&ao_pt.point_index) {
            *ao_pt = updated.clone();
        }
    }

    // Now scale the schedules using the updated ao_points_by_index!
    for schedule in &mut profile.ai.schedules {
        apply_schedule_value_scaling(schedule, &ao_points_by_index);
    }

    let validated_profile = profile.into_validated()?;

    let profile_index = ProfileIndex::from_profile(&validated_profile);

    synthesize_ride_through_profile(validated_profile, &profile_index, profile_name, errors)
}

// ---------------------------------------------------------------------------
// Top-level XLSX profile loader
// ---------------------------------------------------------------------------

pub fn load_xlsx_profile(path: &Path) -> Result<Validated<PicsProfile>, ValidationErrors> {
    let wb = load_workbook(path).map_err(|e| {
        ValidationErrors::from_error(ValidationError {
            point: "N/A".to_string(),
            message: format!("Could not load XLSX profile: {}", e),
        })
    })?;
    let profile_name = path.file_stem().and_then(|stem| stem.to_str());
    workbook_to_profile_with_name(wb, profile_name)
}

/// When called from the web server (uploaded bytes, no filename), pass `None`
/// for `profile_name`; ride-through curve synthesis is skipped for unknown
/// profile types.
pub fn workbook_to_profile(wb: Workbook) -> Result<Validated<PicsProfile>, ValidationErrors> {
    workbook_to_profile_with_name(wb, None)
}

fn workbook_to_profile_with_name(
    wb: Workbook,
    profile_name: Option<&str>,
) -> Result<Validated<PicsProfile>, ValidationErrors> {
    crate::validators::validate_workbook_structure(&wb)
        .with_context(|| "Workbook failed structural validation")
        .map_err(|e| {
            ValidationErrors::from_error(ValidationError {
                point: "N/A".to_string(),
                message: format!("Workbook failed structural validation: {}", e),
            })
        })?;

    let key = get_worksheet(&wb, Sheet::Key.as_str())
        .and_then(load_key_sheet)
        .ok_or_else(|| {
            ValidationErrors::from_error(ValidationError {
                point: "Key".to_string(),
                message: "Missing or invalid Key sheet".to_string(),
            })
        })?;

    let bo = get_worksheet(&wb, Sheet::Bo.as_str())
        .map(load_bo_sheet)
        .transpose()?
        .unwrap_or_default();

    let bi = get_worksheet(&wb, Sheet::Bi.as_str())
        .map(load_bi_sheet)
        .transpose()?
        .unwrap_or_default();

    let mut errors: Vec<LoadError> = Vec::new();

    let ao = get_worksheet(&wb, Sheet::Ao.as_str())
        .map(|r| load_ao_sheet(r, &mut errors))
        .transpose()?
        .unwrap_or_default();

    let ai = get_worksheet(&wb, Sheet::Ai.as_str())
        .map(|r| load_ai_sheet(r, &mut errors))
        .transpose()?
        .unwrap_or_default();

    let ctr = get_worksheet(&wb, Sheet::Ctr.as_str())
        .map(load_ctr_sheet)
        .transpose()?
        .unwrap_or_default();

    let pre_reclassify = PicsProfile::new(key, bo, bi, ao, ai, ctr)?;
    let reclassified = reclassify_by_key(pre_reclassify, &mut errors, profile_name)?;

    if !errors.is_empty() {
        Err(ValidationErrors::from(errors))
    } else {
        Ok(reclassified)
    }
}

// ---------------------------------------------------------------------------
// JSON profile loader
// ---------------------------------------------------------------------------

pub fn load_json_profile(path: &Path) -> Result<Validated<PicsProfile>, ValidationErrors> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Could not read JSON profile: {}", path.display()))?;
    let profile: Validated<PicsProfile> = PicsProfile::into_validated(
        serde_json::from_str(&content)
            .with_context(|| format!("Could not parse JSON profile: {}", path.display()))?,
    )?;
    Ok(profile)
}

#[cfg(test)]
mod tests {
    use super::*;
    use calamine::{Data, Range};

    #[test]
    fn test_cell_i32_required_overflow() {
        let mut range = Range::new((0, 0), (1, 1));
        let mut errors = Vec::new();

        // Test Int overflow
        range.set_value((0, 0), Data::Int(i64::MAX));
        let val = cell_i32_required(
            &range,
            0,
            0,
            "test_field",
            "100",
            "test_sheet",
            &mut errors,
            999,
        );
        assert_eq!(val, 999);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("is outside i32 range"));

        // Test Float overflow
        range.set_value((0, 0), Data::Float(1e15f64));
        let mut errors = Vec::new();
        let val = cell_i32_required(
            &range,
            0,
            0,
            "test_field",
            "100",
            "test_sheet",
            &mut errors,
            999,
        );
        assert_eq!(val, 999);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("is outside i32 range"));

        // Test String parsing but overflow
        range.set_value((0, 0), Data::String("100000000000000".to_string()));
        let mut errors = Vec::new();
        let val = cell_i32_required(
            &range,
            0,
            0,
            "test_field",
            "100",
            "test_sheet",
            &mut errors,
            999,
        );
        assert_eq!(val, 999);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("is outside i32 range"));
    }

    #[test]
    fn test_load_key_sheet_u16_overflow() {
        let mut range = Range::new((0, 0), (35, 10));
        // Put "Start Index" at header row to satisfy schema check
        range.set_value(
            (schema::KEY_HEADER_ROW, schema::KEY_START_COL),
            Data::String("Start Index".to_string()),
        );

        // Put an overflow value in config.bo.start (row 4, column KEY_BO_COL = 3)
        range.set_value((4, schema::KEY_BO_COL), Data::Int(70000));

        let key_sheet = load_key_sheet(&range).expect("should load mock key sheet");
        // It should fall back to 0
        assert_eq!(key_sheet.config.bo.start, 0);

        // Put an underflow value in config.bi.start (row 4, column KEY_BI_COL = 4)
        range.set_value((4, schema::KEY_BI_COL), Data::Int(-1));

        let key_sheet = load_key_sheet(&range).expect("should load mock key sheet");
        // It should fall back to 0
        assert_eq!(key_sheet.config.bi.start, 0);

        // Put a valid value in config.bi.start
        range.set_value((4, schema::KEY_BI_COL), Data::Int(123));

        let key_sheet = load_key_sheet(&range).expect("should load mock key sheet");
        assert_eq!(key_sheet.config.bi.start, 123);
    }
}
