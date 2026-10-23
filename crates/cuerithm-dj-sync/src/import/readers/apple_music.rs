use super::*;
use std::collections::BTreeSet;
pub struct AppleMusicReader;
fn load(
    snapshot: &ImportSnapshot,
    options: &ReadOptions,
    cancel: &CancellationToken,
) -> Result<Value> {
    ensure!(
        snapshot.provider == SourceProvider::AppleMusicXml && snapshot.artifacts.len() == 1,
        "Choose one Apple Music exported plist XML artifact"
    );
    let artifact = &snapshot.artifacts[0];
    ensure!(
        matches!(artifact.kind, ArtifactKind::StableFile),
        "Apple export requires a stable XML file"
    );
    let root = xml::parse(
        &bytes(snapshot, &artifact.id, options, cancel)?,
        &options.limits,
        cancel,
    )?;
    ensure!(
        root.name == "plist"
            && root.children.len() == 1
            && root.text.trim().is_empty()
            && root.attributes.get("version").is_none_or(|v| v == "1.0"),
        "Unsupported Apple plist root/version"
    );
    let value = xml::plist(
        root.children
            .into_iter()
            .next()
            .context("Missing plist dictionary")?,
    )?;
    ensure!(
        value.is_object()
            && value.get("Tracks").is_some_and(Value::is_object)
            && value.get("Playlists").is_none_or(Value::is_array),
        "Not an Apple Music library export"
    );
    for (name, expected) in [("Major Version", 1), ("Minor Version", 1)] {
        if let Some(version) = value.get(name) {
            ensure!(
                version.as_i64() == Some(expected),
                "Unqualified Apple export version"
            );
        }
    }
    if let Some(instance) = value.get("Library Persistent ID") {
        ensure!(
            instance.as_str().is_some_and(|s| !s.is_empty()),
            "Invalid Apple library persistent ID"
        );
    }
    Ok(value)
}
fn safe_native(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .filter(|(key, _)| {
                    let key = key.to_ascii_lowercase().replace([' ', '_', '-'], "");
                    ![
                        "credential",
                        "password",
                        "secret",
                        "token",
                        "account",
                        "appleid",
                        "email",
                        "authorization",
                        "cookie",
                        "settings",
                    ]
                    .iter()
                    .any(|p| key.contains(p))
                })
                .map(|(key, value)| (key.clone(), safe_native(value)))
                .collect(),
        ),
        Value::Array(array) => Value::Array(array.iter().map(safe_native).collect()),
        value => value.clone(),
    }
}
fn presence(value: &Value) -> Presence<Value> {
    if value.as_str().is_some_and(str::is_empty) || value.as_array().is_some_and(Vec::is_empty) {
        Presence::Empty(value.clone())
    } else {
        Presence::Value(value.clone())
    }
}
impl SourceAdapter for AppleMusicReader {
    fn provider(&self) -> SourceProvider {
        SourceProvider::AppleMusicXml
    }
    fn inspect(&self, snapshot: &ImportSnapshot) -> Result<SourceProfile> {
        load(
            snapshot,
            &ReadOptions::default(),
            &CancellationToken::default(),
        )?;
        Ok(profile(
            self.provider(),
            "apple-library-plist-1.1",
            &[
                "collections",
                "descriptive_tags",
                "ratings",
                "bpm",
                "history",
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
        let root = load(snapshot, options, cancel)?;
        let profile = profile(
            self.provider(),
            "apple-library-plist-1.1",
            &[
                "collections",
                "descriptive_tags",
                "ratings",
                "bpm",
                "history",
            ],
        );
        let source_tracks = root["Tracks"].as_object().context("Missing Apple tracks")?;
        let mut ids = BTreeSet::new();
        let mut track_count = 0;
        let mut member_count = 0;
        for (id, native) in source_tracks {
            cancel.check()?;
            let native = native.as_object().context("Malformed Apple track")?;
            let numeric: u64 = id.parse().context("Invalid Apple track dictionary ID")?;
            ensure!(
                native
                    .get("Track ID")
                    .is_none_or(|v| v.as_u64() == Some(numeric)),
                "Apple track ID differs from dictionary ID"
            );
            ensure!(ids.insert(id.clone()), "Duplicate Apple track ID");
            let mut track = TrackObservation {
                external_id: id.clone(),
                ..Default::default()
            };
            if let Some(path) = native.get("Location") {
                track.locations = location(
                    path.as_str().context("Nontext Apple track location")?,
                    None,
                    options,
                )?;
            } else {
                track.locations.availability =
                    if native.get("Track Type").and_then(Value::as_str) == Some("Remote") {
                        "remote"
                    } else {
                        "unresolved"
                    }
                    .into();
            }
            for (source, destination) in [
                ("Name", "title"),
                ("Artist", "artist"),
                ("Album", "album"),
                ("Album Artist", "album_artist"),
                ("Genre", "genre"),
                ("Comments", "comment"),
                ("Composer", "composer"),
                ("Grouping", "grouping"),
                ("Year", "year"),
                ("Track Number", "track_number"),
                ("Track Count", "track_total"),
                ("Disc Number", "disc_number"),
                ("Disc Count", "disc_total"),
            ] {
                if let Some(value) = native.get(source) {
                    track.metadata.insert(destination.into(), presence(value));
                }
            }
            if let Some(rating) = native.get("Rating") {
                let rating = rating
                    .as_u64()
                    .filter(|r| *r <= 100)
                    .context("Apple rating is not on the 0–100 scale")?;
                track.metadata.insert(
                    "rating".into(),
                    Presence::Value(json!({"raw":rating,"scale_max":100})),
                );
            }
            if options.include_history {
                for (source, destination) in [
                    ("Play Count", "play_count"),
                    ("Play Date UTC", "last_played"),
                ] {
                    if let Some(value) = native.get(source) {
                        track.metadata.insert(destination.into(), presence(value));
                    }
                }
            }
            if let Some(bpm) = native.get("BPM") {
                let value = bpm
                    .as_f64()
                    .filter(|n| n.is_finite() && *n >= 0.)
                    .context("Invalid Apple BPM")?;
                track.bpm = Some(ScalarBpm {
                    presence: FieldPresence::Value,
                    value: (value > 0.).then_some(value),
                    extensions: BTreeMap::from([("raw".into(), bpm.clone())]),
                    ..Default::default()
                });
            }
            if let Some(duration) = native.get("Total Time") {
                let ms = duration
                    .as_u64()
                    .context("Invalid Apple duration milliseconds")?;
                track
                    .extensions
                    .insert("duration_seconds".into(), json!(ms as f64 / 1000.));
            }
            track
                .extensions
                .insert("native".into(), safe_native(&json!(native)));
            emit_track(track, options, sink, cancel)?;
            track_count += 1;
        }
        let mut collections = Vec::new();
        let mut memberships = Vec::new();
        if let Some(playlists) = root.get("Playlists").and_then(Value::as_array) {
            for native in playlists {
                cancel.check()?;
                let id = native
                    .get("Playlist Persistent ID")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .context("Apple playlist has no persistent ID")?;
                let parent_id = native
                    .get("Parent Persistent ID")
                    .map(|v| {
                        v.as_str()
                            .map(str::to_owned)
                            .context("Invalid Apple playlist parent")
                    })
                    .transpose()?;
                let kind = if native.get("Folder").and_then(Value::as_bool) == Some(true) {
                    CollectionKind::Folder
                } else if native.get("Smart Info").is_some()
                    || native.get("Smart Criteria").is_some()
                {
                    CollectionKind::Smart
                } else {
                    CollectionKind::Playlist
                };
                let mut retained = safe_native(native);
                if let Some(map) = retained.as_object_mut() {
                    map.remove("Playlist Items");
                }
                collections.push(CollectionObservation {
                    external_id: id.into(),
                    parent_id,
                    kind,
                    name: native
                        .get("Name")
                        .and_then(Value::as_str)
                        .context("Apple playlist name missing")?
                        .into(),
                    extensions: BTreeMap::from([("native".into(), retained)]),
                    ..Default::default()
                });
                if let Some(items) = native.get("Playlist Items") {
                    for (position, item) in items
                        .as_array()
                        .context("Invalid Apple playlist items")?
                        .iter()
                        .enumerate()
                    {
                        cancel.check()?;
                        let track_id = item
                            .get("Track ID")
                            .and_then(Value::as_u64)
                            .context("Invalid Apple playlist track reference")?
                            .to_string();
                        if ids.insert(track_id.clone()) {
                            emit_track(
                                TrackObservation {
                                    external_id: track_id.clone(),
                                    locations: LocationEvidence {
                                        availability: "unresolved".into(),
                                        ..Default::default()
                                    },
                                    issues: vec![ImportIssue {
                                        code: "missing_track_record".into(),
                                        record_id: Some(track_id.clone()),
                                        message: "Export references a track whose record is absent"
                                            .into(),
                                        ..Default::default()
                                    }],
                                    ..Default::default()
                                },
                                options,
                                sink,
                                cancel,
                            )?;
                            track_count += 1;
                        }
                        let mut member = membership(id, &track_id, position);
                        if let ObservationRecord::Membership { occurrence, .. } = &mut member {
                            occurrence
                                .extensions
                                .insert("native".into(), safe_native(item));
                        }
                        memberships.push(member);
                        member_count += 1;
                    }
                }
            }
        }
        let known: BTreeSet<_> = collections.iter().map(|c| c.external_id.clone()).collect();
        let missing: BTreeSet<_> = collections
            .iter()
            .filter_map(|c| c.parent_id.as_ref())
            .filter(|id| !known.contains(*id))
            .cloned()
            .collect();
        for id in missing {
            collections.push(CollectionObservation {
                external_id: id,
                kind: CollectionKind::Folder,
                name: "Unavailable folder".into(),
                extensions: BTreeMap::from([("missing_source_record".into(), json!(true))]),
                ..Default::default()
            });
        }
        validate_hierarchy(&collections, &options.limits)?;
        let collection_count = collections.len() as u64;
        for collection in collections {
            cancel.check()?;
            sink.push(ObservationRecord::Collection(collection))?;
        }
        for member in memberships {
            cancel.check()?;
            sink.push(member)?;
        }
        cancel.check()?;
        snapshot.validate_source(cancel)?;
        Ok(header(
            snapshot,
            options,
            &profile,
            root.get("Library Persistent ID").and_then(Value::as_str),
            track_count,
            collection_count,
            member_count,
        ))
    }
}
