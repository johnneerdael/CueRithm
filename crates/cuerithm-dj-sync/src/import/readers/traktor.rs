use super::*;
use std::collections::BTreeSet;
use xml::Node;
pub struct TraktorReader;
const KEYS: [&str; 24] = [
    "8B", "3B", "10B", "5B", "12B", "7B", "2B", "9B", "4B", "11B", "6B", "1B", "5A", "12A", "7A",
    "2A", "9A", "4A", "11A", "6A", "1A", "8A", "3A", "10A",
];
fn load(
    snapshot: &ImportSnapshot,
    artifact: &str,
    options: &ReadOptions,
    cancel: &CancellationToken,
) -> Result<Node> {
    let root = xml::parse(
        &bytes(snapshot, artifact, options, cancel)?,
        &options.limits,
        cancel,
    )?;
    let head = root.child("HEAD").context("NML has no program marker")?;
    ensure!(
        root.name == "NML"
            && root.attr("VERSION") == Some("19")
            && matches!(head.attr("PROGRAM"), Some("Traktor" | "Traktor Pro"))
            && head
                .attr("VERSION")
                .is_some_and(|v| v.starts_with("3.") || v.starts_with("4."))
            && root.child("COLLECTION").is_some(),
        "Unqualified Traktor NML/program profile"
    );
    Ok(root)
}
fn normalized_path(raw: &str) -> String {
    let raw = raw.replace("/:", "/");
    if raw.as_bytes().get(1) == Some(&b':') || raw.starts_with("//") || raw.starts_with("\\\\") {
        raw.replace('\\', "/").to_ascii_lowercase()
    } else {
        raw
    }
}
fn track_id(path: &str) -> String {
    format!("track:{}", digest(normalized_path(path).as_bytes()))
}
fn source_path(entry: &Node) -> Option<String> {
    let loc = entry.child("LOCATION")?;
    if let Some(web) = loc.attr("WEBADDRESS").filter(|s| !s.is_empty()) {
        return Some(web.into());
    }
    let file = loc.attr("FILE")?;
    let volume = loc.attr("VOLUME").unwrap_or("").trim_end_matches('/');
    let dir = loc.attr("DIR").unwrap_or("").replace("/:", "/");
    let mut path = volume.to_owned();
    if !dir.trim_matches('/').is_empty() {
        path.push('/');
        path.push_str(dir.trim_matches('/'));
    }
    if !path.ends_with('/') {
        path.push('/');
    }
    path.push_str(file);
    Some(path)
}
fn convert(entry: &Node, options: &ReadOptions) -> Result<TrackObservation> {
    let path = source_path(entry);
    let mut track = TrackObservation {
        external_id: path.as_deref().map(track_id).unwrap_or_else(|| {
            format!("unresolved:{}", digest(entry.raw().to_string().as_bytes()))
        }),
        locations: match &path {
            Some(path) => location(path, None, options)?,
            None => LocationEvidence {
                availability: "unresolved".into(),
                ..Default::default()
            },
        },
        extensions: BTreeMap::from([("native".into(), entry.raw())]),
        ..Default::default()
    };
    if path.is_none() {
        issue(&mut track, "missing_location", "location");
    }
    if let Some(loc) = entry.child("LOCATION") {
        track.locations.volume_id = loc.attr("VOLUMEID").map(str::to_owned);
    }
    for (source, target) in [("TITLE", "title"), ("ARTIST", "artist")] {
        if let Some(value) = entry.attr(source) {
            track
                .metadata
                .insert(target.into(), apple_music::presence(&json!(value)));
        }
    }
    for (child, mappings) in [
        (
            "ALBUM",
            &[("TITLE", "album"), ("TRACK", "track_number")][..],
        ),
        (
            "INFO",
            &[
                ("GENRE", "genre"),
                ("COMMENT", "comment"),
                ("LABEL", "label"),
                ("REMIXER", "remixer"),
                ("PRODUCER", "producer"),
                ("KEY_LYRICS", "lyricist"),
                ("MIX", "mix"),
            ][..],
        ),
    ] {
        if let Some(info) = entry.child(child) {
            for (source, target) in mappings {
                if let Some(value) = info.attr(source) {
                    track
                        .metadata
                        .insert((*target).into(), apple_music::presence(&json!(value)));
                }
            }
        }
    }
    if let Some(info) = entry.child("INFO") {
        if let Some(duration) = info.number("PLAYTIME").filter(|v| *v >= 0.) {
            track
                .extensions
                .insert("duration_seconds".into(), json!(duration));
        }
        if let Some(rating) = info.number("RANKING").filter(|v| (0. ..=255.).contains(v)) {
            track.metadata.insert(
                "rating".into(),
                Presence::Value(json!({"raw":rating,"scale_max":255})),
            );
        }
        if options.include_history
            && let Some(value) = info.attr("PLAYCOUNT")
        {
            track
                .metadata
                .insert("play_count".into(), Presence::Value(json!(value)));
        }
    }
    if let Some(tempo) = entry.child("TEMPO") {
        if let Some(value) = tempo.number("BPM").filter(|v| *v > 0.) {
            track.bpm = Some(ScalarBpm {
                presence: FieldPresence::Value,
                value: Some(value),
                ..Default::default()
            });
        } else if tempo.attr("BPM").is_some() {
            issue(&mut track, "invalid_native_bpm", "bpm");
        }
    }
    let info_key = entry.child("INFO").and_then(|n| n.attr("KEY"));
    if let Some(raw) = entry.child("MUSICAL_KEY").and_then(|n| n.attr("VALUE")) {
        let key = raw.parse::<usize>().ok().filter(|v| *v < KEYS.len());
        track.key = Some(MusicalKey {
            presence: FieldPresence::Value,
            raw: Some(raw.into()),
            camelot: key.map(|v| KEYS[v].into()),
            pitch_class: key.map(|v| (v % 12) as u8),
            mode: key.map(|v| if v < 12 { "major" } else { "minor" }.into()),
            provenance: Some("traktor_musical_key".into()),
            extensions: BTreeMap::from([("info_key".into(), json!(info_key))]),
        });
        if key.is_none() {
            issue(&mut track, "unknown_native_key", "key");
        }
    } else if let Some(raw) = info_key {
        track.key = Some(MusicalKey {
            presence: if raw.is_empty() {
                FieldPresence::Empty
            } else {
                FieldPresence::Value
            },
            raw: Some(raw.into()),
            camelot: camelot(raw),
            ..Default::default()
        });
    }
    let mut points = Vec::new();
    let mut slots = BTreeSet::new();
    for (index, cue) in entry
        .children
        .iter()
        .filter(|n| n.name == "CUE_V2")
        .enumerate()
    {
        let kind = cue.attr("TYPE").unwrap_or("");
        let slot = cue
            .attr("HOTCUE")
            .and_then(|v| v.parse::<i32>().ok())
            .filter(|v| *v >= 0);
        let start = cue.number("START").map(|v| v / 1000.);
        if kind == "4" {
            let bpm = match cue.child("GRID") {
                Some(grid) => grid.number("BPM"),
                None => track.bpm.as_ref().and_then(|b| b.value),
            }
            .filter(|v| *v > 0.);
            if let (Some(native_seconds), Some(bpm)) = (start, bpm) {
                points.push(GridPoint {
                    native_seconds,
                    bpm: Some(bpm),
                    original: cue.raw(),
                    ..Default::default()
                });
            } else {
                issue(&mut track, "invalid_native_grid_point", "grid");
            }
            if slot.is_none() {
                continue;
            }
        }
        let length = cue.number("LEN").filter(|v| *v > 0.).map(|v| v / 1000.);
        let end = if kind == "5" {
            start
                .zip(length)
                .map(|(s, l)| s + l)
                .filter(|v| v.is_finite())
        } else {
            None
        };
        if kind == "5" && end.is_none() {
            issue(&mut track, "invalid_loop_length", "cues.loop");
        }
        if let Some(slot) = slot
            && !slots.insert(slot)
        {
            issue(&mut track, "duplicate_hotcue_slot", "cues.hot");
        }
        let family = if slot.is_some() {
            CueFamily::Hot
        } else {
            match kind {
                "0" => CueFamily::Memory,
                "1" => CueFamily::Intro,
                "2" => CueFamily::Outro,
                "3" => CueFamily::Load,
                "5" => CueFamily::Loop,
                _ => CueFamily::Unknown,
            }
        };
        track.cues.push(CueObservation {
            external_id: format!("cue:{index}"),
            family,
            source_slot: slot,
            name: cue.attr("NAME").map(str::to_owned),
            original: cue.raw(),
            native_start_seconds: start,
            native_end_seconds: end,
            flags: BTreeMap::from([("loop".into(), json!(kind == "5"))]),
            ..Default::default()
        });
    }
    if let Some(first) = points.first() {
        track.grid = Some(GridObservation {
            representation: "points".into(),
            original_time_base: Some("milliseconds".into()),
            native_first_beat_seconds: Some(first.native_seconds),
            bpm: first.bpm,
            points,
            ..Default::default()
        });
    }
    Ok(track)
}
fn nodes(
    node: &Node,
    parent: Option<&str>,
    scope: &str,
    collections: &mut Vec<CollectionObservation>,
    references: &mut Vec<(String, String, Value)>,
    cancel: &CancellationToken,
) -> Result<()> {
    cancel.check()?;
    let mut current = parent.map(str::to_owned);
    if node.name == "NODE" && node.attr("NAME") != Some("$ROOT") {
        let playlist = node.child("PLAYLIST");
        let id = node
            .attr("UUID")
            .or_else(|| playlist.and_then(|p| p.attr("UUID")))
            .map(str::to_owned)
            .unwrap_or_else(|| {
                format!(
                    "node:{}",
                    digest(
                        format!("{scope}/{parent:?}/{}", node.attr("NAME").unwrap_or(""))
                            .as_bytes()
                    )
                )
            });
        let smart = playlist.is_some_and(|p| p.child("SEARCH_EXPRESSION").is_some());
        collections.push(CollectionObservation {
            external_id: id.clone(),
            parent_id: parent.map(str::to_owned),
            kind: if smart {
                CollectionKind::Smart
            } else if playlist.is_some() {
                CollectionKind::Playlist
            } else {
                CollectionKind::Folder
            },
            name: node.attr("NAME").unwrap_or("").into(),
            extensions: BTreeMap::from([(
                "native".into(),
                node.raw_without(&["ENTRY", "SUBNODES", "NODE"]),
            )]),
            ..Default::default()
        });
        if let Some(playlist) = playlist {
            for entry in playlist.children.iter().filter(|n| n.name == "ENTRY") {
                if let Some(primary) = entry.child("PRIMARYKEY")
                    && matches!(primary.attr("TYPE"), Some("TRACK" | "STEM"))
                {
                    references.push((
                        id.clone(),
                        primary
                            .attr("KEY")
                            .context("Missing Traktor playlist key")?
                            .into(),
                        entry.raw(),
                    ));
                }
            }
        }
        current = Some(id);
    }
    for child in &node.children {
        if matches!(child.name.as_str(), "NODE" | "SUBNODES") {
            nodes(
                child,
                current.as_deref(),
                scope,
                collections,
                references,
                cancel,
            )?;
        }
    }
    Ok(())
}
impl SourceAdapter for TraktorReader {
    fn provider(&self) -> SourceProvider {
        SourceProvider::Traktor
    }
    fn inspect(&self, snapshot: &ImportSnapshot) -> Result<SourceProfile> {
        ensure!(
            snapshot.provider == self.provider(),
            "Traktor provider mismatch"
        );
        for artifact in &snapshot.artifacts {
            load(
                snapshot,
                &artifact.id,
                &ReadOptions::default(),
                &CancellationToken::default(),
            )?;
        }
        Ok(profile(
            self.provider(),
            "traktor-nml19-pro3-4",
            &[
                "descriptive_tags",
                "key",
                "bpm",
                "beatgrid",
                "hot_cues",
                "memory_cues",
                "load_cues",
                "loops",
                "markers",
                "ratings",
                "history",
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
        ensure!(
            snapshot.provider == self.provider(),
            "Traktor provider mismatch"
        );
        cancel.check()?;
        snapshot.validate_source(cancel)?;
        let profile = profile(
            self.provider(),
            "traktor-nml19-pro3-4",
            &[
                "descriptive_tags",
                "key",
                "bpm",
                "beatgrid",
                "hot_cues",
                "memory_cues",
                "load_cues",
                "loops",
                "markers",
                "ratings",
                "history",
                "collections",
            ],
        );
        let mut known = BTreeSet::new();
        let mut tracks = 0;
        let mut collections = Vec::new();
        let mut references = Vec::new();
        let mut artifacts = snapshot.artifacts.iter().collect::<Vec<_>>();
        let history = |id: &str| {
            snapshot.original_artifact_path(id).is_ok_and(|p| {
                p.strip_prefix(snapshot.source_root())
                    .is_ok_and(history_path)
            })
        };
        artifacts.sort_by_key(|a| history(&a.id));
        for artifact in artifacts {
            let historical = history(&artifact.id);
            if historical && !options.include_history {
                continue;
            }
            let root = load(snapshot, &artifact.id, options, cancel)?;
            let history_id = format!("history:{}", digest(artifact.id.as_bytes()));
            if historical {
                collections.push(CollectionObservation {
                    external_id: history_id.clone(),
                    kind: CollectionKind::History,
                    name: snapshot
                        .original_artifact_path(&artifact.id)?
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into(),
                    ..Default::default()
                });
            }
            let mut in_file = BTreeSet::new();
            for entry in &root
                .child("COLLECTION")
                .context("Missing NML collection")?
                .children
            {
                if entry.name != "ENTRY" {
                    continue;
                }
                cancel.check()?;
                let track = convert(entry, options)?;
                let id = track.external_id.clone();
                ensure!(
                    in_file.insert(id.clone()),
                    "Duplicate Traktor track location in one collection"
                );
                if known.insert(id) {
                    emit_track(track, options, sink, cancel)?;
                    tracks += 1;
                }
                if historical && let Some(path) = source_path(entry) {
                    references.push((history_id.clone(), path, entry.raw()));
                }
            }
            if !historical && let Some(tree) = root.child("PLAYLISTS") {
                for node in &tree.children {
                    nodes(
                        node,
                        None,
                        &artifact.id,
                        &mut collections,
                        &mut references,
                        cancel,
                    )?;
                }
            }
        }
        validate_hierarchy(&collections, &options.limits)?;
        let count = collections.len() as u64;
        for collection in collections {
            sink.push(ObservationRecord::Collection(collection))?;
        }
        let mut positions = BTreeMap::new();
        let members = references.len() as u64;
        for (collection, path, raw) in references {
            cancel.check()?;
            let id = track_id(&path);
            if known.insert(id.clone()) {
                let mut track = TrackObservation {
                    external_id: id.clone(),
                    locations: location(&path.replace("/:", "/"), None, options)?,
                    ..Default::default()
                };
                issue(&mut track, "missing_track_record", "location");
                emit_track(track, options, sink, cancel)?;
                tracks += 1;
            }
            let position = positions.entry(collection.clone()).or_insert(0usize);
            let mut member = membership(&collection, &id, *position);
            *position += 1;
            if let ObservationRecord::Membership { occurrence, .. } = &mut member {
                occurrence.extensions.insert("native".into(), raw);
            }
            sink.push(member)?;
        }
        cancel.check()?;
        snapshot.validate_source(cancel)?;
        Ok(header(
            snapshot, options, &profile, None, tracks, count, members,
        ))
    }
}
