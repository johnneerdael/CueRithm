use crate::{model::*, *};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
};

pub(crate) struct Inner {
    pub conn: Mutex<Connection>,
    pub _lease: File,
    pub root: PathBuf,
}
#[derive(Clone)]
pub struct Library {
    pub(crate) inner: Arc<Inner>,
}
impl Library {
    pub fn open(path: &Path) -> Result<Self> {
        let root = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        std::fs::create_dir_all(root)?;
        let mut lock_path = path.as_os_str().to_owned();
        lock_path.push(".lock");
        let lease = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)?;
        fs2::FileExt::try_lock_exclusive(&lease).context("This library is already open")?;
        let conn = Connection::open(path)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        ensure!(
            (0..=1).contains(&version),
            "Library schema is newer than this application"
        );
        if version == 0 {
            let tables:i64=conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",[],|r|r.get(0))?;
            ensure!(tables == 0, "This is not a Cuerithm library");
            conn.execute_batch(&format!(
                "BEGIN IMMEDIATE;\n{}\nCOMMIT;",
                include_str!("../migrations/001_initial.sql")
            ))?;
        }
        let app: i64 = conn.query_row("PRAGMA application_id", [], |r| r.get(0))?;
        ensure!(app == 1129665093, "This is not a Cuerithm library");
        conn.pragma_update(None, "foreign_keys", true)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.execute(
            "UPDATE jobs SET state='queued',stage='queued',error=NULL WHERE state='running'",
            [],
        )?;
        let lib = Self {
            inner: Arc::new(Inner {
                conn: Mutex::new(conn),
                _lease: lease,
                root: root.to_path_buf(),
            }),
        };
        let count = lib
            .conn()?
            .query_row("SELECT COUNT(*) FROM settings", [], |r| r.get::<_, i64>(0))?;
        if count == 0 {
            lib.save_settings(PreparationSettings::default())?;
        }
        Ok(lib)
    }
    pub(crate) fn conn(&self) -> Result<MutexGuard<'_, Connection>> {
        self.inner
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("Library connection interrupted"))
    }
    pub fn data_dir(&self) -> &Path {
        &self.inner.root
    }
    pub fn save_settings(&self, settings: PreparationSettings) -> Result<SettingsRevision> {
        self.save_settings_migrating(settings, &[])
    }
    pub fn save_settings_migrating(
        &self,
        settings: PreparationSettings,
        obsolete: &[SettingsRevision],
    ) -> Result<SettingsRevision> {
        settings.validate()?;
        let models = camelot_core::Engine::new(&settings.engine)?.model_identity();
        let revision = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(
                &json!({"settings":settings,"models":models})
            )?)
        );
        let snapshot = SettingsSnapshot {
            revision: revision.clone(),
            settings,
            models,
        };
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT OR IGNORE INTO settings VALUES (?1,?2,?3)",
            params![revision, serde_json::to_string(&snapshot)?, now()],
        )?;
        tx.execute(
            "INSERT OR REPLACE INTO selected_settings VALUES (1,?1)",
            params![revision],
        )?;
        Self::requeue_unfinished_tx(&tx, &revision, Some(obsolete))?;
        tx.commit()?;
        Ok(revision)
    }
    pub fn unfinished_settings(&self) -> Result<Vec<SettingsSnapshot>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare("SELECT DISTINCT s.json FROM settings s JOIN jobs j ON j.settings_revision=s.revision JOIN tracks t ON t.current_job=j.id WHERE t.state='incoming' AND t.archived=0")?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.into_iter()
            .map(|s| serde_json::from_str(&s).map_err(Into::into))
            .collect()
    }
    pub fn settings(&self) -> Result<SettingsSnapshot> {
        let conn = self.conn()?;
        let text:String=conn.query_row("SELECT s.json FROM settings s JOIN selected_settings a ON s.revision=a.revision WHERE a.singleton=1",[],|r|r.get(0))?;
        Ok(serde_json::from_str(&text)?)
    }
    pub fn settings_revision(&self, id: &str) -> Result<SettingsSnapshot> {
        let text: String =
            self.conn()?
                .query_row("SELECT json FROM settings WHERE revision=?1", [id], |r| {
                    r.get(0)
                })?;
        Ok(serde_json::from_str(&text)?)
    }
    pub fn revision(&self) -> Result<u64> {
        Ok(self
            .conn()?
            .query_row("SELECT value FROM meta WHERE key='revision'", [], |r| {
                r.get(0)
            })?)
    }
    pub(crate) fn bump(conn: &Connection) -> Result<()> {
        conn.execute("UPDATE meta SET value=value+1 WHERE key='revision'", [])?;
        Ok(())
    }
    pub fn tracks(&self, query: TrackQuery) -> Result<TrackPage> {
        ensure!((1..=500).contains(&query.limit), "Invalid page size");
        let conn = self.conn()?;
        let revision: u64 =
            conn.query_row("SELECT value FROM meta WHERE key='revision'", [], |r| {
                r.get(0)
            })?;
        let query_id = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&json!([
                query.view,
                query.search,
                query.sort,
                query.descending,
                query.limit
            ]))?)
        );
        let offset: u64 = if let Some(cursor) = &query.cursor {
            let parts: Vec<&str> = cursor.split(':').collect();
            ensure!(
                parts.len() == 3 && parts[0] == revision.to_string() && parts[2] == query_id,
                "Stale cursor; refresh the library"
            );
            parts[1].parse()?
        } else {
            0
        };
        let condition = match query.view {
            TrackView::Tracks => "state='ready' AND archived=0",
            TrackView::Incoming => "state='incoming' AND archived=0",
            TrackView::Archive => "archived=1",
        };
        let column = match query.sort {
            TrackSort::Title => "sort_title",
            TrackSort::Artist => "sort_artist",
            TrackSort::Bpm => "bpm",
            TrackSort::Energy => "energy",
            TrackSort::Added => "created",
        };
        let direction = if query.descending { "DESC" } else { "ASC" };
        let filter = format!("{condition} AND instr(search_text,?1)>0");
        let needle = query.search.to_lowercase();
        let total: u64 = conn.query_row(
            &format!("SELECT COUNT(*) FROM tracks WHERE {filter}"),
            [&needle],
            |r| r.get(0),
        )?;
        let mut stmt=conn.prepare(&format!("SELECT id,path,title,artist,album,genre,state,musical_key,bpm,energy,duration,source_hash FROM tracks WHERE {filter} ORDER BY {column} {direction},id ASC LIMIT ?2 OFFSET ?3"))?;
        let rows = stmt
            .query_map(params![needle, query.limit, offset], summary_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let next = offset + rows.len() as u64;
        Ok(TrackPage {
            rows,
            total,
            revision,
            next_cursor: (next < total).then(|| format!("{revision}:{next}:{query_id}")),
        })
    }
    pub fn track_ids(&self, query: TrackQuery) -> Result<Vec<TrackId>> {
        let condition = match query.view {
            TrackView::Tracks => "state='ready' AND archived=0",
            TrackView::Incoming => "state='incoming' AND archived=0",
            TrackView::Archive => "archived=1",
        };
        let conn = self.conn()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT id FROM tracks WHERE {condition} AND instr(search_text,?1)>0 ORDER BY id"
        ))?;
        Ok(stmt
            .query_map([query.search.to_lowercase()], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?)
    }
    pub fn track(&self, id: &str) -> Result<TrackSummary> {
        Ok(self.conn()?.query_row("SELECT id,path,title,artist,album,genre,state,musical_key,bpm,energy,duration,source_hash FROM tracks WHERE id=?1",[id],summary_row)?)
    }
    pub fn source_hash(&self, id: &str) -> Result<String> {
        Ok(self
            .conn()?
            .query_row("SELECT source_hash FROM tracks WHERE id=?1", [id], |r| {
                r.get(0)
            })?)
    }
    pub fn counts(&self) -> Result<LibraryCounts> {
        Ok(self.conn()?.query_row("SELECT COUNT(CASE WHEN state='ready' AND archived=0 THEN 1 END),COUNT(CASE WHEN state='incoming' AND archived=0 THEN 1 END),COUNT(CASE WHEN archived=1 THEN 1 END) FROM tracks",[],|r|Ok(LibraryCounts{tracks:r.get(0)?,incoming:r.get(1)?,archive:r.get(2)?}))?)
    }
    pub fn track_path(&self, id: &str) -> Result<PathBuf> {
        let bytes: Vec<u8> =
            self.conn()?
                .query_row("SELECT path FROM tracks WHERE id=?1", [id], |r| r.get(0))?;
        bytes_path(&bytes)
    }
    pub fn analysis(&self, id: &str) -> Result<Option<camelot_core::FileAnalysis>> {
        let text: Option<String> = self
            .conn()?
            .query_row("SELECT analysis_json FROM tracks WHERE id=?1", [id], |r| {
                r.get(0)
            })
            .optional()?
            .flatten();
        text.map(|s| serde_json::from_str(&s).map_err(Into::into))
            .transpose()
    }
    pub fn cue_revision_count(&self, id: &str) -> Result<u64> {
        Ok(self.conn()?.query_row(
            "SELECT COUNT(*) FROM cue_revisions WHERE track_id=?1",
            [id],
            |r| r.get(0),
        )?)
    }

    pub fn set_archived(&self, id: &str, archived: bool) -> Result<()> {
        let restore = if archived {
            None
        } else {
            let path = self.track_path(id)?;
            let hash = cuerithm_media::file_sha256(&path);
            Some((path, hash, self.settings()?.revision))
        };
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure!(
            tx.execute(
                "UPDATE tracks SET archived=?1,updated=?2 WHERE id=?3",
                params![archived, now(), id]
            )? == 1,
            "Track is missing"
        );
        if archived {
            tx.execute("UPDATE jobs SET state='superseded' WHERE track_id=?1 AND state IN ('queued','running','attention')",[id])?;
        } else if let Some((path, hash, revision)) = restore {
            match hash {
                Ok(hash) => {
                    Self::enqueue_one(&tx, &path, &hash, &revision)?;
                }
                Err(error) => {
                    let hash: String =
                        tx.query_row("SELECT source_hash FROM tracks WHERE id=?1", [id], |r| {
                            r.get(0)
                        })?;
                    let job = Self::enqueue_one(&tx, &path, &hash, &revision)?;
                    tx.execute(
                        "UPDATE jobs SET state='attention',stage='waiting',error=?1 WHERE id=?2",
                        params![format!("Source unavailable: {error:#}"), job],
                    )?;
                    tx.execute("UPDATE tracks SET state='incoming' WHERE id=?1", [id])?;
                }
            }
        }
        Self::bump(&tx)?;
        tx.commit()?;
        Ok(())
    }
}

fn summary_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<TrackSummary> {
    let path: Vec<u8> = r.get(1)?;
    let path = bytes_path(&path).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            1,
            rusqlite::types::Type::Blob,
            e.into_boxed_dyn_error(),
        )
    })?;
    Ok(TrackSummary {
        id: r.get(0)?,
        source_sha256: r.get(11)?,
        display_path: path.to_string_lossy().into_owned(),
        title: r.get(2)?,
        artist: r.get(3)?,
        album: r.get(4)?,
        genre: r.get(5)?,
        state: r.get(6)?,
        key: r.get(7)?,
        bpm: r.get(8)?,
        energy: r.get(9)?,
        duration: r.get(10)?,
    })
}
