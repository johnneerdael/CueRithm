//! Durable local library and background preparation.
#[cfg(feature = "autotag")]
mod artifacts;
#[cfg(feature = "autotag")]
mod cleanup;
#[cfg(feature = "autotag")]
pub use cleanup::{
    CleanupBatchResult, CleanupFailure, CleanupHistory, CleanupPreview, CleanupProposal,
    CleanupScope, CleanupService,
};
#[cfg(feature = "autotag")]
mod album_context;
mod analysis_cache;
mod jobs;
mod metadata;
mod prepare;
#[cfg(feature = "autotag")]
mod publish_files;
#[cfg(feature = "autotag")]
mod tagging;
#[cfg(feature = "autotag")]
pub use tagging::TaggingStorage;
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
