use common::conformance_testing::message_structure::TestName;
use std::collections::HashSet;

use common::profile::validation::Validated;
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;

use common::conformance_testing::point_lists;
use common::profile::PicsProfile;

use crate::update_profile::is_multiplex_selector_ao;

/// A single expected test result within a scenario.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ExpectedTest {
    pub test_id: TestName,
    pub should_pass: bool,
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
        })
        .collect()
}

fn all_fail() -> Vec<ExpectedTest> {
    TestName::iter()
        .map(|id: TestName| ExpectedTest {
            test_id: id,
            should_pass: false,
        })
        .collect()
}

/// Returns true if the test has at least one AO point that exists in the profile.
/// Tests with AO points will fail for lower/upper bounds scenarios because those
/// scenarios set AO write values outside the outstation's accepted range.
fn test_has_ao_points_in_profile(test_id: &TestName, ao_indices_in_profile: &HashSet<u16>) -> bool {
    let ao_uids = match test_id {
        TestName::MON_001 => points_to_ao_indices(point_lists::mon_001().ao),
        TestName::ALARM_001 => points_to_ao_indices(point_lists::alarm_001().ao),
        TestName::CONN_001 => points_to_ao_indices(point_lists::conn_001().ao),
        TestName::SERV_001 => points_to_ao_indices(point_lists::serv_001().ao),
        TestName::OP_001 => points_to_ao_indices(point_lists::op_001().ao),
        TestName::CURVE_001 => points_to_ao_indices(point_lists::curve_001().ao),
        TestName::CURVE_002 => points_to_ao_indices(point_lists::curve_002().ao),
        TestName::CURVE_003 => points_to_ao_indices(point_lists::curve_003().ao),
        TestName::VRT_001 => points_to_ao_indices(point_lists::vrt_001().ao),
        TestName::FRT_001 => points_to_ao_indices(point_lists::frt_001().ao),
        TestName::FW_001 => points_to_ao_indices(point_lists::fw_001().ao),
        TestName::DRCS_001 => points_to_ao_indices(point_lists::drcs_001().ao),
        TestName::DVW_001 => points_to_ao_indices(point_lists::dvw_001().ao),
        TestName::APL_001 => points_to_ao_indices(point_lists::apl_001().ao),
        TestName::CHG_001 => points_to_ao_indices(point_lists::chg_001().ao),
        TestName::CCM_001 => points_to_ao_indices(point_lists::ccm_001().ao),
        TestName::APR1_001 => points_to_ao_indices(point_lists::apr1_001().ao),
        TestName::APR2_001 => points_to_ao_indices(point_lists::apr2_001().ao),
        TestName::APR3_001 => points_to_ao_indices(point_lists::apr3_001().ao),
        TestName::AGC_001 => points_to_ao_indices(point_lists::agc_001().ao),
        TestName::APS_001 => points_to_ao_indices(point_lists::aps_001().ao),
        TestName::VW_001 => points_to_ao_indices(point_lists::vw_001().ao),
        TestName::FWC_001 => points_to_ao_indices(point_lists::fwc_001().ao),
        TestName::CVAR_001 => points_to_ao_indices(point_lists::cvar_001().ao),
        TestName::FPF_001 => points_to_ao_indices(point_lists::fpf_001().ao),
        TestName::VV_001 => points_to_ao_indices(point_lists::vv_001().ao),
        TestName::WV_001 => points_to_ao_indices(point_lists::wv_001().ao),
        TestName::PFC_001 => points_to_ao_indices(point_lists::pfc_001().ao),
        TestName::PSIG_001 => points_to_ao_indices(point_lists::psig_001().ao),
        TestName::SCHED_001 => points_to_ao_indices(point_lists::sched_001().ao),
        TestName::SCHED_002 => points_to_ao_indices(point_lists::sched_002().ao),
    };

    ao_uids
        .iter()
        .any(|index| ao_indices_in_profile.contains(index) && !is_multiplex_selector_ao(*index))
}

fn points_to_ao_indices(ao_uids: &[common::uids::ao_uid::AoUid]) -> Vec<u16> {
    ao_uids.iter().map(|uid| *uid as u16).collect()
}

