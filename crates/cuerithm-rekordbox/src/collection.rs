use crate::*;
use std::collections::BTreeMap;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TargetValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetRecord {
    pub table: String,
    pub id: String,
    pub values: BTreeMap<String, TargetValue>,
    pub fingerprint: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseIdentity {
    pub database_id: String,
    pub device_id: Option<String>,
    pub database_version: Option<String>,
    pub local_update_count: Option<u64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackIdentity {
    pub database_id: String,
    pub content_id: String,
    pub uuid: Option<String>,
    pub master_song_id: Option<String>,
    pub local_usn: Option<u64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackMetadata {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub genre: String,
    pub label: String,
    pub key: String,
    pub comment: String,
    pub remixer: String,
    pub composer: String,
    pub lyricist: String,
    pub mix_name: String,
    pub release_year: Option<u32>,
    pub release_date: Option<String>,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
    pub rating: Option<u8>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackSource {
    pub path: Option<String>,
    pub resolved_path: Option<String>,
    pub original_path: Option<String>,
    pub service_id: Option<i64>,
    pub analysis_path: Option<String>,
    pub artwork_path: Option<String>,
    pub file_type: Option<i64>,
    pub file_size: Option<u64>,
    pub sample_rate: Option<u32>,
    pub bitrate: Option<f64>,
    pub bit_depth: Option<u32>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedCue {
    pub id: String,
    pub uuid: Option<String>,
    pub content_uuid: Option<String>,
    pub kind: i64,
    pub slot: Option<u8>,
    pub memory: bool,
    pub start_seconds: Option<f64>,
    pub end_seconds: Option<f64>,
    pub active_loop: bool,
    pub comment: String,
    pub color: Option<i64>,
    pub color_table_index: Option<i64>,
    pub beat_loop_size: Option<i64>,
    pub raw_hash: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedTrack {
    pub identity: TrackIdentity,
    pub metadata: TrackMetadata,
    pub source: TrackSource,
    pub bpm: Option<f64>,
    pub duration_seconds: Option<f64>,
    pub deleted: Option<bool>,
    pub cues: Vec<ImportedCue>,
    pub tag_ids: Vec<String>,
    pub raw_hash: String,
    pub metadata_hash: String,
}
impl ImportedTrack {
    pub fn importable_local(&self) -> bool {
        self.deleted == Some(false)
            && self
                .source
                .resolved_path
                .as_ref()
                .is_some_and(|p| !p.is_empty())
            && self
                .identity
                .master_song_id
                .as_ref()
                .is_some_and(|s| !s.is_empty())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedPlaylist {
    pub id: String,
    pub uuid: Option<String>,
    pub parent_id: Option<String>,
    pub name: String,
    pub attribute: i64,
    pub sequence: i64,
    pub smart_rule: Option<String>,
    pub deleted: Option<bool>,
    pub raw_hash: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistLink {
    pub id: String,
    pub playlist_id: String,
    pub content_id: String,
    pub position: i64,
    pub deleted: Option<bool>,
    pub raw_hash: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedTag {
    pub id: String,
    #[serde(default)]
    pub uuid: Option<String>,
    pub parent_id: Option<String>,
    pub name: String,
    pub attribute: i64,
    pub sequence: i64,
    pub deleted: Option<bool>,
    pub raw_hash: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagLink {
    pub id: String,
    pub tag_id: String,
    pub content_id: String,
    pub deleted: Option<bool>,
    pub raw_hash: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportIssue {
    pub table: String,
    pub row_id: String,
    pub field: String,
    pub code: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RekordboxCollection {
    pub identity: DatabaseIdentity,
    pub schema_fingerprint: String,
    pub tracks: Vec<ImportedTrack>,
    pub playlists: Vec<ImportedPlaylist>,
    pub playlist_links: Vec<PlaylistLink>,
    pub tags: Vec<ImportedTag>,
    pub tag_links: Vec<TagLink>,
    pub records: BTreeMap<String, Vec<TargetRecord>>,
    pub issues: Vec<ImportIssue>,
}
impl RekordboxCollection {
    pub fn active_local_tracks(&self) -> impl Iterator<Item = &ImportedTrack> {
        self.tracks.iter().filter(|t| t.importable_local())
    }
    pub fn playlist_local_track_ids(&self, id: &str) -> Vec<String> {
        let eligible = self
            .active_local_tracks()
            .map(|t| t.identity.content_id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        let mut seen = std::collections::BTreeSet::new();
        self.playlist_links
            .iter()
            .filter(|l| {
                l.playlist_id == id
                    && l.deleted == Some(false)
                    && eligible.contains(l.content_id.as_str())
                    && seen.insert(l.content_id.clone())
            })
            .map(|l| l.content_id.clone())
            .collect()
    }
}
#[derive(Debug, Clone)]
pub struct CollectionReadOptions {
    pub max_rows: usize,
    pub use_original_locations: bool,
    pub dropbox_root: Option<String>,
}
impl Default for CollectionReadOptions {
    fn default() -> Self {
        Self {
            max_rows: 2_000_000,
            use_original_locations: false,
            dropbox_root: None,
        }
    }
}
fn nonempty(value: Option<String>) -> Option<String> {
    value.filter(|s| !s.is_empty())
}
fn issue(issues: &mut Vec<ImportIssue>, row: &TargetRecord, field: &str, code: &str) {
    issues.push(ImportIssue {
        table: row.table.clone(),
        row_id: row.id.clone(),
        field: field.into(),
        code: code.into(),
    });
}
fn resolve(
    index: &BTreeMap<&str, BTreeMap<&str, &TargetRecord>>,
    table: &str,
    id: Option<String>,
    field: &str,
    owner: &TargetRecord,
    issues: &mut Vec<ImportIssue>,
) -> Result<String> {
    let Some(id) = nonempty(id) else {
        return Ok(String::new());
    };
    match index.get(table).and_then(|m| m.get(id.as_str())) {
        Some(row) => Ok(row.text(field)?.unwrap_or_default()),
        None => {
            issue(issues, owner, table, "missing_foreign_record");
            Ok(String::new())
        }
    }
}
fn absolute_local(path: &str) -> bool {
    path.starts_with('/')
        || path.starts_with("\\\\")
        || (path.as_bytes().get(1) == Some(&b':')
            && path
                .as_bytes()
                .get(2)
                .is_some_and(|c| matches!(c, b'/' | b'\\')))
}
fn resolved_path(row: &TargetRecord, options: &CollectionReadOptions) -> Result<Option<String>> {
    let service = row.int("ServiceID")?;
    let original = nonempty(row.text("OrgFolderPath")?);
    let folder = nonempty(row.text("FolderPath")?);
    let mut path = if options.use_original_locations || service == Some(3) {
        original.clone().or(folder)
    } else {
        folder
    };
    match service {
        Some(0) | Some(3) => {}
        Some(2) => {
            if let Some(root) = &options.dropbox_root {
                if let Some(p) = &path {
                    let p = if p.starts_with("/contents_") {
                        format!("/rekordbox{p}")
                    } else {
                        p.clone()
                    };
                    path = Some(format!(
                        "{}{}",
                        root.trim_end_matches(['/', '\\']),
                        if p.starts_with('/') {
                            p
                        } else {
                            format!("/{p}")
                        }
                    ));
                }
            } else {
                path = original;
            }
        }
        _ => return Ok(None),
    }
    Ok(path.filter(|s| absolute_local(s)))
}
impl ReadOnlyDatabase {
    pub fn read_collection(
        &self,
        options: CollectionReadOptions,
        cancel: &CancellationToken,
    ) -> Result<RekordboxCollection> {
        read_collection_on(&self.conn, options, cancel)
    }
}
pub(crate) fn read_collection_on(
    connection: &Connection,
    options: CollectionReadOptions,
    cancel: &CancellationToken,
) -> Result<RekordboxCollection> {
    cancel.check()?;
    ensure!(
        options.max_rows > 0,
        "Collection read budget must be positive"
    );
    let tx = connection.unchecked_transaction()?;
    let schema = crate::schema_on(&tx)?;
    let mut records: BTreeMap<String, Vec<TargetRecord>> = BTreeMap::new();
    let mut count = 0;
    for table in [
        "djmdProperty",
        "agentRegistry",
        "djmdArtist",
        "djmdAlbum",
        "djmdGenre",
        "djmdLabel",
        "djmdKey",
        "djmdContent",
        "djmdCue",
        "contentCue",
        "djmdPlaylist",
        "djmdSongPlaylist",
        "djmdMyTag",
        "djmdSongMyTag",
    ] {
        cancel.check()?;
        let key = match table {
            "djmdProperty" => "DBID",
            "agentRegistry" => "registry_id",
            _ => "ID",
        };
        records.insert(
            table.into(),
            crate::records::read_records(&tx, table, key, &mut count, options.max_rows, cancel)?,
        );
    }
    ensure!(
        records["djmdProperty"].len() == 1,
        "Target database identity is absent or ambiguous"
    );
    let property = &records["djmdProperty"][0];
    let database_id = property
        .text("DBID")?
        .filter(|s| !s.is_empty())
        .context("Missing database identity")?;
    let identity = DatabaseIdentity {
        database_id: database_id.clone(),
        device_id: property.text("DeviceID")?,
        database_version: property.text("DBVersion")?,
        local_update_count: records["agentRegistry"]
            .iter()
            .find(|r| r.id == "localUpdateCount")
            .map(|r| r.unsigned("int_1"))
            .transpose()?
            .flatten(),
    };
    let index = records
        .iter()
        .map(|(table, rows)| {
            (
                table.as_str(),
                rows.iter()
                    .map(|r| (r.id.as_str(), r))
                    .collect::<BTreeMap<_, _>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut issues = vec![];
    let mut tracks = vec![];
    for r in &records["djmdContent"] {
        cancel.check()?;
        let album_id = r.text("AlbumID")?;
        let album_artist_id = album_id
            .as_deref()
            .and_then(|id| index["djmdAlbum"].get(id))
            .map(|r| r.text("AlbumArtistID"))
            .transpose()?
            .flatten();
        let metadata = TrackMetadata {
            title: r.text("Title")?.unwrap_or_default(),
            artist: resolve(
                &index,
                "djmdArtist",
                r.text("ArtistID")?,
                "Name",
                r,
                &mut issues,
            )?,
            album: resolve(&index, "djmdAlbum", album_id, "Name", r, &mut issues)?,
            album_artist: resolve(
                &index,
                "djmdArtist",
                album_artist_id,
                "Name",
                r,
                &mut issues,
            )?,
            genre: resolve(
                &index,
                "djmdGenre",
                r.text("GenreID")?,
                "Name",
                r,
                &mut issues,
            )?,
            label: resolve(
                &index,
                "djmdLabel",
                r.text("LabelID")?,
                "Name",
                r,
                &mut issues,
            )?,
            key: resolve(
                &index,
                "djmdKey",
                r.text("KeyID")?,
                "ScaleName",
                r,
                &mut issues,
            )?,
            comment: r.text("Commnt")?.unwrap_or_default(),
            remixer: resolve(
                &index,
                "djmdArtist",
                r.text("RemixerID")?,
                "Name",
                r,
                &mut issues,
            )?,
            composer: resolve(
                &index,
                "djmdArtist",
                r.text("ComposerID")?,
                "Name",
                r,
                &mut issues,
            )?,
            lyricist: r.text("Lyricist")?.unwrap_or_default(),
            mix_name: r.text("Subtitle")?.unwrap_or_default(),
            release_year: r.u32("ReleaseYear")?.filter(|v| *v > 0),
            release_date: nonempty(r.text("ReleaseDate")?),
            track_number: r.u32("TrackNo")?.filter(|v| *v > 0),
            disc_number: r.u32("DiscNo")?.filter(|v| *v > 0),
            rating: r
                .int("Rating")?
                .filter(|v| (1..=5).contains(v))
                .map(|v| v as u8),
        };
        let source = TrackSource {
            path: nonempty(r.text("FolderPath")?),
            resolved_path: resolved_path(r, &options)?,
            original_path: nonempty(r.text("OrgFolderPath")?),
            service_id: r.int("ServiceID")?,
            analysis_path: nonempty(r.text("AnalysisDataPath")?),
            artwork_path: nonempty(r.text("ImagePath")?),
            file_type: r.int("FileType")?,
            file_size: r.unsigned("FileSize")?,
            sample_rate: r.u32("SampleRate")?.filter(|v| *v > 0),
            bitrate: r.number("BitRate")?.filter(|v| *v > 0.),
            bit_depth: r.u32("BitDepth")?.filter(|v| *v > 0),
        };
        let target = TrackIdentity {
            database_id: database_id.clone(),
            content_id: r.id.clone(),
            uuid: nonempty(r.text("UUID")?),
            master_song_id: nonempty(r.text("MasterSongID")?),
            local_usn: r.unsigned("rb_local_usn")?,
        };
        if target.master_song_id.is_none() {
            issue(
                &mut issues,
                r,
                "MasterSongID",
                "missing_master_song_identity",
            );
        }
        if target.uuid.is_none() {
            issue(&mut issues, r, "UUID", "missing_uuid");
        }
        if source.resolved_path.is_none() {
            issue(&mut issues, r, "FolderPath", "unresolved_local_location");
        }
        let bpm = r.number("BPM")?.filter(|v| *v > 0.).map(|v| v / 100.);
        let metadata_hash = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&(&metadata, bpm))?)
        );
        tracks.push(ImportedTrack {
            identity: target,
            metadata,
            source,
            bpm,
            duration_seconds: r.number("Length")?.filter(|v| *v > 0.),
            deleted: r.flag("rb_local_deleted")?,
            cues: vec![],
            tag_ids: vec![],
            raw_hash: r.fingerprint.clone(),
            metadata_hash,
        });
    }
    let track_index = tracks
        .iter()
        .enumerate()
        .map(|(i, t)| (t.identity.content_id.clone(), i))
        .collect::<BTreeMap<_, _>>();
    for r in &records["djmdCue"] {
        cancel.check()?;
        if r.flag("rb_local_deleted")? != Some(false) {
            continue;
        }
        let id = r
            .text("ContentID")?
            .context("Cue is missing content identity")?;
        let Some(i) = track_index.get(&id) else {
            issue(&mut issues, r, "ContentID", "orphan_cue");
            continue;
        };
        let kind = r.int("Kind")?.context("Cue kind is absent")?;
        let memory = kind == 0 || kind == 4;
        let slot = match kind {
            1..=3 => Some((kind - 1) as u8),
            5..=17 => Some((kind - 2) as u8),
            _ => None,
        };
        if !memory && slot.is_none() {
            issue(&mut issues, r, "Kind", "unsupported_cue_kind");
        }
        let start = r.number("InMsec")?.filter(|v| *v >= 0.).map(|v| v / 1000.);
        let end = r.number("OutMsec")?.filter(|v| *v >= 0.).map(|v| v / 1000.);
        if start.is_none() || end.is_some_and(|e| start.is_some_and(|s| e <= s)) {
            issue(&mut issues, r, "InMsec", "invalid_cue_interval");
        }
        let cue = ImportedCue {
            id: r.id.clone(),
            uuid: r.text("UUID")?,
            content_uuid: r.text("ContentUUID")?,
            kind,
            slot,
            memory,
            start_seconds: start,
            end_seconds: end,
            active_loop: kind == 4
                || (end.is_some() && r.int("ActiveLoop")?.is_some_and(|v| v != 0)),
            comment: r.text("Comment")?.unwrap_or_default(),
            color: r.int("Color")?,
            color_table_index: r.int("ColorTableIndex")?,
            beat_loop_size: r.int("BeatLoopSize")?,
            raw_hash: r.fingerprint.clone(),
        };
        if cue.content_uuid.is_some() && cue.content_uuid != tracks[*i].identity.uuid {
            issue(&mut issues, r, "ContentUUID", "content_uuid_mismatch");
        }
        tracks[*i].cues.push(cue);
    }
    let playlists = records["djmdPlaylist"]
        .iter()
        .map(|r| {
            Ok(ImportedPlaylist {
                id: r.id.clone(),
                uuid: r.text("UUID")?,
                parent_id: r.text("ParentID")?,
                name: r.text("Name")?.unwrap_or_default(),
                attribute: r.int("Attribute")?.context("Playlist attribute missing")?,
                sequence: r.int("Seq")?.unwrap_or(0),
                smart_rule: nonempty(r.text("SmartList")?),
                deleted: r.flag("rb_local_deleted")?,
                raw_hash: r.fingerprint.clone(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let mut playlist_links = records["djmdSongPlaylist"]
        .iter()
        .map(|r| {
            Ok(PlaylistLink {
                id: r.id.clone(),
                playlist_id: r
                    .text("PlaylistID")?
                    .context("Playlist link has no playlist")?,
                content_id: r
                    .text("ContentID")?
                    .context("Playlist link has no content")?,
                position: r.int("TrackNo")?.context("Playlist link has no sequence")?,
                deleted: r.flag("rb_local_deleted")?,
                raw_hash: r.fingerprint.clone(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    playlist_links.sort_by(|a, b| {
        (&a.playlist_id, a.position, &a.id).cmp(&(&b.playlist_id, b.position, &b.id))
    });
    let tags = records["djmdMyTag"]
        .iter()
        .map(|r| {
            Ok(ImportedTag {
                id: r.id.clone(),
                uuid: nonempty(r.text("UUID")?),
                parent_id: r.text("ParentID")?,
                name: r.text("Name")?.unwrap_or_default(),
                attribute: r.int("Attribute")?.context("Tag attribute missing")?,
                sequence: r.int("Seq")?.unwrap_or(0),
                deleted: r.flag("rb_local_deleted")?,
                raw_hash: r.fingerprint.clone(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let tag_links = records["djmdSongMyTag"]
        .iter()
        .map(|r| {
            Ok(TagLink {
                id: r.id.clone(),
                tag_id: r.text("MyTagID")?.context("Tag link has no tag")?,
                content_id: r.text("ContentID")?.context("Tag link has no content")?,
                deleted: r.flag("rb_local_deleted")?,
                raw_hash: r.fingerprint.clone(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let active_tags = tags
        .iter()
        .filter(|t| t.deleted == Some(false))
        .map(|t| t.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    for link in &tag_links {
        if link.deleted == Some(false)
            && active_tags.contains(link.tag_id.as_str())
            && let Some(i) = track_index.get(&link.content_id)
            && !tracks[*i].tag_ids.contains(&link.tag_id)
        {
            tracks[*i].tag_ids.push(link.tag_id.clone());
        }
    }
    cancel.check()?;
    tx.commit()?;
    Ok(RekordboxCollection {
        identity,
        schema_fingerprint: schema.fingerprint,
        tracks,
        playlists,
        playlist_links,
        tags,
        tag_links,
        records,
        issues,
    })
}
