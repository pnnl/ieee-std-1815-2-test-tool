use poem::http::StatusCode;
use poem::web::Json;
use poem::{IntoResponse, Response};
use poem::{Route, get, handler, web::Query};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, serde::Deserialize, PartialEq, ToSchema)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

#[utoipa::path(
    get,
    path = "/api/health",
    tag = "health",
    responses(
        (status = 200, description = "Service health and version", body = HealthResponse)
    )
)]
#[handler]
pub async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

#[derive(Debug, serde::Deserialize, ToSchema)]
struct TestConnectionQuery {
    host: String,
    port: String,
}

#[utoipa::path(
    get,
    path = "/api/health/test-connection",
    responses(
        (status = 200, description = "Test connection to the specified IP and port", body = ()),
        (status = 400, description = "Invalid IP or port"),
        (status = 500, description = "Failed to connect to the specified IP and port"),
    )
)]
#[handler]
pub async fn test_connection(query: Query<TestConnectionQuery>) -> poem::Result<impl IntoResponse> {
    let host = &query.host;
    let port = &query.port;
    // Quick TCP connection test to the specified host and port
    let address = format!("{}:{}", host, port);
    match tokio::net::TcpStream::connect(address).await {
        Ok(_) => Ok(Response::builder().status(StatusCode::OK).body(())),
        Err(e) => Err(poem::error::InternalServerError(e)),
    }
}

pub fn routes() -> Route {
    Route::new()
        .at("/", get(health_check))
        .at("/test-connection", get(test_connection))
}

#[cfg(test)]
mod tests {
    use super::*;
    use poem::test::{TestClient, TestResponse};

    #[tokio::test]
    async fn test_health_check_returns_ok() {
        let app = routes();
        let client = TestClient::new(app);
        let resp: TestResponse = client.get("/").send().await;
        resp.assert_status_is_ok();
    }

    #[tokio::test]
    async fn test_health_check_body() {
        let app = routes();
        let client = TestClient::new(app);
        let resp: TestResponse = client.get("/").send().await;
        resp.assert_status_is_ok();
        let json = resp.json().await;
        let value = json.value();
        value.object().get("status").assert_string("ok");
        value
            .object()
            .get("version")
            .assert_string(env!("CARGO_PKG_VERSION"));
    }
}
