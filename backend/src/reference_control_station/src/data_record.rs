//! DataRecord type and JSONL writer for measurement output.

use std::fs::File;
use std::io::Write;
use std::sync::Mutex;

use chrono::Utc;
use dnp3::app::measurement::Time;
use serde::Serialize;

/// JSON record written to the JSONL data file.
///
/// Standard DNP3 measurement fields are always populated. Profile enrichment fields
/// (`description`, `uid`, `purpose`, `units`, `engineering_value`) are filled in by
/// the read handler when a PICS profile is loaded.
#[derive(Debug, Serialize)]
pub(crate) struct DataRecord {
    pub(crate) ts: String,
    pub(crate) read_type: String,
    #[serde(rename = "type")]
    pub(crate) data_type: String,
    pub(crate) group: u8,
    pub(crate) variation: u8,
    pub(crate) qualifier: String,
    pub(crate) index: u16,
    pub(crate) value: serde_json::Value,
    pub(crate) flags: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) dnp3_time: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) time_quality: Option<String>,
    // Profile enrichment fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) uid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) purpose: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) units: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) engineering_value: Option<f64>,
    /// Populated when a DNP3 parsing or conformance issue is detected for this point.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) warning: Option<String>,
}

impl DataRecord {
    /// Build a `DataRecord` with all common fields populated and enrichment fields empty.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        read_type: &str,
        data_type: &str,
        group: u8,
        variation: u8,
        qualifier: &str,
        index: u16,
        value: serde_json::Value,
        flags: u8,
        dnp3_time: Option<u64>,
        time_quality: Option<String>,
    ) -> Self {
        Self {
            ts: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            read_type: read_type.to_string(),
            data_type: data_type.to_string(),
            group,
            variation,
            qualifier: qualifier.to_string(),
            index,
            value,
            flags,
            dnp3_time,
            time_quality,
            description: None,
            uid: None,
            purpose: None,
            units: None,
            engineering_value: None,
            warning: None,
        }
    }

    /// Parse a DNP3 [`Time`] value into a raw millisecond timestamp and a quality label.
    pub(crate) fn time_fields(time: &Option<Time>) -> (Option<u64>, Option<String>) {
        match time {
            Some(Time::Synchronized(ts)) => {
                (Some(ts.raw_value()), Some("synchronized".to_string()))
            }
            Some(Time::Unsynchronized(ts)) => {
                (Some(ts.raw_value()), Some("unsynchronized".to_string()))
            }
            None => (None, None),
        }
    }
}

/// Writes [`DataRecord`] values as newline-delimited JSON to an output file.
pub(crate) struct DataRecordWriter {
    file: Mutex<File>,
}

impl DataRecordWriter {
    /// Serialize `record` to JSON and append it as a line to the output file.
    pub(crate) fn write(&self, record: &DataRecord) {
        let line = match serde_json::to_string(record) {
            Ok(line) => line,
            Err(e) => {
                tracing::error!("Failed to serialize data record: {e}");
                return;
            }
        };
        let mut file = self.file.lock().unwrap();
        if let Err(e) = writeln!(file, "{line}") {
            tracing::error!("Failed to write data record to file: {e}");
        } else if let Err(e) = file.flush() {
            tracing::error!("Failed to flush data file: {e}");
        }
    }
}

/// Open a JSONL file for appending and return a [`DataRecordWriter`].
///
/// Returns `None` if `path` is empty or the file cannot be opened.
pub(crate) fn open_data_file(path: &str) -> Option<DataRecordWriter> {
    if path.is_empty() {
        return None;
    }
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        Ok(file) => Some(DataRecordWriter {
            file: Mutex::new(file),
        }),
        Err(e) => {
            tracing::error!("Failed to open data file '{path}': {e}");
            None
        }
    }
}
