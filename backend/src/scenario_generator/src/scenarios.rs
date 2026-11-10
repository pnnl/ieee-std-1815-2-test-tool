use common::conformance_testing::message_structure::TestName;
use std::collections::HashSet;

use common::profile::validation::Validated;
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;

use common::conformance_testing::point_lists;
use common::profile::{AoPoint, PicsProfile, ProfileIndex};

use crate::update_profile::is_multiplex_selector_ao;

/// A single expected test result within a scenario.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ExpectedTest {
    pub test_id: TestName,
    pub should_pass: bool,
    /// Explains the scenario operation's applicability without changing the test outcome.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applicability_note: Option<String>,
}

/// A test scenario with its expected outcomes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Scenario {
    pub id: String,
    pub name: String,
    pub description: String,
    pub expected_tests: Vec<ExpectedTest>,
}

/// Identifies a test scenario.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ScenarioId {
    Configuration,
    ModifyPoints,
    Unsupported,
    BelowLowerBounds,
    AboveUpperBounds,
    MaxCurves,
    MaxSchedules,
    Functional,
    FunctionalRepeat,
}

impl ScenarioId {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Configuration => "configuration",
            Self::ModifyPoints => "modify_points",
            Self::Unsupported => "unsupported",
            Self::BelowLowerBounds => "lower_bounds",
            Self::AboveUpperBounds => "upper_bounds",
            Self::MaxCurves => "max_curves",
            Self::MaxSchedules => "max_schedules",
            Self::Functional => "functional",
            Self::FunctionalRepeat => "functional_repeat",
        }
    }

    /// Returns true if this scenario performs boundary testing that requires
    /// headroom/footroom on base profile points (avoiding i32::MIN and i32::MAX saturation).
    pub fn requires_boundary_headroom(&self) -> bool {
        matches!(self, Self::BelowLowerBounds | Self::AboveUpperBounds)
    }
}

impl std::fmt::Display for ScenarioId {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt.write_str(self.as_str())
    }
}

impl std::str::FromStr for ScenarioId {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "configuration" => Ok(Self::Configuration),
            "modify_points" => Ok(Self::ModifyPoints),
            "unsupported" => Ok(Self::Unsupported),
            "lower_bounds" => Ok(Self::BelowLowerBounds),
            "upper_bounds" => Ok(Self::AboveUpperBounds),
            "max_curves" => Ok(Self::MaxCurves),
            "max_schedules" => Ok(Self::MaxSchedules),
            "functional" => Ok(Self::Functional),
            "functional_repeat" => Ok(Self::FunctionalRepeat),
            other => Err(format!("Unknown scenario id: '{other}'")),
        }
    }
}

fn all_pass() -> Vec<ExpectedTest> {
    TestName::iter()
        .map(|id: TestName| ExpectedTest {
            test_id: id,
            should_pass: true,
            applicability_note: None,
        })
        .collect()
}

fn all_fail() -> Vec<ExpectedTest> {
    TestName::iter()
        .map(|id: TestName| ExpectedTest {
            test_id: id,
            should_pass: false,
            applicability_note: None,
        })
        .collect()
}

