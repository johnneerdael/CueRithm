//! Native DJ library format adapters. No installed JavaScript or Python runtime.
mod model;
pub mod path_serde;
pub use model::*;
pub mod discovery;
pub mod djay_pro;
pub mod engine_dj;
pub mod serato4;
mod validation;
pub mod virtual_dj;
pub use validation::validate_library;
mod path_repair;
mod private_cleanup;
mod sync;
pub use path_repair::*;
pub mod transaction;
pub use sync::*;
pub mod formatting;
pub mod pioneer_usb;
mod smart_rules;
pub mod timing;
pub use smart_rules::smart_fallbacks;
pub mod ownership;
/// Invalidate acknowledgement caches when native serialization or rule semantics change.
pub fn implementation_sha256() -> &'static str {
    env!("CUERITHM_DJ_SYNC_IMPLEMENTATION")
}
