//! Offline, bundled common-format decoding and playable audio clocks.
mod clock;
mod decode;
mod probe;
mod runtime;
pub use clock::AudioClock;
pub use decode::PreparedAudio;
pub use probe::MediaInfo;
pub use runtime::MediaRuntime;
pub use runtime::sha256 as file_sha256;

pub fn source_sha256() -> String {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    for text in [
        include_str!("lib.rs"),
        include_str!("clock.rs"),
        include_str!("decode.rs"),
        include_str!("probe.rs"),
        include_str!("runtime.rs"),
        include_str!("../Cargo.toml"),
    ] {
        hash.update((text.len() as u64).to_le_bytes());
        hash.update(text);
    }
    format!("{:x}", hash.finalize())
}
