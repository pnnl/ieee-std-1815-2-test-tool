use std::sync::Arc;

use poem::web::{Data, Json};
use poem::{Route, get, handler};

use crate::models::scenario::Scenario;

#[utoipa::path(
    get,
    path = "/api/scenarios",
    tag = "scenarios",
    responses(
        (status = 200, description = "Pre-loaded conformance test scenarios", body = Vec<Scenario>)
    )
)]
#[handler]
pub async fn list_scenarios(scenarios: Data<&Arc<Vec<Scenario>>>) -> Json<Vec<Scenario>> {
    Json(scenarios.as_ref().clone())
}

pub fn routes() -> Route {
    Route::new().at("/", get(list_scenarios))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::scenario::ExpectedTest;
    use common::conformance_testing::message_structure::TestName;
    use poem::EndpointExt;
    use poem::middleware::AddDataEndpoint;
    use poem::test::{TestClient, TestResponse};

    fn test_scenarios_app(scenarios: Vec<Scenario>) -> AddDataEndpoint<Route, Arc<Vec<Scenario>>> {
        Route::new().nest("/", routes()).data(Arc::new(scenarios))
    }

    #[tokio::test]
    async fn test_list_scenarios_returns_ok() {
        let app = test_scenarios_app(vec![]);
        let client = TestClient::new(app);
        let resp: TestResponse = client.get("/").send().await;
        resp.assert_status_is_ok();
    }

    #[tokio::test]
    async fn test_list_scenarios_returns_empty_array() {
        let app = test_scenarios_app(vec![]);
        let client = TestClient::new(app);
        let resp: TestResponse = client.get("/").send().await;
        resp.assert_status_is_ok();
        let json = resp.json().await;
        let arr = json.value().deserialize::<Vec<Scenario>>();
        assert!(arr.is_empty());
    }

    #[tokio::test]
    async fn test_list_scenarios_returns_loaded_data() {
        let scenarios = vec![
            Scenario {
                id: "configuration".to_string(),
                name: "Configuration".to_string(),
                description: "Test config.".to_string(),
                expected_tests: vec![ExpectedTest {
                    test_id: TestName::MON_001,
                    should_pass: true,
                }],
            },
            Scenario {
                id: "unsupported".to_string(),
                name: "Unsupported".to_string(),
                description: "Test unsupported.".to_string(),
                expected_tests: vec![],
            },
        ];
        let app = test_scenarios_app(scenarios);
        let client = TestClient::new(app);
        let resp: TestResponse = client.get("/").send().await;
        resp.assert_status_is_ok();
        let json = resp.json().await;
        let arr = json.value().deserialize::<Vec<Scenario>>();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0].id, "configuration");
        assert_eq!(arr[0].expected_tests.len(), 1);
        assert_eq!(arr[1].id, "unsupported");
        assert!(arr[1].expected_tests.is_empty());
    }
}
