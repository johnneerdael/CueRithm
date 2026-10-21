use super::publication::Publication;
use super::*;
use anyhow::{Context, Result, ensure};
use cuerithm_autotag::{
    CommentEdit, CommentIdentity, MetadataPatch, MetadataTextField, RatingEdit, RatingIdentity,
    TextEdit, ValueEdit,
};
use cuerithm_media::CancellationToken;
use rusqlite::params;
fn family(
    service: &CurationService,
    track: &str,
    native: &cuerithm_autotag::MetadataSnapshot,
) -> Result<CurationContainer> {
    if native.capabilities.contains_key("id3_version") {
        return Ok(CurationContainer::Id3);
    }
    let path = service.library.track_path(track)?;
    Ok(
        match path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "flac" | "ogg" | "opus" => CurationContainer::Vorbis,
            "m4a" | "mp4" => CurationContainer::Mp4,
            "wma" => CurationContainer::Asf,
            _ => anyhow::bail!("No proved Curate native writer for this container"),
        },
    )
}
fn text_field(field: CurationTextField) -> MetadataTextField {
    match field {
        CurationTextField::Genre => MetadataTextField::Genre,
        CurationTextField::Style => MetadataTextField::Style,
        CurationTextField::Grouping => MetadataTextField::Grouping,
        CurationTextField::Label => MetadataTextField::Label,
        CurationTextField::Version => MetadataTextField::Version,
        CurationTextField::Composer => MetadataTextField::Composer,
        CurationTextField::Lyricist => MetadataTextField::Lyricist,
    }
}
fn encoded(values: Vec<String>, encoding: CurationEncoding) -> Result<Vec<String>> {
    if values.is_empty() {
        return Ok(vec![]);
    }
    Ok(if encoding == CurationEncoding::Json {
        vec![serde_json::to_string(&values)?]
    } else {
        values
    })
}
fn text_edit(values: Vec<String>) -> TextEdit {
    if values.is_empty() {
        TextEdit::Clear
    } else {
        TextEdit::Set { values }
    }
}
fn patch(
    service: &CurationService,
    publication: &Publication,
    native: &cuerithm_autotag::MetadataSnapshot,
) -> Result<MetadataPatch> {
    let family = family(service, &publication.public.summary.track_id, native)?;
    let mut patch = MetadataPatch::default();
    for frozen in publication
        .mappings
        .iter()
        .filter(|frozen| frozen.mapping.containers.contains(&family))
    {
        let mapping = &frozen.mapping;
        patch.allow_id3_upgrade |= mapping.allow_id3_upgrade;
        let values = frozen.values.clone();
        match &mapping.target {
            CurationMappingTarget::Text { field, encoding } => {
                patch
                    .text
                    .insert(text_field(*field), text_edit(encoded(values, *encoding)?));
            }
            CurationMappingTarget::Comment {
                language,
                description,
                encoding,
            } => {
                ensure!(
                    *encoding == CurationEncoding::Json || values.len() <= 1,
                    "Multiple categories require reversible JSON comment encoding"
                );
                let matches = native
                    .comments
                    .iter()
                    .filter(|comment| {
                        comment.identity.language == *language
                            && comment.identity.description == *description
                    })
                    .collect::<Vec<_>>();
                ensure!(matches.len() <= 1, "Native comment identity is ambiguous");
                let identity = matches
                    .first()
                    .map(|comment| comment.identity.clone())
                    .unwrap_or(CommentIdentity {
                        language: language.clone(),
                        description: description.clone(),
                        sha256: String::new(),
                    });
                patch.comments.push(CommentEdit {
                    identity,
                    value: text_edit(encoded(values, *encoding)?),
                });
            }
            CurationMappingTarget::Rating { owner } => patch.ratings.push(RatingEdit {
                identity: RatingIdentity {
                    owner: owner.clone(),
                },
                value: if let Some(value) = values.first() {
                    ValueEdit::Set {
                        value: value.parse()?,
                    }
                } else {
                    ValueEdit::Clear
                },
            }),
        }
    }
    patch.validate()?;
    Ok(patch)
}
pub(super) fn publish(
    service: &CurationService,
    publication: &Publication,
    cancel: &CancellationToken,
) -> Result<()> {
    publish_with(service, publication, cancel, |_| Ok(()))
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum NativeBoundary {
    PreviewSaved,
    ChildLinked,
    ChildReturned,
}
pub(super) fn publish_with(
    service: &CurationService,
    publication: &Publication,
    cancel: &CancellationToken,
    mut boundary: impl FnMut(NativeBoundary) -> Result<()>,
) -> Result<()> {
    let runtime=service.runtime.clone().context("Native Curate mappings require the native tagging runtime; library-only Curate remains available")?;
    let editor = crate::MetadataEditService::new(service.library.clone(), runtime);
    let id = &publication.public.summary.id;
    let track = &publication.public.summary.track_id;
    if let Some(child) = &publication.public.child_batch_id {
        return finish_child(service, publication, &editor, child, cancel);
    }
    if let Some(original) = &publication.public.undo_of {
        let original = super::publication::load(service, original)?;
        return undo(
            service,
            publication,
            original
                .public
                .child_batch_id
                .as_deref()
                .context("Missing original native Curate child")?,
            cancel,
        );
    }
    if publication.input.state == "incoming" {
        let job = publication
            .input
            .current_job
            .as_deref()
            .context("Incoming Curate track has no preparation job")?;
        let job_state: String = service.library.conn()?.query_row(
            "SELECT state FROM jobs WHERE id=?1",
            [job],
            |row| row.get(0),
        )?;
        if job_state == "running" {
            service.library.conn()?.execute("UPDATE curation_publications SET state='waiting_incoming' WHERE id=?1 AND child_batch_id IS NULL",[id])?;
            service.library.conn()?.execute("UPDATE curation_tracks SET state='waiting_incoming' WHERE active_publication_id=?1",[id])?;
            return Ok(());
        }
        if job_state == "completed" {
            let mut current = super::storage::head(&service.library, track)?.current_input;
            current.state = publication.input.state.clone();
            if current == publication.input {
                // Analysis-only completion changes Ready state without changing the
                // captured source, revisions, path, or logical preparation owner.
                let mut advanced = publication.clone();
                advanced.input.state = "ready".into();
                let payload = super::storage::json(&advanced.input)?;
                let mut conn = service.library.conn()?;
                let tx = conn.transaction()?;
                tx.execute("UPDATE curation_publications SET input_json=?1 WHERE id=?2 AND child_batch_id IS NULL",params![payload,id])?;
                tx.execute(
                    "UPDATE curation_tracks SET input_json=?1 WHERE active_publication_id=?2",
                    params![payload, id],
                )?;
                tx.commit()?;
                drop(conn);
                return publish_with(service, &advanced, cancel, boundary);
            }
        }
    }
    let existing: Option<String> = service.library.conn()?.query_row(
        "SELECT child_preview_id FROM curation_publications WHERE id=?1",
        [id],
        |row| row.get(0),
    )?;
    let preview = if let Some(preview) = existing {
        let state: String = service.library.conn()?.query_row(
            "SELECT state FROM metadata_edit_previews WHERE id=?1",
            [&preview],
            |row| row.get(0),
        )?;
        if state == "retired" {
            let next = editor.restage_curation_preview(&preview, cancel)?;
            let mut conn = service.library.conn()?;
            let tx = conn.transaction()?;
            ensure!(tx.execute("UPDATE curation_publications SET child_preview_id=?1 WHERE id=?2 AND child_preview_id=?3 AND child_batch_id IS NULL",params![next,id,preview])?==1,"Curate preview changed during restaging");
            tx.execute(
                "DELETE FROM metadata_edit_previews WHERE id=?1 AND state='retired'",
                [preview],
            )?;
            tx.commit()?;
            next
        } else {
            preview
        }
    } else {
        let snapshot = editor.snapshot_cancellable(std::slice::from_ref(track), cancel)?;
        let native = &snapshot.tracks[0].native;
        let patch = patch(service, publication, native)?;
        let draft = editor.create_draft_cancellable(std::slice::from_ref(track), cancel)?;
        editor.hide_curation_draft(&draft.id, id)?;
        let draft = editor.update_draft(
            &draft.id,
            draft.revision,
            crate::MetadataEditorPatch {
                fields: patch,
                ..Default::default()
            },
        )?;
        let preview = editor.preview(&draft.id, draft.revision, cancel)?;
        editor.validate_curation_preview(
            &preview.id,
            &publication.input,
            publication.input_proof.as_deref(),
        )?;
        ensure!(service.library.conn()?.execute("UPDATE curation_publications SET child_preview_id=?1 WHERE id=?2 AND child_preview_id IS NULL AND child_batch_id IS NULL",params![preview.id,id])?==1,"Curate preview binding changed");
        preview.id
    };
    boundary(NativeBoundary::PreviewSaved)?;
    editor.apply_curation_preview_with(&preview, id, cancel, || {
        boundary(NativeBoundary::ChildLinked)
    })?;
    boundary(NativeBoundary::ChildReturned)?;
    let child: String = service.library.conn()?.query_row(
        "SELECT child_batch_id FROM curation_publications WHERE id=?1",
        [id],
        |row| row.get(0),
    )?;
    finish_child(service, publication, &editor, &child, cancel)
}
fn finish_child(
    service: &CurationService,
    publication: &Publication,
    editor: &crate::MetadataEditService,
    child: &str,
    cancel: &CancellationToken,
) -> Result<()> {
    let mut batch = editor.batch(child)?;
    if batch.state == "saving" {
        ensure!(
            matches!(
                publication.public.summary.state.as_str(),
                "recovery_required" | "error"
            ),
            "Curate metadata child is already running"
        );
        service.library.conn()?.execute("UPDATE metadata_edit_batches SET state='recovery_required' WHERE id=?1 AND state='saving'",[child])?;
        batch = editor.recover(child, cancel)?;
    } else if batch.can_recover {
        batch = editor.recover(child, cancel)?;
    }
    match batch.state.as_str() {
        "saved" | "undone" => {
            let state: String = service.library.conn()?.query_row(
                "SELECT state FROM curation_publications WHERE id=?1",
                [&publication.public.summary.id],
                |row| row.get(0),
            )?;
            if !matches!(state.as_str(), "saved" | "undone") {
                ensure!(
                    editor.verify_curation_unchanged_child(child)?,
                    "Metadata child did not acknowledge its exact Curate revision"
                );
                let mut conn = service.library.conn()?;
                let tx = conn.transaction()?;
                super::publication::commit_tx(&tx, &publication.public.summary.id)?;
                crate::Library::advance_curation_input_tx(
                    &tx,
                    &publication.public.summary.track_id,
                )?;
                tx.commit()?;
            }
            Ok(())
        }
        "queued" => {
            service.library.conn()?.execute("UPDATE curation_publications SET state='waiting_incoming' WHERE id=?1 AND state NOT IN ('saved','undone')",[&publication.public.summary.id])?;
            service.library.conn()?.execute("UPDATE curation_tracks SET state='waiting_incoming' WHERE active_publication_id=?1",[&publication.public.summary.id])?;
            Ok(())
        }
        _ => anyhow::bail!(
            "Mapped metadata publication {}: {}",
            batch.state,
            batch
                .error
                .as_deref()
                .unwrap_or("inspect the saved metadata result")
        ),
    }
}
pub(super) fn undo(
    service: &CurationService,
    publication: &Publication,
    original_child: &str,
    cancel: &CancellationToken,
) -> Result<()> {
    undo_with(service, publication, original_child, cancel, || Ok(()))
}
pub(super) fn undo_with(
    service: &CurationService,
    publication: &Publication,
    original_child: &str,
    cancel: &CancellationToken,
    after_link: impl FnOnce() -> Result<()>,
) -> Result<()> {
    let editor = crate::MetadataEditService::new(
        service.library.clone(),
        service
            .runtime
            .clone()
            .context("Native tagging runtime unavailable")?,
    );
    if editor.verify_curation_unchanged_child(original_child)? {
        let mut conn = service.library.conn()?;
        let tx = conn.transaction()?;
        super::publication::commit_tx(&tx, &publication.public.summary.id)?;
        tx.commit()?;
        return Ok(());
    }
    let batch = editor.undo_curation_batch_with(
        original_child,
        &publication.public.summary.id,
        cancel,
        after_link,
    )?;
    finish_child(service, publication, &editor, &batch.id, cancel)
}
#[cfg(test)]
mod tests;
