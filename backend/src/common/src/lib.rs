use serde::{Deserialize, Serialize};

pub mod compatibility_matrix;
pub mod conformance_testing;
pub mod profile;
pub mod uids;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TestResultMessage {
    #[serde(default = "default_event_type")]
    pub event_type: String,
    pub point_index: String,
    pub point_group: String,
    pub passed: bool,
    pub details: Option<String>,
}

fn default_event_type() -> String {
    "test_result".to_string()
}

impl TestResultMessage {
    pub fn new(
        point_index: String,
        point_group: String,
        passed: bool,
        details: Option<String>,
    ) -> Self {
        Self {
            event_type: default_event_type(),
            point_index,
            point_group,
            passed,
            details,
        }
    }
}
