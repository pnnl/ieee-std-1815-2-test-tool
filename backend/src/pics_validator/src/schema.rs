/// Centralised cell layout schema for all XLSX files produced and consumed by this tool.
///
/// All indices are 0-based (calamine convention).
/// Column enum discriminants equal the 0-based column index; cast with `as u32`.
// ---------------------------------------------------------------------------
// Data row start
// ---------------------------------------------------------------------------
/// First data row index (0-based calamine).  Rows 0 and 1 are header rows.
pub const DATA_ROW_START: usize = 2;

// ---------------------------------------------------------------------------
// Sheet enum
// ---------------------------------------------------------------------------

/// All sheets present in a PICS workbook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sheet {
    Key,
    Bo,
    Bi,
    Ao,
    Ai,
    Ctr,
}

impl Sheet {
    /// All required sheets in a PICS workbook.
    pub const ALL: &'static [Sheet] = &[
        Sheet::Key,
        Sheet::Bo,
        Sheet::Bi,
        Sheet::Ao,
        Sheet::Ai,
        Sheet::Ctr,
    ];

    /// Data sheets (all except Key).
    pub const DATA: &'static [Sheet] = &[Sheet::Bo, Sheet::Bi, Sheet::Ao, Sheet::Ai, Sheet::Ctr];

    /// The sheet name string as it appears in the XLSX workbook.
    pub fn as_str(self) -> &'static str {
        match self {
            Sheet::Key => "Key",
            Sheet::Bo => "BO",
            Sheet::Bi => "BI",
            Sheet::Ao => "AO",
            Sheet::Ai => "AI",
            Sheet::Ctr => "CTR",
        }
    }

    /// Minimum number of columns expected in this sheet, used by structural validation.
    /// Returns `None` for the Key sheet (not validated by column count).
    pub fn expected_col_count(self) -> Option<usize> {
        match self {
            Sheet::Key => None,
            Sheet::Bo => Some(10),
            Sheet::Bi => Some(11),
            Sheet::Ao => Some(15),
            Sheet::Ai => Some(17),
            Sheet::Ctr => Some(10),
        }
    }

    /// Expected header cell specs for structural validation.
    pub fn headers(self) -> &'static [HeaderSpec] {
        match self {
            Sheet::Bo => BO_HEADERS,
            Sheet::Bi => BI_HEADERS,
            Sheet::Ao => AO_HEADERS,
            Sheet::Ai => AI_HEADERS,
            Sheet::Ctr => CTR_HEADERS,
            Sheet::Key => &[],
        }
    }
}

// ---------------------------------------------------------------------------
// Column enums for each data sheet
// ---------------------------------------------------------------------------
//
// Each variant's discriminant equals its 0-based column index.
// Use `EnumVariant as u32` at call sites (calamine convention).

/// Column indices for the BO sheet.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoCol {
    PointIndex = 0,
    Name = 1,
    State0 = 2,
    State1 = 3,
    Iec61850 = 4,
    AssocBi = 5,
    Purpose = 6,
    Mandatory1815 = 7,
    Mandatory1547 = 8,
}

/// Column indices for the BI sheet.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BiCol {
    PointIndex = 0,
    Name = 1,
    EventClass = 2,
    State0 = 3,
    State1 = 4,
    Iec61850 = 5,
    AssocBo = 6,
    Purpose = 7,
    Mandatory1815 = 8,
    Mandatory1547 = 9,
}

/// Column indices for the AO sheet.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AoCol {
    PointIndex = 0,
    Name = 1,
    Minimum = 2,
    Maximum = 3,
    Multiplier = 4,
    Offset = 5,
    Units = 6,
    Iec61850 = 7,
    AssocAi = 8,
    Purpose = 9,
    Mandatory1815 = 10,
    Mandatory1547 = 11,
}

/// Column indices for the AI sheet.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiCol {
    PointIndex = 0,
    Name = 1,
    EventClass = 2,
    Minimum = 3,
    Maximum = 4,
    Multiplier = 5,
    Offset = 6,
    Units = 7,
    Iec61850 = 8,
    Value = 9,
    AssocAo = 10,
    Purpose = 11,
    Mandatory1815 = 12,
    Mandatory1547 = 13,
}

/// Column indices for the CTR sheet.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CtrCol {
    PointIndex = 0,
    Name = 1,
    CounterEventClass = 2,
    FrozenCounterExists = 3,
    FrozenCounterEventClass = 4,
    Iec61850 = 5,
    Purpose = 6,
    Mandatory1815 = 7,
    Mandatory1547 = 8,
}

