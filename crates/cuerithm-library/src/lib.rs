//! Durable local library and background preparation.
mod jobs;
mod metadata;
mod prepare;
#[cfg(feature = "autotag")]
mod publish_files;
#[cfg(feature = "autotag")]
mod tagging;
pub mod watch;
pub use prepare::{PreparationEvent, PreparationService, ServiceStatus};
mod model;
pub mod playback;
mod settings;
mod store;
pub mod waveform;
pub use model::*;
pub use settings::*;
pub use store::Library;
