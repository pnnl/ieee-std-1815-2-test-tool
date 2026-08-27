/// Logging system for conformance testing results.
///
/// Chronology
/// - A conformance test is defined in code, e.g., `validate_mon_001`
/// - The test is wrapped in a tracing span with the test name. This happens
///   automatically in run_outstation_simulation_steps.
/// - The test runs
/// - The test calls `log_conformance_result` with the serialized message
/// - A custom `ConformanceTrackingLayer` captures these events and stores them in memory
use std::io::Write;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use tracing::field::Field;
#[rustfmt::skip]
use tracing::{debug, Event, Subscriber};
#[rustfmt::skip]
use tracing_subscriber::{layer::Context, registry::LookupSpan, Layer};

use crate::conformance_testing::message_structure::{
    SunSpecComment, SunSpecMessage, TestEvent, TestName,
};

pub const CONFORMANCE_TESTING_TARGET: &str = "conformance";
pub const EVENT_FIELD_NAME: &str = "event_json";

/// Logs a conformance test result as a structured tracing event.
pub fn log_conformance_result(pass: bool, comments: Vec<SunSpecComment>) {
    // TODO: passing in "pass" is unnecesary
    let message = TestEvent { pass, comments };

    if let Ok(event_json) = serde_json::to_string(&message) {
        tracing::debug!(target: CONFORMANCE_TESTING_TARGET, event_json = %event_json);
    }
}

/// A tracing layer that captures conformance testing events and stores them in memory.
pub struct ConformanceTrackingLayer {
    messages: Arc<Mutex<Vec<SunSpecMessage>>>,
    scenario_id: Arc<Mutex<String>>,
}

impl ConformanceTrackingLayer {
    pub fn new(messages: Arc<Mutex<Vec<SunSpecMessage>>>, scenario_id: Arc<Mutex<String>>) -> Self {
        Self {
            messages,
            scenario_id,
        }
    }
}

impl<S> Layer<S> for ConformanceTrackingLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::Id,
        ctx: Context<'_, S>,
    ) {
        let mut visitor = TestNameVisitor::default();
        attrs.record(&mut visitor);
        if let Some(test_name) = visitor.test_name {
            if let Some(span) = ctx.span(id) {
                span.extensions_mut().insert(test_name);
            }
        }
    }

    /// Handles events with target "conformance" and extracts the test result messages.
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        if event.metadata().target() != "conformance" {
            return;
        }

        // Find the span with the test_name field
        // here and add it as the test name in the message
        let Some(scope) = ctx.event_scope(event) else {
            debug!("No scope found for event. Returning.");
            return; // Not logged within the context of a test span, ignore
        };

        let Some(test_name) = scope
            .from_root()
            .find_map(|span| span.extensions().get::<TestName>().copied())
        else {
            debug!("No test name found in span. Returning.");
            return;
        };

        let mut visitor = ConformanceVisitor::default();
        event.record(&mut visitor);

        let Some(data_json) = visitor.event_json else {
            debug!("No event_json field found in event. Returning.");
            return;
        };

        let Ok(message) = serde_json::from_str::<TestEvent>(&data_json) else {
            debug!("Unable to parse event_json into TestEvent. Returning.");
            return;
        };

        if let Ok(mut messages) = self.messages.lock() {
            let current_scenario_id = self
                .scenario_id
                .lock()
                .expect("scenario_id lock poisoned")
                .clone();

            let message = SunSpecMessage {
                test: test_name,
                scenario_id: current_scenario_id,
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("Time went backwards")
                    .as_secs(),
                pass: message.pass,
                comments: message.comments,
            };

            // Print out the message in a way that can be easily parsed
            // by the test runner.
            if let Ok(json) = serde_json::to_string(&message) {
                println!("MESA_CONFORMANCE_EVENT:{json}");
                // Stdout is block-buffered when piped. Flush explicitly so the
                // backend receives each result immediately rather than losing it
                // if the process is killed (SIGKILL) before the buffer fills.
                let _ = std::io::stdout().flush();
            }

            messages.push(message);
        }
    }
}

/// Parses events to extract conformance test results.
#[derive(Default)]
struct ConformanceVisitor {
    event_json: Option<String>,
}

impl tracing::field::Visit for ConformanceVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "event_json" {
            self.event_json = Some(format!("{value:?}"));
        }
    }
}

// Parses events to extract the test name from spans.
#[derive(Default)]
struct TestNameVisitor {
    test_name: Option<TestName>,
}

impl tracing::field::Visit for TestNameVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "test_name" {
            self.test_name = Some(
                TestName::from_str(format!("{value:?}").as_str())
                    .expect("Invalid test name in span"),
            );
        }
    }
}
