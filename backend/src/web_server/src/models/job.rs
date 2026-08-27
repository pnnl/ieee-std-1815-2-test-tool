use common::profile::validation::ValidationErrors;
use poem::IntoResponse;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct ControlStationConfig {
    pub use_reference_control_station: bool,
    #[serde(default)]
    pub ip_address: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct OutstationConfig {
    pub use_reference_outstation: bool,
    #[serde(default)]
    pub ip_address: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeviceUnderTest {
    ControlStation,
    Outstation,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateJobRequest {
    pub control_station_config: ControlStationConfig,
    pub outstation_config: OutstationConfig,
    /// Profile JSON document. The runtime type stays `serde_json::Value`
    /// (preserves the permissive deserialize behavior the existing
    /// frontend relies on); the OpenAPI schema reports the stricter
    /// `PicsProfile` shape via `#[schema(value_type = PicsProfile)]`.
    #[schema(value_type = common::profile::pics_profile::PicsProfile)]
    pub profile: serde_json::Value,
    #[serde(default)]
    pub scenario_ids: Vec<String>,
    pub device_under_test: DeviceUnderTest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Starting,
    Running,
    Stopping,
    Stopped,
    Finished,
    Failed,
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobStatus::Starting => write!(f, "starting"),
            JobStatus::Running => write!(f, "running"),
            JobStatus::Stopping => write!(f, "stopping"),
            JobStatus::Stopped => write!(f, "stopped"),
            JobStatus::Finished => write!(f, "finished"),
            JobStatus::Failed => write!(f, "failed"),
        }
    }
}

/// Response for POST /jobs: returns only the job_id.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct CreateJobResponse {
    pub job_id: String,
}

/// Response for DELETE /jobs/{id}: returns a message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct StopJobResponse {
    pub message: String,
}

/// Response for GET /jobs/{id}: returns job_id and status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct JobStatusResponse {
    pub job_id: String,
    pub status: JobStatus,
}

#[derive(Debug, Clone, Serialize, PartialEq, ToSchema)]
pub struct ValidationErrorsResponse {
    pub errors: ValidationErrors,
}

impl IntoResponse for ValidationErrorsResponse {
    fn into_response(self) -> poem::Response {
        poem::Response::builder()
            .status(poem::http::StatusCode::BAD_REQUEST)
            .content_type("application/json")
            .body(serde_json::to_string(&self).unwrap())
    }
}

