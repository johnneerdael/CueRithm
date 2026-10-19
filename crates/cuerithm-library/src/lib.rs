//! Durable local library and background preparation.
#[cfg(feature = "autotag")]
mod artifacts;
#[cfg(feature = "autotag")]
mod cleanup;
#[cfg(feature = "autotag")]
pub use cleanup::{
    CleanupArtistValue, CleanupBatchResult, CleanupCatalog, CleanupCatalogValue, CleanupFailure,
    CleanupHistory, CleanupPreview, CleanupProposal, CleanupScope, CleanupService,
};
#[cfg(feature = "autotag")]
mod album_context;
mod analysis_cache;
mod jobs;
mod metadata;
mod native_reconcile;
pub use native_reconcile::*;
mod native_sync;
pub use native_sync::*;
mod prepare;
mod publication;
mod rekordbox_cues;
pub use rekordbox_cues::*;
mod retained;
pub use retained::{RetainedDisposal, RetainedOriginal, RetainedPreview};
mod publish_files;
#[cfg(feature = "autotag")]
mod tagging;
#[cfg(feature = "autotag")]
pub use tagging::TaggingStorage;
pub mod watch;
pub use prepare::{PreparationEvent, PreparationService, ServiceStatus};
mod playlists;
pub use playlists::*;
mod maintenance;
mod model;
pub use maintenance::*;
pub mod playback;
mod settings;
mod store;
pub mod waveform;
pub use model::*;
pub use settings::*;
pub use store::Library;
