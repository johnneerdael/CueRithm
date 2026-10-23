//! Guarded library-only musical corrections. Detector output and audio files stay intact.
use crate::{
    Library,
    model::{bytes_path, now},
};
use anyhow::{Context, Result, bail, ensure};
use camelot_core::{
    FileAnalysis,
    lexicon_publication::{
        self, CueTemplate, Marker, Publication, PublicationContext, PublishedCue, PublishedType,
        SavedGrid, StructuralMarker,
    },
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManualPublicationGuard {
    pub track_id: String,
    pub source_sha256: String,
    pub entity_epoch: u64,
    pub cue_revision_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualPublicationSnapshot {
    pub guard: ManualPublicationGuard,
    pub library_revision: u64,
    pub publication: Publication,
    pub duration_seconds: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualPublicationRequest {
    pub guard: ManualPublicationGuard,
    pub expected_revision: u64,
    pub grid: SavedGrid,
    pub cues: Vec<PublishedCue>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplatePublicationRequest {
    pub template_id: String,
    pub guards: Vec<ManualPublicationGuard>,
    pub expected_revision: u64,
    #[serde(default = "preserve_default")]
    pub preserve_manual: bool,
}
fn preserve_default() -> bool {
    true
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualPublicationChange {
    pub track_id: String,
    pub title: String,
    pub before: Publication,
    pub after: Publication,
    pub preserved_manual: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualPublicationPreview {
    pub id: String,
    pub library_revision: u64,
    pub changes: Vec<ManualPublicationChange>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualPublicationResult {
    pub id: String,
    pub track_ids: Vec<String>,
    pub library_revision: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualPublicationHistory {
    pub id: String,
    pub state: String,
    pub created: i64,
    /// Current database ownership/readiness permits an Undo attempt. History
    /// does not open source files; Undo verifies their hashes before restoring.
    pub can_undo: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedCueTemplate {
    pub id: String,
    pub name: String,
    pub template: CueTemplate,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ManualState {
    slots: BTreeSet<u32>,
    grid: bool,
    #[serde(default)]
    owner_revision_id: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredChange {
    guard: ManualPublicationGuard,
    change: ManualPublicationChange,
    before_state: ManualState,
    after_state: ManualState,
    before_bpm: Option<f64>,
    owned_revision: Option<String>,
    applied_epoch: Option<u64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Operation {
    revision: u64,
    changes: Vec<StoredChange>,
}

fn revision(conn: &Connection) -> Result<u64> {
    Ok(
        conn.query_row("SELECT value FROM meta WHERE key='revision'", [], |r| {
            r.get(0)
        })?,
    )
}
fn expect_revision(conn: &Connection, expected: u64) -> Result<()> {
    ensure!(
        revision(conn)? == expected,
        "Library changed; refresh the correction preview"
    );
    Ok(())
}
fn epoch(conn: &Connection, id: &str) -> Result<u64> {
    Ok(conn.query_row(
        "SELECT epoch FROM maintenance_track_epochs WHERE id=?1",
        [id],
        |r| r.get(0),
    )?)
}
fn current_cue(conn: &Connection, id: &str) -> Result<String> {
    conn.query_row(
        "SELECT id FROM cue_revisions WHERE track_id=?1 ORDER BY created DESC,rowid DESC LIMIT 1",
        [id],
        |r| r.get(0),
    )
    .optional()?
    .context("Prepare this track before editing cues")
}
fn manual_state(conn: &Connection, id: &str) -> Result<ManualState> {
    let text: Option<String> = conn
        .query_row(
            "SELECT json FROM manual_publication_state WHERE track_id=?1",
            [id],
            |r| r.get(0),
        )
        .optional()?;
    let state: ManualState = text
        .map(|s| serde_json::from_str(&s))
        .transpose()?
        .unwrap_or_default();
    let current = current_cue(conn, id)?;
    if state.owner_revision_id.as_deref() == Some(current.as_str()) {
        Ok(state)
    } else {
        // Preparation and donor publications replace cue revisions independently
        // of this module. Old slot ownership cannot protect their generated cues.
        // Keep the dormant row so donor Undo restoring the old revisions also
        // restores protection for the actual surviving manual publication.
        Ok(ManualState::default())
    }
}
type SourceSnapshot = (
    Option<Vec<u8>>,
    Option<String>,
    String,
    bool,
    Option<String>,
    Option<f64>,
);
fn load(
    conn: &Connection,
    id: &str,
) -> Result<(ManualPublicationSnapshot, FileAnalysis, Option<f64>)> {
    let (path, source, state, archived, text, bpm): SourceSnapshot = conn.query_row(
        "SELECT path,source_hash,state,archived,analysis_json,bpm FROM tracks WHERE id=?1",
        [id],
        |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        },
    )?;
    let path = path.context("Local file is not linked; relocate the track before editing cues")?;
    let source =
        source.context("File identity is unknown; analyze the track before editing cues")?;
    ensure!(
        state == "ready" && !archived,
        "Prepare and restore this track before editing cues"
    );
    let active: u64 = conn.query_row(
        "SELECT COUNT(*) FROM jobs WHERE track_id=?1 AND state IN ('queued','running','attention')",
        [id],
        |r| r.get(0),
    )?;
    ensure!(
        active == 0,
        "Preparation is active; wait before editing cues"
    );
    ensure!(
        cuerithm_media::file_sha256(&bytes_path(&path)?)? == source,
        "Source changed; prepare the track again"
    );
    let analysis: FileAnalysis =
        serde_json::from_str(&text.context("Prepare this track before editing cues")?)?;
    ensure!(analysis.complete, "Preparation is incomplete");
    let publication = analysis
        .publication
        .clone()
        .context("Prepare this track to create a saved cue/grid publication")?;
    let guard = ManualPublicationGuard {
        track_id: id.into(),
        source_sha256: source,
        entity_epoch: epoch(conn, id)?,
        cue_revision_id: current_cue(conn, id)?,
    };
    Ok((
        ManualPublicationSnapshot {
            guard,
            library_revision: revision(conn)?,
            publication,
            duration_seconds: analysis.analysis.duration_seconds,
        },
        analysis,
        bpm,
    ))
}
fn guarded(
    conn: &Connection,
    guard: &ManualPublicationGuard,
) -> Result<(ManualPublicationSnapshot, FileAnalysis, Option<f64>)> {
    let current = load(conn, &guard.track_id)?;
    ensure!(
        current.0.guard == *guard,
        "Track changed; refresh the correction preview"
    );
    Ok(current)
}
fn validate(grid: &SavedGrid, cues: &[PublishedCue], duration: f64) -> Result<()> {
    ensure!(
        duration.is_finite() && duration > 0.,
        "Invalid track duration"
    );
    ensure!(
        grid.bpm.is_finite() && grid.bpm > 0. && grid.bpm <= 1000.,
        "BPM must be greater than zero and at most 1000"
    );
    ensure!(
        grid.start_time.is_finite() && (0. ..=duration).contains(&grid.start_time),
        "First beat must be within the track; negative anchors are not supported"
    );
    ensure!(
        cues.len() <= 128,
        "At most 128 saved cue slots are supported"
    );
    let mut slots = BTreeSet::new();
    for cue in cues {
        ensure!(
            cue.position < 128 && slots.insert(cue.position),
            "Cue slots must be unique and between 1 and 128"
        );
        ensure!(cue.name.len() <= 4096, "Cue name is too long");
        ensure!(
            cue.color.is_null() || cue.color.as_str().is_some_and(|s| s.len() <= 64),
            "Invalid cue color"
        );
        ensure!(
            cue.start_time.is_finite() && (0. ..=duration).contains(&cue.start_time),
            "Cue position must be within the track"
        );
        match cue.kind {
            PublishedType::Loop => ensure!(
                cue.end_time
                    .is_some_and(|t| t.is_finite() && t > cue.start_time && t <= duration),
                "Saved loops need an end after their start and within the track"
            ),
            PublishedType::Cue => ensure!(
                cue.end_time.is_none() && cue.active_loop != Some(true),
                "Only saved loops can have an end or be active"
            ),
        }
    }
    Ok(())
}
fn hash_publication(publication: &mut Publication) -> Result<()> {
    publication.implementation_sha256 = format!(
        "manual-publication-v1:{}",
        lexicon_publication::implementation_sha256()
    );
    publication.inputs_sha256 = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(&publication.grid, &publication.cues))?)
    );
    Ok(())
}
fn put_operation(conn: &Connection, operation: Operation) -> Result<ManualPublicationPreview> {
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO manual_publication_operations VALUES (?1,'preview',?2,?3)",
        params![id, serde_json::to_string(&operation)?, now()],
    )?;
    Ok(ManualPublicationPreview {
        id,
        library_revision: operation.revision,
        changes: operation.changes.into_iter().map(|c| c.change).collect(),
    })
}
fn undo_owned_in_database(conn: &Connection, change: &StoredChange) -> Result<bool> {
    let (Some(applied_epoch), Some(owned_revision)) =
        (change.applied_epoch, change.owned_revision.as_deref())
    else {
        return Ok(false);
    };
    Ok(conn.query_row(
        "SELECT t.state='ready' AND t.archived=0 AND t.source_hash=?2
         AND COALESCE(e.epoch=?3,0)
         AND COALESCE((SELECT id FROM cue_revisions WHERE track_id=t.id ORDER BY created DESC,rowid DESC LIMIT 1)=?4,0)
         AND NOT EXISTS(SELECT 1 FROM jobs WHERE track_id=t.id AND state IN ('queued','running','attention'))
         FROM tracks t LEFT JOIN maintenance_track_epochs e ON e.id=t.id WHERE t.id=?1",
        params![change.guard.track_id, change.guard.source_sha256, applied_epoch, owned_revision],
        |row| row.get(0),
    ).optional()?.unwrap_or(false))
}

fn operation(conn: &Connection, id: &str, state: &str) -> Result<Operation> {
    let (actual, text): (String, String) = conn.query_row(
        "SELECT state,payload_json FROM manual_publication_operations WHERE id=?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    ensure!(
        actual == state,
        "This correction preview or Undo is no longer available"
    );
    Ok(serde_json::from_str(&text)?)
}
fn write_publication(
    conn: &Connection,
    id: &str,
    publication: &Publication,
    bpm: Option<f64>,
    state: &ManualState,
) -> Result<String> {
    let text: String =
        conn.query_row("SELECT analysis_json FROM tracks WHERE id=?1", [id], |r| {
            r.get(0)
        })?;
    // Modify exactly the saved publication; retain raw detector JSON byte values.
    let mut analysis: serde_json::Value = serde_json::from_str(&text)?;
    analysis["publication"] = serde_json::to_value(publication)?;
    let mut cue = serde_json::to_value(publication)?;
    cue["schema"] = "lexicon-publication-v1".into();
    let owned = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO cue_revisions VALUES (?1,?2,NULL,?3,?4)",
        params![owned, id, serde_json::to_string(&cue)?, now()],
    )?;
    conn.execute(
        "UPDATE tracks SET analysis_json=?1,bpm=?2,updated=?3 WHERE id=?4",
        params![serde_json::to_string(&analysis)?, bpm, now(), id],
    )?;
    let state = ManualState {
        owner_revision_id: Some(owned.clone()),
        ..state.clone()
    };
    conn.execute("INSERT INTO manual_publication_state VALUES (?1,?2) ON CONFLICT(track_id) DO UPDATE SET json=excluded.json",params![id,serde_json::to_string(&state)?])?;
    Ok(owned)
}

impl Library {
    pub fn manual_publication_snapshot(&self, track_id: &str) -> Result<ManualPublicationSnapshot> {
        let conn = self.conn()?;
        Ok(load(&conn, track_id)?.0)
    }
    pub fn preview_manual_publication(
        &self,
        request: ManualPublicationRequest,
    ) -> Result<ManualPublicationPreview> {
        #[cfg(feature = "autotag")]
        let _gate = self
            .inner
            .tag_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Publication gate interrupted"))?;
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        expect_revision(&tx, request.expected_revision)?;
        let (snapshot, _, before_bpm) = guarded(&tx, &request.guard)?;
        validate(&request.grid, &request.cues, snapshot.duration_seconds)?;
        let before_state = manual_state(&tx, &request.guard.track_id)?;
        let mut after_state = before_state.clone();
        let before_slots = snapshot
            .publication
            .cues
            .iter()
            .map(|c| (c.position, c))
            .collect::<BTreeMap<_, _>>();
        let after_slots = request
            .cues
            .iter()
            .map(|c| (c.position, c))
            .collect::<BTreeMap<_, _>>();
        for slot in before_slots.keys().chain(after_slots.keys()) {
            if before_slots.get(slot) != after_slots.get(slot) {
                after_state.slots.insert(*slot);
            }
        }
        after_state.grid |= snapshot.publication.grid != request.grid;
        let mut after = snapshot.publication.clone();
        after.grid = request.grid;
        after.cues = request.cues;
        hash_publication(&mut after)?;
        let change = StoredChange {
            guard: request.guard.clone(),
            change: ManualPublicationChange {
                title: tx.query_row(
                    "SELECT title FROM tracks WHERE id=?1",
                    [&request.guard.track_id],
                    |r| r.get(0),
                )?,
                track_id: request.guard.track_id,
                before: snapshot.publication,
                after,
                preserved_manual: false,
            },
            before_state,
            after_state,
            before_bpm,
            owned_revision: None,
            applied_epoch: None,
        };
        let result = put_operation(
            &tx,
            Operation {
                revision: request.expected_revision,
                changes: vec![change],
            },
        )?;
        tx.commit()?;
        Ok(result)
    }
    pub fn apply_manual_publication(
        &self,
        preview_id: &str,
        expected_revision: u64,
    ) -> Result<ManualPublicationResult> {
        #[cfg(feature = "autotag")]
        let _gate = self
            .inner
            .tag_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Publication gate interrupted"))?;
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        expect_revision(&tx, expected_revision)?;
        let mut op = operation(&tx, preview_id, "preview")?;
        ensure!(
            op.revision == expected_revision,
            "Library changed; make a new correction preview"
        );
        // Validate the entire batch before writing any track.
        for change in &op.changes {
            let (snapshot, _, _) = guarded(&tx, &change.guard)?;
            validate(
                &change.change.after.grid,
                &change.change.after.cues,
                snapshot.duration_seconds,
            )?;
        }
        for change in &mut op.changes {
            change.owned_revision = Some(write_publication(
                &tx,
                &change.guard.track_id,
                &change.change.after,
                Some(change.change.after.grid.bpm),
                &change.after_state,
            )?);
            change.after_state.owner_revision_id = change.owned_revision.clone();
            change.applied_epoch = Some(epoch(&tx, &change.guard.track_id)?);
        }
        Self::bump(&tx)?;
        tx.execute(
            "UPDATE manual_publication_operations SET state='applied',payload_json=?1 WHERE id=?2",
            params![serde_json::to_string(&op)?, preview_id],
        )?;
        let result = ManualPublicationResult {
            id: preview_id.into(),
            track_ids: op
                .changes
                .iter()
                .map(|c| c.guard.track_id.clone())
                .collect(),
            library_revision: revision(&tx)?,
        };
        tx.commit()?;
        Ok(result)
    }
    pub fn undo_manual_publication(
        &self,
        edit_id: &str,
        expected_revision: u64,
    ) -> Result<ManualPublicationResult> {
        #[cfg(feature = "autotag")]
        let _gate = self
            .inner
            .tag_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Publication gate interrupted"))?;
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        expect_revision(&tx, expected_revision)?;
        let op = operation(&tx, edit_id, "applied")?;
        // Reject stale ownership across the entire batch before opening any
        // source. File proofs remain authoritative at this guarded write boundary.
        for change in &op.changes {
            ensure!(
                undo_owned_in_database(&tx, change)?,
                "Track changed after this correction; Undo cannot replace newer work"
            );
        }
        for change in &op.changes {
            let (snapshot, _, _) = load(&tx, &change.guard.track_id)?;
            ensure!(
                snapshot.guard.source_sha256 == change.guard.source_sha256
                    && Some(snapshot.guard.entity_epoch) == change.applied_epoch
                    && Some(&snapshot.guard.cue_revision_id) == change.owned_revision.as_ref(),
                "Track changed after this correction; Undo cannot replace newer work"
            );
        }
        for change in &op.changes {
            write_publication(
                &tx,
                &change.guard.track_id,
                &change.change.before,
                change.before_bpm,
                &change.before_state,
            )?;
        }
        Self::bump(&tx)?;
        tx.execute(
            "UPDATE manual_publication_operations SET state='undone' WHERE id=?1",
            [edit_id],
        )?;
        let result = ManualPublicationResult {
            id: edit_id.into(),
            track_ids: op
                .changes
                .iter()
                .map(|c| c.guard.track_id.clone())
                .collect(),
            library_revision: revision(&tx)?,
        };
        tx.commit()?;
        Ok(result)
    }
    pub fn manual_publication_history(
        &self,
        track_id: &str,
    ) -> Result<Vec<ManualPublicationHistory>> {
        let conn = self.conn()?;
        let mut stmt=conn.prepare("SELECT id,state,created,payload_json FROM manual_publication_operations WHERE state!='preview' ORDER BY created DESC,rowid DESC")?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut history = vec![];
        for (id, state, created, text) in rows {
            let op: Operation = serde_json::from_str(&text)?;
            if !op.changes.iter().any(|c| c.guard.track_id == track_id) {
                continue;
            }
            let mut can_undo = state == "applied";
            if can_undo {
                for change in &op.changes {
                    if !undo_owned_in_database(&conn, change)? {
                        can_undo = false;
                        break;
                    }
                }
            }
            history.push(ManualPublicationHistory {
                id,
                state,
                created,
                can_undo,
            });
        }
        Ok(history)
    }
    pub fn cue_templates(&self) -> Result<Vec<SavedCueTemplate>> {
        let conn = self.conn()?;
        let mut stmt =
            conn.prepare("SELECT id,name,json FROM cue_templates ORDER BY name COLLATE NOCASE,id")?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.into_iter()
            .map(|(id, name, text)| {
                Ok(SavedCueTemplate {
                    id,
                    name,
                    template: serde_json::from_str(&text)?,
                })
            })
            .collect()
    }
    pub fn save_cue_template(
        &self,
        id: Option<&str>,
        name: String,
        template: CueTemplate,
        expected_revision: u64,
    ) -> Result<SavedCueTemplate> {
        let name = name.trim().to_owned();
        ensure!(
            !name.is_empty() && name.len() <= 200,
            "Give the template a name of at most 200 characters"
        );
        template.validate()?;
        validate_custom_anchors(&template)?;
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        expect_revision(&tx, expected_revision)?;
        let id = if let Some(id) = id {
            ensure!(
                tx.query_row(
                    "SELECT COUNT(*) FROM cue_templates WHERE id=?1",
                    [id],
                    |r| r.get::<_, u64>(0)
                )? == 1,
                "Cue template is missing"
            );
            id.to_owned()
        } else {
            uuid::Uuid::new_v4().to_string()
        };
        tx.execute("INSERT INTO cue_templates VALUES (?1,?2,?3,?4) ON CONFLICT(id) DO UPDATE SET name=excluded.name,json=excluded.json,updated=excluded.updated",params![id,name,serde_json::to_string(&template)?,now()])?;
        Self::bump(&tx)?;
        tx.commit()?;
        Ok(SavedCueTemplate { id, name, template })
    }
    pub fn delete_cue_template(&self, id: &str, expected_revision: u64) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        expect_revision(&tx, expected_revision)?;
        ensure!(
            tx.execute("DELETE FROM cue_templates WHERE id=?1", [id])? == 1,
            "Cue template is missing"
        );
        Self::bump(&tx)?;
        tx.commit()?;
        Ok(())
    }
    pub fn preview_cue_template(
        &self,
        request: TemplatePublicationRequest,
    ) -> Result<ManualPublicationPreview> {
        ensure!(
            !request.guards.is_empty() && request.guards.len() <= 1000,
            "Choose 1 to 1000 prepared tracks"
        );
        #[cfg(feature = "autotag")]
        let _gate = self
            .inner
            .tag_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Publication gate interrupted"))?;
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        expect_revision(&tx, request.expected_revision)?;
        let text: String = tx.query_row(
            "SELECT json FROM cue_templates WHERE id=?1",
            [&request.template_id],
            |r| r.get(0),
        )?;
        let template: CueTemplate = serde_json::from_str(&text)?;
        template.validate()?;
        validate_custom_anchors(&template)?;
        let mut ids = BTreeSet::new();
        let mut changes = vec![];
        for guard in request.guards {
            ensure!(
                ids.insert(guard.track_id.clone()),
                "Template selection contains a duplicate track"
            );
            let (snapshot, analysis, before_bpm) = guarded(&tx, &guard)?;
            let markers = template_markers(&analysis, &snapshot.publication, &template)?;
            let generated = lexicon_publication::publish(
                &markers,
                &PublicationContext {
                    bpm: snapshot.publication.grid.bpm,
                    duration: snapshot.duration_seconds,
                    cuepoints: snapshot.publication.cues.clone(),
                },
                &template,
            )?;
            let before_state = manual_state(&tx, &guard.track_id)?;
            let protected = if request.preserve_manual {
                before_state.slots.clone()
            } else {
                BTreeSet::new()
            };
            let mut cues = if template.overwrite {
                snapshot
                    .publication
                    .cues
                    .iter()
                    .filter(|c| protected.contains(&c.position))
                    .cloned()
                    .collect::<Vec<_>>()
            } else {
                snapshot.publication.cues.clone()
            };
            for cue in generated {
                if !protected.contains(&cue.position)
                    && !cues.iter().any(|c| c.position == cue.position)
                {
                    cues.push(cue);
                }
            }
            cues.sort_by_key(|c| c.position);
            let mut after = snapshot.publication.clone();
            after.cues = cues;
            hash_publication(&mut after)?;
            validate(&after.grid, &after.cues, snapshot.duration_seconds)?;
            // Disabling preservation permits replacement in this preview, but
            // must not erase ownership of corrections the template leaves intact.
            let mut after_state = before_state.clone();
            if !request.preserve_manual {
                after_state.slots.retain(|slot| {
                    snapshot
                        .publication
                        .cues
                        .iter()
                        .find(|c| c.position == *slot)
                        == after.cues.iter().find(|c| c.position == *slot)
                });
            }
            changes.push(StoredChange {
                change: ManualPublicationChange {
                    track_id: guard.track_id.clone(),
                    title: tx.query_row(
                        "SELECT title FROM tracks WHERE id=?1",
                        [&guard.track_id],
                        |r| r.get(0),
                    )?,
                    before: snapshot.publication,
                    after,
                    preserved_manual: request.preserve_manual
                        && (!before_state.slots.is_empty() || before_state.grid),
                },
                guard,
                before_state,
                after_state,
                before_bpm,
                owned_revision: None,
                applied_epoch: None,
            });
        }
        let result = put_operation(
            &tx,
            Operation {
                revision: request.expected_revision,
                changes,
            },
        )?;
        tx.commit()?;
        Ok(result)
    }
}

fn validate_custom_anchors(template: &CueTemplate) -> Result<()> {
    if !template.enable_custom_anchors {
        return Ok(());
    }
    let map = template
        .custom_anchors
        .as_object()
        .context("Custom anchors must map a structural role to an existing cue name")?;
    for (role, name) in map {
        let _: Marker =
            serde_json::from_value(role.clone().into()).context("Unknown custom anchor role")?;
        ensure!(
            name.as_str()
                .is_some_and(|s| !s.is_empty() && s.len() <= 4096),
            "Custom anchors need existing cue names"
        );
    }
    Ok(())
}
fn template_markers(
    analysis: &FileAnalysis,
    publication: &Publication,
    template: &CueTemplate,
) -> Result<Vec<StructuralMarker>> {
    let mut markers = vec![];
    if let Some(raw) = &analysis.analysis.lexicon_cues {
        for cue in &raw.cues {
            let marker = match cue.name.as_str() {
                "Start" => Marker::Start,
                "Drop" => Marker::Drop,
                "Breakdown" => Marker::Breakdown,
                "Second Drop" => Marker::SecondDrop,
                "Second Breakdown" => Marker::SecondBreakdown,
                "Last beat" => Marker::Lastbeat,
                name => bail!("Unknown structural cue role: {name}"),
            };
            markers.push(StructuralMarker {
                marker_type: marker,
                start_time_seconds: Some(if marker == Marker::Start {
                    publication.grid.start_time
                } else {
                    cue.time_seconds
                }),
            });
        }
        if let Some(point) = &raw.emergency_loop {
            markers.push(StructuralMarker {
                marker_type: Marker::EmergencyLoop,
                start_time_seconds: Some(point.time_seconds),
            });
        }
    }
    if template.enable_custom_anchors {
        for (role, name) in template
            .custom_anchors
            .as_object()
            .context("Missing custom anchor map")?
        {
            let marker: Marker = serde_json::from_value(role.clone().into())?;
            let name = name.as_str().context("Invalid custom anchor cue")?;
            let matches = publication
                .cues
                .iter()
                .filter(|c| c.name == name)
                .collect::<Vec<_>>();
            ensure!(
                matches.len() == 1,
                "Custom anchor '{name}' must match exactly one existing cue"
            );
            markers.retain(|m| m.marker_type != marker);
            markers.push(StructuralMarker {
                marker_type: marker,
                start_time_seconds: Some(matches[0].start_time),
            });
        }
    }
    markers.sort_by(|a, b| {
        a.start_time_seconds
            .unwrap_or_default()
            .total_cmp(&b.start_time_seconds.unwrap_or_default())
    });
    Ok(markers)
}
