#[macro_use]
extern crate log;
pub mod parser;
pub use parser::TemplateParser;
pub struct RenamerConfig {
    pub separator: String,
}
