#[cfg(feature = "autotag")]
mod donor_state;
#[cfg(feature = "autotag")]
mod donors;
mod files;
mod merge;
#[cfg(feature = "autotag")]
pub use donors::DuplicateMergeService;
mod model;
mod review;
mod scan;
pub use model::*;

mod broken;
mod removal;
mod unused;

#[cfg(feature = "autotag")]
pub(crate) use removal::ensure_unused_unreserved;
