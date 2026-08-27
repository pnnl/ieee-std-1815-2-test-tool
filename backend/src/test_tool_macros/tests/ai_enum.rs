use test_tool_macros::{AiEnumFields, ai_enum};

#[derive(Debug, Clone, Copy, PartialEq)]
struct EngineeringF64(f64);

#[derive(Debug, Clone, PartialEq)]
struct ValidationErrors(Vec<String>);

/// Stub implementation for testing
impl ValidationErrors {
    fn new() -> Self {
        Self(Vec::new())
    }

    fn extend(&mut self, other: Self) {
        self.0.extend(other.0);
    }
}

/// Stub implementation for testing
trait AiEnum: Sized + Copy {
    fn from_int(value: i64) -> Option<Self>;

    fn try_from_value(value: EngineeringF64) -> Option<Self> {
        (value.0 == value.0 as i64 as f64).then(|| Self::from_int(value.0 as i64))?
    }

    fn from_value(value: EngineeringF64) -> Self {
        Self::try_from_value(value).expect("invalid enum value")
    }

    fn from_value_errors(value: EngineeringF64) -> ValidationErrors {
        if Self::try_from_value(value).is_some() {
            ValidationErrors::new()
        } else {
            ValidationErrors(vec![format!("invalid enum value: {}", value.0)])
        }
    }
}

/// Stub implementation for testing
#[derive(Debug, Clone, Copy)]
struct AiPoint(EngineeringF64);

impl AiPoint {
    fn value(&self) -> EngineeringF64 {
        self.0
    }
}

/// Stub implementation for testing
#[ai_enum]
pub enum ConnectionPointType {
    Unknown = 0,
    GridConnected = 1,
}

#[ai_enum]
/// Stub implementation for testing
pub enum CircuitPhases {
    Unknown = 0,
    ThreePhase = 3,
}

#[derive(AiEnumFields)]
/// Stub implementation for testing
struct Meter {
    #[ai_enum_field(ConnectionPointType)]
    connection_type: AiPoint,
    #[ai_enum_field(CircuitPhases)]
    circuit_phases: AiPoint,
    _ordinary_point: AiPoint,
}

#[test]
fn ai_enum_adds_integer_conversion_and_standard_derives() {
    assert_eq!(
        ConnectionPointType::from_int(1),
        Some(ConnectionPointType::GridConnected)
    );
    assert_eq!(ConnectionPointType::from_int(2), None);
    assert_eq!(
        ConnectionPointType::GridConnected.to_string(),
        "GridConnected"
    );

    let serialized = serde_json::to_string(&ConnectionPointType::GridConnected).unwrap();
    assert_eq!(serialized, "\"GridConnected\"");
}

#[test]
fn ai_enum_fields_generates_getters_and_validation() {
    let meter = Meter {
        connection_type: AiPoint(EngineeringF64(2.0)),
        circuit_phases: AiPoint(EngineeringF64(3.0)),
        _ordinary_point: AiPoint(EngineeringF64(99.0)),
    };

    assert_eq!(meter.circuit_phases_value(), CircuitPhases::ThreePhase);
    assert_eq!(
        meter.collect_enum_errors(),
        ValidationErrors(vec!["invalid enum value: 2".to_string()])
    );

    let valid_meter = Meter {
        connection_type: AiPoint(EngineeringF64(1.0)),
        ..meter
    };
    assert_eq!(
        valid_meter.connection_type_value(),
        ConnectionPointType::GridConnected
    );
}
