use crate::{model::*, *};
use anyhow::{Context, Result, ensure};
use camelot_core::FileAnalysis;
use cuerithm_media::{MediaInfo, file_sha256};
use rusqlite::{OptionalExtension, params};
use std::path::PathBuf;

impl Library {
    pub fn requeue_unfinished(&self, revision: &SettingsRevision) -> Result<usize> {
        self.settings_revision(revision)?;
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let rows: Vec<(Vec<u8>, String)> = {
            let mut stmt = tx.prepare(
                "SELECT path,source_hash FROM tracks WHERE state='incoming' AND archived=0",
            )?;
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<_>>()?
        };
        for (path, hash) in &rows {
            Self::enqueue_one(&tx, &bytes_path(path)?, hash, revision)?;
        }
        tx.commit()?;
        Ok(rows.len())
    }
    pub fn enqueue(&self, paths: &[PathBuf], revision: SettingsRevision) -> Result<Vec<JobId>> {
        self.settings_revision(&revision)?;
        let mut ids = Vec::new();
        for path in paths {
            let path = path.canonicalize().context("Incoming file is missing")?;
            ensure!(path.is_file(), "Only audio files can be queued");
            let hash = file_sha256(&path)?;
            let mut conn = self.conn()?;
            let tx = conn.transaction()?;
            ids.push(Self::enqueue_one(&tx, &path, &hash, &revision)?);
            tx.commit()?;
        }
        Ok(ids)
    }
    pub(crate) fn enqueue_one(
        tx: &rusqlite::Transaction<'_>,
        path: &std::path::Path,
        hash: &str,
        revision: &str,
    ) -> Result<JobId> {
        let encoded = path_bytes(path);
        let existing_track: Option<(String, Option<String>, bool)> = tx
            .query_row(
                "SELECT id,current_job,archived FROM tracks WHERE path=?1",
                params![encoded],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let (track, current, archived) =
            existing_track.unwrap_or_else(|| (uuid::Uuid::new_v4().to_string(), None, false));
        if archived {
            return current.context("Archived track has no preparation identity");
        }
        let title = path.file_stem().unwrap_or_default().to_string_lossy();
        tx.execute("INSERT OR IGNORE INTO tracks (id,path,title,source_hash,created,updated,search_text,sort_title) VALUES (?1,?2,?3,?4,?5,?5,?6,?6)", params![track,encoded,title,hash,now(),title.to_lowercase()])?;
        let existing: Option<(String,String)> = tx.query_row(
            "SELECT id,state FROM jobs WHERE track_id=?1 AND source_hash=?2 AND settings_revision=?3",
            params![track,hash,revision], |r| Ok((r.get(0)?,r.get(1)?))).optional()?;
        let id = if let Some((id, state)) = existing {
            if current.as_deref() == Some(id.as_str()) && state != "superseded" {
                return Ok(id);
            }
            tx.execute(
                "UPDATE jobs SET state='queued',stage='queued',error=NULL,updated=?1 WHERE id=?2",
                params![now(), id],
            )?;
            id
        } else {
            let id = uuid::Uuid::new_v4().to_string();
            tx.execute("INSERT INTO jobs(id,track_id,source_hash,settings_revision,state,stage,created,updated) VALUES(?1,?2,?3,?4,'queued','queued',?5,?5)", params![id,track,hash,revision,now()])?;
            id
        };
        tx.execute("UPDATE jobs SET state='superseded' WHERE track_id=?1 AND id!=?2 AND state IN ('queued','running','attention')", params![track,id])?;
        tx.execute("UPDATE tracks SET state='incoming',source_hash=?1,current_job=?2,updated=?3 WHERE id=?4", params![hash,id,now(),track])?;
        Self::bump(tx)?;
        Ok(id)
    }
    pub fn claim_next(&self) -> Result<Option<JobRun>> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let row=tx.query_row("SELECT j.id,j.track_id,t.path,j.source_hash,j.settings_revision FROM jobs j JOIN tracks t ON t.current_job=j.id WHERE j.state='queued' AND t.archived=0 ORDER BY j.created,j.id LIMIT 1",[],
            |r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,Vec<u8>>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?))).optional()?;
        let Some((id, track_id, path, source_sha256, settings_revision)) = row else {
            return Ok(None);
        };
        let text: String = tx.query_row(
            "SELECT json FROM settings WHERE revision=?1",
            [&settings_revision],
            |r| r.get(0),
        )?;
        let snapshot: SettingsSnapshot = serde_json::from_str(&text)?;
        tx.execute("UPDATE jobs SET state='running',stage='waiting',attempts=attempts+1,updated=?1 WHERE id=?2",params![now(),id])?;
        tx.commit()?;
        Ok(Some(JobRun {
            id,
            track_id,
            path: bytes_path(&path)?,
            source_sha256,
            settings_revision,
            snapshot,
        }))
    }
    pub fn jobs(&self) -> Result<Vec<JobSummary>> {
        let conn = self.conn()?;
        let mut stmt=conn.prepare("SELECT j.id,j.track_id,t.title,j.state,j.stage,j.error,j.attempts FROM jobs j JOIN tracks t ON t.current_job=j.id WHERE j.state!='superseded' ORDER BY j.created DESC LIMIT 1000")?;
        Ok(stmt
            .query_map([], |r| {
                Ok(JobSummary {
                    id: r.get(0)?,
                    track_id: r.get(1)?,
                    title: r.get(2)?,
                    state: r.get(3)?,
                    stage: r.get(4)?,
                    error: r.get(5)?,
                    attempts: r.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?)
    }
    pub fn set_stage(&self, id: &str, stage: &str) -> Result<()> {
        ensure!(
            matches!(stage, "waiting" | "decoding" | "analysis" | "committing"),
            "Unknown preparation stage"
        );
        let conn = self.conn()?;
        ensure!(
            conn.execute(
                "UPDATE jobs SET stage=?1,updated=?2 WHERE id=?3 AND state='running'",
                params![stage, now(), id]
            )? == 1,
            "Preparation was superseded"
        );
        Ok(())
    }
    pub fn fail(&self, id: &str, stage: &str, error: &str) -> Result<()> {
        let conn = self.conn()?;
        ensure!(conn.execute("UPDATE jobs SET state='attention',stage=?1,error=?2,updated=?3 WHERE id=?4 AND state='running'",params![stage,error,now(),id])?==1,"Preparation is no longer running");
        Ok(())
    }
    pub fn retry(&self, id: &str, revision: Option<SettingsRevision>) -> Result<JobId> {
        let (track, old_revision): (String, String) = self.conn()?.query_row(
            "SELECT track_id,settings_revision FROM jobs WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let path = self.track_path(&track)?;
        let next = self.enqueue(&[path], revision.unwrap_or(old_revision))?[0].clone();
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure!(
            !tx.query_row("SELECT archived FROM tracks WHERE id=?1", [&track], |r| {
                r.get::<_, bool>(0)
            })?,
            "Restore archived tracks before preparing them"
        );
        tx.execute(
            "UPDATE jobs SET state='queued',stage='queued',error=NULL WHERE id=?1",
            [&next],
        )?;
        tx.execute(
            "UPDATE tracks SET state='incoming',current_job=?1,updated=?2 WHERE id=?3",
            params![next, now(), track],
        )?;
        Self::bump(&tx)?;
        tx.commit()?;
        Ok(next)
    }
    pub fn apply_analysis(
        &self,
        id: &str,
        result: &FileAnalysis,
        info: &MediaInfo,
        source_sha: &str,
    ) -> Result<()> {
        let a = &result.analysis;
        ensure!(
            result.complete && a.duration_seconds.is_finite() && a.duration_seconds > 0.,
            "Incomplete track analysis"
        );
        let key = a.key.as_ref().context("Key is unresolved")?;
        let tempo = a.tempo.as_ref().context("BPM and grid are unresolved")?;
        let energy = a.energy.as_ref().context("Energy is unresolved")?;
        ensure!(
            (1..=10).contains(&energy.level) && tempo.bpm.is_finite() && tempo.bpm > 0.,
            "Invalid analysis values"
        );
        ensure!(
            !tempo.beats_seconds.is_empty() && tempo.beats_seconds.iter().all(|x| x.is_finite()),
            "Invalid beatgrid"
        );
        let cues = a
            .lexicon_cues
            .as_ref()
            .context("Cue analysis has not run")?;
        ensure!(
            !cues.cues.is_empty()
                && cues.cues.iter().all(|c| c.time_seconds.is_finite()
                    && c.time_seconds >= 0.
                    && c.time_seconds < a.duration_seconds),
            "Invalid replacement cues"
        );
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let (track,expected,state,settings,current,path):(String,String,String,String,String,Vec<u8>)=tx.query_row(
            "SELECT j.track_id,j.source_hash,j.state,s.json,t.current_job,t.path FROM jobs j JOIN tracks t ON t.id=j.track_id JOIN settings s ON s.revision=j.settings_revision WHERE j.id=?1",
            [id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?)))?;
        ensure!(
            state == "running" && current == id,
            "Preparation is no longer current"
        );
        ensure!(
            expected == source_sha && file_sha256(&bytes_path(&path)?)? == expected,
            "Source changed before commit"
        );
        let snapshot: SettingsSnapshot = serde_json::from_str(&settings)?;
        ensure!(
            !snapshot.settings.autotag_enabled && !snapshot.settings.cleanup_enabled,
            "Required metadata stages have not completed"
        );
        let json = serde_json::to_string(result)?;
        let _: FileAnalysis =
            serde_json::from_str(&json).context("Analysis contains invalid numeric values")?;
        tx.execute(
            "INSERT INTO cue_revisions VALUES (?1,?2,?3,?4,?5)",
            params![
                uuid::Uuid::new_v4().to_string(),
                track,
                id,
                serde_json::to_string(cues)?,
                now()
            ],
        )?;
        tx.execute("UPDATE tracks SET state='ready',analysis_json=?1,metadata_json=?2,musical_key=?3,bpm=?4,energy=?5,duration=?6,updated=?7 WHERE id=?8",
            params![json,serde_json::to_string(info)?,key.camelot,tempo.bpm,energy.level,a.duration_seconds,now(),track])?;
        for (field, value) in &info.tags {
            let column = match field.as_str() {
                "title" => "title",
                "artist" => "artist",
                "album" => "album",
                "genre" => "genre",
                _ => continue,
            };
            tx.execute(
                &format!("UPDATE tracks SET {column}=?1 WHERE id=?2"),
                params![value, track],
            )?;
        }
        let (title, artist, genre): (String, String, String) = tx.query_row(
            "SELECT title,artist,genre FROM tracks WHERE id=?1",
            [&track],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        tx.execute(
            "UPDATE tracks SET search_text=?1,sort_title=?2,sort_artist=?3 WHERE id=?4",
            params![
                format!("{title} {artist} {genre}").to_lowercase(),
                title.to_lowercase(),
                artist.to_lowercase(),
                track
            ],
        )?;
        tx.execute(
            "UPDATE jobs SET state='completed',stage='completed',error=NULL,updated=?1 WHERE id=?2",
            params![now(), id],
        )?;
        Self::bump(&tx)?;
        tx.commit()?;
        Ok(())
    }
}
