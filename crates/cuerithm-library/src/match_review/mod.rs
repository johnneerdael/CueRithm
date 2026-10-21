//! Explicit source/configuration-bound catalog choices; preparation owns publication.
mod model;
pub use model::*;
mod fields;
mod store;
#[cfg(test)]
mod tests;
mod worker;
mod workflow;

pub(crate) mod effective;
