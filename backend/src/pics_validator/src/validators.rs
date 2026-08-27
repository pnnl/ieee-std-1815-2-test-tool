use anyhow::{Result, bail};
use calamine::Data;

use crate::loader::{Workbook, get_worksheet};
use crate::schema::{self, Sheet};

/// Collapse internal whitespace, strip carriage-return XML artifacts, and lowercase
/// for header comparison.
fn normalize_header(s: &str) -> String {
    s.replace("_x000D_", "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Validate the structural integrity of all sheets in the workbook.
///
/// Fails on the first error found (missing sheet, wrong column count,
/// wrong header text, or non-numeric Key-sheet cell).
pub fn validate_workbook_structure(wb: &Workbook) -> Result<()> {
    for &sheet in Sheet::ALL {
        if get_worksheet(wb, sheet.as_str()).is_none() {
            bail!(
                "Required sheet '{}' is missing from workbook",
                sheet.as_str()
            );
        }
    }

    for &sheet in Sheet::DATA {
        let range =
            get_worksheet(wb, sheet.as_str()).expect("sheet existence verified in ALL loop above");
        let (_, col_count) = range.get_size();

        if let Some(expected) = sheet.expected_col_count() {
            if col_count < expected {
                bail!(
                    "Sheet '{}' has {} column(s), expected at least {}",
                    sheet.as_str(),
                    col_count,
                    expected
                );
            }
        }

        for &((row, col), expected_text) in sheet.headers() {
            let actual = range
                .get((row as usize, col as usize))
                .and_then(|cell| match cell {
                    Data::String(s) => Some(s.as_str()),
                    _ => None,
                })
                .unwrap_or("");

            if normalize_header(actual) != normalize_header(expected_text) {
                bail!(
                    "Sheet '{}' row {} col {}: expected header '{}' but found '{}'",
                    sheet.as_str(),
                    row + 1,
                    col + 1,
                    expected_text,
                    actual
                );
            }
        }
    }

    let key_range = get_worksheet(wb, Sheet::Key.as_str())
        .expect("Key sheet existence verified in ALL loop above");
    for &(row, col) in schema::key_numeric_positions() {
        let cell = key_range.get((row as usize, col as usize));
        let is_acceptable = match cell {
            None | Some(Data::Empty) => true,
            Some(Data::Float(_)) | Some(Data::Int(_)) => true,
            Some(Data::String(s)) => {
                let trimmed = s.trim();
                trimmed.is_empty()
                    || trimmed == "-"
                    || trimmed == "x"
                    || trimmed.parse::<f64>().is_ok()
            }
            _ => false,
        };
        if !is_acceptable {
            bail!(
                "Key sheet row {} col {}: expected a numeric value but found: {:?}",
                row + 1,
                col + 1,
                cell
            );
        }
    }

    Ok(())
}
