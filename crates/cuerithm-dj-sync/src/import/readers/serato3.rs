//! Legacy parent-bounded Serato database/crate reader, with per-volume identities.
use super::*;
use rusqlite::{Connection, OptionalExtension, params};
use std::{
    fs::File,
    io::{BufReader, Read},
};
pub struct Serato3Reader;
fn utf16(bytes: &[u8]) -> Result<String> {
    ensure!(bytes.len().is_multiple_of(2), "Odd UTF16BE Serato field");
    Ok(String::from_utf16(
        &bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| u16::from_be_bytes(*p))
            .collect::<Vec<_>>(),
    )?)
}
fn chunks(bytes: &[u8]) -> Result<Vec<(&str, &[u8])>> {
    let mut at = 0;
    let mut out = Vec::new();
    while at < bytes.len() {
        ensure!(
            out.len() < 65536 && bytes.len() - at >= 8,
            "Truncated or excessive Serato fields"
        );
        let name = std::str::from_utf8(&bytes[at..at + 4])?;
        ensure!(
            name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b' '),
            "Invalid Serato field ID"
        );
        let n = u32::from_be_bytes(bytes[at + 4..at + 8].try_into()?) as usize;
        at += 8;
        let end = at
            .checked_add(n)
            .filter(|end| *end <= bytes.len())
            .context("Serato child exceeds parent length")?;
        out.push((name, &bytes[at..end]));
        at = end;
    }
    Ok(out)
}
fn each_chunk(
    path: &Path,
    options: &ReadOptions,
    cancel: &CancellationToken,
    mut visit: impl FnMut(&str, &[u8]) -> Result<()>,
) -> Result<()> {
    let mut input = BufReader::new(File::open(path)?);
    loop {
        cancel.check()?;
        let mut h = [0; 8];
        if input.read(&mut h[..1])? == 0 {
            break;
        }
        input.read_exact(&mut h[1..])?;
        let name = std::str::from_utf8(&h[..4])?;
        ensure!(
            name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b' '),
            "Invalid Serato chunk ID"
        );
        let n = u32::from_be_bytes(h[4..].try_into()?) as usize;
        ensure!(
            n as u64 <= options.limits.blob_bytes,
            "Serato chunk byte limit"
        );
        let mut data = vec![0; n];
        input.read_exact(&mut data)?;
        cancel.check()?;
        visit(name, &data)?;
    }
    Ok(())
}
fn context(snapshot: &ImportSnapshot, id: &str) -> Result<(PathBuf, String)> {
    let path = snapshot.original_artifact_path(id)?;
    let store = path
        .ancestors()
        .find(|p| p.file_name().is_some_and(|s| s == "_Serato_"))
        .context("Serato3 artifact must be under its original _Serato_ store")?;
    let volume = store
        .parent()
        .context("Missing Serato volume root")?
        .to_path_buf();
    let namespace = digest(volume.as_os_str().as_encoded_bytes());
    Ok((volume, namespace))
}
fn track_id(namespace: &str, path: &str) -> Result<String> {
    ensure!(!path.contains('\0'), "NUL in Serato path");
    let normalized = path.replace('\\', "/");
    Ok(format!("{namespace}:{}", digest(normalized.as_bytes())))
}
fn native_location(raw: &str, volume: &Path, options: &ReadOptions) -> Result<LocationEvidence> {
    let relative = raw.replace('\\', "/");
    let mut result = if raw.contains("://")
        || raw.as_bytes().get(1) == Some(&b':')
        || raw.starts_with("\\\\")
        || raw.starts_with("//")
    {
        location(raw, None, options)?
    } else {
        location(relative.trim_start_matches('/'), Some(volume), options)?
    };
    result
        .extensions
        .insert("serato_portable_path".into(), json!(raw));
    Ok(result)
}
fn stage(
    db: &Connection,
    track: TrackObservation,
    raw: Option<BlobRef>,
    limits: &ImportLimits,
) -> Result<()> {
    let old: Option<String> = db
        .query_row(
            "SELECT json FROM tracks WHERE id=?1",
            [&track.external_id],
            |r| r.get(0),
        )
        .optional()?;
    let track = if let Some(old) = old {
        let mut old: TrackObservation = serde_json::from_str(&old)?;
        if let Some(raw) = raw {
            let entry = old
                .extensions
                .entry("additional_native_records".into())
                .or_insert(json!([]));
            entry
                .as_array_mut()
                .context("Invalid record list")?
                .push(json!(raw));
        }
        if old.metadata != track.metadata || old.key != track.key || old.bpm != track.bpm {
            issue(&mut old, "ambiguous_serato_track_records", "metadata");
            old.metadata
                .retain(|field, value| track.metadata.get(field) == Some(value));
            if old.key != track.key {
                old.key = None
            }
            if old.bpm != track.bpm {
                old.bpm = None
            }
        }
        old
    } else {
        track
    };
    let body = serde_json::to_string(&track)?;
    ensure!(
        body.len() as u64 <= limits.text_bytes,
        "Serato observation byte limit"
    );
    db.execute(
        "INSERT INTO tracks VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET json=excluded.json",
        params![track.external_id, body],
    )?;
    Ok(())
}
fn native_track(
    namespace: &str,
    volume: &Path,
    data: &[u8],
    options: &ReadOptions,
    sink: &mut dyn ObservationSink,
) -> Result<TrackObservation> {
    let fields = chunks(data)?;
    let paths = fields
        .iter()
        .filter(|(n, _)| *n == "pfil")
        .map(|(_, b)| utf16(b))
        .collect::<Result<Vec<_>>>()?;
    ensure!(paths.len() == 1, "Serato track needs one pfil identity");
    let raw = &paths[0];
    let mut track = TrackObservation {
        external_id: track_id(namespace, raw)?,
        locations: native_location(raw, volume, options)?,
        ..Default::default()
    };
    let mut native = Vec::new();
    for (name, bytes) in &fields {
        let value = if name.starts_with(['t', 'p']) {
            json!(utf16(bytes)?)
        } else if name.starts_with('s') && bytes.len() == 2 {
            json!(u16::from_be_bytes((*bytes).try_into()?))
        } else if name.starts_with('b') && bytes.len() == 1 {
            json!(bytes[0])
        } else if bytes.len() == 4 {
            json!(u32::from_be_bytes((*bytes).try_into()?))
        } else {
            json!({"blob":sink.put_blob(bytes,None)?})
        };
        native.push(json!({"name":name,"value":value}));
    }
    for (source, target) in [
        ("tsng", "title"),
        ("tart", "artist"),
        ("talb", "album"),
        ("tgen", "genre"),
        ("tcom", "comment"),
        ("trmx", "remixer"),
        ("tlbl", "label"),
        ("tcmp", "composer"),
        ("tgrp", "grouping"),
        ("tyer", "year"),
        ("tadd", "date_added"),
    ] {
        let values = native
            .iter()
            .filter(|v| v["name"] == source)
            .collect::<Vec<_>>();
        if values.len() == 1 {
            track
                .metadata
                .insert(target.into(), apple_music::presence(&values[0]["value"]));
        } else if values.len() > 1 {
            issue(&mut track, "duplicate_serato_field", target);
        }
    }
    for (field, target) in [("tkey", "key"), ("tbpm", "bpm")] {
        let values = native
            .iter()
            .filter(|v| v["name"] == field)
            .collect::<Vec<_>>();
        if values.len() != 1 {
            continue;
        }
        let raw = values[0]["value"]
            .as_str()
            .context("Invalid Serato text scalar")?;
        if target == "key" {
            track.key = Some(MusicalKey {
                presence: if raw.is_empty() {
                    FieldPresence::Empty
                } else {
                    FieldPresence::Value
                },
                raw: Some(raw.into()),
                camelot: crate::formatting::parse_key(raw).ok().map(|k| k.camelot()),
                provenance: Some("serato_database_v2".into()),
                ..Default::default()
            });
        } else if let Ok(bpm) = raw.parse::<f64>()
            && bpm.is_finite()
            && bpm > 0.
        {
            track.bpm = Some(ScalarBpm {
                presence: FieldPresence::Value,
                value: Some(bpm),
                provenance: Some("serato_database_v2".into()),
                ..Default::default()
            });
        }
    }
    if options.include_history
        && let Some(value) = native.iter().find(|v| v["name"] == "bply")
    {
        track
            .metadata
            .insert("played".into(), Presence::Value(value["value"].clone()));
    }
    track
        .extensions
        .insert("native_fields".into(), json!(native));
    Ok(track)
}
impl SourceAdapter for Serato3Reader {
    fn provider(&self) -> SourceProvider {
        SourceProvider::Serato3
    }
    fn inspect(&self, snapshot: &ImportSnapshot) -> Result<SourceProfile> {
        ensure!(
            snapshot.provider == self.provider(),
            "Serato3 provider mismatch"
        );
        for a in &snapshot.artifacts {
            context(snapshot, &a.id)?;
            ensure!(
                matches!(a.kind, ArtifactKind::StableFile),
                "Legacy Serato requires stable binary artifacts"
            );
            let path = snapshot.original_artifact_path(&a.id)?;
            ensure!(
                path.file_name()
                    .is_some_and(|n| n == "database V2" || n == "neworder.pref")
                    || path
                        .extension()
                        .is_some_and(|n| n == "crate" || n == "scrate"),
                "Unknown Serato3 artifact type"
            );
        }
        Ok(profile(
            self.provider(),
            "serato3-database-v2-crates",
            &[
                "descriptive_tags",
                "key",
                "bpm",
                "hot_cues",
                "loops",
                "beatgrid",
                "color",
                "collections",
            ],
        ))
    }
    fn read(
        &self,
        snapshot: &ImportSnapshot,
        options: &ReadOptions,
        sink: &mut dyn ObservationSink,
        cancel: &CancellationToken,
    ) -> Result<ObservationHeader> {
        cancel.check()?;
        snapshot.validate_source(cancel)?;
        let profile = self.inspect(snapshot)?;
        let mut native_source_chunks = Vec::new();
        let aux_file = tempfile::NamedTempFile::new()?;
        let db = Connection::open(aux_file.path())?;
        db.execute_batch("CREATE TABLE tracks(id TEXT PRIMARY KEY,json TEXT);")?;
        for a in &snapshot.artifacts {
            let original = snapshot.original_artifact_path(&a.id)?;
            if original.file_name().is_none_or(|n| n != "database V2") {
                continue;
            }
            let (volume, namespace) = context(snapshot, &a.id)?;
            each_chunk(snapshot.artifact(&a.id)?, options, cancel, |name, data| {
                if name == "otrk" {
                    let mut track = native_track(&namespace, &volume, data, options, sink)?;
                    let raw = sink.put_blob(data, None)?;
                    track.extensions.insert("native_record".into(), json!(raw));
                    stage(&db, track, Some(raw), &options.limits)?;
                } else {
                    let raw = sink.put_blob(data, None)?;
                    ensure!(
                        native_source_chunks.len() < 65536,
                        "Source chunk count limit"
                    );
                    native_source_chunks.push(json!({"artifact":a.id,"name":name,"blob":raw}));
                }
                Ok(())
            })?;
        }
        let mut collections = BTreeMap::<String, CollectionObservation>::new();
        let mut members = 0u64;
        let mut ordering = BTreeMap::<String, Vec<String>>::new();
        for a in &snapshot.artifacts {
            let original = snapshot.original_artifact_path(&a.id)?;
            if original.file_name().is_some_and(|n| n == "neworder.pref") {
                let (_, namespace) = context(snapshot, &a.id)?;
                let raw = bytes(snapshot, &a.id, options, cancel)?;
                let text = utf16(&raw)?;
                native_source_chunks.push(json!({"artifact":a.id,"name":"neworder.pref","blob":sink.put_blob(&raw,None)?}));
                ordering.insert(
                    namespace,
                    text.trim_start_matches('\u{feff}')
                        .lines()
                        .filter(|s| !s.trim().is_empty() && s.trim() != "[crate]")
                        .map(|s| s.trim_start_matches('\u{feff}').trim().to_string())
                        .collect(),
                );
            }
        }
        for a in &snapshot.artifacts {
            cancel.check()?;
            let original = snapshot.original_artifact_path(&a.id)?;
            let extension = original.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !matches!(extension, "crate" | "scrate") {
                continue;
            }
            let (volume, namespace) = context(snapshot, &a.id)?;
            let stem = original
                .file_stem()
                .and_then(|e| e.to_str())
                .context("Invalid Serato crate name")?;
            let names = stem.replace("≫≫", "%%");
            let mut parent = None;
            let mut cumulative = String::new();
            for (part_index, part) in names.split("%%").enumerate() {
                ensure!(
                    part_index < options.limits.hierarchy_depth,
                    "Serato crate hierarchy depth limit"
                );
                ensure!(!part.is_empty(), "Empty Serato crate component");
                if part_index > 0 {
                    cumulative.push_str("%%")
                }
                cumulative.push_str(part);
                let id = format!("{namespace}:crate:{}", digest(cumulative.as_bytes()));
                collections
                    .entry(id.clone())
                    .or_insert(CollectionObservation {
                        external_id: id.clone(),
                        parent_id: parent.clone(),
                        kind: CollectionKind::Folder,
                        name: part.into(),
                        ..Default::default()
                    });
                parent = Some(id);
                ensure!(collections.len() <= 1_000_000, "Crate hierarchy limit");
            }
            let id = parent.context("Missing crate identity")?;
            let collection = collections.get_mut(&id).context("Missing crate")?;
            collection.kind = if extension == "scrate" {
                CollectionKind::Smart
            } else {
                CollectionKind::Crate
            };
            collection
                .extensions
                .insert("native_name".into(), json!(stem));
            collection
                .extensions
                .insert("source_artifact".into(), json!(a.id));
            if let Some(order) = ordering
                .get(&namespace)
                .and_then(|order| order.iter().position(|s| s == stem))
            {
                collection
                    .extensions
                    .insert("display_order".into(), json!(order));
            }
            if extension == "scrate" {
                let raw = bytes(snapshot, &a.id, options, cancel)?;
                collection.extensions.insert(
                    "smart_rules".into(),
                    json!({"blob":sink.put_blob(&raw,None)?}),
                );
                collection.extensions.insert("issues".into(),json!([{"code":"unsupported_serato_smart_rules","message":"Native predicates retained; no live rule equivalent claimed"}]));
            }
            let mut position = 0usize;
            each_chunk(snapshot.artifact(&a.id)?, options, cancel, |name, data| {
                if name == "otrk" {
                    let fields = chunks(data)?;
                    let refs = fields
                        .iter()
                        .filter(|(n, _)| *n == "ptrk")
                        .collect::<Vec<_>>();
                    ensure!(refs.len() == 1, "Crate occurrence needs one ptrk");
                    let raw = utf16(refs[0].1)?;
                    let track = track_id(&namespace, &raw)?;
                    let exists: bool = db.query_row(
                        "SELECT EXISTS(SELECT 1 FROM tracks WHERE id=?1)",
                        [&track],
                        |r| r.get(0),
                    )?;
                    if !exists {
                        let mut missing = TrackObservation {
                            external_id: track.clone(),
                            locations: native_location(&raw, &volume, options)?,
                            ..Default::default()
                        };
                        issue(&mut missing, "missing_serato_database_record", "track");
                        stage(&db, missing, None, &options.limits)?;
                    }
                    let mut row = membership(&id, &track, position);
                    if let ObservationRecord::Membership { occurrence, .. } = &mut row {
                        occurrence
                            .extensions
                            .insert("native_reference".into(), json!(raw));
                        occurrence
                            .extensions
                            .insert("native_record".into(), json!(sink.put_blob(data, None)?));
                    }
                    sink.push(row)?;
                    position += 1;
                    members += 1;
                } else {
                    collection
                        .extensions
                        .entry("native_chunks".into())
                        .or_insert(json!([]))
                        .as_array_mut()
                        .context("Invalid crate chunk list")?
                        .push(json!({"name":name,"blob":sink.put_blob(data,None)?}));
                }
                Ok(())
            })?;
        }
        let count = collections.len() as u64;
        for collection in collections.into_values() {
            sink.push(ObservationRecord::Collection(collection))?;
        }
        let mut statement = db.prepare("SELECT json FROM tracks ORDER BY id")?;
        let mut rows = statement.query([])?;
        let mut tracks = 0;
        while let Some(row) = rows.next()? {
            cancel.check()?;
            let track = serde_json::from_str(&row.get::<_, String>(0)?)?;
            super::serato_tags::emit_native_track(track, options, sink, cancel)?;
            tracks += 1;
        }
        snapshot.validate_source(cancel)?;
        cancel.check()?;
        let mut header = header(snapshot, options, &profile, None, tracks, count, members);
        header
            .extensions
            .insert("native_source_chunks".into(), json!(native_source_chunks));
        Ok(header)
    }
}