/// Returns true if the test has at least one AO point that exists in the profile.
/// Tests with AO points will fail for lower/upper bounds scenarios because those
/// scenarios set AO write values outside the outstation's accepted range.
fn test_has_ao_points_in_profile(
    test_id: &TestName,
    ao_indices_in_profile: &HashSet<u16>,
    profile: &ProfileIndex,
) -> bool {
    let ao_uids = match test_id {
        TestName::MON_001 => points_to_ao_indices(&point_lists::mon_001(profile).ao),
        TestName::RESIDUAL => points_to_ao_indices(&point_lists::residual(profile).ao),
        TestName::ALARM_001 => points_to_ao_indices(&point_lists::alarm_001(profile).ao),
        TestName::CONN_001 => points_to_ao_indices(&point_lists::conn_001(profile).ao),
        TestName::SERV_001 => points_to_ao_indices(&point_lists::serv_001(profile).ao),
        TestName::OP_001 => points_to_ao_indices(&point_lists::op_001(profile).ao),
        TestName::CURVE_001 => points_to_ao_indices(&point_lists::curve_001(profile).ao),
        TestName::CURVE_002 => points_to_ao_indices(&point_lists::curve_002(profile).ao),
        TestName::CURVE_003 => points_to_ao_indices(&point_lists::curve_003(profile).ao),
        TestName::VRT_001 => points_to_ao_indices(&point_lists::vrt_001(profile).ao),
        TestName::FRT_001 => points_to_ao_indices(&point_lists::frt_001(profile).ao),
        TestName::FW_001 => points_to_ao_indices(&point_lists::fw_001(profile).ao),
        TestName::DRCS_001 => points_to_ao_indices(&point_lists::drcs_001(profile).ao),
        TestName::DVW_001 => points_to_ao_indices(&point_lists::dvw_001(profile).ao),
        TestName::APL_001 => points_to_ao_indices(&point_lists::apl_001(profile).ao),
        TestName::CHG_001 => points_to_ao_indices(&point_lists::chg_001(profile).ao),
        TestName::CCM_001 => points_to_ao_indices(&point_lists::ccm_001(profile).ao),
        TestName::APR1_001 => points_to_ao_indices(&point_lists::apr1_001(profile).ao),
        TestName::APR2_001 => points_to_ao_indices(&point_lists::apr2_001(profile).ao),
        TestName::APR3_001 => points_to_ao_indices(&point_lists::apr3_001(profile).ao),
        TestName::AGC_001 => points_to_ao_indices(&point_lists::agc_001(profile).ao),
        TestName::APS_001 => points_to_ao_indices(&point_lists::aps_001(profile).ao),
        TestName::VW_001 => points_to_ao_indices(&point_lists::vw_001(profile).ao),
        TestName::FWC_001 => points_to_ao_indices(&point_lists::fwc_001(profile).ao),
        TestName::CVAR_001 => points_to_ao_indices(&point_lists::cvar_001(profile).ao),
        TestName::FPF_001 => points_to_ao_indices(&point_lists::fpf_001(profile).ao),
        TestName::VV_001 => points_to_ao_indices(&point_lists::vv_001(profile).ao),
        TestName::WV_001 => points_to_ao_indices(&point_lists::wv_001(profile).ao),
        TestName::PFC_001 => points_to_ao_indices(&point_lists::pfc_001(profile).ao),
        TestName::PSIG_001 => points_to_ao_indices(&point_lists::psig_001(profile).ao),
        TestName::SCHED_001 => points_to_ao_indices(&point_lists::sched_001(profile).ao),
        TestName::SCHED_002 => points_to_ao_indices(&point_lists::sched_002(profile).ao),
    };

    ao_uids
        .iter()
        .any(|index| ao_indices_in_profile.contains(index) && !is_multiplex_selector_ao(*index))
}

fn points_to_ao_indices(ao_points: &[AoPoint]) -> Vec<u16> {
    ao_points.iter().map(|point| point.point_index).collect()
}

