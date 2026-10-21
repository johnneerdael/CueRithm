//! Pure native row planning. Producing a plan does not grant write capability.
use crate::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeOwnedRow {
    pub id: String,
    pub expected_raw_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeTrackDraft {
    pub local_id: String,
    pub identity: Option<TrackIdentity>,
    pub expected_raw_hash: Option<String>,
    pub source_path: PathBuf,
    /// Explicit logical fields only. Absence retains the current native value.
    pub fields: BTreeMap<String, TargetValue>,
    /// Times must already use the shared export timing contract.
    pub cues: Option<Vec<NativeCue>>,
    pub cue_colors: Option<Vec<NativeCueColor>>,
    pub replace_cues: Vec<NativeOwnedRow>,
    pub analysis_path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeCueColor {
    pub color: i64,
    pub color_table_index: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NativeEntityKind {
    Playlist,
    MyTag,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeEntityIdentity {
    pub native_id: String,
    pub uuid: Option<String>,
    pub expected_raw_hash: String,
    pub owned: bool,
    pub owned_links: Vec<NativeOwnedRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NativeEntityRef {
    Local(String),
    Native(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeEntityDraft {
    pub kind: NativeEntityKind,
    pub local_id: String,
    pub identity: Option<NativeEntityIdentity>,
    pub parent: Option<NativeEntityRef>,
    pub name: String,
    pub attribute: i64,
    pub sequence: i64,
    pub smart_rule: Option<String>,
    /// Native content IDs, in reviewed relative order. Unowned links retain
    /// their positions; reject orders that cannot fit around those fixed links.
    pub members: Vec<NativeEntityRef>,
    pub remove: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativePlanRequest {
    pub source: PathBuf,
    /// Native timestamp, supplied by the caller: YYYY-MM-DD HH:mm:ss.SSS Z.
    pub now: String,
    pub iso_now: String,
    /// Fresh random entropy supplied by the caller; deterministic within a preview.
    pub id_seed: [u8; 32],
    pub tracks: Vec<NativeTrackDraft>,
    pub entities: Vec<NativeEntityDraft>,
    /// Optional tables, such as contentFile, read from the same snapshot.
    pub auxiliary_records: Vec<TargetRecord>,
    pub share_root: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeResolvedTrack {
    pub local_id: String,
    pub identity: TrackIdentity,
    pub created: bool,
    pub owned_cue_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeResolvedEntity {
    pub kind: NativeEntityKind,
    pub local_id: String,
    pub native_id: String,
    pub uuid: Option<String>,
    pub owned_link_ids: Vec<String>,
    pub removed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedNativeRows {
    pub plan: NativeWritePlan,
    pub tracks: Vec<NativeResolvedTrack>,
    pub entities: Vec<NativeResolvedEntity>,
    pub local_usn: Option<u64>,
}

fn text(value: impl Into<String>) -> TargetValue {
    TargetValue::Text(value.into())
}
fn integer(value: i64) -> TargetValue {
    TargetValue::Integer(value)
}
type Values = BTreeMap<String, TargetValue>;
fn values(fields: &[(&str, TargetValue)]) -> Values {
    fields
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}

struct Planner<'a> {
    collection: &'a RekordboxCollection,
    request: &'a NativePlanRequest,
    rows: BTreeMap<(String, String), Values>,
    pending: BTreeMap<(String, String), NativeRowMutation>,
    nonce: u64,
    usn: i64,
}

impl<'a> Planner<'a> {
    fn original(&self, table: &str, id: &str) -> Option<&TargetRecord> {
        self.collection
            .records
            .get(table)
            .into_iter()
            .flatten()
            .chain(
                self.request
                    .auxiliary_records
                    .iter()
                    .filter(|r| r.table == table),
            )
            .find(|r| r.id == id)
    }
    fn current(&self, table: &str, id: &str) -> Option<&Values> {
        self.rows.get(&(table.into(), id.into()))
    }
    fn check(&self, table: &str, proof: &NativeOwnedRow) -> Result<&TargetRecord> {
        let row = self
            .original(table, &proof.id)
            .context("Native row identity is missing")?;
        ensure!(
            row.fingerprint == proof.expected_raw_hash,
            "Native row proof changed"
        );
        Ok(row)
    }
    fn random(&mut self) -> [u8; 32] {
        let mut digest = Sha256::new();
        digest.update(self.request.id_seed);
        digest.update(self.nonce.to_be_bytes());
        self.nonce += 1;
        digest.finalize().into()
    }
    fn uuid(&mut self) -> String {
        loop {
            let mut bytes = self.random();
            bytes[6] = (bytes[6] & 15) | 64;
            bytes[8] = (bytes[8] & 63) | 128;
            let uuid = format!(
                "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
                bytes[0],
                bytes[1],
                bytes[2],
                bytes[3],
                bytes[4],
                bytes[5],
                bytes[6],
                bytes[7],
                bytes[8],
                bytes[9],
                bytes[10],
                bytes[11],
                bytes[12],
                bytes[13],
                bytes[14],
                bytes[15]
            );
            if !self
                .rows
                .values()
                .any(|row| row.get("UUID") == Some(&text(&uuid)))
            {
                return uuid;
            }
        }
    }
    /// Keep foreign link bytes and slots fixed. Preserve existing positions for
    /// an unchanged reviewed order; otherwise fit owned members into free slots.
    /// Desired foreign members are fixed anchors, never new ownership claims.
    fn playlist_positions(
        &self,
        playlist: &str,
        contents: &[String],
        owned: &[NativeOwnedRow],
    ) -> Result<Vec<i64>> {
        let owned_ids: BTreeSet<_> = owned.iter().map(|proof| proof.id.as_str()).collect();
        let links: Vec<_> = self
            .rows
            .iter()
            .filter(|((table, _), row)| {
                table == "djmdSongPlaylist"
                    && row.get("PlaylistID") == Some(&text(playlist))
                    && row.get("rb_local_deleted") != Some(&integer(1))
            })
            .collect();
        let position = |row: &Values| match row.get("TrackNo") {
            Some(TargetValue::Integer(value)) if *value > 0 => Some(*value),
            _ => None,
        };
        let mut reserved = BTreeSet::new();
        for ((_, id), row) in &links {
            if !owned_ids.contains(id.as_str()) {
                let slot = position(row)
                    .context("Native playlist order has an invalid external position")?;
                ensure!(
                    reserved.insert(slot),
                    "Native playlist order has duplicate external positions"
                );
            }
        }
        let mut current = Vec::with_capacity(contents.len());
        let mut anchors = Vec::with_capacity(contents.len());
        for content in contents {
            let matches: Vec<_> = links
                .iter()
                .filter(|(_, row)| row.get("ContentID") == Some(&text(content)))
                .collect();
            ensure!(
                matches.len() <= 1,
                "Native playlist order has ambiguous existing membership"
            );
            let existing = matches.first();
            let slot = existing.and_then(|(_, row)| position(row));
            let external = existing.is_some_and(|((_, id), _)| !owned_ids.contains(id.as_str()));
            current.push(slot);
            anchors.push(if external { slot } else { None });
        }
        let preserved: Option<Vec<i64>> = current.into_iter().collect();
        if let Some(preserved) = preserved {
            let valid = preserved.windows(2).all(|pair| pair[0] < pair[1])
                && preserved
                    .iter()
                    .zip(&anchors)
                    .all(|(slot, anchor)| anchor.is_some() || !reserved.contains(slot));
            if valid {
                return Ok(preserved);
            }
        }
        let mut previous = 0i64;
        let mut positions = Vec::with_capacity(contents.len());
        for anchor in anchors {
            let slot = if let Some(anchor) = anchor {
                ensure!(
                    anchor > previous,
                    "Reviewed native playlist order cannot fit around retained external positions"
                );
                anchor
            } else {
                let mut candidate = previous
                    .checked_add(1)
                    .context("Native playlist order position overflow")?;
                while reserved.contains(&candidate) {
                    candidate = candidate
                        .checked_add(1)
                        .context("Native playlist order position overflow")?;
                }
                candidate
            };
            positions.push(slot);
            previous = slot;
        }
        Ok(positions)
    }
    fn allocate(&mut self, table: &str, column: &str, random: bool) -> Result<String> {
        let observed = self
            .rows
            .iter()
            .filter(|((t, _), _)| t == table)
            .filter_map(|((_, id), v)| {
                if column == "ID" {
                    Some(cast_native_id(id))
                } else {
                    match v.get(column) {
                        Some(TargetValue::Text(v)) => Some(cast_native_id(v)),
                        Some(TargetValue::Integer(v)) => Some(*v),
                        _ => None,
                    }
                }
            })
            .collect::<Vec<_>>();
        let used = observed
            .iter()
            .filter_map(|v| u32::try_from(*v).ok())
            .collect::<BTreeSet<_>>();
        if !random {
            let next = observed
                .into_iter()
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .and_then(|v| u32::try_from(v).ok());
            if let Some(next) = next {
                return Ok(next.to_string());
            }
        }
        for _ in 0..100_000 {
            let bytes = self.random();
            let candidate = u32::from_be_bytes(bytes[..4].try_into().expect("four bytes"));
            if !used.contains(&candidate) {
                return Ok(candidate.to_string());
            }
        }
        anyhow::bail!("Native identifier space exhausted")
    }
    fn write(&mut self, table: &str, id: &str, patch: Values, status: i64) -> bool {
        let key = (table.to_string(), id.to_string());
        let before = self.rows.get(&key);
        if before.is_some_and(|v| patch.iter().all(|(k, x)| v.get(k) == Some(x))) {
            return false;
        }
        let created = before.is_none();
        let mut patch = patch;
        patch.insert("rb_local_usn".into(), integer(self.usn));
        if table != "contentFile" {
            patch.insert("rb_data_status".into(), integer(status));
        }
        patch.insert("updated_at".into(), text(&self.request.now));
        if created {
            patch.insert("ID".into(), text(id));
            patch.insert("created_at".into(), text(&self.request.now));
            for name in [
                "rb_local_data_status",
                "rb_local_deleted",
                "rb_local_synced",
            ] {
                if table != "contentFile" || name == "rb_local_deleted" {
                    patch.entry(name.into()).or_insert(integer(0));
                }
            }
        }
        let proof = self.original(table, id).map(|r| r.fingerprint.clone());
        self.rows
            .entry(key.clone())
            .or_default()
            .extend(patch.clone());
        self.pending
            .entry(key)
            .or_insert_with(|| NativeRowMutation {
                table: table.into(),
                id: id.into(),
                expected_fingerprint: proof,
                values: Values::new(),
            })
            .values
            .extend(patch);
        true
    }
    fn foreign(&mut self, table: &str, field: &str, name: &str) -> Result<TargetValue> {
        if name.is_empty() {
            return Ok(TargetValue::Null);
        }
        let existing = self
            .rows
            .iter()
            .find(|((t, _), v)| t == table && v.get(field) == Some(&text(name)))
            .map(|((_, id), v)| (id.clone(), v.clone()));
        if let Some((id, row)) = existing {
            if row.get("rb_local_deleted") == Some(&integer(1)) {
                self.write(table, &id, values(&[("rb_local_deleted", integer(0))]), 257);
            }
            return Ok(text(id));
        }
        let id = self.allocate(table, "ID", false)?;
        let uuid = self.uuid();
        self.write(
            table,
            &id,
            values(&[(field, text(name)), ("UUID", text(uuid))]),
            0,
        );
        Ok(text(id))
    }
    fn metadata(&mut self, fields: &Values, content: &str) -> Result<Values> {
        let mut patch = Values::new();
        if fields.contains_key("album") || fields.contains_key("album_artist") {
            let current_album = self
                .current("djmdContent", content)
                .and_then(|row| row.get("AlbumID"))
                .and_then(|value| match value {
                    TargetValue::Text(id) => self.current("djmdAlbum", id),
                    _ => None,
                });
            let prior_name = current_album.and_then(|row| row.get("Name")).cloned();
            let prior_artist = current_album
                .and_then(|row| row.get("AlbumArtistID"))
                .cloned()
                .unwrap_or(TargetValue::Null);
            let empty_artist = fields
                .get("album_artist")
                .map(logical_name)
                .transpose()?
                .is_some_and(str::is_empty);
            let name = fields
                .get("album")
                .cloned()
                .or(prior_name)
                .or_else(|| empty_artist.then_some(TargetValue::Null))
                .context("Review an album name before setting album artist")?;
            let name = logical_name(&name)?;
            let artist = if let Some(name) = fields.get("album_artist") {
                let name = logical_name(name)?;
                self.foreign("djmdArtist", "Name", name)?
            } else if name.is_empty() {
                TargetValue::Null
            } else {
                prior_artist
            };
            if name.is_empty() {
                ensure!(
                    artist == TargetValue::Null,
                    "Review an album name before setting album artist"
                );
                patch.insert("AlbumID".into(), TargetValue::Null);
            } else {
                let found = self
                    .rows
                    .iter()
                    .find(|((table, _), row)| {
                        table == "djmdAlbum"
                            && row.get("Name") == Some(&text(name))
                            && row.get("AlbumArtistID").unwrap_or(&TargetValue::Null) == &artist
                    })
                    .map(|((_, id), row)| {
                        (id.clone(), row.get("rb_local_deleted") == Some(&integer(1)))
                    });
                let id = if let Some((id, deleted)) = found {
                    if deleted {
                        self.write(
                            "djmdAlbum",
                            &id,
                            values(&[("rb_local_deleted", integer(0))]),
                            257,
                        );
                    }
                    id
                } else {
                    let id = self.allocate("djmdAlbum", "ID", false)?;
                    let uuid = self.uuid();
                    self.write(
                        "djmdAlbum",
                        &id,
                        values(&[
                            ("Name", text(name)),
                            ("AlbumArtistID", artist),
                            ("UUID", text(uuid)),
                        ]),
                        0,
                    );
                    id
                };
                patch.insert("AlbumID".into(), text(id));
            }
        }
        for (logical, value) in fields {
            if matches!(logical.as_str(), "album" | "album_artist") {
                continue;
            }
            if logical == "source_path" {
                let path = logical_name(value)?;
                ensure!(
                    absolute_path(path) && path.len() <= 4096,
                    "Reviewed native source path must be absolute"
                );
                patch.extend(values(&[
                    ("FolderPath", text(path)),
                    ("OrgFolderPath", text(path)),
                    (
                        "FileNameL",
                        text(path.rsplit(['/', '\\']).next().unwrap_or(path)),
                    ),
                ]));
                continue;
            }
            if logical == "artwork_path" {
                patch.insert("ImagePath".into(), text(native_artwork_path(value)?));
                continue;
            }
            let foreign = match logical.as_str() {
                "artist" => Some(("djmdArtist", "Name", "ArtistID")),
                "composer" => Some(("djmdArtist", "Name", "ComposerID")),
                "remixer" => Some(("djmdArtist", "Name", "RemixerID")),
                "original_artist" => Some(("djmdArtist", "Name", "OrgArtistID")),
                "album" => Some(("djmdAlbum", "Name", "AlbumID")),
                "genre" => Some(("djmdGenre", "Name", "GenreID")),
                "label" => Some(("djmdLabel", "Name", "LabelID")),
                "key" => Some(("djmdKey", "ScaleName", "KeyID")),
                _ => None,
            };
            if let Some((table, field, column)) = foreign {
                let name = match value {
                    TargetValue::Null => "",
                    TargetValue::Text(name) => name,
                    _ => anyhow::bail!("Native foreign metadata must be text or null"),
                };
                ensure!(
                    name.len() <= 1_048_576 && !name.contains('\0'),
                    "Invalid native metadata text"
                );
                patch.insert(column.into(), self.foreign(table, field, name)?);
                continue;
            }
            let column = match logical.as_str() {
                "title" => "Title",
                "comment" => "Commnt",
                "lyricist" => "Lyricist",
                "mix_name" => "Subtitle",
                "release_year" => "ReleaseYear",
                "release_date" => "ReleaseDate",
                "track_number" => "TrackNo",
                "disc_number" => "DiscNo",
                "rating" => "Rating",
                "bpm" => "BPM",
                "duration_seconds" => "Length",
                "file_type" => "FileType",
                "file_size" => "FileSize",
                "sample_rate" => "SampleRate",
                "bitrate" => "BitRate",
                "bit_depth" => "BitDepth",
                _ => anyhow::bail!("Unsupported native logical field: {logical}"),
            };
            let textual = matches!(
                logical.as_str(),
                "title" | "comment" | "lyricist" | "mix_name" | "release_date"
            );
            ensure!(
                matches!(value, TargetValue::Null)
                    || if textual {
                        matches!(value, TargetValue::Text(_))
                    } else {
                        matches!(value, TargetValue::Integer(_) | TargetValue::Real(_))
                    },
                "Wrong native metadata field type"
            );
            match value {
                TargetValue::Text(v) => ensure!(
                    v.len() <= 1_048_576 && !v.contains('\0'),
                    "Invalid native metadata text"
                ),
                TargetValue::Integer(v) => ensure!(*v >= 0, "Negative native metadata value"),
                TargetValue::Real(v) => {
                    ensure!(v.is_finite() && *v >= 0., "Invalid native numeric metadata")
                }
                TargetValue::Blob(_) => anyhow::bail!("Binary native metadata is unsupported"),
                TargetValue::Null => {}
            }
            let value = if logical == "bpm" {
                match value {
                    TargetValue::Integer(v) => {
                        integer(v.checked_mul(100).context("Native BPM overflow")?)
                    }
                    TargetValue::Real(v) => {
                        ensure!(*v <= i64::MAX as f64 / 100., "Native BPM overflow");
                        integer((v * 100.).round() as i64)
                    }
                    TargetValue::Null => TargetValue::Null,
                    _ => anyhow::bail!("Native BPM must be numeric"),
                }
            } else if let TargetValue::Real(v) = value {
                if v.fract() == 0. && *v < i64::MAX as f64 {
                    integer(*v as i64)
                } else {
                    value.clone()
                }
            } else {
                value.clone()
            };
            if logical == "rating" {
                ensure!(
                    matches!(value, TargetValue::Null | TargetValue::Integer(0..=5)),
                    "Native rating outside 0..5"
                );
            }
            patch.insert(column.into(), value);
        }
        Ok(patch)
    }
    fn track(&mut self, draft: &NativeTrackDraft) -> Result<NativeResolvedTrack> {
        ensure!(
            !draft.local_id.is_empty(),
            "Missing local native track identity"
        );
        let path = if let Some(path) = draft.fields.get("source_path") {
            logical_name(path)?
        } else {
            draft
                .source_path
                .to_str()
                .context("Native source path is not Unicode")?
        };
        ensure!(
            absolute_path(path) && !path.contains('\0'),
            "Native source path must be absolute"
        );
        let created = draft.identity.is_none();
        let (id, uuid, master) = if let Some(identity) = &draft.identity {
            ensure!(
                identity.database_id == self.collection.identity.database_id,
                "Native track database identity changed"
            );
            let row = self
                .original("djmdContent", &identity.content_id)
                .context("Native content identity is missing")?;
            ensure!(
                Some(&row.fingerprint) == draft.expected_raw_hash.as_ref(),
                "Native content proof changed"
            );
            ensure!(
                row.text("UUID")? == identity.uuid
                    && row.text("MasterSongID")? == identity.master_song_id,
                "Native content identity was reused"
            );
            (
                identity.content_id.clone(),
                identity
                    .uuid
                    .clone()
                    .context("Native content UUID missing")?,
                identity
                    .master_song_id
                    .clone()
                    .context("Native master-song identity missing")?,
            )
        } else {
            ensure!(
                draft.expected_raw_hash.is_none(),
                "Unexpected proof for new native track"
            );
            (
                self.allocate("djmdContent", "ID", false)?,
                self.uuid(),
                self.allocate("djmdContent", "MasterSongID", false)?,
            )
        };
        let mut patch = self.metadata(&draft.fields, &id)?;
        let metadata_changed = self
            .current("djmdContent", &id)
            .is_some_and(|row| patch.iter().any(|(k, v)| row.get(k) != Some(v)));
        if created {
            patch.extend(values(&[
                ("UUID", text(&uuid)),
                ("MasterSongID", text(&master)),
                ("MasterDBID", text(&self.collection.identity.database_id)),
                (
                    "DeviceID",
                    self.collection
                        .identity
                        .device_id
                        .clone()
                        .map(text)
                        .unwrap_or(TargetValue::Null),
                ),
                ("FolderPath", text(path)),
                ("OrgFolderPath", text(path)),
                (
                    "FileNameL",
                    text(path.rsplit(['/', '\\']).next().unwrap_or(path)),
                ),
                ("FileNameS", TargetValue::Null),
                ("ContentLink", integer(0)),
                ("HotCueAutoLoad", text("on")),
                ("SamplerTrackInfo", integer(0)),
                ("Analysed", integer(16)),
                ("CueUpdated", text("0")),
                ("AnalysisUpdated", text("0")),
                ("TrackInfoUpdated", text("0")),
                ("SamplerPlayOffset", integer(0)),
                ("SamplerGain", TargetValue::Real(0.)),
                ("VideoAssociate", text("0")),
                ("LyricStatus", integer(0)),
                ("ServiceID", integer(0)),
                ("ExtInfo", text("null")),
                ("rb_file_id", text("0")),
            ]));
            patch.entry("ImagePath".into()).or_insert_with(|| text(""));
        } else if metadata_changed {
            patch.insert(
                "TrackInfoUpdated".into(),
                text(self.counter(&id, "TrackInfoUpdated")?.to_string()),
            );
        }
        if let Some(path) = &draft.analysis_path {
            ensure!(
                path.starts_with("/PIONEER/USBANLZ/")
                    && !path.contains(['\0', '\\'])
                    && !path.split('/').any(|part| part == ".." || part == "."),
                "Invalid native analysis reference"
            );
            let changed = self
                .current("djmdContent", &id)
                .is_some_and(|row| row.get("AnalysisDataPath") != Some(&text(path)));
            patch.insert("AnalysisDataPath".into(), text(path));
            if !created && changed {
                patch.insert(
                    "AnalysisUpdated".into(),
                    text(self.counter(&id, "AnalysisUpdated")?.to_string()),
                );
            }
            if created || changed {
                self.analysis_file(&id, &uuid, path)?;
            }
        }
        if !patch.is_empty() {
            self.write("djmdContent", &id, patch, if created { 0 } else { 257 });
        }
        let owned_cue_ids = if let Some(cues) = &draft.cues {
            let (owned, changed) = self.cues(
                &id,
                &uuid,
                cues,
                draft.cue_colors.as_deref(),
                &draft.replace_cues,
            )?;
            if changed && !created {
                let count = self.counter(&id, "CueUpdated")?;
                self.write(
                    "djmdContent",
                    &id,
                    values(&[("CueUpdated", text(count.to_string()))]),
                    257,
                );
            }
            owned
        } else {
            ensure!(
                draft.replace_cues.is_empty() && draft.cue_colors.is_none(),
                "Native cue proofs/colors without reviewed cue replacement"
            );
            vec![]
        };
        Ok(NativeResolvedTrack {
            local_id: draft.local_id.clone(),
            identity: TrackIdentity {
                database_id: self.collection.identity.database_id.clone(),
                content_id: id.clone(),
                uuid: Some(uuid),
                master_song_id: Some(master),
                local_usn: if self.pending.contains_key(&("djmdContent".into(), id)) {
                    Some(self.usn as u64)
                } else {
                    draft.identity.as_ref().and_then(|i| i.local_usn)
                },
            },
            created,
            owned_cue_ids,
        })
    }
    fn counter(&self, id: &str, column: &str) -> Result<i64> {
        let v = match self.current("djmdContent", id).and_then(|r| r.get(column)) {
            None | Some(TargetValue::Null) => 0,
            Some(TargetValue::Integer(v)) => *v,
            Some(TargetValue::Text(v)) => {
                v.parse::<i64>().context("Invalid native content counter")?
            }
            _ => anyhow::bail!("Invalid native content counter"),
        };
        ensure!(v >= 0, "Invalid negative native content counter");
        v.checked_add(1).context("Native content counter overflow")
    }

    fn analysis_file(&mut self, content: &str, uuid: &str, path: &str) -> Result<()> {
        let root = self
            .request
            .share_root
            .as_ref()
            .context("Native share root required for analysis reference")?;
        let local = root.join(path.trim_start_matches('/'));
        let local = local
            .to_str()
            .context("Native analysis resource path is not Unicode")?;
        let mut encoded = String::new();
        for byte in path.bytes() {
            if byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')'
                )
            {
                encoded.push(char::from(byte));
            } else {
                encoded.push_str(&format!("%{byte:02X}"));
            }
        }
        let id = format!("{uuid}_{encoded}");
        if let Some(existing) = self.current("contentFile", &id) {
            ensure!(
                existing.get("ContentID") == Some(&text(content))
                    && existing.get("Path") == Some(&text(path)),
                "Native analysis-file identity collision"
            );
            return Ok(());
        }
        let file_uuid = self.uuid();
        let mut patch = values(&[
            ("ContentID", text(content)),
            ("Path", text(path)),
            ("Hash", TargetValue::Null),
            ("Size", TargetValue::Null),
            ("rb_local_path", text(local)),
            ("UUID", text(file_uuid)),
        ]);
        for name in [
            "rb_file_hash_dirty",
            "rb_local_file_status",
            "rb_in_progress",
            "rb_process_type",
            "rb_priority",
            "rb_file_size_dirty",
            "rb_local_deleted",
        ] {
            patch.insert(name.into(), integer(0));
        }
        self.write("contentFile", &id, patch, 0);
        Ok(())
    }

    fn cue_values(
        content: &str,
        content_uuid: &str,
        cue: &NativeCue,
        color: Option<NativeCueColor>,
        kind: i64,
        active: bool,
        bpm: Option<f64>,
    ) -> Result<Values> {
        ensure!(
            cue.end.is_none_or(|end| end.millis() > cue.start.millis()),
            "Native loop end must follow start"
        );
        ensure!(
            !cue.active_loop || cue.end.is_some(),
            "Only native loops can be active"
        );
        ensure!(
            !cue.comment.contains('\0'),
            "Native cue comment contains NUL"
        );
        let loop_size = if let (Some(end), Some(bpm)) = (cue.end, bpm) {
            let beats = ((f64::from(end.millis() - cue.start.millis()) / 1000.) / (60. / bpm))
                .round()
                .max(1.);
            ensure!(
                beats <= (i64::MAX as f64 - 1.) / 65536.,
                "Native loop size overflow"
            );
            integer((beats as i64) * 65536 + 1)
        } else {
            TargetValue::Null
        };
        ensure!(
            cue.memory_color <= 8,
            "Unsupported native memory palette index"
        );
        let color = if let Some(color) = color {
            ensure!(
                (-1..=7).contains(&color.color) || color.color == 255,
                "Invalid native cue color"
            );
            ensure!(
                color
                    .color_table_index
                    .is_none_or(|index| (0..=64).contains(&index)),
                "Invalid native hot palette index"
            );
            let valid = match cue.slot {
                NativeCueSlot::Memory if (0..=7).contains(&color.color) => {
                    color.color_table_index.is_none()
                }
                NativeCueSlot::Memory => color.color_table_index == Some(0),
                NativeCueSlot::Hot(_) => {
                    matches!(color.color, -1 | 255) && color.color_table_index.is_some()
                }
            };
            ensure!(
                valid,
                "Native cue SQL color and palette nullability do not match destination"
            );
            color
        } else {
            match cue.slot {
                NativeCueSlot::Memory => NativeCueColor {
                    color: if cue.memory_color > 0 {
                        i64::from(cue.memory_color) - 1
                    } else if cue.end.is_some() {
                        255
                    } else {
                        -1
                    },
                    color_table_index: if cue.memory_color > 0 { None } else { Some(0) },
                },
                NativeCueSlot::Hot(_) => {
                    ensure!(
                        cue.color == [0; 4],
                        "Colored native hot cue requires explicit palette"
                    );
                    NativeCueColor {
                        color: if cue.end.is_some() { 255 } else { -1 },
                        color_table_index: Some(0),
                    }
                }
            }
        };
        Ok(values(&[
            ("ContentID", text(content)),
            ("ContentUUID", text(content_uuid)),
            ("Kind", integer(kind)),
            ("InMsec", integer(i64::from(cue.start.millis()))),
            (
                "OutMsec",
                integer(cue.end.map_or(-1, |t| i64::from(t.millis()))),
            ),
            ("InFrame", integer(0)),
            ("OutFrame", integer(0)),
            ("InMpegFrame", integer(0)),
            ("InMpegAbs", integer(0)),
            ("OutMpegAbs", integer(0)),
            ("OutMpegFrame", integer(0)),
            ("ActiveLoop", integer(i64::from(active))),
            ("Comment", text(&cue.comment)),
            ("Color", integer(color.color)),
            (
                "ColorTableIndex",
                color
                    .color_table_index
                    .map(integer)
                    .unwrap_or(TargetValue::Null),
            ),
            ("BeatLoopSize", loop_size),
        ]))
    }
    fn cues(
        &mut self,
        content: &str,
        content_uuid: &str,
        cues: &[NativeCue],
        colors: Option<&[NativeCueColor]>,
        owned: &[NativeOwnedRow],
    ) -> Result<(Vec<String>, bool)> {
        ensure!(
            colors.is_none_or(|colors| colors.len() == cues.len()),
            "Native cue palette count differs"
        );
        let mut seen = BTreeSet::new();
        for proof in owned {
            ensure!(seen.insert(&proof.id), "Duplicate owned native cue");
            let row = self.check("djmdCue", proof)?;
            ensure!(
                row.text("ContentID")?.as_deref() == Some(content),
                "Owned cue belongs to another native track"
            );
        }
        let bpm = match self
            .current("djmdContent", content)
            .and_then(|r| r.get("BPM"))
        {
            Some(TargetValue::Integer(v)) if *v > 0 => Some(*v as f64 / 100.),
            Some(TargetValue::Real(v)) if *v > 0. && v.is_finite() => Some(*v / 100.),
            _ => None,
        };
        let mut slots = BTreeSet::new();
        let mut active_memory = 0;
        let mut first_active_hot = false;
        let mut desired = vec![];
        let unowned: Vec<_> = self
            .rows
            .iter()
            .filter(|((table, id), row)| {
                table == "djmdCue"
                    && !owned.iter().any(|proof| proof.id == *id)
                    && row.get("ContentID") == Some(&text(content))
                    && row.get("rb_local_deleted") != Some(&integer(1))
            })
            .map(|(_, row)| row)
            .collect();
        let unowned_marker = unowned
            .iter()
            .any(|row| row.get("Kind") == Some(&integer(4)));
        for (index, cue) in cues.iter().enumerate() {
            let (kind, active) = match cue.slot {
                NativeCueSlot::Memory => {
                    if cue.active_loop {
                        active_memory += 1;
                    }
                    (if cue.active_loop { 4 } else { 0 }, false)
                }
                NativeCueSlot::Hot(slot) => {
                    ensure!(
                        slot < 8 && slots.insert(slot),
                        "Invalid or duplicate native hot cue"
                    );
                    let active = cue.active_loop && !first_active_hot;
                    if active {
                        first_active_hot = true;
                    }
                    (
                        if slot < 3 {
                            i64::from(slot) + 1
                        } else {
                            i64::from(slot) + 2
                        },
                        active,
                    )
                }
            };
            ensure!(
                kind == 0
                    || !unowned
                        .iter()
                        .any(|row| row.get("Kind") == Some(&integer(kind))),
                "Reviewed native cue conflicts with an unowned slot or active marker"
            );
            ensure!(
                !active
                    || !unowned
                        .iter()
                        .any(|row| row.get("ActiveLoop") == Some(&integer(1))),
                "Reviewed native active loop conflicts with an unowned loop"
            );
            desired.push(Self::cue_values(
                content,
                content_uuid,
                cue,
                colors.map(|c| c[index]),
                kind,
                active,
                bpm,
            )?);
        }
        ensure!(active_memory <= 1, "Multiple active native memory loops");
        if active_memory == 0
            && !unowned_marker
            && let Some((index, cue)) = cues
                .iter()
                .enumerate()
                .find(|(_, c)| matches!(c.slot, NativeCueSlot::Hot(_)) && c.active_loop)
        {
            desired.push(Self::cue_values(
                content,
                content_uuid,
                cue,
                colors.map(|c| c[index]),
                4,
                false,
                bpm,
            )?);
        }
        let mut ids = vec![];
        let mut changed = false;
        let mut retained = BTreeSet::new();
        for mut patch in desired {
            patch.insert("rb_local_deleted".into(), integer(0));
            // Ownership proofs need not retain UI or database enumeration order.
            // Reuse an unchanged semantic row first, then its native slot.
            let exact = owned
                .iter()
                .filter(|p| !retained.contains(&p.id))
                .find(|proof| {
                    self.current("djmdCue", &proof.id)
                        .is_some_and(|row| patch.iter().all(|(k, v)| row.get(k) == Some(v)))
                });
            let same_slot = || {
                owned
                    .iter()
                    .filter(|p| !retained.contains(&p.id))
                    .find(|proof| {
                        self.current("djmdCue", &proof.id)
                            .is_some_and(|row| row.get("Kind") == patch.get("Kind"))
                    })
            };
            let proof = exact
                .or_else(same_slot)
                .or_else(|| owned.iter().find(|p| !retained.contains(&p.id)));
            let id = if let Some(proof) = proof {
                retained.insert(proof.id.clone());
                proof.id.clone()
            } else {
                self.allocate("djmdCue", "ID", true)?
            };
            if self.current("djmdCue", &id).is_none() {
                let uuid = self.uuid();
                patch.insert("UUID".into(), text(uuid));
            }
            changed |= self.write("djmdCue", &id, patch, 256);
            ids.push(id);
        }
        for proof in owned.iter().filter(|p| !retained.contains(&p.id)) {
            changed |= self.write(
                "djmdCue",
                &proof.id,
                values(&[("rb_local_deleted", integer(1))]),
                258,
            );
        }
        if changed {
            let mut records = vec![];
            for ((table, _), row) in &self.rows {
                if table == "djmdCue"
                    && row.get("ContentID") == Some(&text(content))
                    && row.get("rb_local_deleted") != Some(&integer(1))
                {
                    let mut record = serde_json::Map::new();
                    for (name, value) in row.iter().filter(|(name, _)| {
                        !matches!(
                            name.as_str(),
                            "rb_data_status"
                                | "rb_local_data_status"
                                | "rb_local_deleted"
                                | "rb_local_synced"
                        )
                    }) {
                        let json = if name == "created_at" || name == "updated_at" {
                            let stamp = match value {
                                TargetValue::Text(v) => native_iso_timestamp(v),
                                _ => None,
                            };
                            serde_json::Value::String(
                                stamp.context("Invalid native cue timestamp")?,
                            )
                        } else {
                            target_json(value)?
                        };
                        record.insert(name.clone(), json);
                    }
                    records.push(serde_json::Value::Object(record));
                }
            }
            let existing = self.current("contentCue", content_uuid);
            let mut patch = values(&[
                ("ContentID", text(content)),
                ("Cues", text(serde_json::to_string(&records)?)),
                ("rb_cue_count", integer(records.len() as i64)),
            ]);
            if existing.is_none() {
                let uuid = self.uuid();
                patch.insert("UUID".into(), text(uuid));
            }
            self.write("contentCue", content_uuid, patch, 257);
        }
        Ok((ids, changed))
    }

    fn entities(
        &mut self,
        drafts: &[NativeEntityDraft],
        tracks: &[NativeResolvedTrack],
    ) -> Result<Vec<NativeResolvedEntity>> {
        let mut local = BTreeMap::new();
        let mut used = BTreeSet::new();
        for draft in drafts {
            ensure!(
                !draft.local_id.is_empty() && !local.contains_key(&draft.local_id),
                "Duplicate native entity draft"
            );
            let (table, _, _) = entity_tables(draft.kind);
            let (id, uuid) = if let Some(identity) = &draft.identity {
                let row = self.check(
                    table,
                    &NativeOwnedRow {
                        id: identity.native_id.clone(),
                        expected_raw_hash: identity.expected_raw_hash.clone(),
                    },
                )?;
                ensure!(
                    row.text("UUID")? == identity.uuid,
                    "Native entity identity was reused"
                );
                ensure!(
                    !draft.remove || identity.owned,
                    "Cannot remove an unowned native entity"
                );
                (identity.native_id.clone(), identity.uuid.clone())
            } else {
                ensure!(!draft.remove, "Cannot remove an uncreated native entity");
                (self.allocate(table, "ID", false)?, Some(self.uuid()))
            };
            ensure!(
                used.insert((table, id.clone())),
                "Duplicate native entity identity"
            );
            // Reserve the identity before allocating another new object.
            self.rows
                .entry((table.into(), id.clone()))
                .or_insert_with(|| values(&[("ID", text(&id))]));
            local.insert(draft.local_id.clone(), (draft.kind, id, uuid));
        }
        let removals = drafts
            .iter()
            .filter(|draft| draft.remove)
            .map(|draft| {
                let (table, _, _) = entity_tables(draft.kind);
                let (_, id, _) = local.get(&draft.local_id).expect("reserved entity");
                (table.to_string(), id.clone())
            })
            .collect::<BTreeSet<_>>();
        // Inspect the complete coherent collection, including relationships that
        // have never been imported into the local ownership ledger.
        for draft in drafts.iter().filter(|draft| draft.remove) {
            let (table, link_table, parent_column) = entity_tables(draft.kind);
            let identity = draft.identity.as_ref().expect("validated removal identity");
            let owned = identity
                .owned_links
                .iter()
                .map(|proof| proof.id.as_str())
                .collect::<BTreeSet<_>>();
            for ((row_table, row_id), row) in &self.rows {
                if row.get("rb_local_deleted") == Some(&integer(1)) {
                    continue;
                }
                if row_table == link_table
                    && row.get(parent_column) == Some(&text(&identity.native_id))
                {
                    ensure!(
                        owned.contains(row_id.as_str()),
                        "Cannot remove native entity with a live unowned member link"
                    );
                }
                if row_table == table && row.get("ParentID") == Some(&text(&identity.native_id)) {
                    ensure!(
                        removals.contains(&(row_table.clone(), row_id.clone())),
                        "Cannot remove native entity with a live child absent from reviewed owned removals"
                    );
                }
            }
        }
        let track_ids: BTreeMap<_, _> = tracks
            .iter()
            .map(|t| (&t.local_id, t.identity.content_id.clone()))
            .collect();
        let mut output = vec![];
        for draft in drafts {
            let (table, link_table, parent_column) = entity_tables(draft.kind);
            let (_, id, uuid) = local.get(&draft.local_id).expect("reserved entity").clone();
            let parent = match &draft.parent {
                None => "root".into(),
                Some(NativeEntityRef::Native(id)) => {
                    ensure!(
                        self.current(table, id).is_some(),
                        "Native entity parent is missing"
                    );
                    id.clone()
                }
                Some(NativeEntityRef::Local(local_id)) => {
                    let (kind, parent, _) = local
                        .get(local_id)
                        .context("Local native entity parent missing")?;
                    ensure!(*kind == draft.kind, "Native parent kind differs");
                    ensure!(parent != &id, "Native entity cannot parent itself");
                    parent.clone()
                }
            };
            if !draft.remove {
                ensure!(
                    !removals.contains(&(table.to_string(), parent.clone()))
                        && self
                            .current(table, &parent)
                            .is_none_or(|row| row.get("rb_local_deleted") != Some(&integer(1))),
                    "Native parent is deleted or scheduled for removal"
                );
            }
            let owned = draft
                .identity
                .as_ref()
                .map(|i| i.owned_links.as_slice())
                .unwrap_or_default();
            let mut owned_ids = BTreeSet::new();
            for proof in owned {
                ensure!(owned_ids.insert(&proof.id), "Duplicate owned native link");
                let row = self.check(link_table, proof)?;
                ensure!(
                    row.text(parent_column)?.as_deref() == Some(&id),
                    "Owned native link belongs to another entity"
                );
            }
            if draft.remove {
                self.write(table, &id, values(&[("rb_local_deleted", integer(1))]), 258);
                for proof in owned {
                    self.write(
                        link_table,
                        &proof.id,
                        values(&[("rb_local_deleted", integer(1))]),
                        258,
                    );
                }
                output.push(NativeResolvedEntity {
                    kind: draft.kind,
                    local_id: draft.local_id.clone(),
                    native_id: id,
                    uuid,
                    owned_link_ids: vec![],
                    removed: true,
                });
                continue;
            }
            ensure!(
                !draft.name.contains('\0') && draft.name.len() <= 1_048_576 && draft.sequence >= 0,
                "Invalid native entity metadata"
            );
            let mut patch = values(&[
                ("Name", text(&draft.name)),
                ("ParentID", text(parent)),
                ("Attribute", integer(draft.attribute)),
                ("Seq", integer(draft.sequence)),
                ("rb_local_deleted", integer(0)),
            ]);
            if self.original(table, &id).is_none() {
                patch.insert(
                    "UUID".into(),
                    uuid.clone().map(text).unwrap_or(TargetValue::Null),
                );
                self.rows.remove(&(table.into(), id.clone()));
            }
            if draft.kind == NativeEntityKind::Playlist {
                patch.insert(
                    "SmartList".into(),
                    draft
                        .smart_rule
                        .clone()
                        .map(text)
                        .unwrap_or(TargetValue::Null),
                );
            }
            self.write(
                table,
                &id,
                patch,
                if draft.identity.is_none() { 0 } else { 257 },
            );
            let mut retained = BTreeSet::new();
            let mut new_links = vec![];
            let mut members = BTreeSet::new();
            let mut contents = Vec::with_capacity(draft.members.len());
            for member in &draft.members {
                let content = match member {
                    NativeEntityRef::Native(id) => id.clone(),
                    NativeEntityRef::Local(id) => track_ids
                        .get(id)
                        .context("Local native member track missing")?
                        .clone(),
                };
                ensure!(
                    self.current("djmdContent", &content)
                        .is_some_and(|r| r.get("rb_local_deleted") != Some(&integer(1))),
                    "Native member track is absent or deleted"
                );
                ensure!(
                    members.insert(content.clone()),
                    "Duplicate native entity member"
                );
                contents.push(content);
            }
            let positions = if draft.kind == NativeEntityKind::Playlist {
                self.playlist_positions(&id, &contents, owned)?
            } else {
                vec![]
            };
            for (position, content) in contents.into_iter().enumerate() {
                let existing_owned = owned.iter().find(|proof| {
                    self.current(link_table, &proof.id)
                        .is_some_and(|r| r.get("ContentID") == Some(&text(&content)))
                });
                if self.rows.iter().any(|((t, link_id), r)| {
                    t == link_table
                        && !owned_ids.contains(link_id)
                        && r.get(parent_column) == Some(&text(&id))
                        && r.get("ContentID") == Some(&text(&content))
                        && r.get("rb_local_deleted") != Some(&integer(1))
                }) {
                    continue;
                }
                let link = if let Some(proof) = existing_owned {
                    retained.insert(proof.id.clone());
                    proof.id.clone()
                } else {
                    self.allocate(link_table, "ID", false)?
                };
                let mut patch = values(&[
                    (parent_column, text(&id)),
                    ("ContentID", text(content)),
                    ("rb_local_deleted", integer(0)),
                ]);
                if draft.kind == NativeEntityKind::Playlist {
                    patch.insert("TrackNo".into(), integer(positions[position]));
                }
                if self.current(link_table, &link).is_none() {
                    let uuid = self.uuid();
                    patch.insert("UUID".into(), text(uuid));
                }
                let status = if self.current(link_table, &link).is_none() {
                    0
                } else {
                    257
                };
                self.write(link_table, &link, patch, status);
                new_links.push(link);
            }
            for proof in owned {
                if !retained.contains(&proof.id) {
                    self.write(
                        link_table,
                        &proof.id,
                        values(&[("rb_local_deleted", integer(1))]),
                        258,
                    );
                }
            }
            output.push(NativeResolvedEntity {
                kind: draft.kind,
                local_id: draft.local_id.clone(),
                native_id: id,
                uuid,
                owned_link_ids: new_links,
                removed: false,
            });
        }
        Ok(output)
    }
}

/// Match SQLite's integer-prefix cast for native decimal allocation columns.
fn cast_native_id(value: &str) -> i64 {
    let value = value.trim_start_matches(|c: char| c.is_ascii_whitespace());
    let (negative, digits) = if let Some(value) = value.strip_prefix('-') {
        (true, value)
    } else {
        (false, value.strip_prefix('+').unwrap_or(value))
    };
    let mut magnitude = 0u64;
    for byte in digits.bytes().take_while(u8::is_ascii_digit) {
        magnitude = magnitude
            .saturating_mul(10)
            .saturating_add(u64::from(byte - b'0'));
    }
    if negative {
        if magnitude > i64::MAX as u64 {
            i64::MIN
        } else {
            -(magnitude as i64)
        }
    } else {
        magnitude.min(i64::MAX as u64) as i64
    }
}

fn entity_tables(kind: NativeEntityKind) -> (&'static str, &'static str, &'static str) {
    match kind {
        NativeEntityKind::Playlist => ("djmdPlaylist", "djmdSongPlaylist", "PlaylistID"),
        NativeEntityKind::MyTag => ("djmdMyTag", "djmdSongMyTag", "MyTagID"),
    }
}
fn target_json(value: &TargetValue) -> Result<serde_json::Value> {
    Ok(match value {
        TargetValue::Null => serde_json::Value::Null,
        TargetValue::Text(v) => v.clone().into(),
        TargetValue::Integer(v) => (*v).into(),
        TargetValue::Real(v) => serde_json::Number::from_f64(*v)
            .context("Nonfinite native cache value")?
            .into(),
        TargetValue::Blob(_) => anyhow::bail!("Unsupported native cue binary cache value"),
    })
}
fn native_iso_timestamp(value: &str) -> Option<String> {
    if !value.is_ascii() {
        return None;
    }
    if value.len() >= 20 && value.as_bytes().get(10) == Some(&b'T') {
        return Some(value.into());
    }
    let (date, zone) = value.rsplit_once(' ')?;
    if date.len() != 23 || date.as_bytes().get(10) != Some(&b' ') || zone.len() != 6 {
        return None;
    }
    Some(format!(
        "{}T{}{}",
        &date[..10],
        &date[11..],
        if zone == "+00:00" { "Z" } else { zone }
    ))
}

fn entity_depth(rows: &BTreeMap<(String, String), Values>, table: &str, id: &str) -> Result<usize> {
    let mut seen = BTreeSet::new();
    let mut current = id.to_string();
    let mut depth = 0;
    while let Some(row) = rows.get(&(table.into(), current.clone())) {
        ensure!(
            seen.insert(current.clone()),
            "Native entity hierarchy contains a cycle"
        );
        let Some(TargetValue::Text(parent)) = row.get("ParentID") else {
            break;
        };
        if parent == "root" || parent.is_empty() {
            break;
        }
        current = parent.clone();
        depth += 1;
    }
    Ok(depth)
}

fn mutation_rank(table: &str) -> u8 {
    match table {
        "djmdArtist" => 0,
        "djmdAlbum" | "djmdGenre" | "djmdLabel" | "djmdKey" => 1,
        "djmdContent" => 2,
        "djmdCue" => 3,
        "contentCue" | "contentFile" => 4,
        "djmdPlaylist" | "djmdMyTag" => 5,
        "djmdSongPlaylist" | "djmdSongMyTag" => 6,
        "agentRegistry" => 7,
        _ => 8,
    }
}

fn absolute_path(path: &str) -> bool {
    path.starts_with('/')
        || path.starts_with("\\\\")
        || (path.as_bytes().get(1) == Some(&b':')
            && path
                .as_bytes()
                .get(2)
                .is_some_and(|c| matches!(c, b'/' | b'\\')))
}

fn logical_name(value: &TargetValue) -> Result<&str> {
    let name = match value {
        TargetValue::Null => "",
        TargetValue::Text(name) => name,
        _ => anyhow::bail!("Native name must be text or null"),
    };
    ensure!(
        name.len() <= 1_048_576 && !name.contains('\0'),
        "Invalid native name"
    );
    Ok(name)
}

fn native_artwork_path(value: &TargetValue) -> Result<&str> {
    let path = logical_name(value)?;
    if path.is_empty() {
        return Ok(path);
    }
    ensure!(
        path.len() <= 4096
            && path.starts_with("/Artwork/")
            && !path.contains(['\\', ':', '?', '#', '%'])
            && !path.chars().any(char::is_control),
        "Invalid native artwork resource reference"
    );
    ensure!(
        path[1..]
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != ".."),
        "Native artwork reference is not normalized"
    );
    Ok(path)
}

/// Plan only reviewed fields and proven owned removals. The observed collection
/// and every expected row proof must come from the same coherent snapshot.
pub fn prepare_native_rows(
    collection: &RekordboxCollection,
    request: &NativePlanRequest,
) -> Result<PreparedNativeRows> {
    ensure!(
        request.tracks.len() + request.entities.len() <= 100_000,
        "Native plan budget exceeded"
    );
    ensure!(
        !request.now.is_empty() && !request.iso_now.is_empty(),
        "Missing native timestamps"
    );
    ensure!(
        native_iso_timestamp(&request.now).as_deref() == Some(request.iso_now.as_str()),
        "Native timestamp clocks disagree"
    );
    let count = collection
        .identity
        .local_update_count
        .context("Missing native local update counter")?;
    let usn = i64::try_from(count.checked_add(1).context("Native USN overflow")?)
        .context("Native USN overflow")?;
    let mut planner = Planner {
        collection,
        request,
        rows: collection
            .records
            .iter()
            .flat_map(|(table, rows)| {
                rows.iter()
                    .map(move |r| ((table.clone(), r.id.clone()), r.values.clone()))
            })
            .collect(),
        pending: BTreeMap::new(),
        nonce: 0,
        usn,
    };
    for row in &request.auxiliary_records {
        ensure!(
            row.table == "contentFile",
            "Unsupported auxiliary native table"
        );
        ensure!(
            planner
                .rows
                .insert((row.table.clone(), row.id.clone()), row.values.clone())
                .is_none(),
            "Duplicate auxiliary native record"
        );
    }
    let mut seen = BTreeSet::new();
    let mut native_tracks = BTreeSet::new();
    let mut tracks = vec![];
    for track in &request.tracks {
        ensure!(
            seen.insert(&track.local_id),
            "Duplicate local native track draft"
        );
        let resolved = planner.track(track)?;
        ensure!(
            native_tracks.insert(resolved.identity.content_id.clone()),
            "Multiple local tracks refer to one native identity"
        );
        tracks.push(resolved);
    }
    let entities = planner.entities(&request.entities, &tracks)?;
    ensure!(
        planner.pending.len() <= 99_999,
        "Native row mutation budget exceeded"
    );
    let changed = !planner.pending.is_empty();
    if changed {
        let row = planner
            .original("agentRegistry", "localUpdateCount")
            .context("Native registry counter missing")?;
        ensure!(
            row.unsigned("int_1")? == Some(count),
            "Native registry counter disagrees with collection"
        );
        planner.pending.insert(
            ("agentRegistry".into(), "localUpdateCount".into()),
            NativeRowMutation {
                table: "agentRegistry".into(),
                id: "localUpdateCount".into(),
                expected_fingerprint: Some(row.fingerprint.clone()),
                values: values(&[("int_1", integer(usn))]),
            },
        );
    }
    let mut ordered = Vec::with_capacity(planner.pending.len());
    for row in planner.pending.into_values() {
        let depth = if matches!(row.table.as_str(), "djmdPlaylist" | "djmdMyTag") {
            let depth = entity_depth(&planner.rows, &row.table, &row.id)?;
            if row.values.get("rb_local_deleted") == Some(&integer(1)) {
                usize::MAX - depth
            } else {
                depth
            }
        } else {
            0
        };
        ordered.push((mutation_rank(&row.table), depth, row));
    }
    ordered
        .sort_by(|a, b| (&a.0, &a.1, &a.2.table, &a.2.id).cmp(&(&b.0, &b.1, &b.2.table, &b.2.id)));
    Ok(PreparedNativeRows {
        plan: NativeWritePlan {
            source: request.source.clone(),
            schema_fingerprint: collection.schema_fingerprint.clone(),
            rows: ordered.into_iter().map(|(_, _, row)| row).collect(),
        },
        tracks,
        entities,
        local_usn: changed.then_some(usn as u64),
    })
}
