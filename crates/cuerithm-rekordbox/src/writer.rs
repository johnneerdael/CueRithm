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
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WritePlan {
    pub tracks: Vec<TrackWrite>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct WriteReport {
    /// The change number used for every touched row; `None` when nothing needed writing.
    pub usn: Option<i64>,
    pub tracks_updated: usize,
    pub tracks_unchanged: usize,
    pub cues_replaced: usize,
    pub foreign_created: BTreeMap<String, usize>,
    pub foreign_revived: BTreeMap<String, usize>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrackConflict {
    pub content_id: String,
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
                    content_id: track.content_id.clone(),
                    reason: "The track no longer exists in rekordbox".into(),
                }),
                Some((fingerprint, values)) => {
                    if matches!(
                        values.get("rb_local_deleted"),
                        Some(TargetValue::Integer(1))
                    ) {
                        conflicts.push(TrackConflict {
                            content_id: track.content_id.clone(),
                            reason: "The track was removed in rekordbox".into(),
                        });
                    } else if fingerprint != track.expected_fingerprint {
                        conflicts.push(TrackConflict {
                            content_id: track.content_id.clone(),
                            reason: "The track changed in rekordbox after it was last read".into(),
                        });
                    }
                }
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
        INSERT INTO djmdProperty VALUES('db','dev');
        INSERT INTO agentRegistry VALUES('localUpdateCount',41);
        INSERT INTO djmdArtist VALUES('7','Old Artist','ua',256,0,0,0,40,'t','t');
        INSERT INTO djmdArtist VALUES('9','Revived Artist','ur',256,0,1,0,40,'t','t');
        INSERT INTO djmdContent(ID,UUID,Title,Commnt,BPM,ArtistID,CueUpdated,TrackInfoUpdated,rb_data_status,rb_local_usn,updated_at)
          VALUES('100','track-uuid','Old Title','keep me',12800,'7','1','2',256,40,'t');
        INSERT INTO djmdContent(ID,UUID,Title,rb_data_status,rb_local_usn,updated_at) VALUES('200','other-uuid','Untouched',256,40,'t');
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
            }],
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
        assert_eq!(conflict.0[0].content_id, "100");
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
                },
                TrackWrite {
                    content_id: "999".into(),
                    expected_fingerprint: "x".into(),
                    fields: ContentFields::default(),
                    cues: None,
                },
            ],
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
}