// ---------------------------------------------------------------------------
// Header positions per data sheet
// ---------------------------------------------------------------------------

/// A header-cell specification: `((row, col), expected_normalized_text)`.
pub type HeaderSpec = ((u32, u32), &'static str);

pub const BO_HEADERS: &[HeaderSpec] = &[
    ((0, 0), "DNP3 Point Index"),
    ((0, 1), "Name / Description"),
    ((0, 2), "Name for State when value is 0"),
    ((0, 3), "Name for State when value is 1"),
    ((0, 4), "IEC 61850"),
    ((0, 5), "Additional Information"),
    ((0, 9), "EUT Capabilities"),
    ((1, 4), "IEC61850UniqueString"),
    ((1, 5), "Assoc. BI"),
    ((1, 6), "Purpose/Mode/Function"),
];

pub const BI_HEADERS: &[HeaderSpec] = &[
    ((0, 0), "DNP3 Point Index"),
    ((0, 1), "Name / Description"),
    ((0, 2), "Default Event Class"),
    ((0, 3), "Name for State when value is 0"),
    ((0, 4), "Name for State when value is 1"),
    ((0, 5), "IEC 61850"),
    ((0, 6), "Additional Information"),
    ((0, 10), "EUT Capabilities"),
    ((1, 5), "IEC61850UniqueString"),
    ((1, 6), "Assoc. BO"),
    ((1, 7), "Purpose/Mode/Function"),
];

pub const AO_HEADERS: &[HeaderSpec] = &[
    ((0, 0), "DNP3 Point Index"),
    ((0, 1), "Name / Description"),
    ((0, 2), "Transmitted Value"),
    ((0, 4), "Scaling"),
    ((0, 6), "Units"),
    ((0, 7), "IEC 61850"),
    ((0, 8), "Additional Information"),
    ((0, 12), "EUT Capabilities"),
    ((1, 2), "Minimum"),
    ((1, 3), "Maximum"),
    ((1, 4), "Multiplier"),
    ((1, 5), "OffSet"),
    ((1, 7), "IEC61850UniqueString"),
    ((1, 8), "Assoc. AI"),
    ((1, 9), "Purpose/Mode/Function"),
];

pub const AI_HEADERS: &[HeaderSpec] = &[
    ((0, 0), "DNP3 Point Index"),
    ((0, 1), "Name / Description"),
    ((0, 2), "Default Event Class"),
    ((0, 3), "Transmitted Value"),
    ((0, 5), "Scaling"),
    ((0, 7), "Units"),
    ((0, 8), "IEC61850"),
    ((0, 9), "Additional Information"),
    ((0, 14), "EUT Capabilities"),
    ((1, 3), "Minimum"),
    ((1, 4), "Maximum"),
    ((1, 5), "Multiplier"),
    ((1, 6), "Offset"),
    ((1, 8), "IEC61850UniqueString"),
    ((1, 9), "Value"),
    ((1, 10), "Assoc. AO"),
    ((1, 11), "Purpose/Mode/Function"),
];

pub const CTR_HEADERS: &[HeaderSpec] = &[
    ((0, 0), "DNP3 Point Index"),
    ((0, 1), "Name / Description"),
    ((0, 2), "Default Class Assigned to Counter Events"),
    ((0, 3), "Frozen Counter Exists (Yes or No)"),
    ((0, 4), "Default Class Assigned to Frozen Counter Events"),
    ((0, 5), "IEC 61850"),
    ((0, 6), "Additional Information"),
    ((0, 9), "EUT Capabilities"),
    ((1, 5), "IEC61850UniqueString"),
    ((1, 6), "Purpose/Mode/ Function"),
];

// ---------------------------------------------------------------------------
// Key sheet cell addresses (0-based, calamine convention)
// ---------------------------------------------------------------------------

/// Row (0-based, relative to range start) containing the "Start Index" / "BO" / "BI" / "AO" / "AI" / "CTR" column headers.
pub const KEY_HEADER_ROW: u32 = 2;

/// Column (0-based, relative to range start) that holds the start index for each section / equipment type.
pub const KEY_START_COL: u32 = 2;

/// Columns holding per-sheet point counts (BO, BI, AO, AI, CTR respectively).
pub const KEY_BO_COL: u32 = 3;
pub const KEY_BI_COL: u32 = 4;
pub const KEY_AO_COL: u32 = 5;
pub const KEY_AI_COL: u32 = 6;
pub const KEY_CTR_COL: u32 = 7;

/// Column holding the equipment-instance count.
pub const KEY_COUNT_COL: u32 = 9;

/// Rows for the five section-start ranges (config/control, functions, curves, system meter,
/// extensions).  Index 0 is "config_control", 1 is "functions", etc.
pub const KEY_SECTION_ROWS: &[(u32, &str)] = &[
    (4, "config_control"),
    (5, "functions"),
    (6, "curves"),
    (7, "system_meter"),
    (8, "extensions"),
];

/// Schedule section rows (0-based calamine row indices; start index is in KEY_START_COL).
pub const KEY_SCHEDULE_BC_ROW: u32 = 12; // Excel row 13: "Schedules (2 items/step)"
pub const KEY_SCHEDULE_BC_STATUS_ROW: u32 = 13; // Excel row 14: "Schedule Status (2 items)"
pub const KEY_SCHEDULE_ROW: u32 = 14; // Excel row 15: "Schedules (4 items/step)"
pub const KEY_SCHEDULE_STATUS_ROW: u32 = 15; // Excel row 16: "Schedule Status (4 items)"

/// Equipment descriptor rows (start-index + count live here).
pub const KEY_METER_ROW: u32 = 19;
pub const KEY_DER_UNIT_ROW: u32 = 22;
pub const KEY_INVERTER_ROW: u32 = 25;
pub const KEY_BATTERY_ROW: u32 = 28;

/// Points-per-equipment rows (one row per equipment type, immediately below descriptor row).
pub const KEY_METER_PER_ROW: u32 = 20;
pub const KEY_DER_UNIT_PER_ROW: u32 = 23;
pub const KEY_INVERTER_PER_ROW: u32 = 26;
pub const KEY_BATTERY_PER_ROW: u32 = 29;

/// Miscellaneous range rows.
pub const KEY_EXPERIMENTAL_ROW: u32 = 31;
pub const KEY_VENDOR_ROW: u32 = 32;
pub const KEY_AUTO_DISC_ROW: u32 = 33;
pub const KEY_MAX_POINTS_ROW: u32 = 34;

/// All equipment descriptor rows in order (used by structural validator and generator).
pub const KEY_EQUIPMENT_ROWS: &[u32] = &[
    KEY_METER_ROW,
    KEY_DER_UNIT_ROW,
    KEY_INVERTER_ROW,
    KEY_BATTERY_ROW,
];

/// All points-per-equipment rows in order (parallel to `KEY_EQUIPMENT_ROWS`).
pub const KEY_PER_EQUIPMENT_ROWS: &[u32] = &[
    KEY_METER_PER_ROW,
    KEY_DER_UNIT_PER_ROW,
    KEY_INVERTER_PER_ROW,
    KEY_BATTERY_PER_ROW,
];

// ---------------------------------------------------------------------------
// Key sheet numeric positions (helper for writer and structural validator)
// ---------------------------------------------------------------------------

use std::sync::LazyLock;

/// Returns all `(row, col)` pairs in the Key sheet that must contain numeric values.
///
/// Used by both `validators::structural::validate_key_sheet_structure` (to flag violations)
pub fn key_numeric_positions() -> &'static [(u32, u32)] {
    static POSITIONS: LazyLock<Vec<(u32, u32)>> = LazyLock::new(|| {
        let mut pos = Vec::new();

        // Section rows: start index + per-sheet counts.
        for &(row, _) in KEY_SECTION_ROWS {
            for col in [
                KEY_START_COL,
                KEY_BO_COL,
                KEY_BI_COL,
                KEY_AO_COL,
                KEY_AI_COL,
                KEY_CTR_COL,
            ] {
                pos.push((row, col));
            }
        }

        // Equipment descriptor rows: start index + instance count.
        for &row in KEY_EQUIPMENT_ROWS {
            pos.push((row, KEY_START_COL));
            pos.push((row, KEY_COUNT_COL));
        }

        // Points-per-equipment rows: per-sheet counts.
        for &row in KEY_PER_EQUIPMENT_ROWS {
            for col in [KEY_BO_COL, KEY_BI_COL, KEY_AO_COL, KEY_AI_COL, KEY_CTR_COL] {
                pos.push((row, col));
            }
        }

        // Miscellaneous range rows: start index only.
        for row in [
            KEY_SCHEDULE_BC_ROW,
            KEY_SCHEDULE_BC_STATUS_ROW,
            KEY_SCHEDULE_ROW,
            KEY_SCHEDULE_STATUS_ROW,
            KEY_EXPERIMENTAL_ROW,
            KEY_VENDOR_ROW,
            KEY_AUTO_DISC_ROW,
            KEY_MAX_POINTS_ROW,
        ] {
            pos.push((row, KEY_START_COL));
        }

        pos
    });
    &POSITIONS
}