/// Generate the full list of test scenarios with expected outcomes derived from the
/// given profile. The lower/upper bounds expected tests are computed dynamically:
/// any test whose AO point list intersects the profile's AO points is expected to fail,
/// since `update_profile` will set those AO writes out of range.
pub fn generate_scenarios(profile: &Validated<PicsProfile>) -> Vec<Scenario> {
    let ao_indices_in_profile: HashSet<u16> = profile
        .ao
        .all_ao_points()
        .iter()
        .map(|pt| pt.point_index)
        .collect();

    // Unused for now until we fix the lower/upper bounds scenarios
    let bounds_expected_tests: Vec<ExpectedTest> = TestName::iter()
        .map(|test_id| {
            let should_pass = !test_has_ao_points_in_profile(&test_id, &ao_indices_in_profile);
            ExpectedTest {
                test_id,
                should_pass,
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
            }
        })
        .collect();

    let schedule_expected_tests: Vec<ExpectedTest> = TestName::iter()
        .map(|test_id| {
            let should_pass = !matches!(test_id, TestName::SCHED_001 | TestName::SCHED_002);
            ExpectedTest {
                test_id,
                should_pass,
            }
        })
        .collect();

    vec![
        Scenario {
            id: ScenarioId::Configuration.as_str().to_string(),
            name: "Outstation ingests PICS".to_string(),
            description: "Use base PICS and verify the outstation has accurately ingested PICS."
                .to_string(),
            expected_tests: all_pass(),
        },
        Scenario {
            id: ScenarioId::ModifyPoints.as_str().to_string(),
            name: "All points can be modified".to_string(),
            description:
                "Update all points that can be modified within boundaries, including curves and \
                schedules if supported."
                    .to_string(),
            expected_tests: all_pass(),
        },
        Scenario {
            id: ScenarioId::Unsupported.as_str().to_string(),
            name: "Unsupported points cannot be modified".to_string(),
            description: "Attempt to modify all points that are not supported.".to_string(),
            expected_tests: all_fail(),
        },
        Scenario {
            id: ScenarioId::BelowLowerBounds.as_str().to_string(),
            name: "Points cannot be modified below their minimum".to_string(),
            description: "Attempt to modify all points outside their minimum values.".to_string(),
            expected_tests: bounds_expected_tests.clone(),
        },
        Scenario {
            id: ScenarioId::AboveUpperBounds.as_str().to_string(),
            name: "Points cannot be modified above their maximum".to_string(),
            description: "Attempt to modify all points outside their maximum values.".to_string(),
            expected_tests: bounds_expected_tests,
        },
        Scenario {
            id: ScenarioId::MaxCurves.as_str().to_string(),
            name: "Curves beyond the maximum are rejected".to_string(),
            description: "Attempt to add more than the maximum number of curves.".to_string(),
            expected_tests: curve_expected_tests,
        },
        Scenario {
            id: ScenarioId::MaxSchedules.as_str().to_string(),
            name: "Schedules beyond the maximum are rejected".to_string(),
            description: "Attempt to add more than the maximum number of schedules.".to_string(),
            expected_tests: schedule_expected_tests,
        },
        Scenario {
            id: ScenarioId::Functional.as_str().to_string(),
            name: "All scheduled modes complete without repeating".to_string(),
            description:
                "Allow EUT to run until all scheduled modes have finished without repeating."
                    .to_string(),
            // Functional scenarios have no pre-defined pass/fail expectations — they are
            // observational runs where the operator validates the output manually.
            expected_tests: vec![],
        },
        Scenario {
            id: ScenarioId::FunctionalRepeat.as_str().to_string(),
            name: "All schedules complete after one repeat".to_string(),
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
    fn test_scenario_names_describe_the_expected_outcome() {
        let profile = load_default_profile();
        let scenarios = generate_scenarios(&profile);
        let expected_names = [
            ("configuration", "Outstation ingests PICS"),
            ("modify_points", "All points can be modified"),
            ("unsupported", "Unsupported points cannot be modified"),
            (
                "lower_bounds",
                "Points cannot be modified below their minimum",
            ),
            (
                "upper_bounds",
                "Points cannot be modified above their maximum",
            ),
            ("max_curves", "Curves beyond the maximum are rejected"),
            (
                "max_schedules",
                "Schedules beyond the maximum are rejected",
            ),
            (
                "functional",
                "All scheduled modes complete without repeating",
            ),
            (
                "functional_repeat",
                "All schedules complete after one repeat",
            ),
        ];

        for (id, name) in expected_names {
            let scenario = scenarios.iter().find(|scenario| scenario.id == id).unwrap();
            assert_eq!(scenario.name, name);
        }
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
}
