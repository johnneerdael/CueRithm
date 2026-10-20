#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;
macro_rules! timestamp {
    () => {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    };
}
pub mod beatport;
pub mod http;
pub mod traxsource;

use std::io::Read;
pub trait BoundedResponse {
    fn bounded_text(self) -> anyhow::Result<String>;
    fn bounded_json<T: serde::de::DeserializeOwned>(self) -> anyhow::Result<T>;
}
impl BoundedResponse for reqwest::blocking::Response {
    fn bounded_text(self) -> anyhow::Result<String> {
        const MAX: u64 = 8 * 1024 * 1024;
        let response = self.error_for_status()?;
        anyhow::ensure!(
            response.content_length().is_none_or(|n| n <= MAX),
            "Catalog response exceeds size limit"
        );
        let mut bytes = Vec::new();
        response.take(MAX + 1).read_to_end(&mut bytes)?;
        anyhow::ensure!(
            bytes.len() as u64 <= MAX,
            "Catalog response exceeds size limit"
        );
        Ok(String::from_utf8(bytes)?)
    }
    fn bounded_json<T: serde::de::DeserializeOwned>(self) -> anyhow::Result<T> {
        Ok(serde_json::from_str(&self.bounded_text()?)?)
    }
}
