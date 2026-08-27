pub mod models;
pub mod routes;
pub mod services;
pub mod startup_checks;

use std::sync::Arc;

use poem::endpoint::StaticFilesEndpoint;
use poem::middleware::{AddDataEndpoint, SetHeader};
use poem::web::Json;
use poem::{EndpointExt, Route, get, handler};
use utoipa::OpenApi;

use crate::models::job::ValidationErrorsResponse;
use crate::models::scenario::Scenario;
use crate::services::job_service::JobService;

/// Top-level OpenAPI document. utoipa is framework-agnostic, so we list the
/// path functions and component schemas explicitly here. As more route modules
/// are converted, append them to `paths(...)` and their types to
/// `components(schemas(...))`.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "1815.2 Test Tool Web Server",
        version = "0.1.0",
    ),
    paths(
        crate::routes::profiles::list_profiles,
        crate::routes::profiles::get_profile,
        crate::routes::profiles::save_profile,
        crate::routes::profiles::delete_profile,
        crate::routes::profiles::validate_profile,
        crate::routes::profiles::parse_xlsx,
        crate::routes::enums::get_enums,
        crate::routes::health::health_check,
        crate::routes::scenarios::list_scenarios,
        crate::routes::jobs::create_job,
        crate::routes::jobs::delete_job,
        // NOTE: jobs::job_events is intentionally absent. The SSE handler
        // streams JobEvent frames whose payload shape varies by event_type;
        // schema work for that endpoint is tracked in #293.
    ),
    components(schemas(
        crate::routes::profiles::ProfileListItem,
        crate::routes::profiles::SaveProfileRequest,
        crate::routes::profiles::ProfileMutationAck,
        ValidationErrorsResponse,
        common::profile::pics_profile::PicsProfile,
        crate::routes::enums::EnumEntry,
        crate::routes::enums::ModeTypeEntry,
        crate::routes::enums::EnumsResponse,
        crate::routes::health::HealthResponse,
        scenario_generator::Scenario,
        scenario_generator::ExpectedTest,
        crate::models::job::ControlStationConfig,
        crate::models::job::OutstationConfig,
        crate::models::job::DeviceUnderTest,
        crate::models::job::CreateJobRequest,
        crate::models::job::CreateJobResponse,
        crate::models::job::StopJobResponse,
        crate::models::job::JobStatus,
        crate::models::job::JobStatusResponse,
    )),
    tags(
        (name = "profiles", description = "Saved PICS profile documents"),
        (name = "enums", description = "Enum lookup tables for the frontend"),
        (name = "health", description = "Service health and version probe"),
        (name = "scenarios", description = "Pre-loaded conformance test scenarios"),
        (name = "jobs", description = "Conformance test job lifecycle (non-SSE)")
    )
)]
pub struct ApiDoc;

/// Serve the live-generated OpenAPI document. The `OpenApi` struct itself
/// implements `Serialize`, so we hand it directly to Poem's `Json` extractor
/// rather than going through `to_json()` (which returns a `String`).
#[handler]
pub async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

/// Build the application routes with shared JobService state and pre-loaded scenarios.
///
/// `frontend_dir` points at the built Vite output (typically `frontend/dist`).
/// It is mounted at `/` with SPA fallback so the same binary serves both the
/// API and the static frontend.
pub fn build_app(
    job_service: Arc<JobService>,
    scenarios: Vec<Scenario>,
    frontend_dir: String,
) -> AddDataEndpoint<AddDataEndpoint<Route, Arc<JobService>>, Arc<Vec<Scenario>>> {
    let data_dir = job_service.data_dir().to_string();

    Route::new()
        .nest("/api", routes::api_routes())
        // OpenAPI spec for the profile routes (more routes to be added as
        // they are migrated). No `/api` prefix by convention.
        .at("/openapi.json", get(openapi_json))
        // Specific files referenced by path remain reachable; the HTML
        // directory listing is intentionally not enabled to avoid exposing
        // an index of the data directory's contents.
        //
        // The static mounts are wrapped with `SetHeader` so MIME-sniff,
        // CSP, and Referrer-Policy hygiene applies to served files but NOT
        // to `/api/*`, which has its own response shape and should not be
        // constrained by `frame-ancestors 'none'`.
        .nest(
            "/data",
            StaticFilesEndpoint::new(&data_dir).with(static_security_headers()),
        )
        // Serve the built frontend at `/`. `fallback_to_index` makes the SPA
        // router work: unknown paths return index.html so client-side routing
        // can handle them. The radix-tree router still gives `/api/*`,
        // `/openapi.json`, and `/data/*` priority via longest-prefix match.
        .nest(
            "/",
            StaticFilesEndpoint::new(&frontend_dir)
                .index_file("index.html")
                .fallback_to_index()
                .with(static_security_headers()),
        )
        .data(job_service)
        .data(Arc::new(scenarios))
}

