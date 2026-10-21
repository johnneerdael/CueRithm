//! Durable local library and background preparation.
#[cfg(feature = "autotag")]
mod artifact_cleanup;
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
mod native_sync;
pub use native_sync::*;
mod native_reconcile;
pub use native_reconcile::*;
mod native_playlist_sync;
pub use native_playlist_sync::*;
mod rekordbox_sync_service;
pub use rekordbox_sync_service::*;
mod prepare;
mod publication;
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
#[cfg(feature = "autotag")]
mod match_evidence;
#[cfg(feature = "autotag")]
mod metadata_edit;
#[cfg(feature = "autotag")]
pub use metadata_edit::*;
#[cfg(feature = "autotag")]
mod match_review;
#[cfg(feature = "autotag")]
pub use match_review::*;
mod model;
pub mod organization;
pub use organization::service::*;
mod organization_references;
pub use maintenance::*;
#[cfg(feature = "autotag")]
mod catalog_access;
#[cfg(feature = "autotag")]
pub use match_evidence::PreparationMatchSnapshot;
pub use organization_references::{
    OrganizationReferenceGuard, OrganizationReferenceInfo, OrganizationReferenceSnapshot,
};
pub mod playback;
mod settings;
mod store;
pub mod waveform;
pub use model::*;
pub use settings::*;
pub use store::Library;

mod dj_sync;
pub use dj_sync::*;
mod dj_sync_service;
pub use dj_sync_service::{DjSyncHistoryEntry, DjSyncRequest};

mod compatible;
pub use compatible::*;

mod tracklist;
pub use tracklist::*;

mod manual_publication;
pub use manual_publication::*;

mod library_backup;
pub use library_backup::*;