/// Generate the full list of test scenarios with expected outcomes derived from the
/// given profile. The lower/upper bounds expected tests are computed dynamically:
/// any test whose AO point list intersects the profile's AO points is expected to fail,
/// since `update_profile` will set those AO writes out of range.
pub fn generate_scenarios(profile: &Validated<PicsProfile>) -> Vec<Scenario> {
    let profile_index = ProfileIndex::from_profile(profile);
    let ao_indices_in_profile: HashSet<u16> = profile
        .ao
        .all_ao_points()
        .iter()
        .map(|pt| pt.point_index)
        .collect();

    let bounds_expected_tests: Vec<ExpectedTest> = TestName::iter()
        .map(|test_id| {
            let should_pass =
                !test_has_ao_points_in_profile(&test_id, &ao_indices_in_profile, &profile_index);
            ExpectedTest {
                test_id,
                should_pass,
                applicability_note: should_pass
                    .then(|| "Bounds check N/A: no eligible AO points.".to_string()),
            }
        })
        .collect();

    let curve_expected_tests: Vec<ExpectedTest> = TestName::iter()
        .map(|test_id| {
            let should_pass = !matches!(
                test_id,
                TestName::CURVE_001 | TestName::CURVE_002 | TestName::CURVE_003
            );
            ExpectedTest {
                test_id,
                should_pass,
                applicability_note: None,
            }
        })
        .collect();

    let schedule_expected_tests: Vec<ExpectedTest> = TestName::iter()
        .map(|test_id| {
            let should_pass = !matches!(test_id, TestName::SCHED_001 | TestName::SCHED_002);
            ExpectedTest {
                test_id,
                should_pass,
                applicability_note: None,
            }
        })
        .collect();

    vec![
        Scenario {
            id: ScenarioId::Configuration.as_str().to_string(),
            name: "Configuration".to_string(),
            description: "Use base PICS and verify the outstation has accurately ingested PICS."
                .to_string(),
            expected_tests: all_pass(),
        },
        Scenario {
            id: ScenarioId::ModifyPoints.as_str().to_string(),
            name: "Modify Points".to_string(),
            description:
                "Update all points that can be modified within boundaries, including curves and \
                schedules if supported."
                    .to_string(),
            expected_tests: all_pass(),
        },
        Scenario {
            id: ScenarioId::Unsupported.as_str().to_string(),
            name: "Unsupported".to_string(),
            description: "Attempt to modify all points that are not supported.".to_string(),
            expected_tests: all_fail(),
        },
        Scenario {
            id: ScenarioId::BelowLowerBounds.as_str().to_string(),
            name: "Lower Bounds".to_string(),
            description: "Attempt to modify all points outside their minimum values. \
                Only tests with eligible AO points are expected to fail."
                .to_string(),
            expected_tests: bounds_expected_tests.clone(),
        },
        Scenario {
            id: ScenarioId::AboveUpperBounds.as_str().to_string(),
            name: "Upper Bounds".to_string(),
            description: "Attempt to modify all points outside their maximum values. \
                Only tests with eligible AO points are expected to fail."
                .to_string(),
            expected_tests: bounds_expected_tests,
        },
        Scenario {
            id: ScenarioId::MaxCurves.as_str().to_string(),
            name: "Max Curves".to_string(),
            description: "Attempt to add more than the maximum number of curves.".to_string(),
            expected_tests: curve_expected_tests,
        },
        Scenario {
            id: ScenarioId::MaxSchedules.as_str().to_string(),
            name: "Max Schedules".to_string(),
            description: "Attempt to add more than the maximum number of schedules.".to_string(),
            expected_tests: schedule_expected_tests,
        },
        Scenario {
            id: ScenarioId::Functional.as_str().to_string(),
            name: "Functional".to_string(),
            description:
                "Allow EUT to run until all scheduled modes have finished without repeating."
                    .to_string(),
            // Functional scenarios have no pre-defined pass/fail expectations — they are
            // observational runs where the operator validates the output manually.
            expected_tests: vec![],
        },
        Scenario {
            id: ScenarioId::FunctionalRepeat.as_str().to_string(),
            name: "Functional Repeat".to_string(),
            description:
                "Allow EUT to run until all schedules have finished, with each one repeating once."
                    .to_string(),
            // See comment on Functional above.
            expected_tests: vec![],
        },
    ]
}

#[cfg(test)]
mod tests {
    use common::profile::validation::Validated;

    use super::*;

    fn load_default_profile() -> Validated<PicsProfile> {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../data/profiles/full.json");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
        let parsed_profile: PicsProfile = serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("failed to parse {}: {e}", path.display()));

