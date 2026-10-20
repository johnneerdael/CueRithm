//! Direct writes to a rekordbox 6/7 `master.db`, reproducing the row-level protocol of the reference
//! writer: one change number (USN) per write, status/USN/timestamps on every touched row, foreign rows
//! found or created by name, and cues replaced as a unit.
//!
//! Only schema revisions listed in [`WRITE_PROFILES`] may be written, and only when every row a write
//! changes still matches what was last observed. Callers write to a copy; publishing a copy over a live
//! library is a separate, explicitly gated step.
use crate::records::row_fingerprint;
use crate::{DatabaseKey, TargetValue, schema_on, set_key};
use anyhow::{Context, Result, ensure};
use cuerithm_media::CancellationToken;
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// A schema revision verified for writes, identified by its object fingerprint.
pub struct WriteProfile {
    pub name: &'static str,
    pub schema_fingerprint: &'static str,
}
/// Schema identities this writer has been built and tested against. A read-only schema match elsewhere
/// does not make a database writable.
pub const WRITE_PROFILES: &[WriteProfile] = &[WriteProfile {
    name: "rekordbox 7.2.19.0342",
    schema_fingerprint: "6bd02ac0a3f9cadfd3ae9b4c95e1ca0663fd3e7717a74e89a4c153834611d2eb",
}];

/// `rb_data_status` values written by the reference writer.
mod status {
    pub const ADDED_NOT_YET_SENT: i64 = 0;
    pub const AVAILABLE: i64 = 256;
    pub const TAGS_CHANGED: i64 = 257;
    pub const DELETED_NOT_YET_SENT: i64 = 258;
}
const MAX_ID: i64 = 4_294_967_295;
const IN_USE: &str =
    "The database is in use. Close rekordbox and its background agent, then try again";

/// Metadata for one existing track. `None` leaves a field alone; an empty string clears it.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ContentFields {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub label: Option<String>,
    pub remixer: Option<String>,
    pub composer: Option<String>,
    /// Musical key name as rekordbox stores it, such as `Am` or `8A`.
    pub key: Option<String>,
    pub comment: Option<String>,
    pub subtitle: Option<String>,
    pub lyricist: Option<String>,
    /// BPM times 100, as rekordbox stores it.
    pub bpm_hundredths: Option<i64>,
    pub release_year: Option<i64>,
    pub release_date: Option<String>,
    pub track_number: Option<i64>,
    pub disc_number: Option<i64>,
    pub rating: Option<i64>,
}
/// A track to add to rekordbox. Its file must already exist at `folder_path`; rekordbox analyses it when it is
/// next opened, and until then holds the grid and cues given here.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewTrack {
    /// Caller's name for the track; playlists in the same plan refer to it as `new:<key>`.
    pub key: String,
    /// Absolute path of the audio file as rekordbox should store it.
    pub folder_path: String,
    pub file_name: String,
    pub fields: ContentFields,
    pub length_seconds: i64,
    pub bit_rate: Option<i64>,
    pub sample_rate: Option<i64>,
    pub file_size: Option<i64>,
    /// rekordbox's file type number: 1 MP3, 2 AAC, 3 MP4, 4 M4A, 5 FLAC, 6 ALAC, 11 WAV, 12 AIFF.
    pub file_type: i64,
    /// `YYYY-MM-DD`.
    pub date_added: String,
    pub color_id: Option<i64>,
    /// First beat of a constant grid in seconds, and the tempo in `fields.bpm_hundredths`.
    pub first_beat_seconds: Option<f64>,
    pub cues: Vec<CueRow>,
    pub tags: Vec<MyTagRef>,
}
/// A file that belongs with the database change, relative to the library's `share` folder.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssetWrite {
    pub relative_path: String,
    pub bytes: Vec<u8>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NewTrackResult {
    pub key: String,
    pub content_id: String,
    pub master_song_id: String,
    pub uuid: String,
    pub analysis_path: String,
}
/// One cue or loop as rekordbox stores it in `djmdCue`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CueRow {
    pub in_msec: i64,
    /// -1 for a plain cue; the loop end for a loop.
    pub out_msec: i64,
    /// 0 memory cue, 4 the load/loop point, 1-3 and 5-9 hot cues A-H.
    pub kind: i64,
    pub active_loop: bool,
    pub beat_loop_size: Option<i64>,
    pub color: Option<i64>,
    pub color_table_index: Option<i64>,
    pub comment: Option<String>,
}
/// A MyTag assignment: a tag inside a category, both found or created by name.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct MyTagRef {
    pub category: String,
    pub tag: String,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PlaylistKind {
    Playlist,
    Folder,
    /// A native intelligent playlist, defined by [`SmartRuleSet`] rather than by tracks.
    Smartlist,
}
/// One condition of a native intelligent playlist, as rekordbox stores it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SmartCondition {
    pub operator: String,
    pub property: String,
    pub left: String,
    pub right: String,
    pub unit: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SmartRuleSet {
    /// Every condition must hold; otherwise any one may.
    pub match_all: bool,
    pub conditions: Vec<SmartCondition>,
}
fn escape(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '&' => "&amp;".to_string(),
            '<' => "&lt;".to_string(),
            '>' => "&gt;".to_string(),
            '"' => "&quot;".to_string(),
            c => c.to_string(),
        })
        .collect()
}
/// The `SmartList` document of an intelligent playlist; its node ID is the playlist's own ID.
pub fn intelligent_playlist_xml(playlist_id: i64, rules: &SmartRuleSet) -> String {
    let mut xml = format!(
        "<NODE Id=\"{}\" AutomaticUpdate=\"0\" LogicalOperator=\"{}\">\n",
        playlist_id as i32,
        if rules.match_all { 1 } else { 2 }
    );
    for c in &rules.conditions {
        xml.push_str(&format!(
            "  <CONDITION Operator=\"{}\" PropertyName=\"{}\" ValueLeft=\"{}\" ValueRight=\"{}\" ValueUnit=\"{}\"/>\n",
            escape(&c.operator),
            escape(&c.property),
            escape(&c.left),
            escape(&c.right),
            escape(&c.unit)
        ));
    }
    xml.push_str("</NODE>");
    xml
}
/// One playlist or folder, addressed by its name path from the root. Only the playlists named in a plan
/// are touched; nothing else in the library is ever removed to "match".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistWrite {
    /// Names from the root down to this playlist, such as `["Sets", "Friday"]`.
    pub path: Vec<String>,
    pub kind: PlaylistKind,
    /// Position among its siblings.
    pub position: i64,
    /// Ordered `djmdContent.ID`s; empty for a folder. Every track must exist and not be removed.
    pub tracks: Vec<String>,
    /// Rules for a [`PlaylistKind::Smartlist`]; `None` for every other kind.
    #[serde(default)]
    pub rules: Option<SmartRuleSet>,
    /// The membership fingerprint from the last observation of an existing playlist ([`membership_fingerprint`]).
    /// A different current membership means rekordbox changed it since, and the whole plan is refused.
    pub expected_members: Option<String>,
}
/// Fingerprint of an ordered track list, the value to record when a playlist is read.
pub fn membership_fingerprint(tracks: &[String]) -> String {
    use sha2::{Digest, Sha256};
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(tracks).unwrap_or_default())
    )
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistRemoval {
    /// `djmdPlaylist.ID`; the playlist and its links are soft-deleted the way rekordbox records a removal.
    pub playlist_id: String,
    pub expected_members: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackWrite {
    /// `djmdContent.ID` of the existing track.
    pub content_id: String,
    /// The row fingerprint from the last observation. A different row means rekordbox changed it since,
    /// and the whole write is refused.
    pub expected_fingerprint: String,
    pub fields: ContentFields,
    /// Replace every cue of the track when set and different from what is stored.
    pub cues: Option<Vec<CueRow>>,
    /// Replace the track's MyTag assignments when set and different from what is stored.
    pub tags: Option<Vec<MyTagRef>>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WritePlan {
    pub tracks: Vec<TrackWrite>,
    /// Tracks to add. A plan that adds none leaves `new_tracks` empty.
    #[serde(default)]
    pub new_tracks: Vec<NewTrack>,
    /// Absolute path of the library's `share` folder, recorded with each new analysis file.
    #[serde(default)]
    pub share_dir: Option<String>,
    pub playlists: Vec<PlaylistWrite>,
    pub remove_playlists: Vec<PlaylistRemoval>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct WriteReport {
    /// The change number used for every touched row; `None` when nothing needed writing.
    pub usn: Option<i64>,
    pub tracks_updated: usize,
    pub tracks_unchanged: usize,
    pub cues_replaced: usize,
    pub tags_replaced: usize,
    pub tracks_added: usize,
    /// Identifiers rekordbox now has for each added track.
    pub new_tracks: Vec<NewTrackResult>,
    /// Files to publish with the database change.
    pub assets: Vec<AssetWrite>,
    pub playlists_created: usize,
    pub playlists_updated: usize,
    pub playlists_unchanged: usize,
    pub playlists_removed: usize,
    pub foreign_created: BTreeMap<String, usize>,
    pub foreign_revived: BTreeMap<String, usize>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrackConflict {
    /// The track's content ID, or `playlist <path or id>` for a playlist.
    pub subject: String,
    pub reason: String,
}
/// Returned, with no change made, when any row differs from its last observation.
#[derive(Debug, Clone)]
pub struct ConflictError(pub Vec<TrackConflict>);
impl std::fmt::Display for ConflictError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} track(s) changed in rekordbox since they were last read; nothing was written",
            self.0.len()
        )
    }
}
impl std::error::Error for ConflictError {}

pub struct WriteDatabase {
    conn: Connection,
    profile: &'static WriteProfile,
}

