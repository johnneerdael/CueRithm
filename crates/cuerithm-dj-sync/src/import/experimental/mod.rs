//! Disabled-by-default, synthetic fixture-only persistence experiments.
//! There is no live-root constructor, production capability or desktop command.
mod dj_studio;
mod journal;
pub use dj_studio::*;
