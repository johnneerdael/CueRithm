mod deletion;
#[cfg(feature = "autotag")]
mod donor_state;
#[cfg(feature = "autotag")]
mod donors;
mod files;
mod merge;
#[cfg(feature = "autotag")]
pub use donors::DuplicateMergeService;
mod model;
mod records;
mod review;
mod scan;
mod trash;
pub use model::*;
pub use trash::{FileTrash, SystemTrash};

mod broken;
mod removal;
mod unused;

#[cfg(feature = "autotag")]
pub(crate) use removal::ensure_unused_unreserved;
