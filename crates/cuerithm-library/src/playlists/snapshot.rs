use super::*;
use cuerithm_smartlists::{
    CueKind, FieldCatalog, FieldDescriptor, FieldKind, FieldValue, FileAvailability, SmartCue,
    SmartDefinition, SmartTrack,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::SystemTime,
};
#[derive(Debug, Clone)]
pub(crate) struct SnapshotTrack {
    pub track: SmartTrack,
    pub unavailable: BTreeMap<String, String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Observation {
    pub path: PathBuf,
    pub availability: FileAvailability,
    pub length: Option<u64>,
    pub modified: Option<SystemTime>,
}
pub(crate) fn observe(path: &Path) -> Observation {
    match std::fs::metadata(path) {
        Ok(m) => Observation {
            path: path.into(),
            availability: FileAvailability::Present,
            length: Some(m.len()),
            modified: m.modified().ok(),
        },
        Err(e) => Observation {
            path: path.into(),
            availability: if e.kind() == std::io::ErrorKind::NotFound {
                FileAvailability::Missing
            } else {
                FileAvailability::Unavailable(e.to_string())
            },
            length: None,
            modified: None,
        },
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CachedStatus {
    pub state: String,
    pub error: Option<String>,
    pub input: Option<u64>,
    pub playlist: Option<u64>,
    pub preference: Option<u64>,
    pub at: Option<i64>,
    pub next: Option<i64>,
    pub good: Option<u64>,
    pub signature: Option<String>,
}
impl CachedStatus {
    pub(crate) fn from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Self::from_row_at(r, 0)
    }
    pub(crate) fn from_row_at(r: &rusqlite::Row<'_>, at: usize) -> rusqlite::Result<Self> {
        Ok(Self {
            state: r.get(at)?,
            error: r.get(at + 1)?,
            input: r.get(at + 2)?,
            playlist: r.get(at + 3)?,
            preference: r.get(at + 4)?,
            at: r.get(at + 5)?,
            next: r.get(at + 6)?,
            good: r.get(at + 7)?,
            signature: r.get(at + 8)?,
        })
    }
}
#[derive(Debug)]
pub(crate) struct Snapshot {
    pub tree: Tree,
    pub field_catalog: SmartFieldCatalog,
    pub tracks: Vec<SnapshotTrack>,
    pub all_ids: BTreeSet<String>,
    pub preferences: PlaylistPreferences,
    pub zone: String,
    pub now: i64,
    pub statuses: BTreeMap<String, CachedStatus>,
    pub protected: BTreeSet<String>,
    pub observations: Vec<Observation>,
}
fn metadata_value(v: &serde_json::Value) -> FieldValue {
    match v {
        serde_json::Value::String(s) => FieldValue::Text(s.clone()),
        serde_json::Value::Bool(b) => FieldValue::Boolean(*b),
        serde_json::Value::Number(n) => n
            .as_f64()
            .map(FieldValue::Number)
            .unwrap_or(FieldValue::Null),
        serde_json::Value::Null => FieldValue::Null,
        _ => FieldValue::Text(v.to_string()),
    }
}
fn numeric_field(field: &str, value: &FieldValue) -> Option<f64> {
    let number = match value {
        FieldValue::Number(n) => *n,
        FieldValue::Text(text) => {
            let mut text = text.trim();
            if field == "trackNumber" {
                return text
                    .split('/')
                    .next()?
                    .trim()
                    .parse::<u32>()
                    .ok()
                    .map(f64::from);
            }
            if field == "year" && text.as_bytes().get(4) == Some(&b'-') {
                let year = text.get(..4)?;
                if !year.bytes().all(|c| c.is_ascii_digit()) {
                    return None;
                }
                text = year;
            }
            text.parse::<f64>().ok()?
        }
        _ => return None,
    };
    number.is_finite().then_some(number)
}
fn insert(catalog: &mut FieldCatalog, id: &str, label: &str, kind: FieldKind, editable: bool) {
    catalog.fields.insert(
        id.into(),
        FieldDescriptor {
            label: label.into(),
            kind,
            any_text: editable && kind == FieldKind::Text,
            unavailable_reason: None,
        },
    );
}
fn cue_data(track: &mut SnapshotTrack, cues: &serde_json::Value) -> Result<()> {
    if let Some(facts) = cues.get("native_facts") {
        let facts: NativePlaylistFacts = serde_json::from_value(facts.clone())?;
        ensure!(
            facts.version == 1 && !facts.source.trim().is_empty() && facts.source.len() <= 128,
            "Unsupported native cue facts"
        );
        ensure!(
            facts.tempos.iter().all(|b| b.is_finite()),
            "Invalid native tempo facts"
        );
        track.track.cues = facts.cues;
        track.track.tempos = facts.tempos;
        return Ok(());
    }
    let cues: camelot_core::lexicon_cues::StructuralCueResult =
        serde_json::from_value(cues.clone())?;
    track.track.cues = cues
        .cues
        .into_iter()
        .map(|c| SmartCue {
            label: Some(c.name),
            color: Some(c.color),
            kind: CueKind::Cue,
            active_loop: false,
            memory: false,
        })
        .collect();
    if cues.emergency_loop.is_some() {
        track.track.cues.push(SmartCue {
            label: Some("Emergency loop".into()),
            color: None,
            kind: CueKind::Loop,
            active_loop: true,
            memory: false,
        });
    }
    Ok(())
}
fn waveform_observation(
    root: &Path,
    analysis: &camelot_core::FileAnalysis,
    observations: &mut Vec<Observation>,
) -> Result<bool> {
    let Some(media) = &analysis.media else {
        return Ok(false);
    };
    let key = format!(
        "{:x}",
        Sha256::digest(format!("{}:{}:16384", media.source_sha256, media.decoder))
    );
    let path = root.join("cache/waveforms").join(format!("{key}.json"));
    let observed = observe(&path);
    // Pin unsuccessful reads too: a repaired cache must invalidate a captured error.
    observations.push(observed.clone());
    let found = match observed.availability {
        FileAvailability::Missing => false,
        FileAvailability::Unavailable(ref reason) => {
            anyhow::bail!("Waveform availability unknown: {reason}")
        }
        FileAvailability::Present => {
            use std::io::Read;
            let file = std::fs::File::open(&path)?;
            let w: crate::waveform::Waveform = serde_json::from_reader(file.take(4 * 1024 * 1024))?;
            ensure!(
                serde_json::to_value(&w.clock)? == serde_json::to_value(&media.clock)?
                    && w.duration_seconds == media.clock.duration_seconds()
                    && w.bins.len() == 16384
                    && w.bins
                        .iter()
                        .all(|b| b.min.is_finite() && b.max.is_finite() && b.min <= b.max),
                "Waveform cache does not match analysis"
            );
            true
        }
    };
    Ok(found)
}
pub(crate) fn signature(
    definition: &SmartDefinition,
    zone: &str,
    preferences: &PlaylistPreferences,
) -> Result<String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(
            &serde_json::json!({"definition":definition,"zone":zone,"mixing_mode":preferences.key_mixing_mode,"engine":env!("CUERITHM_SMART_ENGINE_ID"),"native_metadata":cfg!(feature="autotag")})
        )?)
    ))
}
pub(crate) fn dynamic(definition: &SmartDefinition) -> bool {
    definition.rules.iter().any(|r| {
        matches!(
            r.field.as_str(),
            "CustomFileMissing" | "coverartPreview" | "waveform"
        ) || matches!(
            r.operator,
            cuerithm_smartlists::Operator::DateRecent
                | cuerithm_smartlists::Operator::DateNotRecent
        )
    })
}
impl Library {
    pub fn smart_field_catalog(&self) -> Result<SmartFieldCatalog> {
        Ok(self
            .smart_snapshot(crate::now(), None, false)?
            .field_catalog)
    }
    pub(crate) fn smart_snapshot(
        &self,
        now: i64,
        extra: Option<&SmartDefinition>,
        inspect_files: bool,
    ) -> Result<Snapshot> {
        self.smart_snapshot_cancellable(now, extra, inspect_files, None)
    }
    pub(super) fn smart_snapshot_cancellable(
        &self,
        now: i64,
        extra: Option<&SmartDefinition>,
        inspect_files: bool,
        stop: Option<&AtomicBool>,
    ) -> Result<Snapshot> {
        ensure!(
            !stop.is_some_and(|s| s.load(Ordering::Acquire)),
            "Smart refresh cancelled"
        );
        #[cfg(feature = "autotag")]
        let _gate = self
            .inner
            .tag_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Metadata access interrupted"))?;
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let stamp = preferences::stamp(&tx)?;
        let prefs = preferences::preferences(&tx)?.preferences;
        let zone = preferences::zone(&prefs)?;
        let _ = cuerithm_smartlists::EvaluationContext::new(now, &zone)?;
        let tree = load(&tx)?;
        let mut catalog = FieldCatalog::standard();
        let mut tag_choices = vec![];
        let mut category_choices = vec![];
        for (id, label) in [
            ("title", "Title"),
            ("artist", "Artist"),
            ("album", "Album"),
            ("albumArtist", "Album artist"),
            ("bpm", "BPM"),
            ("key", "Key"),
            ("energy", "Energy"),
            ("dateAdded", "Date added"),
            ("dateModified", "Library modified"),
            ("bitrate", "Bitrate (kbps)"),
            ("sizeBytes", "Size (bytes)"),
            ("cuepoints", "Cue count"),
            ("tempomarkers", "Tempo marker count"),
            ("AnyTextField", "Any text field"),
            ("EveryTextField", "Every text field"),
        ] {
            catalog.fields.get_mut(id).unwrap().label = label.into();
        }
        for (id, kind, label) in [
            ("popularity", FieldKind::Number, "Popularity"),
            ("happiness", FieldKind::Number, "Happiness"),
            ("streamingService", FieldKind::Text, "Streaming service"),
            (
                "CustomUploadedCloudStorage",
                FieldKind::Boolean,
                "Uploaded to cloud storage",
            ),
            ("cloudUpload", FieldKind::Boolean, "Cloud upload"),
        ] {
            catalog.fields.insert(
                id.into(),
                FieldDescriptor {
                    label: label.into(),
                    kind,
                    any_text: false,
                    unavailable_reason: Some(
                        "No local source is available for this service field".into(),
                    ),
                },
            );
        }
        insert(
            &mut catalog,
            "coverartPreview",
            "Embedded artwork",
            FieldKind::Boolean,
            false,
        );
        #[cfg(not(feature = "autotag"))]
        {
            catalog
                .fields
                .get_mut("coverartPreview")
                .unwrap()
                .unavailable_reason =
                Some("Native metadata reading is unavailable in this build".into());
        }
        let mut statement = tx.prepare("SELECT id,name FROM custom_tag_categories ORDER BY id")?;
        for row in
            statement.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        {
            let (id, name) = row?;
            catalog.categories.insert(id.clone(), Default::default());
            category_choices.push(SmartCategoryChoice { id, name });
        }
        drop(statement);
        let mut statement =
            tx.prepare("SELECT id,category_id,name FROM custom_tags ORDER BY id")?;
        for row in statement.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })? {
            let (id, category_id, name) = row?;
            catalog.tags.insert(id.clone());
            catalog
                .categories
                .get_mut(&category_id)
                .context("Tag category unavailable")?
                .insert(id.clone());
            tag_choices.push(SmartTagChoice {
                id,
                category_id,
                name,
            });
        }
        drop(statement);
        for row in tree.values().filter(|r| !r.node.deleted) {
            catalog.playlists.insert(row.node.id.clone(), row.node.kind);
        }
        let mut linked: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut statement = tx.prepare("SELECT track_id,tag_id FROM track_custom_tags")?;
        for row in
            statement.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        {
            let (id, tag) = row?;
            linked.entry(id).or_default().insert(tag);
        }
        drop(statement);
        let mut need_file = false;
        let mut need_art = false;
        let mut need_wave = false;
        for def in tree
            .values()
            .filter(|r| !r.node.deleted)
            .filter_map(|r| r.node.definition.as_ref())
            .chain(extra)
        {
            for r in &def.rules {
                need_file |= r.field == "CustomFileMissing";
                need_art |= r.field == "coverartPreview";
                need_wave |= r.field == "waveform";
            }
        }
        need_file &= inspect_files;
        need_art &= inspect_files;
        need_wave &= inspect_files;
        let mut statement=tx.prepare("SELECT id,path,title,artist,album,genre,state,archived,analysis_json,metadata_json,native_metadata_json,created,updated,(SELECT json FROM cue_revisions c WHERE c.track_id=tracks.id ORDER BY c.created DESC,c.rowid DESC LIMIT 1) FROM tracks ORDER BY sort_title,id")?;
        let rows = statement
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Vec<u8>>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, bool>(7)?,
                    r.get::<_, Option<String>>(8)?,
                    r.get::<_, String>(9)?,
                    r.get::<_, Option<String>>(10)?,
                    r.get::<_, i64>(11)?,
                    r.get::<_, i64>(12)?,
                    r.get::<_, Option<String>>(13)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(statement);
        let mut statuses = BTreeMap::new();
        let mut statement=tx.prepare("SELECT playlist_id,state,error,input_revision,playlist_revision,preference_revision,evaluated_at,next_refresh,last_good_revision,definition_hash FROM smart_playlist_status")?;
        for row in statement.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                CachedStatus {
                    state: r.get(1)?,
                    error: r.get(2)?,
                    input: r.get(3)?,
                    playlist: r.get(4)?,
                    preference: r.get(5)?,
                    at: r.get(6)?,
                    next: r.get(7)?,
                    good: r.get(8)?,
                    signature: r.get(9)?,
                },
            ))
        })? {
            let (id, status) = row?;
            statuses.insert(id, status);
        }
        drop(statement);
        let protected = self
            .settings_from_connection(&tx)?
            .settings
            .cleanup
            .map(|c| c.genres.protected_values)
            .unwrap_or_default();
        tx.commit()?;
        drop(conn);
        let mut all_ids = BTreeSet::new();
        let mut tracks = vec![];
        let mut observations = vec![];
        for (
            id,
            path,
            title,
            artist,
            album,
            genre,
            state,
            archived,
            analysis,
            info,
            native,
            created,
            updated,
            cue_json,
        ) in rows
        {
            ensure!(
                !stop.is_some_and(|s| s.load(Ordering::Acquire)),
                "Smart refresh cancelled"
            );
            all_ids.insert(id.clone());
            if state != "ready" {
                continue;
            }
            let mut row = SnapshotTrack {
                track: SmartTrack::new(&id),
                unavailable: Default::default(),
            };
            row.track.archived = archived;
            row.track.tags = linked.remove(&id).unwrap_or_default();
            for (key, value) in [
                ("title", title),
                ("artist", artist),
                ("album", album),
                ("genre", genre),
            ] {
                row.track.fields.insert(key.into(), FieldValue::Text(value));
            }
            row.track
                .fields
                .insert("dateAdded".into(), FieldValue::Date(created));
            row.track
                .fields
                .insert("dateModified".into(), FieldValue::Date(updated));
            let media = serde_json::from_str::<cuerithm_media::MediaInfo>(&info);
            if let Ok(info) = &media {
                for (key, value) in info.columns() {
                    let typed = metadata_value(&value);
                    let kind = match typed {
                        FieldValue::Number(_) => FieldKind::Number,
                        FieldValue::Boolean(_) => FieldKind::Boolean,
                        _ => FieldKind::Text,
                    };
                    catalog
                        .fields
                        .entry(key.clone())
                        .or_insert(FieldDescriptor {
                            label: key.clone(),
                            kind,
                            any_text: key.starts_with("tag."),
                            unavailable_reason: None,
                        });
                    row.track.fields.insert(key, typed);
                }
                row.track
                    .fields
                    .insert("fileType".into(), FieldValue::Text(info.codec.clone()));
                row.track.fields.insert(
                    "fileTypeLabel".into(),
                    FieldValue::Text(info.codec.to_uppercase()),
                );
                row.track.fields.insert(
                    "sampleRate".into(),
                    FieldValue::Number(info.sample_rate as f64),
                );
                for (key, sources, scale) in [
                    ("bitrate", &["audio.bit_rate"][..], 1000.),
                    ("sizeBytes", &["format.size"][..], 1.),
                    ("year", &["tag.year", "tag.date"][..], 1.),
                    (
                        "trackNumber",
                        &["tag.tracknumber", "tag.track", "tag.track_number"][..],
                        1.,
                    ),
                    ("rating", &["tag.rating"][..], 1.),
                ] {
                    if let Some(n) = sources.iter().find_map(|source| {
                        row.track
                            .fields
                            .get(*source)
                            .and_then(|v| numeric_field(key, v))
                    }) {
                        row.track
                            .fields
                            .insert(key.into(), FieldValue::Number(n / scale));
                    }
                }
                for field in [
                    "comment",
                    "grouping",
                    "label",
                    "remixer",
                    "mix",
                    "producer",
                    "composer",
                    "lyricist",
                    "extra1",
                    "extra2",
                    "isrc",
                    "catalogNumber",
                    "albumArtist",
                ] {
                    let key = format!("tag.{}", field.to_lowercase());
                    if let Some(value) = row.track.fields.get(&key).cloned() {
                        row.track.fields.insert(field.into(), value);
                    }
                }
                if !row.track.fields.contains_key("albumArtist") {
                    for alias in ["tag.album_artist", "tag.album artist"] {
                        if let Some(value) = row.track.fields.get(alias).cloned() {
                            row.track.fields.insert("albumArtist".into(), value);
                            break;
                        }
                    }
                }
                if let Some(color) = info.tags.get("color") {
                    row.track
                        .fields
                        .insert("color".into(), FieldValue::Color(color.clone()));
                }
            }
            if let Some(native) = native {
                match serde_json::from_str::<cuerithm_cleanup::Metadata>(&native).and_then(|m| {
                    m.validate().map_err(serde::de::Error::custom)?;
                    Ok(m)
                }) {
                    Ok(m) => {
                        for (field, values) in m.fields {
                            let key = match field {
                                cuerithm_cleanup::MetadataField::AlbumArtist => {
                                    "albumArtist".into()
                                }
                                _ => serde_json::to_value(field)?.as_str().unwrap().to_owned(),
                            };
                            row.track
                                .fields
                                .insert(key.clone(), FieldValue::Text(values.join("; ")));
                            if key == "genre" {
                                row.track.genres = values;
                            }
                        }
                    }
                    Err(e) => {
                        for field in cuerithm_cleanup::MetadataField::ALL {
                            let key = serde_json::to_value(field)?.as_str().unwrap().to_owned();
                            row.unavailable.insert(key, e.to_string());
                        }
                    }
                }
            }
            if row.track.genres.is_empty()
                && let Some(FieldValue::Text(genre)) = row.track.fields.get("genre")
                && !genre.is_empty()
            {
                row.track.genres.push(genre.clone());
            }
            let parsed = analysis
                .as_deref()
                .map(serde_json::from_str::<camelot_core::FileAnalysis>);
            if let Some(parsed) = &parsed {
                match parsed {
                    Ok(a) => {
                        row.track.key = a.analysis.key.as_ref().map(|k| k.camelot.clone());
                        row.track.fields.insert(
                            "duration".into(),
                            FieldValue::Number(a.analysis.duration_seconds),
                        );
                        if let Some(t) = &a.analysis.tempo {
                            row.track.tempos = vec![t.bpm];
                            row.track
                                .fields
                                .insert("bpm".into(), FieldValue::Number(t.bpm));
                        }
                        if let Some(e) = &a.analysis.energy {
                            row.track
                                .fields
                                .insert("energy".into(), FieldValue::Number(e.level.into()));
                        }
                        if cue_json.is_none()
                            && let Some(c) = &a.analysis.lexicon_cues
                        {
                            cue_data(&mut row, &serde_json::to_value(c)?)?;
                        }
                    }
                    Err(e) => {
                        for field in [
                            "key",
                            "duration",
                            "bpm",
                            "energy",
                            "cuepoints",
                            "cues",
                            "tempomarkers",
                            "tempos",
                            "IsAnalyzed",
                            "waveform",
                        ] {
                            row.unavailable.insert(field.into(), e.to_string());
                        }
                    }
                }
            }
            if let Some(cues) = cue_json {
                let read = serde_json::from_str(&cues)
                    .map_err(anyhow::Error::from)
                    .and_then(|c| cue_data(&mut row, &c));
                if let Err(e) = read {
                    for field in ["cuepoints", "cues", "tempomarkers", "tempos", "IsAnalyzed"] {
                        row.unavailable.insert(field.into(), e.to_string());
                    }
                } else {
                    for field in ["cuepoints", "cues"] {
                        row.unavailable.remove(field);
                    }
                }
            }
            match crate::bytes_path(&path) {
                Ok(path) => {
                    row.track.fields.insert(
                        "location".into(),
                        FieldValue::Text(path.to_string_lossy().into_owned()),
                    );
                    if need_file || need_art {
                        let observed = observe(&path);
                        row.track.file = observed.availability.clone();
                        observations.push(observed);
                    }
                    #[cfg(feature = "autotag")]
                    if need_art {
                        let art = media
                            .as_ref()
                            .map_err(|e| anyhow::anyhow!(e.to_string()))
                            .and_then(|info| cuerithm_autotag::has_embedded_artwork(&path, info));
                        match art {
                            Ok(true) => {
                                row.track
                                    .fields
                                    .insert("coverartPreview".into(), FieldValue::Boolean(true));
                            }
                            Ok(false) => {}
                            Err(e) => {
                                row.unavailable
                                    .insert("coverartPreview".into(), e.to_string());
                            }
                        }
                    }
                }
                Err(e) => {
                    row.track.file = FileAvailability::Unavailable(e.to_string());
                    row.unavailable.insert("location".into(), e.to_string());
                }
            }
            if need_wave && let Some(Ok(a)) = &parsed {
                match waveform_observation(self.data_dir(), a, &mut observations) {
                    Ok(found) => row.track.waveform = found,
                    Err(e) => {
                        row.unavailable.insert("waveform".into(), e.to_string());
                    }
                }
            }
            tracks.push(row);
        }
        let playlists = tree
            .values()
            .filter(|r| !r.node.deleted)
            .map(|r| r.node.clone())
            .collect();
        Ok(Snapshot {
            tree,
            field_catalog: SmartFieldCatalog {
                catalog,
                tags: tag_choices,
                categories: category_choices,
                playlists,
                stamp,
            },
            tracks,
            all_ids,
            preferences: prefs,
            zone,
            now,
            statuses,
            protected,
            observations,
        })
    }
    fn settings_from_connection(&self, conn: &Connection) -> Result<crate::SettingsSnapshot> {
        let json:String=conn.query_row("SELECT s.json FROM settings s JOIN selected_settings a ON a.revision=s.revision WHERE a.singleton=1",[],|r|r.get(0))?;
        Ok(serde_json::from_str(&json)?)
    }
}
