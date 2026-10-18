//! Durable local library and background preparation.
mod jobs;
mod prepare;
pub mod watch;
pub use prepare::{PreparationEvent, PreparationService, ServiceStatus};
mod model;
mod settings;
mod store;
pub use model::*;
pub use settings::*;
pub use store::Library;
