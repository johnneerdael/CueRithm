mod id3;
use id3::read_id3;
mod footer;
mod native;
use super::{BlobRef, ImportIssue, ImportLimits, Presence};
use anyhow::{Context, Result, ensure};
use cuerithm_media::CancellationToken;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawTagRecord {
    pub namespace: String,
    pub native_id: String,
    pub ordinal: usize,
    pub flags: u16,
    pub identity: BTreeMap<String, String>,
    pub payload: BlobRef,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtworkObservation {
    pub mime_type: Option<String>,
    pub picture_type: Option<u32>,
    pub description: Option<String>,
    pub ordinal: usize,
    pub payload: BlobRef,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTagObservation {
    pub source_sha256: String,
    pub source_bytes: u64,
    pub normalized: BTreeMap<String, Vec<String>>,
    pub records: Vec<RawTagRecord>,
    pub blobs: BTreeMap<String, Vec<u8>>,
    pub issues: Vec<ImportIssue>,
    pub artwork: Vec<ArtworkObservation>,
    #[serde(skip)]
    blob_bytes: u64,
}
impl FileTagObservation {
    fn blob(&mut self, bytes: &[u8], limits: &ImportLimits) -> Result<BlobRef> {
        let sha256 = format!("{:x}", Sha256::digest(bytes));
        let total = self.blob_bytes;
        ensure!(
            bytes.len() as u64 <= limits.blob_bytes
                && (self.blobs.contains_key(&sha256)
                    || total
                        .checked_add(bytes.len() as u64)
                        .is_some_and(|n| n <= limits.blob_bytes)),
            "Tag capture byte limit exceeded"
        );
        if !self.blobs.contains_key(&sha256) {
            self.blob_bytes += bytes.len() as u64;
        }
        self.blobs
            .entry(sha256.clone())
            .or_insert_with(|| bytes.to_vec());
        Ok(BlobRef {
            sha256,
            bytes: bytes.len() as u64,
            media_type: None,
        })
    }
    fn add_artwork(
        &mut self,
        mime_type: Option<String>,
        picture_type: Option<u32>,
        description: Option<String>,
        bytes: &[u8],
        limits: &ImportLimits,
    ) -> Result<()> {
        let payload = self.blob(bytes, limits)?;
        self.artwork.push(ArtworkObservation {
            mime_type,
            picture_type,
            description,
            ordinal: self.artwork.len(),
            payload,
        });
        Ok(())
    }
    fn record(
        &mut self,
        namespace: &str,
        id: &str,
        flags: u16,
        bytes: &[u8],
        limits: &ImportLimits,
    ) -> Result<()> {
        ensure!(
            self.records.len() < 65536,
            "Tag record count limit exceeded"
        );
        let payload = self.blob(bytes, limits)?;
        self.records.push(RawTagRecord {
            namespace: namespace.into(),
            native_id: id.into(),
            ordinal: self.records.len(),
            flags,
            identity: BTreeMap::new(),
            payload,
        });
        Ok(())
    }
}
/// Absence alone permits a file-tag fallback. Explicit empty/null/deleted values do not.
pub fn select_imported_value<T: Clone>(
    native: &Presence<T>,
    file: Option<&Presence<T>>,
) -> Presence<T> {
    if matches!(native, Presence::Absent) {
        file.cloned().unwrap_or(Presence::Absent)
    } else {
        native.clone()
    }
}
pub fn read_file_tags(
    path: &Path,
    limits: &ImportLimits,
    cancel: &CancellationToken,
) -> Result<FileTagObservation> {
    cancel.check()?;
    let mut file = File::open(path)?;
    let before = file.metadata()?;
    ensure!(before.is_file(), "Media reference is not a regular file");
    let mut sha = Sha256::new();
    let mut buffer = [0; 65536];
    let mut read = 0u64;
    loop {
        cancel.check()?;
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        read = read.checked_add(n as u64).context("Media size overflow")?;
        ensure!(
            read <= limits.snapshot_bytes,
            "Media fingerprint budget exceeded"
        );
        sha.update(&buffer[..n]);
    }
    let mut out = FileTagObservation {
        source_sha256: format!("{:x}", sha.finalize()),
        source_bytes: read,
        normalized: BTreeMap::new(),
        records: Vec::new(),
        blobs: BTreeMap::new(),
        issues: Vec::new(),
        artwork: Vec::new(),
        blob_bytes: 0,
    };
    file.seek(SeekFrom::Start(0))?;
    let mut magic = [0; 12];
    let n = file.read(&mut magic)?;
    file.seek(SeekFrom::Start(0))?;
    let mpeg = n >= 2 && magic[0] == 0xff && magic[1] & 0xe0 == 0xe0;
    let id3 = n >= 3 && &magic[..3] == b"ID3";
    if id3 {
        read_id3(&mut file, &mut out, limits, cancel)?;
    } else if !mpeg {
        ensure!(
            native::read(&mut file, &magic[..n], &mut out, limits, cancel)?,
            "Native tag capture for this container is not implemented"
        );
    }
    if mpeg || id3 {
        footer::read(&mut file, &mut out, limits, cancel)?;
    }
    cancel.check()?;
    let after = file.metadata()?;
    let current = path.metadata()?;
    ensure!(
        read == before.len()
            && before.len() == after.len()
            && before.modified().ok() == after.modified().ok()
            && current.len() == before.len()
            && current.modified().ok() == before.modified().ok(),
        "Media changed during tag capture"
    );
    Ok(out)
}
