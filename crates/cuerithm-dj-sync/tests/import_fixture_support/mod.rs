#![allow(dead_code)]
use cuerithm_dj_sync::import::*;
use cuerithm_media::CancellationToken;
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Default)]
pub struct Records {
    pub rows: Vec<ObservationRecord>,
    pub blobs: std::collections::BTreeMap<String, Vec<u8>>,
}
impl ObservationSink for Records {
    fn push(&mut self, row: ObservationRecord) -> anyhow::Result<()> {
        self.rows.push(row);
        Ok(())
    }
    fn put_blob(&mut self, bytes: &[u8], media_type: Option<&str>) -> anyhow::Result<BlobRef> {
        let sha256 = format!("{:x}", Sha256::digest(bytes));
        self.blobs.insert(sha256.clone(), bytes.to_vec());
        Ok(BlobRef {
            sha256,
            bytes: bytes.len() as u64,
            media_type: media_type.map(str::to_owned),
        })
    }
}
impl Records {
    pub fn tracks(&self) -> Vec<&TrackObservation> {
        self.rows
            .iter()
            .filter_map(|r| match r {
                ObservationRecord::Track(t) => Some(t.as_ref()),
                _ => None,
            })
            .collect()
    }
    pub fn collections(&self) -> Vec<&CollectionObservation> {
        self.rows
            .iter()
            .filter_map(|r| match r {
                ObservationRecord::Collection(c) => Some(c),
                _ => None,
            })
            .collect()
    }
    pub fn members(&self) -> Vec<(&str, &MembershipObservation)> {
        self.rows
            .iter()
            .filter_map(|r| match r {
                ObservationRecord::Membership {
                    collection_id,
                    occurrence,
                } => Some((collection_id.as_str(), occurrence)),
                _ => None,
            })
            .collect()
    }
}
pub fn snapshot(root: &Path, provider: SourceProvider, names: &[&str]) -> ImportSnapshot {
    let request = SourceCaptureRequest::from_selected_root(
        provider,
        root,
        names.iter().map(|n| ArtifactRequest::stable(n)).collect(),
    )
    .unwrap();
    capture_source(
        &request,
        root,
        &ImportLimits::default(),
        &CancellationToken::default(),
    )
    .unwrap()
}
pub fn read(provider: SourceProvider, snapshot: &ImportSnapshot) -> (ObservationHeader, Records) {
    let mut records = Records::default();
    let header = adapter(provider)
        .read(
            snapshot,
            &ReadOptions::default(),
            &mut records,
            &CancellationToken::default(),
        )
        .unwrap();
    let mut reader = records.rows.clone().into_iter();
    validate_observation(&header, &mut reader, &ImportLimits::default()).unwrap();
    snapshot
        .validate_source(&CancellationToken::default())
        .unwrap();
    let mut again = Records::default();
    let second = adapter(provider)
        .read(
            snapshot,
            &ReadOptions::default(),
            &mut again,
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(header.snapshot_hash, second.snapshot_hash);
    assert_eq!(records.rows, again.rows);
    (header, records)
}
