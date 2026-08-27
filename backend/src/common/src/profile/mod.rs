pub mod curve;
pub mod curve_db;
pub mod databases_config;
pub mod indexed_db;
#[allow(clippy::module_inception)]
pub mod profile;
pub mod scale_curve;
pub mod schedule_bc_db;
pub mod schedule_db;
pub mod validation;
pub mod values;

mod ai_meter;
mod ai_schedule;
mod enums;

pub use curve_db::CurveDatabase;
pub use databases_config::{DEFAULT_MAX_DATABASE_ENTRIES, DatabasesConfig};
pub use indexed_db::{AiValue, DatabaseEntry};
pub use profile::{ActionType, CurveType, PicsProfile, ProfileIndex};
pub use schedule_bc_db::ScheduleBCDatabase;
pub use schedule_db::ScheduleDatabase;