impl WriteDatabase {
    /// Open an encrypted `master.db` copy for writing. Fails fast when another process holds it, and for
    /// any schema that is not a verified write profile.
    pub fn open(path: &Path, key: &DatabaseKey) -> Result<Self> {
        Self::open_with_profiles(path, key, WRITE_PROFILES)
    }
    pub(crate) fn open_with_profiles(
        path: &Path,
        key: &DatabaseKey,
        profiles: &'static [WriteProfile],
    ) -> Result<Self> {
        let meta = std::fs::symlink_metadata(path).context("Database is unavailable")?;
        ensure!(
            meta.is_file() && !meta.file_type().is_symlink(),
            "Choose a regular database file"
        );
        let conn = Connection::open_with_flags(
            path.canonicalize().context("Resolve database path")?,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .context("Open database for writing")?;
        conn.busy_timeout(std::time::Duration::ZERO)?;
        set_key(&conn, key)?;
        conn.query_row("SELECT COUNT(*) FROM sqlite_schema", [], |r| {
            r.get::<_, i64>(0)
        })
        .map_err(|error| {
            if matches!(
                error.sqlite_error_code(),
                Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked)
            ) {
                anyhow::anyhow!(IN_USE)
            } else {
                anyhow::anyhow!("Encrypted database authentication or format check failed")
            }
        })?;
        let schema = schema_on(&conn)?;
        let profile = profiles
            .iter()
            .find(|p| p.schema_fingerprint == schema.fingerprint)
            .with_context(|| {
                format!(
                    "This rekordbox database schema ({}) is not verified for writes",
                    &schema.fingerprint[..12]
                )
            })?;
        // Exclusive locking makes a database that rekordbox or its agent has open fail here, not midway.
        conn.pragma_update(None, "locking_mode", "EXCLUSIVE")?;
        // Exclusive mode is lazy; taking and releasing a write lock now makes it hold from here on.
        conn.execute_batch("BEGIN IMMEDIATE; COMMIT;")
            .map_err(|_| anyhow::anyhow!(IN_USE))?;
        Ok(Self { conn, profile })
    }
    pub(crate) fn connection(&self) -> &Connection {
        &self.conn
    }
    pub(crate) fn connection_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }
    pub fn profile(&self) -> &'static str {
        self.profile.name
    }

    /// Check every track against its last observation, then apply the whole plan in one transaction.
    pub fn apply(&mut self, plan: &WritePlan, cancel: &CancellationToken) -> Result<WriteReport> {
        cancel.check()?;
        ensure!(
            plan.tracks.len() <= 100_000
                && plan
                    .tracks
                    .iter()
                    .map(|t| t.content_id.as_str())
                    .collect::<BTreeSet<_>>()
                    .len()
                    == plan.tracks.len(),
            "A write plan needs distinct tracks"
        );
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut conflicts = vec![];
        for track in &plan.tracks {
            cancel.check()?;
            match row_fingerprint(&tx, "djmdContent", "ID", &track.content_id)? {
                None => conflicts.push(TrackConflict {
                    subject: track.content_id.clone(),
                    reason: "The track no longer exists in rekordbox".into(),
                }),
                Some((fingerprint, values)) => {
                    if matches!(
                        values.get("rb_local_deleted"),
                        Some(TargetValue::Integer(1))
                    ) {
                        conflicts.push(TrackConflict {
                            subject: track.content_id.clone(),
                            reason: "The track was removed in rekordbox".into(),
                        });
                    } else if fingerprint != track.expected_fingerprint {
                        conflicts.push(TrackConflict {
                            subject: track.content_id.clone(),
                            reason: "The track changed in rekordbox after it was last read".into(),
                        });
                    }
                }
            }
        }
        let new_keys: BTreeSet<&str> = plan.new_tracks.iter().map(|t| t.key.as_str()).collect();
        ensure!(
            new_keys.len() == plan.new_tracks.len() && plan.new_tracks.len() <= 100_000,
            "New tracks need distinct keys"
        );
        for new in &plan.new_tracks {
            cancel.check()?;
            ensure!(
                !new.key.is_empty()
                    && !new.key.starts_with("new:")
                    && !new.file_name.trim().is_empty()
                    && (new.folder_path.starts_with('/')
                        || new.folder_path.chars().nth(1) == Some(':'))
                    && new.length_seconds >= 0
                    && new.file_type >= 0,
                "A new track needs a key, an absolute file path and a name"
            );
            let existing: Option<String> = tx
                .query_row(
                    "SELECT ID FROM djmdContent WHERE FolderPath=?1 AND COALESCE(rb_local_deleted,0)=0 LIMIT 1",
                    [&new.folder_path],
                    |r| r.get(0),
                )
                .optional()?;
            if existing.is_some() {
                conflicts.push(TrackConflict {
                    subject: format!("new track {}", new.key),
                    reason: "A track with this file is already in rekordbox".into(),
                });
            }
        }
        for playlist in &plan.playlists {
            cancel.check()?;
            ensure!(
                !playlist.path.is_empty()
                    && playlist
                        .path
                        .iter()
                        .all(|n| !n.trim().is_empty() && n.len() <= 255),
                "A playlist needs a name"
            );
            ensure!(
                playlist.kind == PlaylistKind::Playlist || playlist.tracks.is_empty(),
                "Only an ordinary playlist can hold tracks"
            );
            ensure!(
                (playlist.kind == PlaylistKind::Smartlist) == playlist.rules.is_some()
                    && playlist
                        .rules
                        .as_ref()
                        .is_none_or(|r| !r.conditions.is_empty() && r.conditions.len() <= 64),
                "A smart list needs between one and 64 rules, and no other playlist may have any"
            );
            let subject = format!("playlist {}", playlist.path.join("/"));
            let distinct = playlist.tracks.iter().collect::<BTreeSet<_>>().len();
            ensure!(
                distinct == playlist.tracks.len(),
                "{subject} lists a track twice"
            );
            for track in &playlist.tracks {
                if let Some(key) = track.strip_prefix("new:") {
                    if !new_keys.contains(key) {
                        conflicts.push(TrackConflict {
                            subject: subject.clone(),
                            reason: format!("Track {track} is not in the plan"),
                        });
                    }
                    continue;
                }
                let live: Option<i64> = tx
                    .query_row(
                        "SELECT COALESCE(rb_local_deleted,0) FROM djmdContent WHERE ID=?1",
                        [track],
                        |r| r.get(0),
                    )
                    .optional()?;
                if live != Some(0) {
                    conflicts.push(TrackConflict {
                        subject: subject.clone(),
                        reason: format!("Track {track} is missing or removed in rekordbox"),
                    });
                }
            }
            if let (Some(expected), Some(id)) = (
                &playlist.expected_members,
                find_playlist(&tx, &playlist.path, playlist.kind)?,
            ) && &membership_fingerprint(&members(&tx, &id)?) != expected
            {
                conflicts.push(TrackConflict {
                    subject,
                    reason: "The playlist changed in rekordbox after it was last read".into(),
                });
            }
        }
        for removal in &plan.remove_playlists {
            cancel.check()?;
            let subject = format!("playlist {}", removal.playlist_id);
            let row: Option<i64> = tx
                .query_row(
                    "SELECT COALESCE(rb_local_deleted,0) FROM djmdPlaylist WHERE ID=?1",
                    [&removal.playlist_id],
                    |r| r.get(0),
                )
                .optional()?;
            match row {
                Some(0) => {
                    if let Some(expected) = &removal.expected_members
                        && &membership_fingerprint(&members(&tx, &removal.playlist_id)?) != expected
                    {
                        conflicts.push(TrackConflict {
                            subject,
                            reason: "The playlist changed in rekordbox after it was last read"
                                .into(),
                        });
                    }
                }
                _ => conflicts.push(TrackConflict {
                    subject,
                    reason: "The playlist no longer exists in rekordbox".into(),
                }),
            }
        }
        if !conflicts.is_empty() {
            return Err(ConflictError(conflicts).into());
        }
        let mut session = Session::new(&tx)?;
        let mut report = WriteReport::default();
        for track in &plan.tracks {
            cancel.check()?;
            let changed = session.write_track(track, &mut report)?;
            if changed {
                report.tracks_updated += 1;
            } else {
                report.tracks_unchanged += 1;
            }
        }
        let mut added: BTreeMap<String, String> = BTreeMap::new();
        for new in &plan.new_tracks {
            cancel.check()?;
            let result = session.insert_track(new, plan.share_dir.as_deref(), &mut report)?;
            added.insert(new.key.clone(), result.content_id.clone());
            report.new_tracks.push(result);
            report.tracks_added += 1;
        }
        let mut playlists: Vec<PlaylistWrite> = plan
            .playlists
            .iter()
            .map(|p| PlaylistWrite {
                tracks: p
                    .tracks
                    .iter()
                    .map(|t| {
                        t.strip_prefix("new:")
                            .and_then(|key| added.get(key).cloned())
                            .unwrap_or_else(|| t.clone())
                    })
                    .collect(),
                ..p.clone()
            })
            .collect();
        // Parents before children, so a new folder exists when its contents are written.
        playlists.sort_by_key(|p| p.path.len());
        for playlist in &playlists {
            cancel.check()?;
            session.write_playlist(playlist, &mut report)?;
        }
        for removal in &plan.remove_playlists {
            cancel.check()?;
            session.remove_playlist(&removal.playlist_id)?;
            report.playlists_removed += 1;
        }
        if session.used_usn {
            session.save_usn()?;
            report.usn = Some(session.usn);
        }
        drop(session);
        cancel.check()?;
        tx.commit()?;
        Ok(report)
    }
}

struct Session<'a> {
    tx: &'a rusqlite::Transaction<'a>,
    usn: i64,
    used_usn: bool,
    now: String,
    now_iso: String,
    next: BTreeMap<(&'static str, &'static str), i64>,
    cue_ids: Option<BTreeSet<i64>>,
    created_folders: BTreeMap<Vec<String>, String>,
}

/// `YYYY-MM-DD HH:mm:ss.SSS +00:00`, the reference writer's row timestamp (UTC).
pub(crate) fn timestamp(now: std::time::SystemTime) -> (String, String) {
    let since = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let (secs, millis) = (since.as_secs() as i64, since.subsec_millis());
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Civil-from-days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    let (h, mi, s) = (rem / 3600, rem % 3600 / 60, rem % 60);
    (
        format!("{y:04}-{m:02}-{d:02} {h:02}:{mi:02}:{s:02}.{millis:03} +00:00"),
        format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}.{millis:03}Z"),
    )
}

