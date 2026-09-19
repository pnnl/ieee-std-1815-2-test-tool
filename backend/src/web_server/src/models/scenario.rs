pub use scenario_generator::{ExpectedTest, Scenario};

#[cfg(test)]
mod tests {
    use common::conformance_testing::message_structure::TestName;

    use super::*;

    #[test]
    fn test_scenario_deserializes_valid_json() {
        let json = serde_json::json!({
            "id": "configuration",
            "name": "Outstation ingests PICS",
            "description": "Test description.",
            "expected_tests": [
                { "test_id": "MON_001", "should_pass": true },
                { "test_id": "VV_001", "should_pass": false }
            ]
        });
        let scenario: Scenario = serde_json::from_value(json).unwrap();
        assert_eq!(scenario.id, "configuration");
        assert_eq!(scenario.name, "Outstation ingests PICS");
        assert_eq!(scenario.expected_tests.len(), 2);
        assert!(scenario.expected_tests[0].should_pass);
        assert!(!scenario.expected_tests[1].should_pass);
    }

    #[test]
    fn test_scenario_deserializes_empty_expected_tests() {
        let json = serde_json::json!({
            "id": "lower_bounds",
            "name": "Points cannot be modified below their minimum",
            "description": "Attempt to modify all points outside their minimum values.",
            "expected_tests": []
        });
        let scenario: Scenario = serde_json::from_value(json).unwrap();
        assert_eq!(scenario.id, "lower_bounds");
        assert!(scenario.expected_tests.is_empty());
    }

    #[test]
    fn test_scenario_fails_on_missing_field() {
        let json = serde_json::json!({
            "id": "bad",
            "name": "Bad"
        });
        let result = serde_json::from_value::<Scenario>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_expected_test_fails_on_missing_should_pass() {
        let json = serde_json::json!({
            "test_id": "MON_001"
        });
        let result = serde_json::from_value::<ExpectedTest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_scenario_serializes_roundtrip() {
        let scenario = Scenario {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: "A test scenario.".to_string(),
            expected_tests: vec![ExpectedTest {
                test_id: TestName::MON_001,
                should_pass: true,
            }],
        };
        let json = serde_json::to_value(&scenario).unwrap();
        let deserialized: Scenario = serde_json::from_value(json).unwrap();
        assert_eq!(scenario, deserialized);
    }

    #[test]
    fn test_scenarios_vec_deserializes_from_array() {
        let json = serde_json::json!([
            {
                "id": "a",
                "name": "A",
                "description": "Scenario A.",
                "expected_tests": []
            },
            {
                "id": "b",
                "name": "B",
                "description": "Scenario B.",
                "expected_tests": [
                    { "test_id": "FW_001", "should_pass": true }
                ]
            }
        ]);
        let scenarios: Vec<Scenario> = serde_json::from_value(json).unwrap();
        assert_eq!(scenarios.len(), 2);
        assert_eq!(scenarios[0].id, "a");
        assert_eq!(scenarios[1].expected_tests.len(), 1);
    }
}