/// Events emitted by jobs and streamed via SSE to the frontend.
/// Streamed to the frontend via SSE.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobEvent {
    /// One of: "log", "status_update", "conformance_test_result", "scenario_status"
    pub event_type: String,
    /// Source process: "outstation", "control_station", "test_runner"
    pub source: String,
    pub job_id: String,
    pub message: serde_json::Value,
    pub timestamp: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_job_request_deserializes() {
        let json = serde_json::json!({
            "control_station_config": {
                "use_reference_control_station": true,
                "ip_address": null,
                "port": null
            },
            "outstation_config": {
                "use_reference_outstation": false,
                "ip_address": "127.0.0.1",
                "port": 20000
            },
            "profile": {"key": "value"},
            "scenario_ids": ["scenario_1"],
            "device_under_test": "outstation"
        });

        let req: CreateJobRequest = serde_json::from_value(json).unwrap();
        assert!(req.control_station_config.use_reference_control_station);
        assert_eq!(
            req.outstation_config.ip_address,
            Some("127.0.0.1".to_string())
        );
        assert_eq!(req.outstation_config.port, Some(20000));
        assert_eq!(req.scenario_ids, vec!["scenario_1"]);
        assert_eq!(req.device_under_test, DeviceUnderTest::Outstation);
    }

    #[test]
    fn test_create_job_request_defaults_scenario_ids() {
        let json = serde_json::json!({
            "control_station_config": {
                "use_reference_control_station": true
            },
            "outstation_config": {
                "use_reference_outstation": true
            },
            "profile": {},
            "device_under_test": "control_station"
        });

        let req: CreateJobRequest = serde_json::from_value(json).unwrap();
        assert!(req.scenario_ids.is_empty());
        assert_eq!(req.device_under_test, DeviceUnderTest::ControlStation);
    }

    #[test]
    fn test_create_job_response_serializes() {
        let resp = CreateJobResponse {
            job_id: "abc-123".to_string(),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["job_id"], "abc-123");
        assert!(json.get("status").is_none());
    }

    #[test]
    fn test_stop_job_response_serializes() {
        let resp = StopJobResponse {
            message: "Job stopped".to_string(),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["message"], "Job stopped");
    }

    #[test]
    fn test_job_status_response_serializes() {
        let resp = JobStatusResponse {
            job_id: "abc-123".to_string(),
            status: JobStatus::Running,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["job_id"], "abc-123");
        assert_eq!(json["status"], "running");
    }

    #[test]
    fn test_job_status_variants_serialize_snake_case() {
        assert_eq!(
            serde_json::to_value(JobStatus::Starting).unwrap(),
            serde_json::json!("starting")
        );
        assert_eq!(
            serde_json::to_value(JobStatus::Running).unwrap(),
            serde_json::json!("running")
        );
        assert_eq!(
            serde_json::to_value(JobStatus::Stopping).unwrap(),
            serde_json::json!("stopping")
        );
        assert_eq!(
            serde_json::to_value(JobStatus::Stopped).unwrap(),
            serde_json::json!("stopped")
        );
        assert_eq!(
            serde_json::to_value(JobStatus::Finished).unwrap(),
            serde_json::json!("finished")
        );
        assert_eq!(
            serde_json::to_value(JobStatus::Failed).unwrap(),
            serde_json::json!("failed")
        );
    }

    #[test]
    fn test_job_status_display() {
        assert_eq!(JobStatus::Starting.to_string(), "starting");
        assert_eq!(JobStatus::Running.to_string(), "running");
        assert_eq!(JobStatus::Failed.to_string(), "failed");
    }

    #[test]
    fn test_job_event_serializes() {
        let event = JobEvent {
            event_type: "log".to_string(),
            source: "test_runner".to_string(),
            job_id: "test-123".to_string(),
            message: serde_json::json!({"text": "hello"}),
            timestamp: 1234567890.123,
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: JobEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.event_type, "log");
        assert_eq!(parsed.source, "test_runner");
        assert_eq!(parsed.job_id, "test-123");
        assert!((parsed.timestamp - 1234567890.123).abs() < 0.001);
    }

    #[test]
    fn test_job_event_with_conformance_result_message() {
        let event = JobEvent {
            event_type: "conformance_test_result".to_string(),
            source: "control_station".to_string(),
            job_id: "job-456".to_string(),
            message: serde_json::json!({
                "test": "volt_var_curve",
                "scenario_id": "scenario_1",
                "pass": true,
                "comments": []
            }),
            timestamp: 1234567890.0,
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["event_type"], "conformance_test_result");
        assert_eq!(json["message"]["pass"], true);
    }

    #[test]
    fn test_job_status_response_deserializes() {
        let json = serde_json::json!({
            "job_id": "uuid-here",
            "status": "failed"
        });
        let resp: JobStatusResponse = serde_json::from_value(json).unwrap();
        assert_eq!(resp.job_id, "uuid-here");
        assert_eq!(resp.status, JobStatus::Failed);
    }

    #[test]
    fn test_device_under_test_roundtrip() {
        let dut = DeviceUnderTest::ControlStation;
        let json = serde_json::to_value(&dut).unwrap();
        assert_eq!(json, "control_station");
        let parsed: DeviceUnderTest = serde_json::from_value(json).unwrap();
        assert_eq!(parsed, DeviceUnderTest::ControlStation);
    }

    #[test]
    fn test_control_station_config_defaults() {
        let json = serde_json::json!({
            "use_reference_control_station": true
        });
        let config: ControlStationConfig = serde_json::from_value(json).unwrap();
        assert!(config.use_reference_control_station);
        assert!(config.ip_address.is_none());
        assert!(config.port.is_none());
    }

    #[test]
    fn test_outstation_config_with_address() {
        let json = serde_json::json!({
            "use_reference_outstation": false,
            "ip_address": "192.168.1.100",
            "port": 20001
        });
        let config: OutstationConfig = serde_json::from_value(json).unwrap();
        assert!(!config.use_reference_outstation);
        assert_eq!(config.ip_address, Some("192.168.1.100".to_string()));
        assert_eq!(config.port, Some(20001));
    }
}
