use std::sync::Arc;
use std::time::Duration;

use futures_util::{StreamExt, stream};
use poem::web::sse::{Event as SseEvent, SSE};
use poem::web::{Data, Json, Path};
use poem::{Route, delete, get, handler, post};
use tokio_stream::wrappers::BroadcastStream;

use crate::models::job::{CreateJobRequest, CreateJobResponse, StopJobResponse};
use crate::services::job_service::JobService;

#[utoipa::path(
    post,
    path = "/api/jobs",
    tag = "jobs",
    request_body = CreateJobRequest,
    responses(
        (status = 200, description = "Job created", body = CreateJobResponse),
        (status = 400, description = "Invalid profile or job configuration")
    )
)]
#[handler]
pub async fn create_job(
    service: Data<&Arc<JobService>>,
    body: Json<CreateJobRequest>,
) -> poem::Result<Json<CreateJobResponse>> {
    match service.create_job(body.0).await {
        Ok(response) => Ok(Json(response)),
        Err(e) => Err(poem::Error::from_string(
            e,
            poem::http::StatusCode::BAD_REQUEST,
        )),
    }
}

#[utoipa::path(
    delete,
    path = "/api/jobs/{job_id}",
    tag = "jobs",
    params(
        ("job_id" = String, Path, description = "Job identifier")
    ),
    responses(
        (status = 200, description = "Job stopped", body = StopJobResponse),
        (status = 404, description = "Job not found")
    )
)]
#[handler]
pub async fn delete_job(
    service: Data<&Arc<JobService>>,
    Path(job_id): Path<String>,
) -> poem::Result<Json<StopJobResponse>> {
    match service.stop_job(&job_id).await {
        Ok(response) => Ok(Json(response)),
        Err(e) => Err(poem::Error::from_string(
            e,
            poem::http::StatusCode::NOT_FOUND,
        )),
    }
}

#[handler]
pub async fn job_events(
    service: Data<&Arc<JobService>>,
    Path(job_id): Path<String>,
) -> poem::Result<SSE> {
    let (replay, rx) = service
        .subscribe_events(&job_id)
        .await
        .map_err(|e| poem::Error::from_string(e, poem::http::StatusCode::NOT_FOUND))?;

    // Replay the snapshot of current per-process statuses first so the
    // late-arriving subscriber sees the right pill values immediately.
    // `tokio::sync::broadcast` does not retain history, so without this
    // any `status_update` emitted before the EventSource attached would
    // be lost forever (the bug this fix targets).
    let replay_stream = stream::iter(
        replay
            .into_iter()
            .filter_map(|event| serde_json::to_string(&event).ok().map(SseEvent::message)),
    );

    let live_stream = BroadcastStream::new(rx).filter_map(|result| async move {
        match result {
            Ok(event) => {
                let data = serde_json::to_string(&event).ok()?;
                Some(SseEvent::message(data))
            }
            Err(_) => None, // Lagged messages are dropped
        }
    });

    let stream = replay_stream.chain(live_stream);

    // keep_alive forces poem to periodically write a `:` comment frame to the
    // response body, which flushes any pending data buffered in the underlying
    // hyper/h2 writer. Without this, small SSE frames (status updates,
    // conformance test results) can sit unflushed for a long time and the
    // browser's EventSource never fires `onmessage`. The interval also keeps
    // proxies from idle-closing the connection.
    Ok(SSE::new(stream).keep_alive(Duration::from_secs(15)))
}

pub fn routes() -> Route {
    Route::new()
        .at("/", post(create_job))
        .at("/:job_id", delete(delete_job))
        .at("/:job_id/events", get(job_events))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::test_service;
    use common::profile::PicsProfile;
    use poem::EndpointExt;
    use poem::middleware::AddDataEndpoint;
    use poem::test::{TestClient, TestResponse};

    fn test_jobs_app() -> AddDataEndpoint<Route, Arc<JobService>> {
        let service = Arc::new(test_service());
        Route::new().nest("/", routes()).data(service)
    }

    #[tokio::test]
    async fn test_create_job_with_valid_profile_returns_ok() {
        let app = test_jobs_app();
        let client = TestClient::new(app);
        let resp: TestResponse = client
            .post("/")
            .body_json(&serde_json::json!({
                "control_station_config": {
                    "use_reference_control_station": false
                },
                "outstation_config": {
                    "use_reference_outstation": false
                },
                "profile": PicsProfile::load_full_profile(),
                "device_under_test": "outstation"
            }))
            .send()
            .await;
        resp.assert_status_is_ok();
        let json = resp.json().await;
        // POST /jobs returns only {"job_id": "..."} (no status field)
        let job_id = json.value().object().get("job_id").string().to_string();
        assert!(!job_id.is_empty());
    }

    #[tokio::test]
    async fn test_create_job_with_invalid_profile_returns_bad_request() {
        let app = test_jobs_app();
        let client = TestClient::new(app);
        let resp: TestResponse = client
            .post("/")
            .body_json(&serde_json::json!({
                "control_station_config": {
                    "use_reference_control_station": false
                },
                "outstation_config": {
                    "use_reference_outstation": false
                },
                "profile": "not a valid profile",
                "device_under_test": "outstation"
            }))
            .send()
            .await;
        resp.assert_status(poem::http::StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_delete_nonexistent_job_returns_not_found() {
        let app = test_jobs_app();
        let client = TestClient::new(app);
        let resp: TestResponse = client.delete("/nonexistent-id").send().await;
        resp.assert_status(poem::http::StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_subscribe_nonexistent_job_events_returns_not_found() {
        let app = test_jobs_app();
        let client = TestClient::new(app);
        let resp: TestResponse = client.get("/nonexistent-id/events").send().await;
        resp.assert_status(poem::http::StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_create_then_delete_job() {
        let app = test_jobs_app();
        let client = TestClient::new(app);

        // Create
        let resp: TestResponse = client
            .post("/")
            .body_json(&serde_json::json!({
                "control_station_config": {
                    "use_reference_control_station": false
                },
                "outstation_config": {
                    "use_reference_outstation": false
                },
                "profile": PicsProfile::load_full_profile(),
                "device_under_test": "outstation"
            }))
            .send()
            .await;
        resp.assert_status_is_ok();
        let json = resp.json().await;
        let job_id = json.value().object().get("job_id").string().to_string();

        // Delete
        let del_resp: TestResponse = client.delete(format!("/{}", job_id)).send().await;
        del_resp.assert_status_is_ok();
        let del_json = del_resp.json().await;
        del_json
            .value()
            .object()
            .get("message")
            .assert_string("Job stopped");
    }

    #[tokio::test]
    async fn test_create_then_subscribe_events_returns_ok() {
        let app = test_jobs_app();
        let client = TestClient::new(app);

        // Create
        let resp: TestResponse = client
            .post("/")
            .body_json(&serde_json::json!({
                "control_station_config": {
                    "use_reference_control_station": false
                },
                "outstation_config": {
                    "use_reference_outstation": false
                },
                "profile": PicsProfile::load_full_profile(),
                "device_under_test": "outstation"
            }))
            .send()
            .await;
        resp.assert_status_is_ok();
        let json = resp.json().await;
        let job_id = json.value().object().get("job_id").string().to_string();

        // Subscribe to events - should return 200 with SSE content type
        let events_resp: TestResponse = client.get(format!("/{}/events", job_id)).send().await;
        events_resp.assert_status_is_ok();
        events_resp.assert_content_type("text/event-stream");
    }
}
