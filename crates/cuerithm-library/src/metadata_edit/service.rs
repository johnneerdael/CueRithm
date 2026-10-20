use super::*;
use crate::{
    Library,
    model::{bytes_path, now, path_bytes},
};
use anyhow::{Context, Result, ensure};
use cuerithm_autotag::{
    AnalysisTags, ArtworkEdit, ArtworkValue, AutotagConfig, MatchStatus, MetadataPatch, TagInput,
    TagMatchOutcome, TagWritePlan,
};
use cuerithm_media::{CancellationToken, MediaRuntime};
use rusqlite::params;
use serde::{Deserialize, Serialize};
#[derive(Clone)]
pub struct MetadataEditService {
    pub(super) library: Library,
    pub(super) runtime: MediaRuntime,
}
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct StoredStage {
    pub path: Vec<u8>,
    pub backup: Vec<u8>,
    pub workspace: Vec<u8>,
    pub before: String,
    pub after: String,
    pub info: cuerithm_media::MediaInfo,
    pub native: cuerithm_cleanup::Metadata,
    pub display: super::capture::Display,
    pub workspace_authority: crate::organization::Authority,
    pub staged_fingerprint: crate::organization::Fingerprint,
    pub backup_fingerprint: crate::organization::Fingerprint,
}
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct FrozenRow {
    pub public: MetadataEditRow,
    pub capture: Capture,
    pub stage: Option<StoredStage>,
}
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct FrozenPreview {
    pub public: MetadataEditPreview,
    pub rows: Vec<FrozenRow>,
    pub patch: MetadataEditorPatch,
}
impl MetadataEditService {
    pub fn new(library: Library, runtime: MediaRuntime) -> Self {
        Self { library, runtime }
    }
    pub fn snapshot(&self, ids: &[String]) -> Result<MetadataEditorSnapshot> {
        self.snapshot_cancellable(ids, &CancellationToken::default())
    }
    pub fn snapshot_cancellable(
        &self,
        ids: &[String],
        cancel: &CancellationToken,
    ) -> Result<MetadataEditorSnapshot> {
        let captures = self.capture_ids(ids, cancel)?;
        let tracks = captures.into_iter().map(|c| c.public).collect::<Vec<_>>();
        let mut mixed = vec![];
        if let Some(first) = tracks.first() {
            let first = serde_json::to_value(&first.native)?;
            for key in first.as_object().context("Invalid native snapshot")?.keys() {
                if tracks
                    .iter()
                    .skip(1)
                    .any(|t| serde_json::to_value(&t.native).is_ok_and(|s| s[key] != first[key]))
                {
                    mixed.push(key.clone());
                }
            }
        }
        let public = MetadataEditorSnapshot { tracks, mixed };
        bounded_json(&public)?;
        Ok(public)
    }
    pub fn create_draft(&self, ids: &[String]) -> Result<MetadataDraft> {
        self.create_draft_cancellable(ids, &CancellationToken::default())
    }
    pub fn create_draft_cancellable(
        &self,
        ids: &[String],
        cancel: &CancellationToken,
    ) -> Result<MetadataDraft> {
        let tracks = self.capture_ids(ids, cancel)?;
        let created = now();
        let draft = MetadataDraft {
            id: uuid::Uuid::new_v4().to_string(),
            revision: 1,
            track_ids: tracks.into_iter().map(|t| t.public.track_id).collect(),
            patch: Default::default(),
            state: "editing".into(),
            created,
            updated: created,
            error: None,
        };
        let json = bounded_json(&draft)?;
        ensure!(
            json.len() <= 2 * 1024 * 1024,
            "Metadata draft exceeds2MiB; narrow selection"
        );
        self.library.conn()?.execute(
            "INSERT INTO metadata_edit_drafts VALUES(?1,?2,?3,?4,?5,?5)",
            params![draft.id, draft.revision, draft.state, json, created],
        )?;
        Ok(draft)
    }
    pub fn draft(&self, id: &str) -> Result<MetadataDraft> {
        let (text, state, revision, updated): (String, String, u64, i64) = self
            .library
            .conn()?
            .query_row(
                "SELECT payload_json,state,revision,updated FROM metadata_edit_drafts WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .context("Unknown metadata draft")?;
        let mut draft: MetadataDraft = serde_json::from_str(&text)?;
        draft.state = state;
        draft.revision = revision;
        draft.updated = updated;
        Ok(draft)
    }
    pub fn drafts(&self) -> Result<Vec<MetadataDraft>> {
        let ids = {
            let conn = self.library.conn()?;
            let mut q=conn.prepare("SELECT id FROM metadata_edit_drafts WHERE state NOT IN ('discarded','saved','undone') ORDER BY updated DESC,id")?;
            q.query_map([], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        let mut result = vec![];
        let mut bytes = 0usize;
        for id in ids {
            let draft = self.draft(&id)?;
            bytes = bytes.saturating_add(bounded_json(&draft)?.len());
            ensure!(
                bytes <= MAX_METADATA_JSON,
                "Pending metadata drafts exceed64MiB"
            );
            result.push(draft);
        }
        Ok(result)
    }
    pub fn update_draft(
        &self,
        id: &str,
        revision: u64,
        patch: MetadataEditorPatch,
    ) -> Result<MetadataDraft> {
        patch.fields.validate()?;
        ensure!(
            patch.fields.artwork.is_empty(),
            "Use opaque artwork assets in editor drafts"
        );
        ensure!(
            patch.form_values.len() <= 7
                && patch.form_values.iter().all(|(k, v)| [
                    "release_date",
                    "original_date",
                    "track_number",
                    "track_total",
                    "disc_number",
                    "disc_total",
                    "bpm"
                ]
                .contains(&k.as_str())
                    && v.len() <= 4096
                    && !v.contains('\0')),
            "Invalid pending editor inputs"
        );
        for choice in &patch.artwork {
            if let Some(asset) = &choice.asset_id {
                self.artwork_bytes(asset)?;
            } else {
                ensure!(
                    choice.target.is_some(),
                    "Artwork removal needs a selected image"
                );
            }
        }
        let mut draft = self.draft(id)?;
        ensure!(draft.state != "discarded", "Draft was discarded");
        draft.revision = revision.checked_add(1).context("Draft revision overflow")?;
        draft.patch = patch;
        draft.state = "editing".into();
        draft.updated = now();
        draft.error = None;
        let json = bounded_json(&draft)?;
        ensure!(json.len() <= 2 * 1024 * 1024, "Metadata draft exceeds2MiB");
        let mut conn = self.library.conn()?;
        let tx = conn.transaction()?;
        ensure!(tx.execute("UPDATE metadata_edit_drafts SET revision=?1,state='editing',payload_json=?2,updated=?3 WHERE id=?4 AND revision=?5 AND state!='discarded'",params![draft.revision,json,draft.updated,id,revision])?==1,"Draft changed; reload before saving");
        tx.execute(
            "UPDATE metadata_edit_previews SET state='stale' WHERE draft_id=?1 AND state='preview'",
            [id],
        )?;
        tx.commit()?;
        drop(conn);
        self.library.retire_metadata_previews()?;
        Ok(draft)
    }
    pub fn discard_draft(&self, id: &str, revision: u64) -> Result<()> {
        let mut draft = self.draft(id)?;
        draft.state = "discarded".into();
        draft.updated = now();
        let json = bounded_json(&draft)?;
        let mut conn = self.library.conn()?;
        let tx = conn.transaction()?;
        ensure!(!tx.query_row("SELECT EXISTS(SELECT 1 FROM metadata_edit_batches WHERE draft_id=?1 AND state='saving')",[id],|r|r.get::<_,bool>(0))?,"A metadata save is still running");
        ensure!(tx.execute("UPDATE metadata_edit_drafts SET state='discarded',payload_json=?1,updated=?2 WHERE id=?3 AND revision=?4",params![json,now(),id,revision])?==1,"Draft changed");
        tx.execute("UPDATE metadata_edit_previews SET state='discarded' WHERE draft_id=?1 AND state='preview'",[id])?;
        tx.commit()?;
        drop(conn);
        self.library.retire_metadata_previews()?;
        Ok(())
    }
    pub fn import_artwork(&self, bytes: Vec<u8>) -> Result<MetadataArtworkAsset> {
        let (mime, bytes) = cuerithm_autotag::validate_metadata_artwork(bytes)?;
        use sha2::{Digest, Sha256};
        let sha256 = format!("{:x}", Sha256::digest(&bytes));
        let asset = MetadataArtworkAsset {
            id: uuid::Uuid::new_v4().to_string(),
            mime,
            sha256,
            bytes: bytes.len() as u64,
        };
        let folder = self.library.data_dir().join("metadata-edits/assets");
        std::fs::create_dir_all(&folder)?;
        let path = folder.join(&asset.id);
        use std::io::Write;
        let mut output = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)?;
        output.write_all(&bytes)?;
        output.sync_all()?;
        self.library.conn()?.execute(
            "INSERT INTO metadata_artwork_assets VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                asset.id,
                asset.sha256,
                asset.mime,
                path_bytes(&path),
                asset.bytes,
                now()
            ],
        )?;
        Ok(asset)
    }
    pub fn artwork_bytes(&self, id: &str) -> Result<Vec<u8>> {
        let (path, hash, bytes): (Vec<u8>, String, u64) = self.library.conn()?.query_row(
            "SELECT path,sha256,bytes FROM metadata_artwork_assets WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        ensure!(bytes <= 8 * 1024 * 1024, "Artwork exceeds8MiB");
        let path = bytes_path(&path)?;
        crate::artifacts::owned_file(&path, &self.library.data_dir().canonicalize()?, &hash)?;
        let content = std::fs::read(path)?;
        cuerithm_autotag::validate_metadata_artwork(content.clone())?;
        Ok(content)
    }
    pub fn extract_artwork(
        &self,
        track: &str,
        source: &str,
        identity: cuerithm_autotag::ArtworkIdentity,
    ) -> Result<MetadataArtworkAsset> {
        self.extract_artwork_cancellable(track, source, identity, &CancellationToken::default())
    }
    pub fn extract_artwork_cancellable(
        &self,
        track: &str,
        source: &str,
        identity: cuerithm_autotag::ArtworkIdentity,
        cancel: &CancellationToken,
    ) -> Result<MetadataArtworkAsset> {
        let captured = self.capture(track, cancel)?;
        ensure!(
            captured.fingerprint.sha256 == source,
            "Artwork source changed"
        );
        let path = captured.path.path()?;
        let info = self.runtime.probe_cancellable(&path, cancel)?;
        let bytes = cuerithm_autotag::metadata_artwork_bytes(&path, &info, &identity)?;
        Self::verify_source(&captured)?;
        cancel.check()?;
        self.import_artwork(bytes)
    }
    pub(super) fn native_patch(&self, patch: &MetadataEditorPatch) -> Result<MetadataPatch> {
        resolve_patch(&self.library, patch)
    }

    pub fn preview(
        &self,
        id: &str,
        revision: u64,
        cancel: &CancellationToken,
    ) -> Result<MetadataEditPreview> {
        let draft = self.draft(id)?;
        ensure!(
            draft.revision == revision && draft.state != "discarded",
            "Draft changed; refresh preview"
        );
        let patch = self.native_patch(&draft.patch)?;
        let captures = self.capture_ids(&draft.track_ids, cancel)?;
        let mut rows = vec![];
        let mut unpublished = super::previews::UnpersistedStages {
            library: self.library.clone(),
            stages: vec![],
            persisted: false,
        };
        for capture in captures {
            cancel.check()?;
            let before = capture.public.native.clone();
            let mut row = FrozenRow {
                public: MetadataEditRow {
                    track_id: capture.public.track_id.clone(),
                    title: capture.public.title.clone(),
                    before: before.clone(),
                    after: before,
                    outcome: "ready".into(),
                    fields: vec![],
                    error: None,
                },
                capture,
                stage: None,
            };
            match self.stage_row(&row.capture, &patch, cancel) {
                Ok((stage, after)) => {
                    row.public.after = after;
                    row.public.outcome = if row.public.before == row.public.after
                        && (row.capture.public.state != "incoming" || patch.is_noop())
                    {
                        "unchanged"
                    } else {
                        "ready"
                    }
                    .into();
                    unpublished.stages.push(stage.clone());
                    row.stage = Some(stage)
                }
                Err(error) => {
                    row.public.outcome = "failed".into();
                    row.public.error = Some(format!("{error:#}"));
                }
            }
            row.public.fields = field_outcomes(
                &patch,
                if row.public.outcome == "failed" {
                    "failed"
                } else {
                    "ready"
                },
                row.public.error.as_deref(),
            );
            if row.public.before.capabilities.get("id3_version")
                != row.public.after.capabilities.get("id3_version")
            {
                row.public.fields.push(MetadataFieldOutcome {
                    field: "id3_version".into(),
                    status: "ready".into(),
                    reason: Some(format!(
                        "Explicit ID3 conversion: {} to {}",
                        row.public
                            .before
                            .capabilities
                            .get("id3_version")
                            .map(String::as_str)
                            .unwrap_or("none"),
                        row.public
                            .after
                            .capabilities
                            .get("id3_version")
                            .map(String::as_str)
                            .unwrap_or("none")
                    )),
                });
            }
            rows.push(row);
        }
        let public = MetadataEditPreview {
            id: uuid::Uuid::new_v4().to_string(),
            draft_id: id.into(),
            draft_revision: revision,
            created: now(),
            can_apply: rows.iter().any(|r| r.public.outcome == "ready"),
            rows: rows.iter().map(|r| r.public.clone()).collect(),
        };
        let plan = FrozenPreview {
            public: public.clone(),
            rows,
            patch: draft.patch,
        };
        let json = bounded_json(&plan)?;
        ensure!(
            json.len() <= 64 * 1024 * 1024,
            "Complete metadata preview exceeds64MiB; narrow selection"
        );
        let mut conn = self.library.conn()?;
        let tx = conn.transaction()?;
        ensure!(
            tx.query_row(
                "SELECT revision=?2 AND state!='discarded' FROM metadata_edit_drafts WHERE id=?1",
                params![id, revision],
                |r| r.get::<_, bool>(0)
            )?,
            "Draft changed while staging"
        );
        for row in &plan.rows {
            Self::validate_capture(&tx, &row.capture, None)?;
        }
        tx.execute(
            "UPDATE metadata_edit_previews SET state='stale' WHERE draft_id=?1 AND state='preview'",
            [id],
        )?;
        tx.execute(
            "INSERT INTO metadata_edit_previews VALUES(?1,?2,?3,'preview',?4,?5)",
            params![public.id, id, revision, json, public.created],
        )?;
        tx.commit()?;
        unpublished.persisted = true;
        drop(conn);
        self.library.retire_metadata_previews()?;
        Ok(public)
    }
    pub(super) fn stage_row(
        &self,
        capture: &Capture,
        patch: &MetadataPatch,
        cancel: &CancellationToken,
    ) -> Result<(StoredStage, cuerithm_autotag::MetadataSnapshot)> {
        Self::verify_source(capture)?;
        let cache = self.library.data_dir().join("metadata-edits/staging");
        let audio = self
            .runtime
            .decode_cancellable(&capture.path.path()?, &cache, cancel)?;
        ensure!(
            audio.source_sha256 == capture.fingerprint.sha256,
            "Source changed during snapshot"
        );
        let input = TagInput {
            path: audio.source_path.clone(),
            source_sha256: audio.source_sha256.clone(),
            media: audio.info.clone(),
            duration_seconds: audio.clock.duration_seconds(),
        };
        let mut config = AutotagConfig::default();
        config.options.tags.clear();
        config.write_energy = false;
        let mut staged = TagWritePlan::build(
            input,
            &TagMatchOutcome {
                status: MatchStatus::NoMatch,
                matches: vec![],
                attempted: vec![],
                warnings: vec![],
            },
            &config,
            &AnalysisTags::default(),
        )?
        .with_editor_patch(patch.clone())?
        .stage_prepared_cancellable(&self.runtime, &audio, &cache, cancel)?;
        let after = cuerithm_autotag::read_metadata_snapshot(&staged.path, &staged.metadata)?;
        let result = StoredStage {
            path: path_bytes(&staged.path),
            backup: path_bytes(&staged.backup_path),
            workspace: path_bytes(staged.workspace()),
            before: staged.source_before_sha256.clone(),
            after: staged.source_after_sha256.clone(),
            info: staged.metadata.clone(),
            native: staged.native_metadata.clone(),
            display: edited_display(capture, patch, &staged.native_metadata),
            workspace_authority: crate::organization::NativeParent::capture(staged.workspace())?.1,
            staged_fingerprint: crate::organization::capture_regular_source(&staged.path)?.1,
            backup_fingerprint: crate::organization::capture_regular_source(&staged.backup_path)?.1,
        };
        staged.retain_for_journal();
        Ok((result, after))
    }
    pub fn apply_reviewed_metadata_patch(
        &self,
        track: &str,
        source: &str,
        path_revision: u64,
        metadata_revision: u64,
        mut patch: MetadataPatch,
        cancel: &CancellationToken,
    ) -> Result<MetadataEditBatch> {
        let capture = self.capture(track, cancel)?;
        ensure!(
            capture.public.state == "ready"
                && capture.fingerprint.sha256 == source
                && capture.public.path_revision == path_revision
                && capture.public.metadata_revision == metadata_revision,
            "Reviewed metadata source changed"
        );
        let mut artwork = vec![];
        for edit in std::mem::take(&mut patch.artwork) {
            let (asset_id, kind, description) = if let Some(image) = edit.replacement {
                let asset = self.import_artwork(image.bytes)?;
                (Some(asset.id), image.kind, image.description)
            } else {
                (None, None, None)
            };
            artwork.push(MetadataArtworkChoice {
                target: edit.target,
                asset_id,
                kind,
                description,
            });
        }
        let draft = self.create_draft_cancellable(&[track.into()], cancel)?;
        let draft = self.update_draft(
            &draft.id,
            draft.revision,
            MetadataEditorPatch {
                fields: patch,
                artwork,
                ..Default::default()
            },
        )?;
        let preview = self.preview(&draft.id, draft.revision, cancel)?;
        self.apply(&preview.id, cancel)
    }
}
pub(super) fn field_outcomes(
    patch: &MetadataPatch,
    status: &str,
    reason: Option<&str>,
) -> Vec<MetadataFieldOutcome> {
    let mut fields = patch
        .text
        .iter()
        .filter(|(_, e)| !matches!(e, cuerithm_autotag::TextEdit::Keep))
        .map(|(f, _)| {
            serde_json::to_value(f)
                .expect("serialize field")
                .as_str()
                .expect("field string")
                .to_owned()
        })
        .collect::<Vec<_>>();
    for (name, touched) in [
        (
            "release_date",
            !matches!(patch.release_date, cuerithm_autotag::ValueEdit::Keep),
        ),
        (
            "original_date",
            !matches!(patch.original_date, cuerithm_autotag::ValueEdit::Keep),
        ),
        (
            "track_number",
            !matches!(patch.track_number, cuerithm_autotag::ValueEdit::Keep),
        ),
        (
            "track_total",
            !matches!(patch.track_total, cuerithm_autotag::ValueEdit::Keep),
        ),
        (
            "disc_number",
            !matches!(patch.disc_number, cuerithm_autotag::ValueEdit::Keep),
        ),
        (
            "disc_total",
            !matches!(patch.disc_total, cuerithm_autotag::ValueEdit::Keep),
        ),
        (
            "bpm",
            !matches!(patch.bpm, cuerithm_autotag::ValueEdit::Keep),
        ),
        ("rating", !patch.ratings.is_empty()),
        ("comments", !patch.comments.is_empty()),
        ("artwork", !patch.artwork.is_empty()),
    ] {
        if touched {
            fields.push(name.into())
        }
    }
    fields
        .into_iter()
        .map(|field| MetadataFieldOutcome {
            field,
            status: status.into(),
            reason: reason.map(str::to_owned),
        })
        .collect()
}

pub(super) fn resolve_patch(
    library: &Library,
    patch: &MetadataEditorPatch,
) -> Result<MetadataPatch> {
    ensure!(
        patch.form_values.is_empty(),
        "Complete invalid date/number inputs before preview or save"
    );
    let mut out = patch.fields.clone();
    for art in &patch.artwork {
        let replacement = if let Some(id) = &art.asset_id {
            let (path, hash, bytes): (Vec<u8>, String, u64) = library.conn()?.query_row(
                "SELECT path,sha256,bytes FROM metadata_artwork_assets WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?;
            ensure!(bytes <= 8 * 1024 * 1024, "Artwork exceeds8MiB");
            let path = bytes_path(&path)?;
            crate::artifacts::owned_file(&path, &library.data_dir().canonicalize()?, &hash)?;
            let bytes = std::fs::read(path)?;
            cuerithm_autotag::validate_metadata_artwork(bytes.clone())?;
            Some(ArtworkValue {
                kind: art.kind,
                description: art.description.clone(),
                bytes,
            })
        } else {
            None
        };
        out.artwork.push(ArtworkEdit {
            target: art.target.clone(),
            replacement,
        });
    }
    out.validate()?;
    Ok(out)
}

fn edited_display(
    capture: &Capture,
    patch: &MetadataPatch,
    native: &cuerithm_cleanup::Metadata,
) -> super::capture::Display {
    use cuerithm_autotag::{MetadataTextField as F, TextEdit};
    use cuerithm_cleanup::MetadataField as N;
    let mut display = capture.display.clone();
    for (field, native_field, value) in [
        (F::Title, N::Title, &mut display.title),
        (F::Artist, N::Artist, &mut display.artist),
        (F::Album, N::Album, &mut display.album),
        (F::Genre, N::Genre, &mut display.genre),
    ] {
        if patch
            .text
            .get(&field)
            .is_some_and(|edit| !matches!(edit, TextEdit::Keep))
        {
            *value = native
                .fields
                .get(&native_field)
                .map(|v| v.join(", "))
                .unwrap_or_default();
        }
    }
    display
}
