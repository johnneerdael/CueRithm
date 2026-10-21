mod model;
pub use model::*;
mod assignments;
mod service;
pub(crate) use assignments::PreparedPreparationAssignments;
mod portable;
mod resolution;
pub const MAX_PREPARATION_PRESET_DOCUMENT_BYTES: usize = 1_048_576;
#[cfg(test)]
mod tests;
