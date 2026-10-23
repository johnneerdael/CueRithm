//! Metadata-only portable library archives. Source archives are never extracted by name.
mod container;
mod format;
mod lexicon;
mod projection;
mod validate;
pub use container::{
    ArchiveFormat, detect_archive_format, read_portable_archive,
    read_portable_archive_with_control, write_portable_archive,
    write_portable_archive_with_control,
};
pub use format::*;
pub use lexicon::{
    ImportPreview, ImportPreviewSpooled, ImportReport, read_lexicon_archive,
    read_lexicon_archive_with_control, read_lexicon_spooled,
};
pub use validate::{parse_strict_json, parse_strict_json_with_budget, validate_logical_archive};

mod spool;
pub use spool::{SpoolArchive, read_portable_spooled, write_portable_spooled};