/// CSP directive string applied to all static-file responses. Defined once so
/// the header builder and the regression test both reference the same value.
const CSP_DIRECTIVES: &str = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; object-src 'none'; frame-ancestors 'none'";

/// Baseline security headers applied only to the static-file mounts.
///
/// - `X-Content-Type-Options: nosniff` prevents browsers from MIME-sniffing
///   when Poem's `StaticFilesEndpoint` cannot derive a Content-Type from the
///   filename (it omits the header in that case).
/// - `Content-Security-Policy` restricts the served assets to same-origin
///   subresources and forbids framing entirely.
///   `style-src` allows `'unsafe-inline'` because the React frontend uses
///   plain `style={{...}}` props for dynamic values (pixel heights, conditional
///   `display`, color-coded borders) in components such as `App.tsx`,
///   `SchedulingTab.tsx`, `GanttVisualization.tsx`, `ScheduleEditor.tsx`,
///   `ScheduleList.tsx`, `PointRow.tsx`, `TestResultsTree.tsx`, and
///   `OffsetSection.tsx`. There is no `@mui` or `@emotion` runtime in the
///   bundle. `script-src` is kept strict at `'self'` with no `'unsafe-inline'`
///   or `'unsafe-eval'`. Future tightening: migrate those inline styles to
///   Tailwind classes (with CSS custom properties for the dynamic cases) so
///   `'unsafe-inline'` can come back off `style-src`.
/// - `Referrer-Policy: same-origin` prevents leaking the page URL to
///   cross-origin resources.
fn static_security_headers() -> SetHeader {
    SetHeader::new()
        .overriding("X-Content-Type-Options", "nosniff")
        .overriding("Content-Security-Policy", CSP_DIRECTIVES)
        .overriding("Referrer-Policy", "same-origin")
}

/// Shared test fixtures and helpers used across all test modules.
#[cfg(test)]
pub(crate) mod test_helpers {
    use crate::services::job_service::JobService;