        PicsProfile::into_validated(parsed_profile)
            .unwrap_or_else(|e| panic!("profile validation failed: {e:?}"))
    }

    #[test]
    fn test_scenario_id_round_trips() {
        let ids = [
            ScenarioId::Configuration,
            ScenarioId::ModifyPoints,
            ScenarioId::Unsupported,
            ScenarioId::BelowLowerBounds,
            ScenarioId::AboveUpperBounds,
            ScenarioId::MaxCurves,
            ScenarioId::MaxSchedules,
            ScenarioId::Functional,
            ScenarioId::FunctionalRepeat,
        ];
        for id in &ids {
            let s = id.as_str();
            let parsed: ScenarioId = s.parse().expect("should parse back");
            assert_eq!(*id, parsed);
        }
    }

    #[test]
    fn test_configuration_all_pass() {
        let profile = load_default_profile();
        let scenarios = generate_scenarios(&profile);
        let config = scenarios.iter().find(|s| s.id == "configuration").unwrap();
        assert!(config.expected_tests.iter().all(|t| t.should_pass));
    }

    #[test]
    fn test_unsupported_all_fail() {
        let profile = load_default_profile();
        let scenarios = generate_scenarios(&profile);
        let unsupported = scenarios.iter().find(|s| s.id == "unsupported").unwrap();
        assert!(
            !unsupported.expected_tests.is_empty(),
            "unsupported should have expected tests"
        );
        assert!(unsupported.expected_tests.iter().all(|t| !t.should_pass));
    }

    #[test]
    fn test_lower_bounds_cvar_fails() {
        let profile = load_default_profile();
        let scenarios = generate_scenarios(&profile);
        let lower = scenarios.iter().find(|s| s.id == "lower_bounds").unwrap();
        let cvar = lower
            .expected_tests
            .iter()
            .find(|t| t.test_id == TestName::CVAR_001);
        // CVAR_001 has AO points → should fail
        assert_eq!(cvar.map(|t| t.should_pass), Some(false));
    }

    #[test]
    fn test_lower_bounds_mon_passes() {
        let profile = load_default_profile();
        let scenarios = generate_scenarios(&profile);
        let lower = scenarios.iter().find(|s| s.id == "lower_bounds").unwrap();
        let mon = lower
            .expected_tests
            .iter()
            .find(|t| t.test_id == TestName::MON_001);
        // MON_001 has no AO points → should pass
        assert_eq!(mon.map(|t| t.should_pass), Some(true));
    }

    #[test]
    fn test_bounds_applicability_and_expected_failures() {
        let scenarios = generate_scenarios(&load_default_profile());
        for scenario in &scenarios {
            if !matches!(scenario.id.as_str(), "lower_bounds" | "upper_bounds") {
                assert!(
                    scenario
                        .expected_tests
                        .iter()
                        .all(|t| t.applicability_note.is_none())
                );
                continue;
            }

            let expected_failures: HashSet<_> = scenario
                .expected_tests
                .iter()
                .filter(|t| !t.should_pass)
                .map(|t| t.test_id)
                .collect();
            assert_eq!(
                expected_failures,
                HashSet::from([
                    TestName::RESIDUAL,
                    TestName::CONN_001,
                    TestName::SERV_001,
                    TestName::VRT_001,
                    TestName::FRT_001,
                    TestName::FW_001,
                    TestName::DRCS_001,
                    TestName::DVW_001,
                    TestName::APL_001,
                    TestName::CHG_001,
                    TestName::CCM_001,
                    TestName::APR1_001,
                    TestName::APR2_001,
                    TestName::APR3_001,
                    TestName::AGC_001,
                    TestName::APS_001,
                    TestName::VW_001,
                    TestName::FWC_001,
                    TestName::CVAR_001,
                    TestName::FPF_001,
                    TestName::VV_001,
                    TestName::WV_001,
                    TestName::PFC_001,
                    TestName::PSIG_001,
                    TestName::SCHED_001,
                    TestName::SCHED_002,
                ])
            );

            // Covers MON/ALARM (no AO points) and CURVE tests (selectors only).
            for test in &scenario.expected_tests {
                assert_eq!(
                    test.applicability_note.as_deref(),
                    test.should_pass
                        .then_some("Bounds check N/A: no eligible AO points.")
                );
            }
        }
    }

    #[test]
    fn test_bounds_applicability_when_ao_points_are_absent_from_profile() {
        let mut profile = load_default_profile().into_unvalidated();
        profile.ao = Default::default();
        let profile = PicsProfile::into_validated(profile).unwrap();
        for scenario in generate_scenarios(&profile)
            .into_iter()
            .filter(|s| matches!(s.id.as_str(), "lower_bounds" | "upper_bounds"))
        {
            assert!(
                scenario
                    .expected_tests
                    .iter()
                    .any(|test| test.test_id == TestName::RESIDUAL)
            );
            assert!(scenario.expected_tests.iter().all(|test| {
                test.should_pass
                    && test.applicability_note.as_deref()
                        == Some("Bounds check N/A: no eligible AO points.")
            }));
        }
    }

    #[test]
    fn test_legacy_expected_test_omits_applicability_note() {
        let json = serde_json::json!({ "test_id": "MON_001", "should_pass": true });
        let expected: ExpectedTest = serde_json::from_value(json.clone()).unwrap();
        assert!(expected.applicability_note.is_none());
        assert_eq!(serde_json::to_value(expected).unwrap(), json);
    }
}
