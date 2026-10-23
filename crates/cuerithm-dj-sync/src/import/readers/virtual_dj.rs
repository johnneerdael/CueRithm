use super::*;
use std::collections::BTreeSet;
use xml::Node;
pub struct VirtualDjReader;
type References = BTreeMap<String, Vec<String>>;
fn identity(
    raw: &str,
    directory: Option<&Path>,
    options: &ReadOptions,
) -> Result<(LocationEvidence, String)> {
    let location = location(raw, directory, options)?;
    let key = if let Some(path) = &location.native_path {
        digest(path.as_os_str().as_encoded_bytes())
    } else {
        digest(
            location
                .extensions
                .get("normalized_source_path")
                .and_then(Value::as_str)
                .unwrap_or(raw)
                .as_bytes(),
        )
    };
    Ok((location, key))
}
fn database(
    snapshot: &ImportSnapshot,
    id: &str,
    options: &ReadOptions,
    cancel: &CancellationToken,
) -> Result<Node> {
    let root = xml::parse(
        &bytes(snapshot, id, options, cancel)?,
        &options.limits,
        cancel,
    )?;
    ensure!(
        root.name == "VirtualDJ_Database" && matches!(root.attr("Version"), Some("8.4" | "8.5")),
        "Unqualified VirtualDJ XML profile"
    );
    Ok(root)
}
fn song(
    node: &Node,
    id: &str,
    locations: LocationEvidence,
    options: &ReadOptions,
) -> Result<TrackObservation> {
    let mut track = TrackObservation {
        external_id: id.into(),
        locations,
        extensions: BTreeMap::from([("native".into(), node.raw())]),
        ..Default::default()
    };
    let tags = node.child("Tags");
    if let Some(tags) = tags {
        for (source, target) in [
            ("Title", "title"),
            ("Author", "artist"),
            ("Album", "album"),
            ("Genre", "genre"),
            ("Label", "label"),
            ("Remixer", "remixer"),
            ("Composer", "composer"),
            ("TrackNumber", "track_number"),
            ("Grouping", "grouping"),
            ("Year", "year"),
            ("Remix", "mix"),
            ("User1", "virtualdj_user1"),
            ("User2", "virtualdj_user2"),
        ] {
            if let Some(value) = tags.attr(source) {
                track
                    .metadata
                    .insert(target.into(), apple_music::presence(&json!(value)));
            }
        }
        if let Some(value) = tags.attr("Bpm") {
            track
                .cached_tags
                .insert("bpm".into(), apple_music::presence(&json!(value)));
        }
        if let Some(value) = tags.number("Stars").filter(|v| (0. ..=5.).contains(v)) {
            track.metadata.insert(
                "rating".into(),
                Presence::Value(json!({"raw":value,"scale_max":5})),
            );
        }
    }
    if let Some(comment) = node.child("Comment") {
        track.metadata.insert(
            "comment".into(),
            apple_music::presence(&json!(comment.text)),
        );
    }
    if let Some(info) = node.child("Infos") {
        if let Some(value) = info.number("SongLength").filter(|v| *v >= 0.) {
            track
                .extensions
                .insert("duration_seconds".into(), json!(value));
        }
        if options.include_history {
            for (source, target) in [("PlayCount", "play_count"), ("LastPlay", "last_played")] {
                if let Some(value) = info.attr(source) {
                    track
                        .metadata
                        .insert(target.into(), Presence::Value(json!(value)));
                }
            }
        }
    }
    let scan = node.child("Scan");
    let seconds = scan.and_then(|n| n.number("Bpm")).filter(|v| *v > 0.);
    if let Some(value) = seconds.map(|v| 60. / v).filter(|v| v.is_finite()) {
        track.bpm = Some(ScalarBpm {
            presence: FieldPresence::Value,
            value: Some(value),
            ..Default::default()
        });
    } else if scan.is_some_and(|n| n.attr("Bpm").is_some()) {
        issue(&mut track, "invalid_native_bpm", "bpm");
    }
    if let Some(raw) = tags
        .and_then(|n| n.attr("Key"))
        .or_else(|| scan.and_then(|n| n.attr("Key")))
    {
        track.key = Some(MusicalKey {
            presence: if raw.is_empty() {
                FieldPresence::Empty
            } else {
                FieldPresence::Value
            },
            raw: Some(raw.into()),
            camelot: camelot(raw),
            provenance: Some(
                if tags.and_then(|n| n.attr("Key")).is_some() {
                    "virtualdj_tags"
                } else {
                    "virtualdj_scan"
                }
                .into(),
            ),
            ..Default::default()
        });
    }
    let mut points = Vec::new();
    let mut slots = BTreeSet::new();
    for (index, poi) in node.children.iter().filter(|n| n.name == "Poi").enumerate() {
        let kind = poi.attr("Type").unwrap_or("");
        let start = poi.number("Pos");
        if kind == "beatgrid" {
            let bpm = if poi.attr("Bpm").is_some() {
                poi.number("Bpm")
            } else {
                track.bpm.as_ref().and_then(|b| b.value)
            }
            .filter(|v| *v > 0.);
            if let (Some(native_seconds), Some(bpm)) = (start, bpm) {
                points.push(GridPoint {
                    native_seconds,
                    bpm: Some(bpm),
                    original: poi.raw(),
                    ..Default::default()
                });
            } else {
                issue(&mut track, "invalid_native_grid_point", "grid");
            }
            continue;
        }
        let slot = poi
            .attr("Num")
            .and_then(|v| v.parse::<i32>().ok())
            .filter(|v| *v > 0)
            .map(|v| v - 1);
        let family = match kind {
            "cue" | "loop" if slot.is_some() => CueFamily::Hot,
            "cue" => CueFamily::Memory,
            "loop" => CueFamily::Loop,
            "load" => CueFamily::Load,
            _ => CueFamily::Unknown,
        };
        if family == CueFamily::Hot
            && let Some(slot) = slot
            && !slots.insert(slot)
        {
            issue(&mut track, "duplicate_hotcue_slot", "cues.hot");
        }
        let end = if kind == "loop" {
            start
                .zip(seconds)
                .zip(poi.number("Size").filter(|v| *v > 0.))
                .map(|((s, seconds), beats)| s + seconds * beats)
                .filter(|v| v.is_finite())
        } else {
            None
        };
        if kind == "loop" && end.is_none() {
            issue(&mut track, "invalid_loop_length", "cues.loop");
        }
        track.cues.push(CueObservation {
            external_id: format!("poi:{index}"),
            family,
            source_slot: if family == CueFamily::Hot { slot } else { None },
            name: poi.attr("Name").map(str::to_owned),
            color_rgb: poi
                .attr("Color")
                .and_then(|v| v.parse::<u32>().ok())
                .map(|v| v & 0xffffff),
            original: poi.raw(),
            native_start_seconds: start,
            native_end_seconds: end,
            flags: BTreeMap::from([
                (
                    "active_loop".into(),
                    json!(kind == "loop" && poi.attr("AutoTrigger") == Some("yes")),
                ),
                ("loop".into(), json!(kind == "loop")),
            ]),
            ..Default::default()
        });
    }
    if let Some(first) = points.first() {
        track.grid = Some(GridObservation {
            representation: "points".into(),
            original_time_base: Some("seconds".into()),
            native_first_beat_seconds: Some(first.native_seconds),
            bpm: first.bpm,
            points,
            ..Default::default()
        });
    }
    Ok(track)
}
fn collection_id(path: &Path) -> String {
    format!("list:{}", digest(path.as_os_str().as_encoded_bytes()))
}
fn parent_folders(
    relative: &Path,
    collections: &mut BTreeMap<String, CollectionObservation>,
    limits: &ImportLimits,
) -> Result<Option<String>> {
    let parent = relative.parent().unwrap_or_else(|| Path::new(""));
    let mut prefix = PathBuf::new();
    let mut current = None;
    for (depth, component) in parent.components().enumerate() {
        ensure!(
            depth < limits.hierarchy_depth,
            "VirtualDJ folder depth limit"
        );
        prefix.push(component.as_os_str());
        let name = component
            .as_os_str()
            .to_str()
            .context("Non-UTF8 VirtualDJ folder name requires native resolution")?;
        if current.is_none()
            && matches!(
                name,
                "MyLists" | "My Lists" | "Playlists" | "Folders" | "Filters"
            )
        {
            continue;
        }
        let id = collection_id(&prefix);
        collections
            .entry(id.clone())
            .or_insert_with(|| CollectionObservation {
                external_id: id.clone(),
                parent_id: current.clone(),
                kind: CollectionKind::Folder,
                name: name.strip_suffix(".subfolders").unwrap_or(name).into(),
                ..Default::default()
            });
        current = Some(id);
    }
    Ok(current)
}
fn reference(
    raw: &str,
    directory: Option<&Path>,
    options: &ReadOptions,
    known: &References,
    tracks: &mut BTreeMap<String, TrackObservation>,
) -> Result<String> {
    let (locations, key) = identity(raw, directory, options)?;
    if let Some(ids) = known.get(&key)
        && ids.len() == 1
    {
        return Ok(ids[0].clone());
    }
    let id = format!("reference:{key}");
    if !tracks.contains_key(&id) {
        let mut track = TrackObservation {
            external_id: id.clone(),
            locations,
            ..Default::default()
        };
        issue(
            &mut track,
            if known.contains_key(&key) {
                "ambiguous_track_reference"
            } else {
                "missing_track_record"
            },
            "location",
        );
        if let Some(candidates) = known.get(&key) {
            track
                .extensions
                .insert("candidate_track_ids".into(), json!(candidates));
        }
        tracks.insert(id.clone(), track);
    }
    Ok(id)
}
impl SourceAdapter for VirtualDjReader {
    fn provider(&self) -> SourceProvider {
        SourceProvider::VirtualDj
    }
    fn inspect(&self, snapshot: &ImportSnapshot) -> Result<SourceProfile> {
        ensure!(
            snapshot.provider == self.provider(),
            "VirtualDJ provider mismatch"
        );
        for artifact in &snapshot.artifacts {
            let path = snapshot.original_artifact_path(&artifact.id)?;
            if path.file_name().is_some_and(|n| n == "database.xml") {
                database(
                    snapshot,
                    &artifact.id,
                    &ReadOptions::default(),
                    &CancellationToken::default(),
                )?;
            } else {
                ensure!(
                    path.file_name().is_some_and(|n| n == "order")
                        || path
                            .extension()
                            .is_some_and(|e| e == "vdjfolder" || e == "m3u" || e == "m3u8"),
                    "Unsupported VirtualDJ artifact"
                );
            }
        }
        Ok(profile(
            self.provider(),
            "virtualdj-xml8.4-8.5-mylists",
            &[
                "descriptive_tags",
                "custom_tags",
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
        cancel.check()?;
        snapshot.validate_source(cancel)?;
        let profile = self.inspect(snapshot)?;
        let mut tracks = BTreeMap::new();
        let mut known: References = BTreeMap::new();
        let mut collections = BTreeMap::new();
        let mut members = Vec::new();
        for artifact in &snapshot.artifacts {
            let path = snapshot.original_artifact_path(&artifact.id)?;
            if !path.file_name().is_some_and(|n| n == "database.xml") {
                continue;
            }
            let root = database(snapshot, &artifact.id, options, cancel)?;
            for node in root.children.iter().filter(|n| n.name == "Song") {
                cancel.check()?;
                let raw = node
                    .attr("FilePath")
                    .context("VirtualDJ Song has no path identity")?;
                let (locations, key) = identity(raw, None, options)?;
                let id = format!(
                    "song:{}",
                    digest(serde_json::to_string(&(&artifact.id, &key))?.as_bytes())
                );
                ensure!(
                    !tracks.contains_key(&id),
                    "Duplicate VirtualDJ song path in one database"
                );
                let mut track = song(node, &id, locations, options)?;
                track
                    .locations
                    .extensions
                    .insert("database_artifact".into(), json!(artifact.id));
                tracks.insert(id.clone(), track);
                known.entry(key).or_default().push(id);
            }
        }
        for artifact in &snapshot.artifacts {
            cancel.check()?;
            let path = snapshot.original_artifact_path(&artifact.id)?;
            if path.file_name().is_some_and(|n| n == "database.xml") {
                continue;
            }
            let relative = path.strip_prefix(snapshot.source_root())?;
            let historical = history_path(relative);
            if historical && !options.include_history {
                continue;
            }
            let parent = parent_folders(relative, &mut collections, &options.limits)?;
            if path.file_name().is_some_and(|n| n == "order") {
                let content = m3u::text(&bytes(snapshot, &artifact.id, options, cancel)?)?;
                for (position, line) in content
                    .lines()
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .enumerate()
                {
                    ensure!(
                        !line.contains(['/', '\\', ':', '\0']) && !matches!(line, "." | ".."),
                        "VirtualDJ order traversal or unsafe child"
                    );
                    let child = relative
                        .parent()
                        .context("Order has no parent folder")?
                        .join(line);
                    let folder = line.ends_with(".subfolders");
                    ensure!(
                        folder
                            || line.ends_with(".vdjfolder")
                            || line.ends_with(".m3u")
                            || line.ends_with(".m3u8"),
                        "Unsupported VirtualDJ order entry"
                    );
                    let id = collection_id(&child);
                    let entry =
                        collections
                            .entry(id.clone())
                            .or_insert_with(|| CollectionObservation {
                                external_id: id,
                                parent_id: parent.clone(),
                                kind: if folder {
                                    CollectionKind::Folder
                                } else {
                                    CollectionKind::Playlist
                                },
                                name: Path::new(line)
                                    .file_stem()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .into(),
                                ..Default::default()
                            });
                    entry
                        .extensions
                        .insert("sibling_position".into(), json!(position));
                }
                continue;
            }
            let id = collection_id(relative);
            let mut native_members = Vec::new();
            let mut collection = CollectionObservation {
                external_id: id.clone(),
                parent_id: parent,
                name: path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into(),
                kind: if historical {
                    CollectionKind::History
                } else {
                    CollectionKind::Playlist
                },
                ..Default::default()
            };
            if path.extension().is_some_and(|e| e == "vdjfolder") {
                let root = xml::parse(
                    &bytes(snapshot, &artifact.id, options, cancel)?,
                    &options.limits,
                    cancel,
                )?;
                ensure!(
                    matches!(root.name.as_str(), "VirtualFolder" | "FilterFolder"),
                    "Unqualified VirtualDJ list root"
                );
                collection
                    .extensions
                    .insert("native".into(), root.raw_without(&["song", "Song"]));
                if root.name == "FilterFolder" || root.attr("filter").is_some() {
                    collection.kind = CollectionKind::Smart;
                    collection.extensions.insert("issues".into(),json!([{"code":"unsupported_filter","message":"Original filter retained; only exported materialized membership imported"}]));
                }
                for node in root
                    .children
                    .iter()
                    .filter(|n| n.name.eq_ignore_ascii_case("song"))
                {
                    let raw = node
                        .attr("path")
                        .or_else(|| node.attr("FilePath"))
                        .context("VirtualDJ list song has no path")?;
                    native_members.push((raw.to_owned(), node.raw()));
                }
            } else {
                let content = m3u::text(&bytes(snapshot, &artifact.id, options, cancel)?)?;
                for line in content
                    .split(['\r', '\n'])
                    .map(str::trim)
                    .filter(|v| !v.is_empty() && !v.starts_with('#'))
                {
                    native_members.push((line.into(), json!({"native_reference":line})));
                }
            }
            if let Some(previous) = collections.insert(id.clone(), collection)
                && let Some(position) = previous.extensions.get("sibling_position")
            {
                collections
                    .get_mut(&id)
                    .context("Missing list")?
                    .extensions
                    .insert("sibling_position".into(), position.clone());
            }
            for (position, (raw, native)) in native_members.into_iter().enumerate() {
                cancel.check()?;
                let track = reference(&raw, path.parent(), options, &known, &mut tracks)?;
                let mut member = membership(&id, &track, position);
                if let ObservationRecord::Membership { occurrence, .. } = &mut member {
                    occurrence.extensions.insert("native".into(), native);
                }
                members.push(member);
            }
        }
        let collections = collections.into_values().collect::<Vec<_>>();
        validate_hierarchy(&collections, &options.limits)?;
        let result = header(
            snapshot,
            options,
            &profile,
            None,
            tracks.len() as u64,
            collections.len() as u64,
            members.len() as u64,
        );
        for track in tracks.into_values() {
            emit_track(track, options, sink, cancel)?;
        }
        for collection in collections {
            cancel.check()?;
            sink.push(ObservationRecord::Collection(collection))?;
        }
        for member in members {
            cancel.check()?;
            sink.push(member)?;
        }
        cancel.check()?;
        snapshot.validate_source(cancel)?;
        Ok(result)
    }
}
