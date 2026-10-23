//! Read-only source adapters. D2 parser evidence never grants native write authority.
pub mod apple_music;
pub mod bookmark;
pub mod dj_studio;
pub mod dj_studio_records;
pub mod djay_codec;
pub mod djay_pro;
mod engine_codec;
pub mod engine_dj;
pub mod keyed_archive;
pub mod m3u;
pub mod mixed_in_key11;
pub mod mixxx;
mod mixxx_codec;
mod mixxx_schema;
mod pioneer_pdb;
pub mod pioneer_usb;
pub mod rekordbox;
pub mod serato3;
pub mod serato4;
pub mod serato_tags;
pub mod traktor;
pub mod virtual_dj;
mod xml;
use super::*;
use anyhow::{Context, Result, ensure};
use cuerithm_media::CancellationToken;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
};

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn profile(provider: SourceProvider, name: &str, groups: &[&str]) -> SourceProfile {
    SourceProfile {
        provider, profile_id: name.into(), schema_fingerprint: digest(name.as_bytes()),
        origin: BTreeMap::new(), issues: vec![],
        fields: groups.iter().map(|g| (g.to_string(), FieldCapability {
            read: true, write: false, lossless: false, evidence: EvidenceLevel::D2,
            reason: Some("Controlled parser fixtures; optional file-tag capture is read-only; native-app acceptance unrun".into()),
        })).collect(),
    }
}
fn header(
    snapshot: &ImportSnapshot,
    options: &ReadOptions,
    profile: &SourceProfile,
    instance: Option<&str>,
    tracks: u64,
    collections: u64,
    memberships: u64,
) -> ObservationHeader {
    ObservationHeader {
        contract_version: 1,
        source: SourceKey {
            provider: snapshot.provider,
            library_instance_id: options
                .library_instance_id
                .clone()
                .or_else(|| instance.map(str::to_owned))
                .unwrap_or_else(|| {
                    format!(
                        "root:{}",
                        digest(snapshot.source_root().as_os_str().as_encoded_bytes())
                    )
                }),
        },
        product_version: String::new(),
        schema_fingerprint: profile.schema_fingerprint.clone(),
        snapshot_hash: snapshot.snapshot_hash.clone(),
        adapter_version: format!("{}:1", profile.profile_id),
        completeness: Completeness::SelectedScope,
        scope: json!({"artifacts":snapshot.artifacts.iter().map(|a| &a.id).collect::<Vec<_>>(),"include_history":options.include_history,"include_queue":options.include_queue}),
        observed_at: chrono::DateTime::<chrono::Utc>::from(std::time::SystemTime::now())
            .to_rfc3339(),
        accounting: RecordAccounting {
            tracks_seen: Some(tracks),
            collections_seen: Some(collections),
            memberships_seen: Some(memberships),
            unrepresented_records: 0,
        },
        extensions: BTreeMap::new(),
    }
}
fn issue(track: &mut TrackObservation, code: &str, field: &str) {
    track.issues.push(ImportIssue {
        code: code.into(),
        field: Some(field.into()),
        record_id: Some(track.external_id.clone()),
        message: code.replace('_', " "),
    });
}
fn history_path(relative: &Path) -> bool {
    relative.components().next().is_some_and(|c| {
        c.as_os_str()
            .to_str()
            .is_some_and(|s| s.eq_ignore_ascii_case("History"))
    })
}
fn camelot(raw: &str) -> Option<String> {
    if !raw.is_ascii() {
        return None;
    }
    let raw = raw.trim().to_ascii_uppercase();
    let (number, mode) = raw.split_at(raw.len().checked_sub(1)?);
    let number: u8 = number.parse().ok()?;
    ((1..=12).contains(&number) && matches!(mode, "A" | "B")).then(|| format!("{number}{mode}"))
}
fn bytes(
    snapshot: &ImportSnapshot,
    id: &str,
    options: &ReadOptions,
    cancel: &CancellationToken,
) -> Result<Vec<u8>> {
    cancel.check()?;
    let path = snapshot.artifact(id)?;
    ensure!(
        fs::metadata(path)?.len() <= options.limits.text_bytes,
        "Source text byte limit exceeded"
    );
    let bytes = fs::read(path)?;
    ensure!(
        bytes.len() as u64 <= options.limits.text_bytes,
        "Source text byte limit exceeded"
    );
    cancel.check()?;
    Ok(bytes)
}
fn lexical(path: &Path) -> Result<PathBuf> {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => ensure!(
                result.pop(),
                "Relative media reference escapes filesystem root"
            ),
            c => result.push(c.as_os_str()),
        }
    }
    Ok(result)
}
fn location(
    raw: &str,
    directory: Option<&Path>,
    options: &ReadOptions,
) -> Result<LocationEvidence> {
    ensure!(!raw.contains('\0'), "NUL in media reference");
    let windows_syntax = raw.starts_with("\\\\")
        || raw.starts_with("//")
        || raw.as_bytes().get(1) == Some(&b':')
        || raw
            .strip_prefix("file:///")
            .is_some_and(|v| v.as_bytes().get(1) == Some(&b':'))
        || raw.starts_with("file://")
            && !raw.starts_with("file:///")
            && !raw.starts_with("file://localhost/");
    let windows = windows_syntax
        || options
            .origin
            .as_ref()
            .is_some_and(|o| o.origin_os == "windows");
    let mut evidence = LocationEvidence {
        original: Some(raw.into()),
        platform: if windows {
            Some("windows".into())
        } else {
            options.origin.as_ref().map(|o| o.origin_os.clone())
        },
        availability: "unresolved".into(),
        ..Default::default()
    };
    if raw.contains("://") && !raw.starts_with("file://") {
        evidence.availability = "remote".into();
        return Ok(evidence);
    }
    if !windows_syntax
        && !raw.starts_with("file://")
        && !raw.starts_with(['/', '\\'])
        && !Path::new(raw).is_absolute()
        && let Some(directory) = directory
    {
        let relative = if windows {
            raw.replace('\\', "/")
        } else {
            raw.into()
        };
        let native = lexical(&directory.join(relative))?;
        evidence
            .extensions
            .insert("original_entry".into(), json!(raw));
        evidence.original = native.to_str().map(str::to_owned);
        evidence.platform = Some(std::env::consts::OS.into());
        evidence.native_path = Some(native);
    } else {
        // Validate file URI escapes even when the source remains unresolved.
        let normalized =
            super::paths::normalized(raw, windows)?.context("Invalid local media reference")?;
        evidence.extensions.insert(
            "normalized_source_path".into(),
            json!(if windows {
                normalized.to_ascii_lowercase()
            } else {
                normalized.clone()
            }),
        );
        if evidence.platform.as_deref() == Some(std::env::consts::OS)
            && Path::new(&normalized).is_absolute()
        {
            evidence.native_path = Some(normalized.into());
        }
    }
    let resolution = resolve_location(&evidence, &options.path_mappings)?;
    if let Some(path) = resolution.resolved {
        evidence.resolved = path.to_str().map(str::to_owned);
        evidence.availability = if path.is_file() {
            "available"
        } else {
            "missing"
        }
        .into();
    }
    Ok(evidence)
}
fn emit_track(
    mut track: TrackObservation,
    options: &ReadOptions,
    sink: &mut dyn ObservationSink,
    cancel: &CancellationToken,
) -> Result<()> {
    cancel.check()?;
    if options.capture_file_tags {
        let path = resolve_location(&track.locations, &options.path_mappings)?.resolved;
        if let Some(path) = path.filter(|p| p.is_file()) {
            match read_file_tags(&path, &options.limits, cancel) {
                Ok(mut tags) => {
                    for data in tags.blobs.values() {
                        sink.put_blob(data, None)?;
                    }
                    tags.blobs.clear();
                    track.file_tags =
                        Some(sink.put_blob(&serde_json::to_vec(&tags)?, Some("application/json"))?);
                }
                Err(error) => {
                    cancel.check()?;
                    track.issues.push(ImportIssue {
                        code: "file_tags_unavailable".into(),
                        field: Some("file_tags".into()),
                        message: error.to_string(),
                        ..Default::default()
                    });
                }
            }
        }
    }
    validate_track_observation(&track, &options.limits)?;
    sink.push(ObservationRecord::Track(Box::new(track)))
}
fn membership(collection: &str, track: &str, position: usize) -> ObservationRecord {
    ObservationRecord::Membership {
        collection_id: collection.into(),
        occurrence: MembershipObservation {
            external_occurrence_id: format!("{collection}:{position}"),
            track_external_id: track.into(),
            position: Some(position as i64),
            ..Default::default()
        },
    }
}
fn validate_hierarchy(collections: &[CollectionObservation], limits: &ImportLimits) -> Result<()> {
    let map: BTreeMap<_, _> = collections
        .iter()
        .map(|c| (c.external_id.as_str(), c.parent_id.as_deref()))
        .collect();
    ensure!(map.len() == collections.len(), "Duplicate collection ID");
    for collection in collections {
        let mut seen = std::collections::BTreeSet::new();
        let mut current = Some(collection.external_id.as_str());
        while let Some(id) = current {
            ensure!(
                seen.insert(id) && seen.len() <= limits.hierarchy_depth,
                "Collection cycle or depth limit"
            );
            current = *map.get(id).context("Unknown collection parent")?;
        }
    }
    Ok(())
}