    /// Create a `JobService` configured for unit tests (no real data directory).
    pub fn test_service() -> JobService {
        JobService::new("/tmp/mesa-test-data".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::test_service;
    use common::profile::PicsProfile;
    use poem::test::{TestClient, TestResponse};

    fn test_app() -> AddDataEndpoint<AddDataEndpoint<Route, Arc<JobService>>, Arc<Vec<Scenario>>> {
        // Tests that don't exercise the frontend mount pass a non-existent
        // directory; the static-files endpoint just 404s on read. We derive
        // the path under a fresh tempdir so it's guaranteed to not exist on
        // any machine (and doesn't collide on shared hosts). The TempDir is
        // dropped at end of this fn -- the joined child path never existed,
        // and the parent's cleanup doesn't affect that.
        let parent = tempfile::tempdir().expect("create temp parent");
        let missing = parent.path().join("frontend-does-not-exist");
        test_app_with_frontend(missing.to_str().expect("utf-8 path"))
    }

    fn test_app_with_frontend(
        frontend_dir: &str,
    ) -> AddDataEndpoint<AddDataEndpoint<Route, Arc<JobService>>, Arc<Vec<Scenario>>> {
        let service = Arc::new(test_service());
        let scenarios = vec![Scenario {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: "A test scenario.".to_string(),
            expected_tests: vec![],
        }];
        build_app(service, scenarios, frontend_dir.to_string())
    }

    #[tokio::test]
    async fn test_health_endpoint_accessible() {
        let app = test_app();
        let client = TestClient::new(app);
        let resp: TestResponse = client.get("/api/health").send().await;
        resp.assert_status_is_ok();
    }

    #[tokio::test]
    async fn test_health_endpoint_returns_ok_status() {
        let app = test_app();
        let client = TestClient::new(app);
        let resp: TestResponse = client.get("/api/health").send().await;
        resp.assert_status_is_ok();
        let json = resp.json().await;
        json.value().object().get("status").assert_string("ok");
    }

    #[tokio::test]
    async fn test_create_job_with_invalid_profile() {
        let app = test_app();
        let client = TestClient::new(app);
        let resp: TestResponse = client
            .post("/api/jobs")
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
    async fn test_delete_nonexistent_job() {
        let app = test_app();
        let client = TestClient::new(app);
        let resp: TestResponse = client.delete("/api/jobs/nonexistent-id").send().await;
        resp.assert_status(poem::http::StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_subscribe_nonexistent_job_events() {
        let app = test_app();
        let client = TestClient::new(app);
        let resp: TestResponse = client.get("/api/jobs/nonexistent-id/events").send().await;
        resp.assert_status(poem::http::StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_nonexistent_route_returns_404() {
        // SPA fallback at `/` now catches unmounted top-level paths, so assert
        // 404 against an unmounted API path to preserve the API-surface check.
        let app = test_app();
        let client = TestClient::new(app);
        let resp: TestResponse = client.get("/api/does-not-exist").send().await;
        resp.assert_status(poem::http::StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_root_serves_frontend_index() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let index_path = dir.path().join("index.html");
        std::fs::write(
            &index_path,
            "<!DOCTYPE html><html><body>mesa frontend</body></html>",
        )
        .expect("write index.html");

        let app = test_app_with_frontend(dir.path().to_str().expect("utf-8 path"));
        let client = TestClient::new(app);
        let resp: TestResponse = client.get("/").send().await;
        resp.assert_status_is_ok();
        let body = resp.0.into_body().into_string().await.expect("body");
        assert!(
            body.contains("<!DOCTYPE html>"),
            "expected index.html contents, got: {body}"
        );
    }

    #[tokio::test]
    async fn test_unknown_spa_route_falls_back_to_index() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let index_path = dir.path().join("index.html");
        let index_body = "<!DOCTYPE html><html><body>mesa spa fallback</body></html>";
        std::fs::write(&index_path, index_body).expect("write index.html");

        let app = test_app_with_frontend(dir.path().to_str().expect("utf-8 path"));
        let client = TestClient::new(app);
        let resp: TestResponse = client.get("/some-fake-spa-route").send().await;
        resp.assert_status_is_ok();
        let body = resp.0.into_body().into_string().await.expect("body");
        assert_eq!(body, index_body, "SPA fallback should return index.html");
    }

    #[tokio::test]
    async fn test_create_job() {
        let app = test_app();
        let client = TestClient::new(app);

        // Create a job with a valid profile (no reference processes)
        let resp: TestResponse = client
            .post("/api/jobs")
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
    }

    #[tokio::test]
    async fn test_create_and_delete_job() {
        let app = test_app();
        let client = TestClient::new(app);

        // Create a job
        let resp: TestResponse = client
            .post("/api/jobs")
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

        // Delete it
        let del_resp: TestResponse = client.delete(format!("/api/jobs/{}", job_id)).send().await;
        del_resp.assert_status_is_ok();
        let del_json = del_resp.json().await;
        del_json
            .value()
            .object()
            .get("message")
            .assert_string("Job stopped");
    }

    #[tokio::test]
    async fn test_scenarios_endpoint_returns_ok() {
        let app = test_app();
        let client = TestClient::new(app);
        let resp: TestResponse = client.get("/api/scenarios").send().await;
        resp.assert_status_is_ok();
    }

    #[tokio::test]
    async fn test_scenarios_endpoint_returns_array() {
        let app = test_app();
        let client = TestClient::new(app);
        let resp: TestResponse = client.get("/api/scenarios").send().await;
        resp.assert_status_is_ok();
        let json = resp.json().await;
        let arr = json.value().deserialize::<Vec<Scenario>>();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0].id, "test");
    }

    // ----- Security headers on static mounts (M1) -----

    #[tokio::test]
    async fn test_static_mount_sets_security_headers() {
        let dir = tempfile::tempdir().expect("create temp dir");
        std::fs::write(
            dir.path().join("index.html"),
            "<!DOCTYPE html><html><body>mesa</body></html>",
        )
        .expect("write index.html");

        let app = test_app_with_frontend(dir.path().to_str().expect("utf-8 path"));
        let client = TestClient::new(app);
        let resp: TestResponse = client.get("/").send().await;
        resp.assert_status_is_ok();
        resp.assert_header("X-Content-Type-Options", "nosniff");
        // Assert the exact CSP shape so any future directive change is a
        // deliberate, reviewed change — not an accidental regression.
        resp.assert_header("Content-Security-Policy", CSP_DIRECTIVES);
        resp.assert_header("Referrer-Policy", "same-origin");
    }

    #[tokio::test]
    async fn test_api_mount_does_not_get_static_csp() {
        // The static-mount middleware must not bleed into /api responses.
        let app = test_app();
        let client = TestClient::new(app);
        let resp: TestResponse = client.get("/api/scenarios").send().await;
        resp.assert_status_is_ok();
        // No CSP at all from this middleware on the API surface.
        resp.assert_header_is_not_exist("Content-Security-Policy");
    }
}
