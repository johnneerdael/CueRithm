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
    for record in &records.rows {
        let serialized = serde_json::to_vec(record).unwrap();
        let restored: ObservationRecord = serde_json::from_slice(&serialized)
            .expect("Observation must roundtrip through durable storage");
        assert_eq!(&restored, record);
    }
    export_acceptance_fixture(snapshot, &records);
    (header, records)
}

/// Opt-in generation of portable, authored native inputs for the library's
/// public-service acceptance suite. This helper never visits user libraries.
pub fn export_acceptance_fixture(snapshot: &ImportSnapshot, records: &Records) {
    let Some(output) = std::env::var_os("CUERITHM_IMPORT_FIXTURE_EXPORT") else {
        return;
    };
    if records.tracks().is_empty() {
        return;
    }
    let mut artifacts = Vec::new();
    for artifact in &snapshot.artifacts {
        let path = snapshot.original_artifact_path(&artifact.id).unwrap();
        let Ok(relative) = path.strip_prefix(snapshot.source_root()) else {
            return;
        };
        artifacts.push(ArtifactRequest {
            id: artifact.id.clone(),
            relative_path: relative.to_owned(),
            kind: artifact.kind,
            selected_root: None,
        });
    }
    let name = serde_json::to_value(snapshot.provider)
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned();
    let root = std::path::PathBuf::from(output).join(&name);
    if std::fs::create_dir(&root).is_err() {
        return;
    }
    for (artifact, request) in snapshot.artifacts.iter().zip(&artifacts) {
        let target = root.join("source").join(&request.relative_path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::copy(snapshot.artifact(&artifact.id).unwrap(), target).unwrap();
    }
    std::fs::write(
        root.join("selection.json"),
        serde_json::to_vec_pretty(&artifacts).unwrap(),
    )
    .unwrap();
}