fn text(values: &BTreeMap<String, TargetValue>, name: &str) -> Option<String> {
    match values.get(name) {
        Some(TargetValue::Text(v)) => Some(v.clone()),
        Some(TargetValue::Integer(v)) => Some(v.to_string()),
        _ => None,
    }
}
impl<'a> Session<'a> {
    fn new(tx: &'a rusqlite::Transaction<'a>) -> Result<Self> {
        let current: Option<i64> = tx
            .query_row(
                "SELECT int_1 FROM agentRegistry WHERE registry_id='localUpdateCount'",
                [],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        let (now, now_iso) = timestamp(std::time::SystemTime::now());
        Ok(Self {
            tx,
            // One change number for the whole write, the next after the last one the database recorded.
            usn: current.context("The database has no local update counter")? + 1,
            used_usn: false,
            now,
            now_iso,
            next: BTreeMap::new(),
            cue_ids: None,
            created_folders: BTreeMap::new(),
        })
    }
    fn use_usn(&mut self) -> i64 {
        self.used_usn = true;
        self.usn
    }
    fn save_usn(&self) -> Result<()> {
        ensure!(
            self.tx.execute(
                "UPDATE agentRegistry SET int_1=?1 WHERE registry_id='localUpdateCount'",
                [self.usn],
            )? == 1,
            "The local update counter disappeared"
        );
        Ok(())
    }
    /// The next unused numeric ID, the way the reference writer allocates (highest plus one).
    fn allocate(&mut self, table: &'static str, column: &'static str) -> Result<i64> {
        if !self.next.contains_key(&(table, column)) {
            let max: Option<i64> = self.tx.query_row(
                &format!("SELECT MAX(CAST({column} AS INTEGER)) FROM {table}"),
                [],
                |r| r.get(0),
            )?;
            self.next.insert((table, column), max.unwrap_or(0));
        }
        let slot = self.next.get_mut(&(table, column)).unwrap();
        ensure!(
            *slot < MAX_ID,
            "{table}.{column} has no identifiers left that rekordbox can read back"
        );
        *slot += 1;
        Ok(*slot)
    }
    fn random_cue_id(&mut self) -> Result<i64> {
        if self.cue_ids.is_none() {
            let mut q = self.tx.prepare("SELECT ID FROM djmdCue")?;
            let ids = q
                .query_map([], |r| r.get::<_, String>(0))?
                .filter_map(|r| r.ok()?.parse::<i64>().ok())
                .collect();
            self.cue_ids = Some(ids);
        }
        let ids = self.cue_ids.as_mut().unwrap();
        loop {
            let bytes = *uuid::Uuid::new_v4().as_bytes();
            let value = 1_000_000_000
                + i64::from(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
                    % (MAX_ID - 1_000_000_000);
            if ids.insert(value) {
                return Ok(value);
            }
        }
    }

    /// Find a foreign row by name, reviving a soft-deleted one, or create it. `None` for an empty name.
    fn foreign(
        &mut self,
        table: &'static str,
        name_column: &'static str,
        name: &str,
        report: &mut WriteReport,
    ) -> Result<Option<String>> {
        if name.is_empty() {
            return Ok(None);
        }
        let found: Option<(String, i64)> = self
            .tx
            .query_row(
                &format!(
                    "SELECT ID,COALESCE(rb_local_deleted,0) FROM {table} WHERE {name_column}=?1 ORDER BY COALESCE(rb_local_deleted,0),CAST(ID AS INTEGER) LIMIT 1"
                ),
                [name],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((id, deleted)) = found {
            if deleted == 1 {
                let usn = self.use_usn();
                self.tx.execute(
                    &format!(
                        "UPDATE {table} SET rb_local_deleted=0,rb_local_usn=?1,rb_data_status=?2,updated_at=?3 WHERE ID=?4"
                    ),
                    params![usn, status::TAGS_CHANGED, self.now, id],
                )?;
                *report.foreign_revived.entry(table.into()).or_default() += 1;
            }
            return Ok(Some(id));
        }
        let id = self.allocate(table, "ID")?.to_string();
        let usn = self.use_usn();
        self.tx.execute(
            &format!(
                "INSERT INTO {table}(ID,{name_column},UUID,rb_data_status,rb_local_deleted,rb_local_synced,rb_local_data_status,rb_local_usn,created_at,updated_at) VALUES(?1,?2,?3,?4,0,0,0,?5,?6,?6)"
            ),
            params![
                id,
                name,
                uuid::Uuid::new_v4().to_string(),
                status::ADDED_NOT_YET_SENT,
                usn,
                self.now
            ],
        )?;
        *report.foreign_created.entry(table.into()).or_default() += 1;
        Ok(Some(id))
    }

    fn write_track(&mut self, track: &TrackWrite, report: &mut WriteReport) -> Result<bool> {
        let (_, current) = row_fingerprint(self.tx, "djmdContent", "ID", &track.content_id)?
            .context("The track disappeared during the write")?;
        let f = &track.fields;
        // Desired column values, as (column, value) in the order they will be written.
        let mut desired: Vec<(&'static str, rusqlite::types::Value)> = vec![];
        let mut set_text = |column: &'static str, value: &Option<String>| {
            if let Some(v) = value {
                desired.push((
                    column,
                    if v.is_empty() {
                        rusqlite::types::Value::Null
                    } else {
                        rusqlite::types::Value::Text(v.clone())
                    },
                ));
            }
        };
        set_text("Title", &f.title);
        set_text("Commnt", &f.comment);
        set_text("Subtitle", &f.subtitle);
        set_text("Lyricist", &f.lyricist);
        set_text("ReleaseDate", &f.release_date);
        let mut set_int = |column: &'static str, value: Option<i64>| {
            if let Some(v) = value {
                desired.push((column, rusqlite::types::Value::Integer(v)));
            }
        };
        set_int("BPM", f.bpm_hundredths);
        set_int("ReleaseYear", f.release_year);
        set_int("TrackNo", f.track_number);
        set_int("DiscNo", f.disc_number);
        set_int("Rating", f.rating);
        for (column, table, name_column, value) in [
            ("ArtistID", "djmdArtist", "Name", &f.artist),
            ("AlbumID", "djmdAlbum", "Name", &f.album),
            ("GenreID", "djmdGenre", "Name", &f.genre),
            ("LabelID", "djmdLabel", "Name", &f.label),
            ("RemixerID", "djmdArtist", "Name", &f.remixer),
            ("ComposerID", "djmdArtist", "Name", &f.composer),
            ("KeyID", "djmdKey", "ScaleName", &f.key),
        ] {
            let Some(name) = value else { continue };
            // Only look a name up when the row does not already carry it, so a no-op never creates rows.
            let existing_id = text(&current, column);
            let same = existing_id.as_ref().is_some_and(|id| {
                self.tx
                    .query_row(
                        &format!("SELECT {name_column} FROM {table} WHERE ID=?1"),
                        [id],
                        |r| r.get::<_, Option<String>>(0),
                    )
                    .optional()
                    .ok()
                    .flatten()
                    .flatten()
                    .as_deref()
                    == Some(name.as_str())
            }) || (name.is_empty() && existing_id.is_none());
            if same {
                continue;
            }
            let id = self.foreign(table, name_column, name, report)?;
            desired.push((
                column,
                id.map_or(rusqlite::types::Value::Null, rusqlite::types::Value::Text),
            ));
        }
        let changed: Vec<_> = desired
            .into_iter()
            .filter(|(column, value)| !same_value(current.get(*column), value))
            .collect();
        let mut touched = false;
        let mut bumps = (false, false);
        if !changed.is_empty() {
            let usn = self.use_usn();
            let assignments = changed
                .iter()
                .enumerate()
                .map(|(i, (column, _))| format!("{column}=?{}", i + 1))
                .collect::<Vec<_>>()
                .join(",");
            let n = changed.len();
            let mut values: Vec<rusqlite::types::Value> =
                changed.into_iter().map(|(_, v)| v).collect();
            values.push(rusqlite::types::Value::Text(self.now.clone()));
            values.push(rusqlite::types::Value::Integer(status::TAGS_CHANGED));
            values.push(rusqlite::types::Value::Integer(usn));
            values.push(rusqlite::types::Value::Text(track.content_id.clone()));
            self.tx.execute(
                &format!(
                    "UPDATE djmdContent SET {assignments},updated_at=?{},rb_data_status=?{},rb_local_usn=?{} WHERE ID=?{}",
                    n + 1,
                    n + 2,
                    n + 3,
                    n + 4
                ),
                rusqlite::params_from_iter(values),
            )?;
            touched = true;
            bumps.1 = true;
        }
        if let Some(cues) = &track.cues
            && self.replace_cues(&track.content_id, text(&current, "UUID"), cues)?
        {
            report.cues_replaced += 1;
            touched = true;
            bumps.0 = true;
        }
        if let Some(tags) = &track.tags
            && self.replace_tags(&track.content_id, tags)?
        {
            report.tags_replaced += 1;
            touched = true;
        }
        if bumps.0 || bumps.1 {
            // Counters other tools watch to notice that cues or track info changed.
            self.tx.execute(
                "UPDATE djmdContent SET CueUpdated=CASE WHEN ?1 THEN COALESCE(CueUpdated,0)+1 ELSE CueUpdated END,TrackInfoUpdated=CASE WHEN ?2 THEN COALESCE(TrackInfoUpdated,0)+1 ELSE TrackInfoUpdated END WHERE ID=?3",
                params![bumps.0, bumps.1, track.content_id],
            )?;
            if bumps.0 && !bumps.1 {
                let usn = self.use_usn();
                self.tx.execute(
                    "UPDATE djmdContent SET updated_at=?1,rb_data_status=?2,rb_local_usn=?3 WHERE ID=?4",
                    params![self.now, status::TAGS_CHANGED, usn, track.content_id],
                )?;
            }
        }
        Ok(touched)
    }

    /// Add a track the way the reference exporter does: a content row with ID and master-song ID after the
    /// highest in use, foreign rows by name, an analysis path derived from its UUID, its cues and tags, and a
    /// file record. The analysis file itself is returned as an asset for the caller to publish.
    fn insert_track(
        &mut self,
        track: &NewTrack,
        share_dir: Option<&str>,
        report: &mut WriteReport,
    ) -> Result<NewTrackResult> {
        let (db_id, device_id): (Option<String>, Option<String>) =
            self.tx
                .query_row("SELECT DBID,DeviceID FROM djmdProperty LIMIT 1", [], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })?;
        let f = &track.fields;
        let id = self.allocate("djmdContent", "ID")?.to_string();
        let master = self.allocate("djmdContent", "MasterSongID")?.to_string();
        let uuid = uuid::Uuid::new_v4().to_string();
        let analysis_path = format!(
            "/PIONEER/USBANLZ/{}/{}/ANLZ0000.DAT",
            &uuid[..3],
            &uuid[3..]
        );
        let mut foreign = |table: &'static str,
                           column: &'static str,
                           name: &Option<String>|
         -> Result<Option<String>> {
            match name {
                Some(name) => self.foreign(table, column, name, report),
                None => Ok(None),
            }
        };
        let artist = foreign("djmdArtist", "Name", &f.artist)?;
        let album = foreign("djmdAlbum", "Name", &f.album)?;
        let genre = foreign("djmdGenre", "Name", &f.genre)?;
        let label = foreign("djmdLabel", "Name", &f.label)?;
        let remixer = foreign("djmdArtist", "Name", &f.remixer)?;
        let composer = foreign("djmdArtist", "Name", &f.composer)?;
        let key = foreign("djmdKey", "ScaleName", &f.key)?;
        let usn = self.use_usn();
        let non_empty = |v: &Option<String>| v.clone().filter(|t| !t.is_empty());
        self.tx.execute(
            "INSERT INTO djmdContent(ID,MasterSongID,MasterDBID,DeviceID,UUID,FolderPath,OrgFolderPath,FileNameL,Title,ArtistID,AlbumID,GenreID,LabelID,RemixerID,ComposerID,KeyID,BPM,Length,TrackNo,DiscNo,BitRate,BitDepth,SampleRate,FileSize,FileType,Commnt,Subtitle,Lyricist,Rating,ReleaseYear,ReleaseDate,StockDate,DateCreated,ColorID,ImagePath,ContentLink,HotCueAutoLoad,SamplerTrackInfo,SamplerPlayOffset,SamplerGain,Analysed,CueUpdated,AnalysisUpdated,TrackInfoUpdated,AnalysisDataPath,VideoAssociate,LyricStatus,ServiceID,ExtInfo,rb_file_id,rb_data_status,rb_local_deleted,rb_local_data_status,rb_local_synced,rb_local_usn,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,0,?21,?22,?23,?24,?25,?26,?27,?28,?29,?30,?30,?31,'',0,'on',0,0,0,16,0,0,0,?32,'0',0,0,'null',0,?33,0,0,0,?34,?35,?35)",
            params![
                id,
                master,
                db_id,
                device_id,
                uuid,
                track.folder_path,
                track.file_name,
                non_empty(&f.title),
                artist,
                album,
                genre,
                label,
                remixer,
                composer,
                key,
                f.bpm_hundredths.unwrap_or(0).max(0),
                track.length_seconds,
                f.track_number,
                f.disc_number.unwrap_or(0),
                track.bit_rate,
                track.sample_rate.unwrap_or(0),
                track.file_size,
                track.file_type,
                non_empty(&f.comment),
                non_empty(&f.subtitle),
                non_empty(&f.lyricist),
                f.rating.unwrap_or(0),
                f.release_year.unwrap_or(0),
                non_empty(&f.release_date),
                track.date_added,
                track.color_id.map(|c| c.to_string()),
                analysis_path,
                status::ADDED_NOT_YET_SENT,
                usn,
                self.now,
            ],
        )?;
        self.tx.execute(
            "INSERT INTO contentFile(ID,ContentID,Path,Hash,Size,rb_local_path,rb_file_hash_dirty,rb_local_file_status,rb_in_progress,rb_process_type,rb_priority,rb_file_size_dirty,UUID,rb_data_status,rb_local_data_status,rb_local_deleted,rb_local_synced,rb_local_usn,created_at,updated_at) VALUES(?1,?2,?3,NULL,NULL,?4,0,0,0,0,0,0,?5,?6,0,0,0,?7,?8,?8)",
            params![
                format!("{uuid}_{}", encode_uri_component(&analysis_path)),
                id,
                analysis_path,
                share_dir.map(|dir| format!("{}{analysis_path}", dir.trim_end_matches(['/', '\\']))),
                uuid::Uuid::new_v4().to_string(),
                status::ADDED_NOT_YET_SENT,
                usn,
                self.now,
            ],
        )?;
        if !track.cues.is_empty() {
            self.replace_cues(&id, Some(uuid.clone()), &track.cues)?;
        }
        if !track.tags.is_empty() {
            self.replace_tags(&id, &track.tags)?;
        }
        let beats = match (f.bpm_hundredths, track.first_beat_seconds) {
            (Some(bpm), Some(first)) if bpm > 0 => {
                crate::anlz::beat_grid(bpm as f64 / 100., first, track.length_seconds as f64)
            }
            _ => vec![],
        };
        report.assets.push(AssetWrite {
            relative_path: analysis_path.trim_start_matches('/').to_owned(),
            bytes: crate::anlz::placeholder_analysis(&track.file_name, &beats)?,
        });
        Ok(NewTrackResult {
            key: track.key.clone(),
            content_id: id,
            master_song_id: master,
            uuid,
            analysis_path,
        })
    }

    /// Replace a track's cues when they differ. Returns whether anything was written.
    fn replace_cues(
        &mut self,
        content_id: &str,
        content_uuid: Option<String>,
        cues: &[CueRow],
    ) -> Result<bool> {
        let content_uuid = content_uuid.context("The track has no UUID")?;
        let stored = {
            let mut q = self.tx.prepare(
                "SELECT InMsec,OutMsec,Kind,ActiveLoop,BeatLoopSize,Color,ColorTableIndex,Comment FROM djmdCue WHERE ContentID=?1 AND COALESCE(rb_local_deleted,0)=0",
            )?;
            let mut rows = q
                .query_map([content_id], |r| {
                    Ok(CueRow {
                        in_msec: r.get(0)?,
                        out_msec: r.get(1)?,
                        kind: r.get(2)?,
                        active_loop: r.get::<_, Option<i64>>(3)?.unwrap_or(0) != 0,
                        beat_loop_size: r.get(4)?,
                        color: r.get(5)?,
                        color_table_index: r.get(6)?,
                        comment: r.get(7)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows.sort_by_key(cue_key);
            rows
        };
        let mut wanted = cues.to_vec();
        wanted.sort_by_key(cue_key);
        if stored == wanted {
            return Ok(false);
        }
        self.tx
            .execute("DELETE FROM djmdCue WHERE ContentID=?1", [content_id])?;
        self.tx
            .execute("DELETE FROM contentCue WHERE ContentID=?1", [content_id])?;
        let mut json_cues = vec![];
        for cue in cues {
            ensure!(
                cue.in_msec >= 0 && (cue.out_msec == -1 || cue.out_msec >= cue.in_msec),
                "A cue has an invalid time range"
            );
            let id = self.random_cue_id()?.to_string();
            let uuid = uuid::Uuid::new_v4().to_string();
            self.tx.execute(
                "INSERT INTO djmdCue(ID,ContentID,InMsec,InFrame,InMpegFrame,InMpegAbs,OutMsec,OutFrame,OutMpegFrame,OutMpegAbs,Kind,Color,ColorTableIndex,ActiveLoop,Comment,BeatLoopSize,ContentUUID,UUID,rb_data_status,rb_local_data_status,rb_local_deleted,rb_local_synced,created_at,updated_at) VALUES(?1,?2,?3,0,0,0,?4,0,0,0,?5,?6,?7,?8,?9,?10,?11,?12,?13,0,0,0,?14,?14)",
                params![
                    id,
                    content_id,
                    cue.in_msec,
                    cue.out_msec,
                    cue.kind,
                    cue.color,
                    cue.color_table_index,
                    cue.active_loop as i64,
                    cue.comment,
                    cue.beat_loop_size,
                    content_uuid,
                    uuid,
                    status::AVAILABLE,
                    self.now
                ],
            )?;
            // Key order matches the reference writer's serialized cue list.
            let mut object = serde_json::Map::new();
            for (key, value) in [
                ("ContentID", serde_json::json!(content_id)),
                ("UUID", serde_json::json!(uuid)),
                ("ContentUUID", serde_json::json!(content_uuid)),
                ("InMsec", serde_json::json!(cue.in_msec)),
                ("InFrame", serde_json::json!(0)),
                ("InMpegFrame", serde_json::json!(0)),
                ("InMpegAbs", serde_json::json!(0)),
                ("OutMpegAbs", serde_json::json!(0)),
                ("OutMsec", serde_json::json!(cue.out_msec)),
                ("OutFrame", serde_json::json!(0)),
                ("OutMpegFrame", serde_json::json!(0)),
                ("Kind", serde_json::json!(cue.kind)),
                ("ActiveLoop", serde_json::json!(cue.active_loop as i64)),
                ("BeatLoopSize", serde_json::json!(cue.beat_loop_size)),
                ("Color", serde_json::json!(cue.color)),
                ("ColorTableIndex", serde_json::json!(cue.color_table_index)),
                ("Comment", serde_json::json!(cue.comment)),
                ("created_at", serde_json::json!(self.now_iso)),
                ("updated_at", serde_json::json!(self.now_iso)),
                ("ID", serde_json::json!(id)),
            ] {
                object.insert(key.into(), value);
            }
            json_cues.push(serde_json::Value::Object(object));
        }
        if !json_cues.is_empty() {
            let usn = self.use_usn();
            self.tx.execute(
                "INSERT INTO contentCue(ID,ContentID,Cues,rb_cue_count,UUID,rb_data_status,rb_local_data_status,rb_local_deleted,rb_local_synced,rb_local_usn,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,0,0,0,?7,?8,?8)",
                params![
                    content_uuid,
                    content_id,
                    serde_json::to_string(&json_cues)?,
                    json_cues.len() as i64,
                    uuid::Uuid::new_v4().to_string(),
                    status::TAGS_CHANGED,
                    usn,
                    self.now
                ],
            )?;
        }
        Ok(true)
    }

    /// Resolve a path of folder names to a playlist row ID, creating nothing.
    fn parent_of(
        &self,
        path: &[String],
        created: &BTreeMap<Vec<String>, String>,
    ) -> Result<String> {
        let mut parent = "root".to_string();
        for depth in 1..path.len() {
            let prefix = &path[..depth];
            parent = match created.get(prefix) {
                Some(id) => id.clone(),
                None => {
                    find_playlist_under(self.tx, &parent, &path[depth - 1], PlaylistKind::Folder)?
                        .with_context(|| format!("Folder {} does not exist", prefix.join("/")))?
                }
            };
        }
        Ok(parent)
    }

    fn write_playlist(&mut self, playlist: &PlaylistWrite, report: &mut WriteReport) -> Result<()> {
        // Folders created earlier in this write are remembered by path.
        let parent = self.parent_of(&playlist.path, &self.created_folders.clone())?;
        let name = playlist.path.last().unwrap();
        let attribute = match playlist.kind {
            PlaylistKind::Playlist => 0,
            PlaylistKind::Folder => 1,
            PlaylistKind::Smartlist => 4,
        };
        let existing = find_playlist_under(self.tx, &parent, name, playlist.kind)?;
        let id = match existing {
            Some(id) => id,
            None => {
                let id = self.allocate("djmdPlaylist", "ID")?.to_string();
                let usn = self.use_usn();
                let smart = playlist
                    .rules
                    .as_ref()
                    .map(|rules| intelligent_playlist_xml(id.parse().unwrap_or_default(), rules));
                self.tx.execute(
                    "INSERT INTO djmdPlaylist(ID,ParentID,Name,Seq,Attribute,SmartList,UUID,rb_data_status,rb_local_deleted,rb_local_data_status,rb_local_synced,rb_local_usn,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,0,0,0,?9,?10,?10)",
                    params![id, parent, name, playlist.position, attribute, smart, uuid::Uuid::new_v4().to_string(), status::ADDED_NOT_YET_SENT, usn, self.now],
                )?;
                report.playlists_created += 1;
                if playlist.kind == PlaylistKind::Folder {
                    self.created_folders
                        .insert(playlist.path.clone(), id.clone());
                }
                self.replace_members(&id, &playlist.tracks)?;
                return Ok(());
            }
        };
        if playlist.kind == PlaylistKind::Folder {
            self.created_folders
                .insert(playlist.path.clone(), id.clone());
        }
        let mut changed = false;
        let position: Option<i64> =
            self.tx
                .query_row("SELECT Seq FROM djmdPlaylist WHERE ID=?1", [&id], |r| {
                    r.get(0)
                })?;
        if position != Some(playlist.position) {
            let usn = self.use_usn();
            self.tx.execute(
                "UPDATE djmdPlaylist SET Seq=?1,updated_at=?2,rb_data_status=?3,rb_local_usn=?4 WHERE ID=?5",
                params![playlist.position, self.now, status::TAGS_CHANGED, usn, id],
            )?;
            changed = true;
        }
        if let Some(rules) = &playlist.rules {
            let wanted = intelligent_playlist_xml(id.parse().unwrap_or_default(), rules);
            let current: Option<String> = self.tx.query_row(
                "SELECT SmartList FROM djmdPlaylist WHERE ID=?1",
                [&id],
                |r| r.get(0),
            )?;
            if current.as_deref() != Some(wanted.as_str()) {
                let usn = self.use_usn();
                self.tx.execute(
                    "UPDATE djmdPlaylist SET SmartList=?1,updated_at=?2,rb_data_status=?3,rb_local_usn=?4 WHERE ID=?5",
                    params![wanted, self.now, status::TAGS_CHANGED, usn, id],
                )?;
                changed = true;
            }
        }
        if playlist.kind == PlaylistKind::Playlist && members(self.tx, &id)? != playlist.tracks {
            self.replace_members(&id, &playlist.tracks)?;
            changed = true;
        }
        if changed {
            report.playlists_updated += 1;
        } else {
            report.playlists_unchanged += 1;
        }
        Ok(())
    }

    /// Replace a playlist's tracks. The previous links are kept as removals, the way rekordbox records them.
    fn replace_members(&mut self, playlist_id: &str, tracks: &[String]) -> Result<()> {
        let usn = self.use_usn();
        self.tx.execute(
            "DELETE FROM djmdSongPlaylist WHERE PlaylistID=?1 AND COALESCE(rb_local_deleted,0)=1",
            [playlist_id],
        )?;
        self.tx.execute(
            "UPDATE djmdSongPlaylist SET rb_data_status=?1,rb_local_usn=?2,rb_local_deleted=1,updated_at=?3 WHERE PlaylistID=?4",
            params![status::DELETED_NOT_YET_SENT, usn, self.now, playlist_id],
        )?;
        for (index, track) in tracks.iter().enumerate() {
            self.tx.execute(
                "INSERT INTO djmdSongPlaylist(ID,PlaylistID,ContentID,TrackNo,UUID,rb_data_status,rb_local_data_status,rb_local_deleted,rb_local_synced,rb_local_usn,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,0,0,0,?7,?8,?8)",
                params![uuid::Uuid::new_v4().to_string(), playlist_id, track, index as i64 + 1, uuid::Uuid::new_v4().to_string(), status::ADDED_NOT_YET_SENT, usn, self.now],
            )?;
        }
        Ok(())
    }

    fn remove_playlist(&mut self, id: &str) -> Result<()> {
        let usn = self.use_usn();
        // A folder takes its contents with it, as removing it in rekordbox does.
        let mut doomed = vec![id.to_string()];
        let mut next = 0;
        while next < doomed.len() {
            let mut q = self.tx.prepare(
                "SELECT ID FROM djmdPlaylist WHERE ParentID=?1 AND COALESCE(rb_local_deleted,0)=0",
            )?;
            let children = q
                .query_map([&doomed[next]], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            doomed.extend(children);
            next += 1;
        }
        for id in doomed {
            self.tx.execute(
                "UPDATE djmdPlaylist SET rb_data_status=?1,rb_local_usn=?2,rb_local_deleted=1,updated_at=?3 WHERE ID=?4",
                params![status::DELETED_NOT_YET_SENT, usn, self.now, id],
            )?;
            self.tx.execute(
                "UPDATE djmdSongPlaylist SET rb_data_status=?1,rb_local_usn=?2,rb_local_deleted=1,updated_at=?3 WHERE PlaylistID=?4 AND COALESCE(rb_local_deleted,0)=0",
                params![status::DELETED_NOT_YET_SENT, usn, self.now, id],
            )?;
        }
        Ok(())
    }

    /// Replace a track's MyTag assignments when they differ. Returns whether anything was written.
    fn replace_tags(&mut self, content_id: &str, tags: &[MyTagRef]) -> Result<bool> {
        let current = {
            let mut q = self.tx.prepare(
                "SELECT c.Name,t.Name FROM djmdSongMyTag l JOIN djmdMyTag t ON t.ID=l.MyTagID JOIN djmdMyTag c ON c.ID=t.ParentID WHERE l.ContentID=?1 AND COALESCE(l.rb_local_deleted,0)=0 AND COALESCE(t.rb_local_deleted,0)=0",
            )?;
            q.query_map([content_id], |r| {
                Ok(MyTagRef {
                    category: r.get(0)?,
                    tag: r.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<BTreeSet<_>>>()?
        };
        let wanted: BTreeSet<_> = tags.iter().cloned().collect();
        if current == wanted {
            return Ok(false);
        }
        let usn = self.use_usn();
        self.tx.execute(
            "DELETE FROM djmdSongMyTag WHERE ContentID=?1 AND COALESCE(rb_local_deleted,0)=1",
            [content_id],
        )?;
        self.tx.execute(
            "UPDATE djmdSongMyTag SET rb_data_status=?1,rb_local_usn=?2,rb_local_deleted=1,updated_at=?3 WHERE ContentID=?4",
            params![status::DELETED_NOT_YET_SENT, usn, self.now, content_id],
        )?;
        for tag in &wanted {
            ensure!(
                !tag.category.trim().is_empty() && !tag.tag.trim().is_empty(),
                "A MyTag needs a category and a name"
            );
            let tag_id = self.my_tag(tag)?;
            self.tx.execute(
                "INSERT INTO djmdSongMyTag(ID,MyTagID,ContentID,UUID,rb_data_status,rb_local_deleted,rb_local_usn,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,0,?6,?7,?7)",
                params![uuid::Uuid::new_v4().to_string(), tag_id, content_id, uuid::Uuid::new_v4().to_string(), status::ADDED_NOT_YET_SENT, usn, self.now],
            )?;
        }
        Ok(true)
    }

    /// Find or create a MyTag category and tag, returning the tag's row ID.
    fn my_tag(&mut self, tag: &MyTagRef) -> Result<String> {
        let category: Option<String> = self.tx.query_row(
            "SELECT ID FROM djmdMyTag WHERE Attribute=1 AND Name=?1 AND COALESCE(rb_local_deleted,0)=0 ORDER BY CAST(ID AS INTEGER) LIMIT 1",
            [&tag.category],
            |r| r.get(0),
        ).optional()?;
        let category = match category {
            Some(id) => id,
            None => {
                let id = self.allocate("djmdMyTag", "ID")?.to_string();
                let usn = self.use_usn();
                let seq: i64 = self.tx.query_row(
                    "SELECT COALESCE(MAX(Seq),0)+1 FROM djmdMyTag WHERE Attribute=1",
                    [],
                    |r| r.get(0),
                )?;
                self.tx.execute(
                    "INSERT INTO djmdMyTag(ID,Name,Seq,Attribute,ParentID,UUID,rb_data_status,rb_local_deleted,rb_local_usn,created_at,updated_at) VALUES(?1,?2,?3,1,'root',?4,?5,0,?6,?7,?7)",
                    params![id, tag.category, seq, uuid::Uuid::new_v4().to_string(), status::ADDED_NOT_YET_SENT, usn, self.now],
                )?;
                id
            }
        };
        let found: Option<String> = self.tx.query_row(
            "SELECT ID FROM djmdMyTag WHERE Attribute=0 AND ParentID=?1 AND Name=?2 AND COALESCE(rb_local_deleted,0)=0 ORDER BY CAST(ID AS INTEGER) LIMIT 1",
            params![category, tag.tag],
            |r| r.get(0),
        ).optional()?;
        if let Some(id) = found {
            return Ok(id);
        }
        let id = self.allocate("djmdMyTag", "ID")?.to_string();
        let usn = self.use_usn();
        let seq: i64 = self.tx.query_row(
            "SELECT COALESCE(MAX(Seq),0)+1 FROM djmdMyTag WHERE Attribute=0 AND ParentID=?1",
            [&category],
            |r| r.get(0),
        )?;
        self.tx.execute(
            "INSERT INTO djmdMyTag(ID,Name,Seq,Attribute,ParentID,UUID,rb_data_status,rb_local_deleted,rb_local_usn,created_at,updated_at) VALUES(?1,?2,?3,0,?4,?5,?6,0,?7,?8,?8)",
            params![id, tag.tag, seq, category, uuid::Uuid::new_v4().to_string(), status::ADDED_NOT_YET_SENT, usn, self.now],
        )?;
        Ok(id)
    }
}

/// Ordered, live track IDs of a playlist.
fn members(conn: &Connection, playlist_id: &str) -> Result<Vec<String>> {
    let mut q = conn.prepare(
        "SELECT ContentID FROM djmdSongPlaylist WHERE PlaylistID=?1 AND COALESCE(rb_local_deleted,0)=0 ORDER BY TrackNo,rowid",
    )?;
    Ok(q.query_map([playlist_id], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?)
}
fn find_playlist_under(
    conn: &Connection,
    parent: &str,
    name: &str,
    kind: PlaylistKind,
) -> Result<Option<String>> {
    let attributes: &[i64] = match kind {
        PlaylistKind::Playlist => &[0, -128],
        PlaylistKind::Folder => &[1],
        PlaylistKind::Smartlist => &[4],
    };
    let mut q = conn.prepare(
        "SELECT ID,Attribute FROM djmdPlaylist WHERE ParentID=?1 AND Name=?2 AND COALESCE(rb_local_deleted,0)=0 ORDER BY CAST(ID AS INTEGER)",
    )?;
    let rows = q
        .query_map(params![parent, name], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows
        .into_iter()
        .find(|(_, a)| attributes.contains(a))
        .map(|(id, _)| id))
}
/// A playlist by its name path, or `None` when any part of the path does not exist.
fn find_playlist(conn: &Connection, path: &[String], kind: PlaylistKind) -> Result<Option<String>> {
    let mut parent = "root".to_string();
    for (depth, name) in path.iter().enumerate() {
        let wanted = if depth + 1 == path.len() {
            kind
        } else {
            PlaylistKind::Folder
        };
        match find_playlist_under(conn, &parent, name, wanted)? {
            Some(id) => parent = id,
            None => return Ok(None),
        }
    }
    Ok(Some(parent))
}

/// JavaScript's `encodeURIComponent`, which the reference exporter uses to build file record IDs.
fn encode_uri_component(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}
fn cue_key(cue: &CueRow) -> (i64, i64, i64, Option<String>) {
    (cue.in_msec, cue.out_msec, cue.kind, cue.comment.clone())
}
fn same_value(current: Option<&TargetValue>, wanted: &rusqlite::types::Value) -> bool {
    use rusqlite::types::Value;
    match (current, wanted) {
        (None | Some(TargetValue::Null), Value::Null) => true,
        (Some(TargetValue::Integer(a)), Value::Integer(b)) => a == b,
        (Some(TargetValue::Real(a)), Value::Integer(b)) => *a == *b as f64,
        (Some(TargetValue::Text(a)), Value::Text(b)) => a == b,
        (Some(TargetValue::Integer(a)), Value::Text(b)) => a.to_string() == *b,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // Only the columns the writer touches, so no application schema is reproduced here.
    const TABLES: &str = "
        CREATE TABLE djmdProperty(DBID TEXT PRIMARY KEY,DeviceID TEXT);
        CREATE TABLE agentRegistry(registry_id TEXT PRIMARY KEY,int_1 INTEGER);
        CREATE TABLE djmdContent(ID TEXT PRIMARY KEY,UUID TEXT,Title TEXT,Commnt TEXT,Subtitle TEXT,Lyricist TEXT,ReleaseDate TEXT,
          BPM INTEGER,ReleaseYear INTEGER,TrackNo INTEGER,DiscNo INTEGER,Rating INTEGER,ArtistID TEXT,AlbumID TEXT,GenreID TEXT,
          LabelID TEXT,RemixerID TEXT,ComposerID TEXT,KeyID TEXT,CueUpdated TEXT,TrackInfoUpdated TEXT,
          rb_data_status INTEGER DEFAULT 0,rb_local_deleted INTEGER DEFAULT 0,rb_local_usn INTEGER,updated_at TEXT);
        CREATE TABLE djmdArtist(ID TEXT PRIMARY KEY,Name TEXT,UUID TEXT,rb_data_status INTEGER,rb_local_data_status INTEGER,rb_local_deleted INTEGER,rb_local_synced INTEGER,rb_local_usn INTEGER,created_at TEXT,updated_at TEXT);
        CREATE TABLE djmdAlbum(ID TEXT PRIMARY KEY,Name TEXT,UUID TEXT,rb_data_status INTEGER,rb_local_data_status INTEGER,rb_local_deleted INTEGER,rb_local_synced INTEGER,rb_local_usn INTEGER,created_at TEXT,updated_at TEXT);
        CREATE TABLE djmdGenre(ID TEXT PRIMARY KEY,Name TEXT,UUID TEXT,rb_data_status INTEGER,rb_local_data_status INTEGER,rb_local_deleted INTEGER,rb_local_synced INTEGER,rb_local_usn INTEGER,created_at TEXT,updated_at TEXT);
        CREATE TABLE djmdLabel(ID TEXT PRIMARY KEY,Name TEXT,UUID TEXT,rb_data_status INTEGER,rb_local_data_status INTEGER,rb_local_deleted INTEGER,rb_local_synced INTEGER,rb_local_usn INTEGER,created_at TEXT,updated_at TEXT);
        CREATE TABLE djmdKey(ID TEXT PRIMARY KEY,ScaleName TEXT,UUID TEXT,rb_data_status INTEGER,rb_local_data_status INTEGER,rb_local_deleted INTEGER,rb_local_synced INTEGER,rb_local_usn INTEGER,created_at TEXT,updated_at TEXT);
        CREATE TABLE djmdCue(ID TEXT PRIMARY KEY,ContentID TEXT,InMsec INTEGER,InFrame INTEGER,InMpegFrame INTEGER,InMpegAbs INTEGER,OutMsec INTEGER,OutFrame INTEGER,OutMpegFrame INTEGER,OutMpegAbs INTEGER,Kind INTEGER,Color INTEGER,ColorTableIndex INTEGER,ActiveLoop INTEGER,Comment TEXT,BeatLoopSize INTEGER,ContentUUID TEXT,UUID TEXT,rb_data_status INTEGER,rb_local_data_status INTEGER,rb_local_deleted INTEGER,rb_local_synced INTEGER,created_at TEXT,updated_at TEXT);
        CREATE TABLE contentCue(ID TEXT PRIMARY KEY,ContentID TEXT,Cues TEXT,rb_cue_count INTEGER,UUID TEXT,rb_data_status INTEGER,rb_local_data_status INTEGER,rb_local_deleted INTEGER,rb_local_synced INTEGER,rb_local_usn INTEGER,created_at TEXT,updated_at TEXT);
        CREATE TABLE djmdPlaylist(ID TEXT PRIMARY KEY,Seq INTEGER,Name TEXT,Attribute INTEGER,ParentID TEXT,UUID TEXT,rb_data_status INTEGER,rb_local_deleted INTEGER,rb_local_data_status INTEGER,rb_local_synced INTEGER,rb_local_usn INTEGER,created_at TEXT,updated_at TEXT);
        CREATE TABLE djmdSongPlaylist(ID TEXT PRIMARY KEY,PlaylistID TEXT,ContentID TEXT,TrackNo INTEGER,UUID TEXT,rb_data_status INTEGER,rb_local_data_status INTEGER,rb_local_deleted INTEGER,rb_local_synced INTEGER,rb_local_usn INTEGER,created_at TEXT,updated_at TEXT);
        CREATE TABLE djmdMyTag(ID TEXT PRIMARY KEY,Seq INTEGER,Name TEXT,Attribute INTEGER,ParentID TEXT,UUID TEXT,rb_data_status INTEGER,rb_local_deleted INTEGER,rb_local_usn INTEGER,created_at TEXT,updated_at TEXT);
        CREATE TABLE djmdSongMyTag(ID TEXT PRIMARY KEY,MyTagID TEXT,ContentID TEXT,UUID TEXT,rb_data_status INTEGER,rb_local_deleted INTEGER,rb_local_usn INTEGER,created_at TEXT,updated_at TEXT);
        INSERT INTO djmdPlaylist VALUES('50',1,'Sets',1,'root','pf',256,0,0,0,40,'t','t');
        INSERT INTO djmdPlaylist VALUES('51',1,'Friday',0,'50','pp',256,0,0,0,40,'t','t');
        INSERT INTO djmdSongPlaylist VALUES('l1','51','100',1,'u1',256,0,0,0,40,'t','t');
        INSERT INTO djmdSongPlaylist VALUES('l2','51','200',2,'u2',256,0,0,0,40,'t','t');
        INSERT INTO djmdSongPlaylist VALUES('l0','51','100',9,'u0',258,0,1,0,39,'t','t');
        INSERT INTO djmdMyTag VALUES('60',1,'Mood',1,'root','uc',256,0,40,'t','t');
        INSERT INTO djmdMyTag VALUES('61',1,'Dark',0,'60','ut',256,0,40,'t','t');
        INSERT INTO djmdSongMyTag VALUES('m1','61','200','um',256,0,40,'t','t');
        ALTER TABLE djmdPlaylist ADD COLUMN SmartList TEXT;
        ALTER TABLE djmdContent ADD COLUMN MasterSongID TEXT;
        ALTER TABLE djmdContent ADD COLUMN MasterDBID TEXT;
        ALTER TABLE djmdContent ADD COLUMN DeviceID TEXT;
        ALTER TABLE djmdContent ADD COLUMN FolderPath TEXT;
        ALTER TABLE djmdContent ADD COLUMN OrgFolderPath TEXT;
        ALTER TABLE djmdContent ADD COLUMN FileNameL TEXT;
        ALTER TABLE djmdContent ADD COLUMN Length INTEGER;
        ALTER TABLE djmdContent ADD COLUMN BitRate INTEGER;
        ALTER TABLE djmdContent ADD COLUMN BitDepth INTEGER;
        ALTER TABLE djmdContent ADD COLUMN SampleRate INTEGER;
        ALTER TABLE djmdContent ADD COLUMN FileSize INTEGER;
        ALTER TABLE djmdContent ADD COLUMN FileType INTEGER;
        ALTER TABLE djmdContent ADD COLUMN StockDate TEXT;
        ALTER TABLE djmdContent ADD COLUMN DateCreated TEXT;
        ALTER TABLE djmdContent ADD COLUMN ColorID TEXT;
        ALTER TABLE djmdContent ADD COLUMN ImagePath TEXT;
        ALTER TABLE djmdContent ADD COLUMN ContentLink INTEGER;
        ALTER TABLE djmdContent ADD COLUMN HotCueAutoLoad TEXT;
        ALTER TABLE djmdContent ADD COLUMN SamplerTrackInfo INTEGER;
        ALTER TABLE djmdContent ADD COLUMN SamplerPlayOffset INTEGER;
        ALTER TABLE djmdContent ADD COLUMN SamplerGain REAL;
        ALTER TABLE djmdContent ADD COLUMN Analysed INTEGER;
        ALTER TABLE djmdContent ADD COLUMN AnalysisUpdated TEXT;
        ALTER TABLE djmdContent ADD COLUMN AnalysisDataPath TEXT;
        ALTER TABLE djmdContent ADD COLUMN VideoAssociate TEXT;
        ALTER TABLE djmdContent ADD COLUMN LyricStatus INTEGER;
        ALTER TABLE djmdContent ADD COLUMN ServiceID INTEGER;
        ALTER TABLE djmdContent ADD COLUMN ExtInfo TEXT;
        ALTER TABLE djmdContent ADD COLUMN rb_file_id TEXT;
        ALTER TABLE djmdContent ADD COLUMN rb_local_data_status INTEGER;
        ALTER TABLE djmdContent ADD COLUMN rb_local_synced INTEGER;
        ALTER TABLE djmdContent ADD COLUMN created_at TEXT;
        CREATE TABLE contentFile(ID TEXT PRIMARY KEY,ContentID TEXT,Path TEXT,Hash TEXT,Size INTEGER,rb_local_path TEXT,rb_file_hash_dirty INTEGER,rb_local_file_status INTEGER,rb_in_progress INTEGER,rb_process_type INTEGER,rb_priority INTEGER,rb_file_size_dirty INTEGER,UUID TEXT,rb_data_status INTEGER,rb_local_data_status INTEGER,rb_local_deleted INTEGER,rb_local_synced INTEGER,rb_local_usn INTEGER,created_at TEXT,updated_at TEXT);
        INSERT INTO djmdProperty VALUES('db','dev');
        INSERT INTO agentRegistry VALUES('localUpdateCount',41);
        INSERT INTO djmdArtist VALUES('7','Old Artist','ua',256,0,0,0,40,'t','t');
        INSERT INTO djmdArtist VALUES('9','Revived Artist','ur',256,0,1,0,40,'t','t');
        INSERT INTO djmdContent(ID,UUID,Title,Commnt,BPM,ArtistID,CueUpdated,TrackInfoUpdated,rb_data_status,rb_local_usn,updated_at)
          VALUES('100','track-uuid','Old Title','keep me',12800,'7','1','2',256,40,'t');
        INSERT INTO djmdContent(ID,UUID,Title,rb_data_status,rb_local_usn,updated_at) VALUES('200','other-uuid','Untouched',256,40,'t');
        UPDATE djmdContent SET MasterSongID=ID;
        UPDATE djmdContent SET FolderPath='/music/existing-100.mp3' WHERE ID='100';
        UPDATE djmdContent SET FolderPath='/music/existing-200.mp3' WHERE ID='200';
        UPDATE djmdProperty SET DeviceID='dev';
    ";
    static PROFILE: &[WriteProfile] = &[WriteProfile {
        name: "synthetic",
        schema_fingerprint: "",
    }];
    fn key() -> DatabaseKey {
        DatabaseKey::new(b"generated writer test key").unwrap()
    }
    /// A generated encrypted database. Its fingerprint is computed so the synthetic profile can match.
    fn database() -> (tempfile::TempDir, PathBuf, &'static [WriteProfile]) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("master.db");
        let conn = Connection::open(&path).unwrap();
        set_key(&conn, &key()).unwrap();
        conn.execute_batch(TABLES).unwrap();
        let fingerprint = schema_on(&conn).unwrap().fingerprint;
        drop(conn);
        let profile: &'static [WriteProfile] = Box::leak(Box::new([WriteProfile {
            name: "synthetic",
            schema_fingerprint: Box::leak(fingerprint.into_boxed_str()),
        }]));
        (dir, path, profile)
    }
    fn fingerprint(path: &Path, id: &str) -> String {
        let conn = Connection::open(path).unwrap();
        set_key(&conn, &key()).unwrap();
        row_fingerprint(&conn, "djmdContent", "ID", id)
            .unwrap()
            .unwrap()
            .0
    }
    fn query<T: rusqlite::types::FromSql>(path: &Path, sql: &str) -> T {
        let conn = Connection::open(path).unwrap();
        set_key(&conn, &key()).unwrap();
        conn.query_row(sql, [], |r| r.get(0)).unwrap()
    }
    fn plan(path: &Path) -> WritePlan {
        WritePlan {
            tracks: vec![TrackWrite {
                content_id: "100".into(),
                expected_fingerprint: fingerprint(path, "100"),
                fields: ContentFields {
                    title: Some("New Title".into()),
                    artist: Some("Revived Artist".into()),
                    genre: Some("Tech House".into()),
                    key: Some("8A".into()),
                    bpm_hundredths: Some(12850),
                    ..Default::default()
                },
                cues: Some(vec![
                    CueRow {
                        in_msec: 1000,
                        out_msec: -1,
                        kind: 1,
                        active_loop: false,
                        beat_loop_size: None,
                        color: None,
                        color_table_index: Some(41),
                        comment: Some("Start".into()),
                    },
                    CueRow {
                        in_msec: 60000,
                        out_msec: 68000,
                        kind: 4,
                        active_loop: true,
                        beat_loop_size: Some(65536 * 16 + 1),
                        color: Some(0),
                        color_table_index: None,
                        comment: Some("S.O.S.".into()),
                    },
                ]),
                tags: None,
            }],
            ..Default::default()
        }
    }

    #[test]
    fn timestamps_use_the_reference_row_format() {
        let (row, iso) =
            timestamp(std::time::UNIX_EPOCH + std::time::Duration::from_millis(1_760_923_445_678));
        assert_eq!(row, "2025-10-20 01:24:05.678 +00:00");
        assert_eq!(iso, "2025-10-20T01:24:05.678Z");
        assert_eq!(
            timestamp(std::time::UNIX_EPOCH).0,
            "1970-01-01 00:00:00.000 +00:00"
        );
        assert_eq!(
            timestamp(std::time::UNIX_EPOCH + std::time::Duration::from_secs(951_782_400)).0,
            "2000-02-29 00:00:00.000 +00:00"
        );
    }

    #[test]
    fn unverified_schemas_are_not_writable() {
        let (_dir, path, _) = database();
        let error = WriteDatabase::open(&path, &key())
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("not verified for writes"), "{error}");
        let _ = PROFILE;
    }

    #[test]
    fn a_write_changes_only_the_planned_track_with_one_usn_and_row_status() {
        let (_dir, path, profile) = database();
        let plan = plan(&path);
        let untouched = fingerprint(&path, "200");
        let mut db = WriteDatabase::open_with_profiles(&path, &key(), profile).unwrap();
        let report = db.apply(&plan, &CancellationToken::default()).unwrap();
        drop(db);
        assert_eq!(report.usn, Some(42));
        assert_eq!(
            (
                report.tracks_updated,
                report.tracks_unchanged,
                report.cues_replaced
            ),
            (1, 0, 1)
        );
        // The soft-deleted artist is revived rather than duplicated; new genre and key rows are created.
        assert_eq!(report.foreign_revived["djmdArtist"], 1);
        assert_eq!(report.foreign_created["djmdGenre"], 1);
        assert_eq!(report.foreign_created["djmdKey"], 1);
        assert!(!report.foreign_created.contains_key("djmdArtist"));
        assert_eq!(query::<i64>(&path, "SELECT COUNT(*) FROM djmdArtist"), 2);
        assert_eq!(
            query::<String>(&path, "SELECT Title FROM djmdContent WHERE ID='100'"),
            "New Title"
        );
        assert_eq!(
            query::<i64>(&path, "SELECT BPM FROM djmdContent WHERE ID='100'"),
            12850
        );
        // Fields not in the plan are preserved.
        assert_eq!(
            query::<String>(&path, "SELECT Commnt FROM djmdContent WHERE ID='100'"),
            "keep me"
        );
        assert_eq!(
            query::<String>(&path, "SELECT ArtistID FROM djmdContent WHERE ID='100'"),
            "9"
        );
        assert_eq!(
            query::<i64>(
                &path,
                "SELECT rb_local_deleted FROM djmdArtist WHERE ID='9'"
            ),
            0
        );
        // Row bookkeeping uses the single new change number and the changed-tags status.
        assert_eq!(
            query::<i64>(&path, "SELECT rb_local_usn FROM djmdContent WHERE ID='100'"),
            42
        );
        assert_eq!(
            query::<i64>(
                &path,
                "SELECT rb_data_status FROM djmdContent WHERE ID='100'"
            ),
            257
        );
        assert_eq!(query::<i64>(&path, "SELECT int_1 FROM agentRegistry"), 42);
        assert_eq!(
            query::<String>(&path, "SELECT CueUpdated FROM djmdContent WHERE ID='100'"),
            "2"
        );
        assert_eq!(
            query::<String>(
                &path,
                "SELECT TrackInfoUpdated FROM djmdContent WHERE ID='100'"
            ),
            "3"
        );
        assert_eq!(fingerprint(&path, "200"), untouched);
        // Cues land in both tables, with the list keyed by the track's UUID.
        assert_eq!(
            query::<i64>(&path, "SELECT COUNT(*) FROM djmdCue WHERE ContentID='100'"),
            2
        );
        assert_eq!(
            query::<String>(&path, "SELECT ID FROM contentCue WHERE ContentID='100'"),
            "track-uuid"
        );
        assert_eq!(
            query::<i64>(&path, "SELECT rb_cue_count FROM contentCue"),
            2
        );
        let list: serde_json::Value =
            serde_json::from_str(&query::<String>(&path, "SELECT Cues FROM contentCue")).unwrap();
        assert_eq!(list[1]["Kind"], 4);
        assert_eq!(list[1]["BeatLoopSize"], 65536 * 16 + 1);
        assert_eq!(list[0]["Comment"], "Start");
        assert!(list[0].get("rb_data_status").is_none());
        let id: i64 = query::<String>(&path, "SELECT ID FROM djmdCue ORDER BY InMsec LIMIT 1")
            .parse()
            .unwrap();
        assert!((1_000_000_000..=MAX_ID).contains(&id));
    }

    #[test]
    fn repeating_a_write_is_a_noop_that_changes_nothing_and_takes_no_change_number() {
        let (_dir, path, profile) = database();
        let first = plan(&path);
        let mut db = WriteDatabase::open_with_profiles(&path, &key(), profile).unwrap();
        db.apply(&first, &CancellationToken::default()).unwrap();
        drop(db);
        let repeat = WritePlan {
            tracks: vec![TrackWrite {
                expected_fingerprint: fingerprint(&path, "100"),
                ..plan(&path).tracks.remove(0)
            }],
            ..Default::default()
        };
        let before = cuerithm_media::file_sha256(&path).unwrap();
        let mut db = WriteDatabase::open_with_profiles(&path, &key(), profile).unwrap();
        let report = db.apply(&repeat, &CancellationToken::default()).unwrap();
        drop(db);
        assert_eq!(report.usn, None);
        assert_eq!(
            (
                report.tracks_updated,
                report.tracks_unchanged,
                report.cues_replaced
            ),
            (0, 1, 0)
        );
        assert!(report.foreign_created.is_empty() && report.foreign_revived.is_empty());
        assert_eq!(query::<i64>(&path, "SELECT int_1 FROM agentRegistry"), 42);
        assert_eq!(cuerithm_media::file_sha256(&path).unwrap(), before);
    }

    #[test]
    fn a_track_changed_in_rekordbox_after_it_was_read_refuses_the_whole_write() {
        let (_dir, path, profile) = database();
        let mut plan = plan(&path);
        plan.tracks.push(TrackWrite {
            content_id: "200".into(),
            expected_fingerprint: fingerprint(&path, "200"),
            fields: ContentFields {
                title: Some("Second".into()),
                ..Default::default()
            },
            cues: None,
            tags: None,
        });
        let conn = Connection::open(&path).unwrap();
        set_key(&conn, &key()).unwrap();
        conn.execute(
            "UPDATE djmdContent SET Title='Edited in rekordbox' WHERE ID='100'",
            [],
        )
        .unwrap();
        drop(conn);
        let before = cuerithm_media::file_sha256(&path).unwrap();
        let mut db = WriteDatabase::open_with_profiles(&path, &key(), profile).unwrap();
        let error = db.apply(&plan, &CancellationToken::default()).unwrap_err();
        drop(db);
        let conflict = error.downcast_ref::<ConflictError>().unwrap();
        assert_eq!(conflict.0.len(), 1);
        assert_eq!(conflict.0[0].subject, "100");
        // Not even the unaffected second track was written.
        assert_eq!(
            query::<String>(&path, "SELECT Title FROM djmdContent WHERE ID='200'"),
            "Untouched"
        );
        assert_eq!(cuerithm_media::file_sha256(&path).unwrap(), before);
    }

    #[test]
    fn missing_and_removed_tracks_are_conflicts_and_a_failure_rolls_everything_back() {
        let (_dir, path, profile) = database();
        let conn = Connection::open(&path).unwrap();
        set_key(&conn, &key()).unwrap();
        conn.execute(
            "UPDATE djmdContent SET rb_local_deleted=1 WHERE ID='200'",
            [],
        )
        .unwrap();
        drop(conn);
        let removed = WritePlan {
            tracks: vec![
                TrackWrite {
                    content_id: "200".into(),
                    expected_fingerprint: fingerprint(&path, "200"),
                    fields: ContentFields::default(),
                    cues: None,
                    tags: None,
                },
                TrackWrite {
                    content_id: "999".into(),
                    expected_fingerprint: "x".into(),
                    fields: ContentFields::default(),
                    cues: None,
                    tags: None,
                },
            ],
            ..Default::default()
        };
        // An invalid cue fails midway through the plan; the earlier field changes must not persist.
        let mut bad = plan(&path);
        bad.tracks[0].cues.as_mut().unwrap()[0].out_msec = 5;
        let mut db = WriteDatabase::open_with_profiles(&path, &key(), profile).unwrap();
        let error = db
            .apply(&removed, &CancellationToken::default())
            .unwrap_err();
        assert_eq!(error.downcast_ref::<ConflictError>().unwrap().0.len(), 2);
        let error = db.apply(&bad, &CancellationToken::default()).unwrap_err();
        assert!(error.to_string().contains("invalid time range"));
        drop(db);
        assert_eq!(
            query::<String>(&path, "SELECT Title FROM djmdContent WHERE ID='100'"),
            "Old Title"
        );
        assert_eq!(query::<i64>(&path, "SELECT int_1 FROM agentRegistry"), 41);
        assert_eq!(query::<i64>(&path, "SELECT COUNT(*) FROM djmdGenre"), 0);
    }

    #[test]
    fn cancellation_before_commit_writes_nothing() {
        let (_dir, path, profile) = database();
        let cancel = CancellationToken::default();
        cancel.cancel();
        let plan = plan(&path);
        let mut db = WriteDatabase::open_with_profiles(&path, &key(), profile).unwrap();
        assert!(db.apply(&plan, &cancel).is_err());
        drop(db);
        assert_eq!(query::<i64>(&path, "SELECT int_1 FROM agentRegistry"), 41);
    }

    #[test]
    fn a_database_held_open_elsewhere_fails_fast_instead_of_blocking() {
        let (_dir, path, profile) = database();
        let first = WriteDatabase::open_with_profiles(&path, &key(), profile).unwrap();
        let second = WriteDatabase::open_with_profiles(&path, &key(), profile);
        assert!(second.is_err());
        assert!(
            second
                .err()
                .unwrap()
                .to_string()
                .contains("Close rekordbox")
        );
        // Once the first writer is gone the database is available again.
        drop(first);
        assert!(WriteDatabase::open_with_profiles(&path, &key(), profile).is_ok());
    }

    fn base_plan() -> WritePlan {
        WritePlan::default()
    }
    fn friday(tracks: &[&str], expected: Option<String>) -> PlaylistWrite {
        PlaylistWrite {
            path: vec!["Sets".into(), "Friday".into()],
            kind: PlaylistKind::Playlist,
            position: 1,
            tracks: tracks.iter().map(|t| t.to_string()).collect(),
            expected_members: expected,
            rules: None,
        }
    }
    fn run(path: &Path, profile: &'static [WriteProfile], plan: &WritePlan) -> Result<WriteReport> {
        let mut db = WriteDatabase::open_with_profiles(path, &key(), profile).unwrap();
        db.apply(plan, &CancellationToken::default())
    }

    #[test]
    fn playlists_are_created_ordered_and_reordered_with_removals_recorded() {
        let (_dir, path, profile) = database();
        let mut plan = base_plan();
        plan.playlists = vec![
            // A child listed before its new parent folder is still written parent-first.
            PlaylistWrite {
                path: vec!["New Folder".into(), "Warmup".into()],
                kind: PlaylistKind::Playlist,
                position: 1,
                tracks: vec!["200".into(), "100".into()],
                expected_members: None,
                rules: None,
            },
            PlaylistWrite {
                path: vec!["New Folder".into()],
                kind: PlaylistKind::Folder,
                position: 2,
                tracks: vec![],
                expected_members: None,
                rules: None,
            },
            friday(
                &["200", "100"],
                Some(membership_fingerprint(&["100".into(), "200".into()])),
            ),
        ];
        let report = run(&path, profile, &plan).unwrap();
        assert_eq!(
            (
                report.playlists_created,
                report.playlists_updated,
                report.playlists_unchanged
            ),
            (2, 1, 0)
        );
        assert_eq!(report.usn, Some(42));
        let folder: String = query(&path, "SELECT ID FROM djmdPlaylist WHERE Name='New Folder'");
        assert_eq!(
            query::<i64>(
                &path,
                "SELECT Attribute FROM djmdPlaylist WHERE Name='New Folder'"
            ),
            1
        );
        assert_eq!(
            query::<String>(
                &path,
                "SELECT ParentID FROM djmdPlaylist WHERE Name='New Folder'"
            ),
            "root"
        );
        assert_eq!(
            query::<String>(
                &path,
                "SELECT ParentID FROM djmdPlaylist WHERE Name='Warmup'"
            ),
            folder
        );
        // New playlist: tracks numbered from one, in the planned order.
        let order: String = query(
            &path,
            "SELECT group_concat(ContentID||':'||TrackNo,',') FROM (SELECT ContentID,TrackNo FROM djmdSongPlaylist WHERE PlaylistID=(SELECT ID FROM djmdPlaylist WHERE Name='Warmup') ORDER BY TrackNo)",
        );
        assert_eq!(order, "200:1,100:2");
        // Reordered playlist: old links remain only as recorded removals; the stale removal was dropped.
        assert_eq!(
            query::<String>(
                &path,
                "SELECT group_concat(ContentID,',') FROM (SELECT ContentID FROM djmdSongPlaylist WHERE PlaylistID='51' AND rb_local_deleted=0 ORDER BY TrackNo)"
            ),
            "200,100"
        );
        assert_eq!(
            query::<i64>(
                &path,
                "SELECT COUNT(*) FROM djmdSongPlaylist WHERE PlaylistID='51' AND rb_local_deleted=1"
            ),
            2
        );
        assert_eq!(
            query::<i64>(&path, "SELECT COUNT(*) FROM djmdSongPlaylist WHERE ID='l0'"),
            0
        );
        assert_eq!(
            query::<i64>(
                &path,
                "SELECT MIN(rb_data_status) FROM djmdSongPlaylist WHERE PlaylistID='51' AND rb_local_deleted=1"
            ),
            258
        );
        assert_eq!(
            query::<i64>(
                &path,
                "SELECT rb_local_usn FROM djmdSongPlaylist WHERE PlaylistID='51' AND rb_local_deleted=0 LIMIT 1"
            ),
            42
        );
    }

    #[test]
    fn repeating_a_playlist_plan_changes_nothing() {
        let (_dir, path, profile) = database();
        let mut plan = base_plan();
        plan.playlists = vec![friday(&["200", "100"], None)];
        run(&path, profile, &plan).unwrap();
        let before = cuerithm_media::file_sha256(&path).unwrap();
        let report = run(&path, profile, &plan).unwrap();
        assert_eq!(
            (
                report.usn,
                report.playlists_unchanged,
                report.playlists_updated
            ),
            (None, 1, 0)
        );
        assert_eq!(cuerithm_media::file_sha256(&path).unwrap(), before);
    }

    #[test]
    fn a_playlist_edited_in_rekordbox_or_naming_a_missing_track_refuses_the_plan() {
        let (_dir, path, profile) = database();
        let mut plan = base_plan();
        plan.playlists = vec![friday(
            &["100"],
            Some(membership_fingerprint(&["200".into()])),
        )];
        let error = run(&path, profile, &plan).unwrap_err();
        assert_eq!(
            error.downcast_ref::<ConflictError>().unwrap().0[0].subject,
            "playlist Sets/Friday"
        );
        plan.playlists = vec![friday(&["100", "999"], None)];
        let error = run(&path, profile, &plan).unwrap_err();
        assert!(
            error.downcast_ref::<ConflictError>().unwrap().0[0]
                .reason
                .contains("999")
        );
        // A track listed twice or a folder with tracks is a malformed plan, not a conflict.
        plan.playlists = vec![friday(&["100", "100"], None)];
        assert!(
            run(&path, profile, &plan)
                .unwrap_err()
                .to_string()
                .contains("twice")
        );
        assert_eq!(query::<i64>(&path, "SELECT int_1 FROM agentRegistry"), 41);
        assert_eq!(
            query::<i64>(
                &path,
                "SELECT COUNT(*) FROM djmdSongPlaylist WHERE rb_local_deleted=0"
            ),
            2
        );
    }

    #[test]
    fn removing_a_folder_takes_its_playlists_and_links_as_recorded_removals() {
        let (_dir, path, profile) = database();
        let mut plan = base_plan();
        plan.remove_playlists = vec![PlaylistRemoval {
            playlist_id: "50".into(),
            expected_members: None,
        }];
        let report = run(&path, profile, &plan).unwrap();
        assert_eq!(report.playlists_removed, 1);
        assert_eq!(
            query::<i64>(
                &path,
                "SELECT COUNT(*) FROM djmdPlaylist WHERE rb_local_deleted=1"
            ),
            2
        );
        assert_eq!(
            query::<i64>(
                &path,
                "SELECT COUNT(*) FROM djmdSongPlaylist WHERE PlaylistID='51' AND rb_local_deleted=0"
            ),
            0
        );
        // Removing it again is a conflict: it is already gone.
        let error = run(&path, profile, &plan).unwrap_err();
        assert!(error.downcast_ref::<ConflictError>().is_some());
    }

    #[test]
    fn tags_reuse_existing_rows_create_missing_ones_and_replace_assignments() {
        let (_dir, path, profile) = database();
        let tag = |category: &str, tag: &str| MyTagRef {
            category: category.into(),
            tag: tag.into(),
        };
        let mut plan = base_plan();
        plan.tracks = vec![TrackWrite {
            content_id: "200".into(),
            expected_fingerprint: fingerprint(&path, "200"),
            fields: ContentFields::default(),
            cues: None,
            tags: Some(vec![tag("Mood", "Dark"), tag("Energy", "Peak")]),
        }];
        let report = run(&path, profile, &plan).unwrap();
        assert_eq!((report.tags_replaced, report.tracks_updated), (1, 1));
        // The existing category and tag were reused; one category and one tag were created.
        assert_eq!(
            query::<i64>(&path, "SELECT COUNT(*) FROM djmdMyTag WHERE Attribute=1"),
            2
        );
        assert_eq!(
            query::<i64>(&path, "SELECT COUNT(*) FROM djmdMyTag WHERE Attribute=0"),
            2
        );
        assert_eq!(
            query::<String>(
                &path,
                "SELECT group_concat(Name,',') FROM (SELECT t.Name FROM djmdSongMyTag l JOIN djmdMyTag t ON t.ID=l.MyTagID WHERE l.ContentID='200' AND l.rb_local_deleted=0 ORDER BY t.Name)"
            ),
            "Dark,Peak"
        );
        assert_eq!(
            query::<i64>(
                &path,
                "SELECT rb_local_deleted FROM djmdSongMyTag WHERE ID='m1'"
            ),
            1
        );
        // The same assignment again writes nothing; fewer tags remove the rest as recorded removals.
        plan.tracks[0].expected_fingerprint = fingerprint(&path, "200");
        let repeat = run(&path, profile, &plan).unwrap();
        assert_eq!((repeat.usn, repeat.tags_replaced), (None, 0));
        plan.tracks[0].tags = Some(vec![tag("Energy", "Peak")]);
        run(&path, profile, &plan).unwrap();
        assert_eq!(
            query::<i64>(
                &path,
                "SELECT COUNT(*) FROM djmdSongMyTag WHERE ContentID='200' AND rb_local_deleted=0"
            ),
            1
        );
    }

    // ---- publication of a verified copy over a live library ----
    use crate::publish::{PublishRequest, publish_with, recover_with};

    /// A live library and a write copy made from it, with the plan applied to the copy.
    fn live_and_work() -> (tempfile::TempDir, PathBuf, PathBuf, &'static [WriteProfile]) {
        let (dir, live, profile) = database();
        let work = dir.path().join("work.db");
        std::fs::copy(&live, &work).unwrap();
        let plan = plan(&work);
        let mut db = WriteDatabase::open_with_profiles(&work, &key(), profile).unwrap();
        db.apply(&plan, &CancellationToken::default()).unwrap();
        drop(db);
        (dir, live, work, profile)
    }
    fn request<'a>(
        live: &'a Path,
        work: &'a Path,
        backups: &'a Path,
        base_usn: i64,
        key: &'a DatabaseKey,
    ) -> PublishRequest<'a> {
        PublishRequest {
            live,
            work,
            key,
            backup_dir: backups,
            assets: &[],
            share_dir: None,
            base_usn,
        }
    }
    fn publish(
        live: &Path,
        work: &Path,
        backups: &Path,
        profile: &'static [WriteProfile],
        check: &dyn Fn(&Connection) -> Result<()>,
    ) -> Result<crate::PublishReport> {
        let key = key();
        publish_with(
            &request(live, work, backups, 41, &key),
            &CancellationToken::default(),
            check,
            profile,
        )
    }

    #[test]
    fn publishing_replaces_the_live_library_after_a_verified_backup() {
        let (dir, live, work, profile) = live_and_work();
        let backups = dir.path().join("backups");
        let report = publish(&live, &work, &backups, profile, &|_| Ok(())).unwrap();
        assert_eq!((report.usn_before, report.usn_after), (41, 42));
        assert_eq!(
            query::<String>(&live, "SELECT Title FROM djmdContent WHERE ID='100'"),
            "New Title"
        );
        assert_eq!(query::<i64>(&live, "SELECT int_1 FROM agentRegistry"), 42);
        // The backup is the library as it was, and the journal describes it.
        assert_eq!(
            query::<String>(
                &report.backup,
                "SELECT Title FROM djmdContent WHERE ID='100'"
            ),
            "Old Title"
        );
        let journal: crate::PublishJournal =
            serde_json::from_slice(&std::fs::read(&report.journal).unwrap()).unwrap();
        assert_eq!(journal.state, "committed");
        assert_eq!(
            journal.backup_sha256,
            cuerithm_media::file_sha256(&report.backup).unwrap()
        );
    }

    #[test]
    fn publishing_keeps_a_wal_mode_library_in_wal_mode() {
        let (dir, live, work, profile) = live_and_work();
        let conn = Connection::open(&live).unwrap();
        set_key(&conn, &key()).unwrap();
        conn.pragma_update(None, "journal_mode", "WAL").unwrap();
        drop(conn);
        publish(&live, &work, &dir.path().join("backups"), profile, &|_| {
            Ok(())
        })
        .unwrap();
        assert_eq!(
            query::<String>(&live, "PRAGMA journal_mode").to_lowercase(),
            "wal"
        );
        assert_eq!(
            query::<String>(&live, "SELECT Title FROM djmdContent WHERE ID='100'"),
            "New Title"
        );
    }

    #[test]
    fn a_library_that_changed_or_is_in_use_is_never_touched() {
        let (dir, live, work, profile) = live_and_work();
        let backups = dir.path().join("backups");
        // rekordbox advanced the change number after the copy was made.
        let conn = Connection::open(&live).unwrap();
        set_key(&conn, &key()).unwrap();
        conn.execute("UPDATE agentRegistry SET int_1=45", [])
            .unwrap();
        drop(conn);
        let error = publish(&live, &work, &backups, profile, &|_| Ok(())).unwrap_err();
        assert!(
            error.to_string().contains("changed the library"),
            "{error:#}"
        );
        assert_eq!(
            query::<String>(&live, "SELECT Title FROM djmdContent WHERE ID='100'"),
            "Old Title"
        );
        // Held open elsewhere, as the agent would hold it.
        let conn = Connection::open(&live).unwrap();
        set_key(&conn, &key()).unwrap();
        conn.execute("UPDATE agentRegistry SET int_1=41", [])
            .unwrap();
        drop(conn);
        let held = WriteDatabase::open_with_profiles(&live, &key(), profile).unwrap();
        let error = publish(&live, &work, &backups, profile, &|_| Ok(())).unwrap_err();
        assert!(error.to_string().contains("in use"), "{error:#}");
        drop(held);
        assert!(
            !backups.exists(),
            "no backup is made for a publication that cannot start"
        );
        // A copy with no write in it, or without a verified schema, is refused too.
        let empty = dir.path().join("empty.db");
        std::fs::copy(&live, &empty).unwrap();
        assert!(
            publish(&live, &empty, &backups, profile, &|_| Ok(()))
                .unwrap_err()
                .to_string()
                .contains("no changes")
        );
        assert!(publish(&live, &work, &backups, &[], &|_| Ok(())).is_err());
    }

    #[test]
    fn a_publication_that_does_not_verify_restores_the_original_library() {
        let (dir, live, work, profile) = live_and_work();
        let backups = dir.path().join("backups");
        let error = publish(&live, &work, &backups, profile, &|_| {
            anyhow::bail!("simulated verification failure")
        })
        .unwrap_err();
        assert!(
            format!("{error:#}").contains("original library was restored"),
            "{error:#}"
        );
        assert_eq!(
            query::<String>(&live, "SELECT Title FROM djmdContent WHERE ID='100'"),
            "Old Title"
        );
        assert_eq!(query::<i64>(&live, "SELECT int_1 FROM agentRegistry"), 41);
        let journal_path = std::fs::read_dir(&backups)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path()
            .join("journal.json");
        let journal: crate::PublishJournal =
            serde_json::from_slice(&std::fs::read(journal_path).unwrap()).unwrap();
        assert_eq!(journal.state, "rolled_back");
        assert!(journal.error.unwrap().contains("simulated"));
    }

    #[test]
    fn an_interrupted_publication_is_settled_from_what_is_actually_in_the_live_library() {
        let key = key();
        let prepared = |journal_path: &Path| {
            let mut journal: crate::PublishJournal =
                serde_json::from_slice(&std::fs::read(journal_path).unwrap()).unwrap();
            journal.state = "prepared".into();
            std::fs::write(journal_path, serde_json::to_vec(&journal).unwrap()).unwrap();
        };
        // Fully copied but never marked committed: keep it.
        let (dir, live, work, profile) = live_and_work();
        let report = publish(&live, &work, &dir.path().join("b1"), profile, &|_| Ok(())).unwrap();
        prepared(&report.journal);
        let settled = recover_with(&report.journal, &key, profile).unwrap();
        assert_eq!(settled.state, "committed");
        assert_eq!(
            query::<String>(&live, "SELECT Title FROM djmdContent WHERE ID='100'"),
            "New Title"
        );
        // Never started: nothing to undo.
        let (dir, live, work, profile) = live_and_work();
        let report = publish(&live, &work, &dir.path().join("b2"), profile, &|_| Ok(())).unwrap();
        // Put the original back by hand, as if the copy never happened.
        let backup = report.backup.clone();
        std::fs::copy(&backup, &live).unwrap();
        prepared(&report.journal);
        let settled = recover_with(&report.journal, &key, profile).unwrap();
        assert_eq!(settled.state, "rolled_back");
        assert_eq!(
            query::<String>(&live, "SELECT Title FROM djmdContent WHERE ID='100'"),
            "Old Title"
        );
        // A partial or foreign state is replaced by the verified backup.
        let (dir, live, work, profile) = live_and_work();
        let report = publish(&live, &work, &dir.path().join("b3"), profile, &|_| Ok(())).unwrap();
        let conn = Connection::open(&live).unwrap();
        set_key(&conn, &key).unwrap();
        conn.execute("UPDATE agentRegistry SET int_1=7", [])
            .unwrap();
        drop(conn);
        prepared(&report.journal);
        let settled = recover_with(&report.journal, &key, profile).unwrap();
        assert_eq!(settled.state, "rolled_back");
        assert_eq!(query::<i64>(&live, "SELECT int_1 FROM agentRegistry"), 41);
        assert_eq!(
            query::<String>(&live, "SELECT Title FROM djmdContent WHERE ID='100'"),
            "Old Title"
        );
        // An already settled journal is left alone.
        assert_eq!(
            recover_with(&report.journal, &key, profile).unwrap().state,
            "rolled_back"
        );
    }

    fn rules(left: &str) -> SmartRuleSet {
        SmartRuleSet {
            match_all: false,
            conditions: vec![
                SmartCondition {
                    operator: "8".into(),
                    property: "artist".into(),
                    left: left.into(),
                    right: "".into(),
                    unit: "".into(),
                },
                SmartCondition {
                    operator: "5".into(),
                    property: "bpm".into(),
                    left: "12000".into(),
                    right: "13000".into(),
                    unit: "0".into(),
                },
            ],
        }
    }
    fn smart(left: &str) -> PlaylistWrite {
        PlaylistWrite {
            path: vec!["Sets".into(), "Peak".into()],
            kind: PlaylistKind::Smartlist,
            position: 2,
            tracks: vec![],
            rules: Some(rules(left)),
            expected_members: None,
        }
    }

    #[test]
    fn native_smart_lists_carry_their_own_id_in_the_rule_document_and_update_only_when_rules_change()
     {
        let (_dir, path, profile) = database();
        let mut plan = base_plan();
        plan.playlists = vec![smart("A & B \"x\"")];
        let report = run(&path, profile, &plan).unwrap();
        assert_eq!(report.playlists_created, 1);
        let id: String = query(&path, "SELECT ID FROM djmdPlaylist WHERE Name='Peak'");
        assert_eq!(
            query::<i64>(
                &path,
                "SELECT Attribute FROM djmdPlaylist WHERE Name='Peak'"
            ),
            4
        );
        let xml: String = query(
            &path,
            "SELECT SmartList FROM djmdPlaylist WHERE Name='Peak'",
        );
        assert_eq!(
            xml,
            format!(
                "<NODE Id=\"{id}\" AutomaticUpdate=\"0\" LogicalOperator=\"2\">\n  <CONDITION Operator=\"8\" PropertyName=\"artist\" ValueLeft=\"A &amp; B &quot;x&quot;\" ValueRight=\"\" ValueUnit=\"\"/>\n  <CONDITION Operator=\"5\" PropertyName=\"bpm\" ValueLeft=\"12000\" ValueRight=\"13000\" ValueUnit=\"0\"/>\n</NODE>"
            )
        );
        // A smart list holds no tracks.
        assert_eq!(
            query::<i64>(
                &path,
                "SELECT COUNT(*) FROM djmdSongPlaylist WHERE PlaylistID=(SELECT ID FROM djmdPlaylist WHERE Name='Peak')"
            ),
            0
        );
        // Same rules again: nothing written. Different rules: the document is replaced, the ID kept.
        let before = cuerithm_media::file_sha256(&path).unwrap();
        let again = run(&path, profile, &plan).unwrap();
        assert_eq!((again.usn, again.playlists_unchanged), (None, 1));
        assert_eq!(cuerithm_media::file_sha256(&path).unwrap(), before);
        plan.playlists = vec![smart("Other")];
        let changed = run(&path, profile, &plan).unwrap();
        assert_eq!((changed.playlists_updated, changed.usn), (1, Some(43)));
        assert_eq!(
            query::<String>(&path, "SELECT ID FROM djmdPlaylist WHERE Name='Peak'"),
            id
        );
        assert!(
            query::<String>(
                &path,
                "SELECT SmartList FROM djmdPlaylist WHERE Name='Peak'"
            )
            .contains("ValueLeft=\"Other\"")
        );
    }

    #[test]
    fn a_smart_list_without_rules_or_a_playlist_with_rules_is_malformed() {
        let (_dir, path, profile) = database();
        let mut plan = base_plan();
        let mut bad = smart("x");
        bad.rules = None;
        plan.playlists = vec![bad];
        assert!(
            run(&path, profile, &plan)
                .unwrap_err()
                .to_string()
                .contains("smart list needs")
        );
        let mut also_bad = friday(&["100"], None);
        also_bad.rules = Some(rules("x"));
        plan.playlists = vec![also_bad];
        assert!(
            run(&path, profile, &plan)
                .unwrap_err()
                .to_string()
                .contains("smart list needs")
        );
        let mut folder_with_tracks = smart("x");
        folder_with_tracks.tracks = vec!["100".into()];
        plan.playlists = vec![folder_with_tracks];
        assert!(run(&path, profile, &plan).is_err());
    }

    fn new_track(key: &str, path: &str) -> NewTrack {
        NewTrack {
            key: key.into(),
            folder_path: path.into(),
            file_name: path.rsplit('/').next().unwrap().into(),
            fields: ContentFields {
                title: Some("Brand New".into()),
                artist: Some("Fresh Artist".into()),
                genre: Some("Techno".into()),
                key: Some("8A".into()),
                bpm_hundredths: Some(12800),
                release_year: Some(2024),
                rating: Some(3),
                ..Default::default()
            },
            length_seconds: 200,
            bit_rate: Some(320),
            sample_rate: Some(44100),
            file_size: Some(8_000_000),
            file_type: 1,
            date_added: "2026-10-20".into(),
            color_id: Some(5),
            first_beat_seconds: Some(0.5),
            cues: vec![CueRow {
                in_msec: 1000,
                out_msec: -1,
                kind: 1,
                active_loop: false,
                beat_loop_size: None,
                color: Some(-1),
                color_table_index: Some(41),
                comment: Some("Start".into()),
            }],
            tags: vec![MyTagRef {
                category: "Mood".into(),
                tag: "Dark".into(),
            }],
        }
    }

    #[test]
    fn a_new_track_gets_ids_foreign_rows_cues_tags_a_file_record_and_an_analysis_asset() {
        let (_dir, path, profile) = database();
        let mut plan = base_plan();
        plan.share_dir = Some("/lib/share/".into());
        plan.new_tracks = vec![new_track("n1", "/music/new one.mp3")];
        plan.playlists = vec![PlaylistWrite {
            path: vec!["Sets".into(), "Friday".into()],
            kind: PlaylistKind::Playlist,
            position: 1,
            tracks: vec!["100".into(), "new:n1".into()],
            rules: None,
            expected_members: None,
        }];
        let report = run(&path, profile, &plan).unwrap();
        assert_eq!((report.tracks_added, report.usn), (1, Some(42)));
        let added = &report.new_tracks[0];
        assert_eq!(
            (added.key.as_str(), added.content_id.as_str()),
            ("n1", "201")
        );
        assert_eq!(added.master_song_id, "201");
        assert!(
            added.analysis_path.starts_with("/PIONEER/USBANLZ/")
                && added.analysis_path.ends_with("/ANLZ0000.DAT")
        );
        let value = |sql: &str| {
            query::<String>(
                &path,
                &format!("SELECT {sql} FROM djmdContent WHERE ID='201'"),
            )
        };
        let number = |sql: &str| {
            query::<i64>(
                &path,
                &format!("SELECT {sql} FROM djmdContent WHERE ID='201'"),
            )
        };
        assert_eq!(value("Title"), "Brand New");
        assert_eq!(value("FolderPath"), "/music/new one.mp3");
        assert_eq!(value("OrgFolderPath"), "/music/new one.mp3");
        assert_eq!(value("FileNameL"), "new one.mp3");
        assert_eq!(value("MasterDBID"), "db");
        assert_eq!(value("DeviceID"), "dev");
        assert_eq!(value("UUID"), added.uuid);
        assert_eq!(
            (
                number("BPM"),
                number("Length"),
                number("FileType"),
                number("Rating")
            ),
            (12800, 200, 1, 3)
        );
        // New tracks wait for rekordbox to analyse them (16), and carry the sync bookkeeping.
        assert_eq!(
            (
                number("Analysed"),
                number("rb_data_status"),
                number("rb_local_usn")
            ),
            (16, 0, 42)
        );
        assert_eq!(value("AnalysisDataPath"), added.analysis_path);
        assert_eq!(value("ColorID"), "5");
        assert_eq!(value("HotCueAutoLoad"), "on");
        assert_eq!(
            query::<String>(
                &path,
                "SELECT Name FROM djmdArtist WHERE ID=(SELECT ArtistID FROM djmdContent WHERE ID='201')"
            ),
            "Fresh Artist"
        );
        assert_eq!(
            query::<String>(
                &path,
                "SELECT ScaleName FROM djmdKey WHERE ID=(SELECT KeyID FROM djmdContent WHERE ID='201')"
            ),
            "8A"
        );
        // Cues, the MyTag, and the file record.
        assert_eq!(
            query::<i64>(&path, "SELECT COUNT(*) FROM djmdCue WHERE ContentID='201'"),
            1
        );
        assert_eq!(
            query::<String>(&path, "SELECT ID FROM contentCue WHERE ContentID='201'"),
            added.uuid
        );
        assert_eq!(
            query::<i64>(
                &path,
                "SELECT COUNT(*) FROM djmdSongMyTag WHERE ContentID='201' AND rb_local_deleted=0"
            ),
            1
        );
        let file_id: String = query(&path, "SELECT ID FROM contentFile WHERE ContentID='201'");
        assert!(
            file_id.starts_with(&format!("{}_%2FPIONEER%2FUSBANLZ%2F", added.uuid)),
            "{file_id}"
        );
        assert_eq!(
            query::<String>(
                &path,
                "SELECT rb_local_path FROM contentFile WHERE ContentID='201'"
            ),
            format!("/lib/share{}", added.analysis_path)
        );
        // The playlist refers to the new track by its new ID, in order.
        assert_eq!(
            query::<String>(
                &path,
                "SELECT group_concat(ContentID,',') FROM (SELECT ContentID FROM djmdSongPlaylist WHERE PlaylistID='51' AND rb_local_deleted=0 ORDER BY TrackNo)"
            ),
            "100,201"
        );
        // The analysis file is returned for publication, and parses with the beat grid.
        let asset = &report.assets[0];
        assert_eq!(format!("/{}", asset.relative_path), added.analysis_path);
        let file = crate::anlz::AnlzFile::parse(&asset.bytes, Default::default()).unwrap();
        assert_eq!(file.tags()[0].audio_path().unwrap(), "?/new one.mp3");
        assert_eq!(file.tags()[2].beats().unwrap()[0].tempo, 12800);
    }

    #[test]
    fn a_file_already_in_rekordbox_or_an_unknown_new_key_refuses_the_plan() {
        let (_dir, path, profile) = database();
        let mut plan = base_plan();
        plan.new_tracks = vec![new_track("n1", "/music/existing-100.mp3")];
        let error = run(&path, profile, &plan)
            .err()
            .expect("the plan is refused");
        assert!(
            error.downcast_ref::<ConflictError>().unwrap().0[0]
                .reason
                .contains("already in rekordbox")
        );
        plan.new_tracks = vec![new_track("n1", "/music/other.mp3")];
        plan.playlists = vec![friday(&["new:missing"], None)];
        let error = run(&path, profile, &plan)
            .err()
            .expect("the plan is refused");
        assert!(
            error.downcast_ref::<ConflictError>().unwrap().0[0]
                .reason
                .contains("not in the plan")
        );
        plan.playlists = vec![];
        plan.new_tracks = vec![
            new_track("n1", "/music/a.mp3"),
            new_track("n1", "/music/b.mp3"),
        ];
        assert!(
            run(&path, profile, &plan)
                .unwrap_err()
                .to_string()
                .contains("distinct keys")
        );
        plan.new_tracks = vec![new_track("n1", "relative/path.mp3")];
        assert!(
            run(&path, profile, &plan)
                .unwrap_err()
                .to_string()
                .contains("absolute file path")
        );
        assert_eq!(query::<i64>(&path, "SELECT COUNT(*) FROM djmdContent"), 2);
        assert_eq!(query::<i64>(&path, "SELECT int_1 FROM agentRegistry"), 41);
    }

    #[test]
    fn uri_component_encoding_matches_javascript() {
        assert_eq!(
            encode_uri_component("/PIONEER/USBANLZ/abc/def/ANLZ0000.DAT"),
            "%2FPIONEER%2FUSBANLZ%2Fabc%2Fdef%2FANLZ0000.DAT"
        );
        assert_eq!(encode_uri_component("a b~c'(d)*!é"), "a%20b~c'(d)*!%C3%A9");
    }

    // ---- publishing assets with the database ----

    /// A live library and a write copy that adds one track, with the analysis file the writer returned.
    fn live_and_work_with_new_track() -> (
        tempfile::TempDir,
        PathBuf,
        PathBuf,
        &'static [WriteProfile],
        Vec<AssetWrite>,
    ) {
        let (dir, live, profile) = database();
        let work = dir.path().join("work.db");
        std::fs::copy(&live, &work).unwrap();
        let mut plan = base_plan();
        plan.new_tracks = vec![new_track("n1", "/music/published.mp3")];
        let mut db = WriteDatabase::open_with_profiles(&work, &key(), profile).unwrap();
        let report = db.apply(&plan, &CancellationToken::default()).unwrap();
        drop(db);
        (dir, live, work, profile, report.assets)
    }
    fn publish_with_assets(
        live: &Path,
        work: &Path,
        backups: &Path,
        share: &Path,
        assets: &[AssetWrite],
        profile: &'static [WriteProfile],
        check: &dyn Fn(&Connection) -> Result<()>,
    ) -> Result<crate::PublishReport> {
        let key = key();
        publish_with(
            &PublishRequest {
                live,
                work,
                key: &key,
                backup_dir: backups,
                assets,
                share_dir: Some(share),
                base_usn: 41,
            },
            &CancellationToken::default(),
            check,
            profile,
        )
    }

    #[test]
    fn assets_are_created_with_the_database_change_and_recorded_in_the_journal() {
        let (dir, live, work, profile, assets) = live_and_work_with_new_track();
        let share = dir.path().join("share");
        let report = publish_with_assets(
            &live,
            &work,
            &dir.path().join("b"),
            &share,
            &assets,
            profile,
            &|_| Ok(()),
        )
        .unwrap();
        let file = share.join(&assets[0].relative_path);
        assert_eq!(std::fs::read(&file).unwrap(), assets[0].bytes);
        assert_eq!(
            query::<String>(&live, "SELECT Title FROM djmdContent WHERE ID='201'"),
            "Brand New"
        );
        let journal: crate::PublishJournal =
            serde_json::from_slice(&std::fs::read(&report.journal).unwrap()).unwrap();
        assert_eq!(
            (journal.state.as_str(), journal.assets.len()),
            ("committed", 1)
        );
        assert_eq!(journal.assets[0].path, file);
    }

    #[test]
    fn an_asset_that_already_exists_or_escapes_the_share_folder_stops_the_publication_untouched() {
        let (dir, live, work, profile, assets) = live_and_work_with_new_track();
        let share = dir.path().join("share");
        let existing = share.join(&assets[0].relative_path);
        std::fs::create_dir_all(existing.parent().unwrap()).unwrap();
        std::fs::write(&existing, b"someone else's file").unwrap();
        let error = publish_with_assets(
            &live,
            &work,
            &dir.path().join("b"),
            &share,
            &assets,
            profile,
            &|_| Ok(()),
        )
        .unwrap_err();
        assert!(error.to_string().contains("already exists"), "{error:#}");
        assert_eq!(std::fs::read(&existing).unwrap(), b"someone else's file");
        assert_eq!(query::<i64>(&live, "SELECT COUNT(*) FROM djmdContent"), 2);
        assert_eq!(query::<i64>(&live, "SELECT int_1 FROM agentRegistry"), 41);
        let escaping = [AssetWrite {
            relative_path: "../outside.dat".into(),
            bytes: vec![1],
        }];
        let error = publish_with_assets(
            &live,
            &work,
            &dir.path().join("b"),
            &share,
            &escaping,
            profile,
            &|_| Ok(()),
        )
        .unwrap_err();
        assert!(
            error.to_string().contains("inside the share folder"),
            "{error:#}"
        );
        assert!(!dir.path().join("outside.dat").exists());
    }

    #[test]
    fn a_failed_verification_restores_the_library_and_removes_the_files_it_created() {
        let (dir, live, work, profile, assets) = live_and_work_with_new_track();
        let share = dir.path().join("share");
        let error = publish_with_assets(
            &live,
            &work,
            &dir.path().join("b"),
            &share,
            &assets,
            profile,
            &|_| anyhow::bail!("simulated"),
        )
        .unwrap_err();
        assert!(format!("{error:#}").contains("restored"));
        assert_eq!(query::<i64>(&live, "SELECT COUNT(*) FROM djmdContent"), 2);
        assert!(!share.join(&assets[0].relative_path).exists());
        // The folders it made are gone too, and unrelated files in the share folder are untouched.
        assert!(
            !share.join("PIONEER/USBANLZ").exists()
                || std::fs::read_dir(share.join("PIONEER/USBANLZ"))
                    .unwrap()
                    .next()
                    .is_none()
        );
    }

    #[test]
    fn recovering_an_interrupted_publication_removes_its_files_unless_it_committed() {
        let key = key();
        let prepared = |journal_path: &Path| {
            let mut journal: crate::PublishJournal =
                serde_json::from_slice(&std::fs::read(journal_path).unwrap()).unwrap();
            journal.state = "prepared".into();
            std::fs::write(journal_path, serde_json::to_vec(&journal).unwrap()).unwrap();
        };
        // Committed in the database but not marked: the files stay with it.
        let (dir, live, work, profile, assets) = live_and_work_with_new_track();
        let share = dir.path().join("share");
        let report = publish_with_assets(
            &live,
            &work,
            &dir.path().join("b"),
            &share,
            &assets,
            profile,
            &|_| Ok(()),
        )
        .unwrap();
        prepared(&report.journal);
        assert_eq!(
            recover_with(&report.journal, &key, profile).unwrap().state,
            "committed"
        );
        assert!(share.join(&assets[0].relative_path).exists());
        // The database never changed: the files that were already written are removed.
        let (dir, live, work, profile, assets) = live_and_work_with_new_track();
        let share = dir.path().join("share");
        let report = publish_with_assets(
            &live,
            &work,
            &dir.path().join("b"),
            &share,
            &assets,
            profile,
            &|_| Ok(()),
        )
        .unwrap();
        std::fs::copy(&report.backup, &live).unwrap();
        prepared(&report.journal);
        assert_eq!(
            recover_with(&report.journal, &key, profile).unwrap().state,
            "rolled_back"
        );
        assert!(!share.join(&assets[0].relative_path).exists());
        // A file that is no longer the one written is never removed.
        let (dir, live, work, profile, assets) = live_and_work_with_new_track();
        let share = dir.path().join("share");
        let report = publish_with_assets(
            &live,
            &work,
            &dir.path().join("b"),
            &share,
            &assets,
            profile,
            &|_| Ok(()),
        )
        .unwrap();
        std::fs::copy(&report.backup, &live).unwrap();
        std::fs::write(
            share.join(&assets[0].relative_path),
            b"rekordbox rewrote this",
        )
        .unwrap();
        prepared(&report.journal);
        recover_with(&report.journal, &key, profile).unwrap();
        assert_eq!(
            std::fs::read(share.join(&assets[0].relative_path)).unwrap(),
            b"rekordbox rewrote this"
        );
    }
}
