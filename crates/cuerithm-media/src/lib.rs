//! Offline, bundled common-format decoding and playable audio clocks.
mod cancel;
mod clock;
mod decode;
mod fingerprint;
mod probe;
mod runtime;
pub use cancel::{CancellationToken, MediaTaskError};
pub use clock::AudioClock;
pub use decode::PreparedAudio;
pub use fingerprint::cancellable_sha256 as file_sha256_cancellable;
pub use fingerprint::{AudioFingerprint, FingerprintOutcome, UnsupportedAudio};
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
        include_str!("artwork.rs"),
        include_str!("runtime.rs"),
        include_str!("cancel.rs"),
        include_str!("fingerprint.rs"),
        include_str!("../Cargo.toml"),
    ] {
        hash.update((text.len() as u64).to_le_bytes());
        hash.update(text);
    }
    format!("{:x}", hash.finalize())
}
mod artwork;
pub use artwork::{Artwork, artwork_mime};
