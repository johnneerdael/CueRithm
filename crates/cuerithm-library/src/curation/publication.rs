use super::storage::*;
use super::*;
use crate::{Library, model::now};
#[cfg(feature = "autotag")]
use anyhow::Context;
use anyhow::{Result, ensure};
use cuerithm_media::CancellationToken;
#[cfg(feature = "autotag")]
use rusqlite::OptionalExtension;
use rusqlite::{Connection, params};
#[derive(Clone)]
pub(super) struct Publication {
    pub public: CurationPublication,
    #[cfg(feature = "autotag")]
    pub mappings: Vec<super::mapping::FrozenMapping>,
    #[cfg(feature = "autotag")]
    pub input: Input,
    #[cfg(feature = "autotag")]
    pub input_proof: Option<String>,
}
fn can_undo(
    conn: &Connection,
    id: &str,
    track: &str,
    revision: u64,
    state: &str,
    native: bool,
    child: Option<&str>,
) -> Result<bool> {
    if state != "saved" {
        return Ok(false);
    }
    let _ = revision;
    let current:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM curation_tracks c JOIN curation_publications p ON p.id=?2 WHERE c.track_id=?1 AND EXISTS(SELECT 1 FROM tracks t WHERE t.id=c.track_id AND t.archived=0) AND c.revision=c.acknowledged_revision AND c.active_publication_id IS NULL AND c.last_publication_id=p.id AND c.committed_json=p.after_json)",params![track,id],|row|row.get(0))?;
    if !current {
        return Ok(false);
    }
    if native {
        #[cfg(feature = "autotag")]
        {
            return child
                .map(|child| crate::metadata_edit::curation_child_undo_available(conn, child))
                .transpose()
                .map(|value| value.unwrap_or(false));
        }
        #[cfg(not(feature = "autotag"))]
        {
            let _ = child;
            return Ok(false);
        }
    }
    Ok(true)
}
pub(super) fn summary(conn: &Connection, id: &str) -> Result<CurationPublicationSummary> {
    let(track,title,revision,state,native,created,error,child):(String,String,u64,String,bool,i64,Option<String>,Option<String>)=conn.query_row("SELECT p.track_id,t.title,p.revision,p.state,p.native_required,p.created,p.error,p.child_batch_id FROM curation_publications p JOIN tracks t ON t.id=p.track_id WHERE p.id=?1",[id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?)))?;
    let undo = can_undo(conn, id, &track, revision, &state, native, child.as_deref())?;
    let recover=matches!(state.as_str(),"recovery_required"|"error"|"waiting_incoming")&&conn.query_row("SELECT EXISTS(SELECT 1 FROM curation_tracks c JOIN tracks t ON t.id=c.track_id WHERE c.track_id=?1 AND c.active_publication_id=?2 AND c.revision=?3 AND t.archived=0)",params![track,id,revision],|row|row.get::<_,bool>(0))?;
    Ok(CurationPublicationSummary {
        id: id.into(),
        track_id: track,
        title,
        revision,
        can_recover: recover,
        state,
        native_required: native,
        created,
        error,
        can_undo: undo,
    })
}
pub(super) fn load(service: &CurationService, id: &str) -> Result<Publication> {
    let (public, before, after, taxonomy_revision, child, undo, mappings) = {
        let conn = service.library.conn()?;
        let public = summary(&conn, id)?;
        let(before,after,taxonomy_revision,child,undo,mappings):(String,String,u64,Option<String>,Option<String>,String)=conn.query_row("SELECT before_json,after_json,taxonomy_revision,child_batch_id,undo_of,mappings_json FROM curation_publications WHERE id=?1",[id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?)))?;
        (
            public,
            before,
            after,
            taxonomy_revision,
            child,
            undo,
            mappings,
        )
    };
    ensure!(
        before.len() + after.len() + mappings.len() <= MAX_JSON,
        "Curate publication exceeds bounded size"
    );
    Ok(Publication {
        public: CurationPublication {
            summary: public,
            before: serde_json::from_str(&before)?,
            after: serde_json::from_str(&after)?,
            taxonomy_revision,
            child_batch_id: child,
            undo_of: undo,
        },
        #[cfg(feature = "autotag")]
        mappings: serde_json::from_str(&mappings)?,
        #[cfg(feature = "autotag")]
        input: {
            let text: String = service.library.conn()?.query_row(
                "SELECT input_json FROM curation_publications WHERE id=?1",
                [id],
                |row| row.get(0),
            )?;
            serde_json::from_str(&text)?
        },
        #[cfg(feature = "autotag")]
        input_proof: service.library.conn()?.query_row(
            "SELECT input_proof_json FROM curation_publications WHERE id=?1",
            [id],
            |row| row.get(0),
        )?,
    })
}
pub(super) fn history(
    service: &CurationService,
    offset: usize,
    limit: usize,
) -> Result<CurationHistoryPage> {
    page(offset, limit)?;
    let conn = service.library.conn()?;
    let total = conn.query_row("SELECT COUNT(*) FROM curation_publications", [], |row| {
        row.get(0)
    })?;
    let ids = {
        let mut query=conn.prepare("SELECT id FROM curation_publications ORDER BY created DESC,rowid DESC LIMIT ?1 OFFSET ?2")?;
        query
            .query_map(params![limit, offset], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    let rows = ids
        .iter()
        .map(|id| summary(&conn, id))
        .collect::<Result<_>>()?;
    Ok(CurationHistoryPage {
        total,
        offset,
        limit,
        rows,
    })
}
pub(super) fn claim(
    service: &CurationService,
    track: &str,
    force: bool,
) -> Result<Option<Publication>> {
    let head = head(&service.library, track)?;
    if let Some(active) = head.active {
        return Ok(Some(load(service, &active)?));
    }
    if head.revision == head.acknowledged || head.state != "pending" {
        return Ok(None);
    }
    let catalog = service.catalog()?;
    let maps = head.mappings.clone();
    if !maps.is_empty() && !force {
        let updated: i64 = service.library.conn()?.query_row(
            "SELECT updated FROM curation_tracks WHERE track_id=?1",
            [track],
            |row| row.get(0),
        )?;
        if now().saturating_sub(updated) < 350 {
            return Ok(None);
        }
    }
    let id = uuid::Uuid::new_v4().to_string();
    let before = json(&head.committed)?;
    let after = json(&head.desired)?;
    let map_json = json(&maps)?;
    let created = now();
    let mut conn = service.library.conn()?;
    let tx = conn.transaction()?;
    ensure!(
        tax_revision(&tx)? == catalog.revision,
        "Category catalog changed before publication"
    );
    ensure!(tx.execute("UPDATE curation_tracks SET active_publication_id=?1,state='saving',error=NULL WHERE track_id=?2 AND revision=?3 AND active_publication_id IS NULL AND state='pending'",params![id,track,head.revision])?==1,"Curate publication was superseded");
    tx.execute("INSERT INTO curation_publications(id,track_id,revision,before_revision,before_json,after_json,taxonomy_revision,mappings_json,native_required,state,created,updated,predecessor_id,reexport,input_json,input_proof_json)VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,'saving',?10,?10,?11,?12,?13,?14)",params![id,track,head.revision,head.acknowledged,before,after,head.taxonomy_revision,map_json,head.native_required,created,head.last_native,head.reexport,json(&head.input)?,head.input_proof])?;
    tx.commit()?;
    drop(conn);
    Ok(Some(load(service, &id)?))
}
pub(super) fn commit_tx(conn: &Connection, id: &str) -> Result<()> {
    let(track,revision,before_revision,after,taxrev,undo):(String,u64,u64,String,u64,Option<String>)=conn.query_row("SELECT track_id,revision,before_revision,after_json,taxonomy_revision,undo_of FROM curation_publications WHERE id=?1 AND state IN ('saving','waiting_incoming','recovery_required')",[id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?)))?;
    let mut values: CurationValues = serde_json::from_str(&after)?;
    let (before_text, desired_text): (String,String) = conn.query_row("SELECT p.before_json,c.desired_json FROM curation_publications p JOIN curation_tracks c ON c.track_id=p.track_id WHERE p.id=?1",[id],|row|Ok((row.get(0)?,row.get(1)?)))?;
    let mut before: CurationValues = serde_json::from_str(&before_text)?;
    let mut desired: CurationValues = serde_json::from_str(&desired_text)?;
    let actual = categories(conn, &track)?;
    let mut external = false;
    let groups = before
        .categories
        .keys()
        .chain(values.categories.keys())
        .chain(actual.keys())
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    for group in groups {
        let expected = before.categories.get(&group);
        let published = values.categories.get(&group);
        let current = actual.get(&group);
        if expected != published {
            ensure!(
                current == expected,
                "Category assignment changed outside this pending Curate revision; refresh or discard before publishing"
            );
        } else if current != expected {
            external = true;
            if desired.categories.get(&group) == published {
                match current {
                    Some(ids) => {
                        desired.categories.insert(group.clone(), ids.clone());
                    }
                    None => {
                        desired.categories.remove(&group);
                    }
                }
            }
            match current {
                Some(ids) => {
                    before.categories.insert(group.clone(), ids.clone());
                    values.categories.insert(group, ids.clone());
                }
                None => {
                    before.categories.remove(&group);
                    values.categories.remove(&group);
                }
            }
        }
    }
    let after = json(&values)?;
    conn.execute(
        "UPDATE curation_publications SET before_json=?1,after_json=?2 WHERE id=?3",
        params![json(&before)?, after, id],
    )?;
    conn.execute(
        "UPDATE curation_tracks SET desired_json=?1 WHERE track_id=?2",
        params![json(&desired)?, track],
    )?;
    ensure!(conn.query_row("SELECT active_publication_id=?2 AND acknowledged_revision=?3 FROM curation_tracks WHERE track_id=?1",params![track,id,before_revision],|row|row.get::<_,bool>(0))?,"Curate acknowledgment no longer owns this publication");
    apply_values(conn, &track, &values)?;
    let native: bool = conn.query_row(
        "SELECT native_required FROM curation_publications WHERE id=?1",
        [id],
        |row| row.get(0),
    )?;
    let stale = native && (tax_revision(conn)? != taxrev || external);
    conn.execute("UPDATE curation_tracks SET acknowledged_revision=?1,committed_json=?2,state=CASE WHEN revision=?1 THEN 'saved' ELSE 'pending' END,error=NULL,active_publication_id=NULL,last_publication_id=?3,native_stale=CASE WHEN ?9 THEN ?5 WHEN ?8 THEN MAX(native_stale,?5) ELSE native_stale END,updated=?6,notes_search=?7,last_native_publication_id=CASE WHEN ?8 THEN ?3 ELSE last_native_publication_id END WHERE track_id=?4",params![revision,after,id,track,stale,now(),values.notes.to_lowercase(),native,conn.query_row("SELECT reexport FROM curation_publications WHERE id=?1",[id],|row|row.get::<_,bool>(0))?])?;
    if conn.query_row(
        "SELECT reexport FROM curation_publications WHERE id=?1",
        [id],
        |row| row.get::<_, bool>(0),
    )? && !stale
    {
        conn.execute(
            "DELETE FROM curation_native_stale WHERE track_id=?1",
            [&track],
        )?;
    }
    conn.execute(
        "UPDATE curation_publications SET state=?1,error=NULL,updated=?2 WHERE id=?3",
        params![if undo.is_some() { "undone" } else { "saved" }, now(), id],
    )?;
    if let Some(original) = undo {
        conn.execute(
            "UPDATE curation_publications SET state='undone' WHERE id=?1 AND state='saved'",
            [original],
        )?;
    }
    Library::bump(conn)?;
    Ok(())
}
fn local(service: &CurationService, publication: &Publication) -> Result<()> {
    let mut conn = service.library.conn()?;
    let tx = conn.transaction()?;
    commit_tx(&tx, &publication.public.summary.id)?;
    tx.commit()?;
    Ok(())
}
pub(super) fn flush(
    service: &CurationService,
    limit: usize,
    cancel: &CancellationToken,
    force: bool,
) -> Result<CurationFlushResult> {
    ensure!(
        (1..=200).contains(&limit),
        "Curate flush requires limit1..200"
    );
    let ids = {
        let conn = service.library.conn()?;
        let mut query=conn.prepare("SELECT track_id FROM curation_tracks WHERE state IN ('pending','saving','waiting_incoming','recovery_required') ORDER BY updated,track_id LIMIT ?1")?;
        query
            .query_map([limit], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut result = CurationFlushResult::default();
    for track in ids {
        cancel.check()?;
        let publication = match claim(service, &track, force) {
            Ok(Some(publication)) => publication,
            Ok(None) => continue,
            Err(error) => {
                let message = format!("{error:#}");
                let mut conn = service.library.conn()?;
                let tx = conn.transaction()?;
                tx.execute("UPDATE curation_publications SET state='error',error=?1,updated=?2 WHERE id=(SELECT active_publication_id FROM curation_tracks WHERE track_id=?3) AND state NOT IN ('saved','undone')",params![message,now(),track])?;
                tx.execute("UPDATE curation_tracks SET state='error',error=?1,updated=?2 WHERE track_id=?3",params![message,now(),track])?;
                tx.commit()?;
                drop(conn);
                service.emit(CurationEvent {
                    track_id: Some(track),
                    revision: 0,
                    state: "error".into(),
                });
                result.failed += 1;
                continue;
            }
        };
        let outcome = if publication.public.summary.native_required {
            native(service, &publication, cancel)
        } else {
            local(service, &publication)
        };
        if let Err(error) = outcome {
            let message = format!("{error:#}");
            let id = &publication.public.summary.id;
            let state = if cancel.is_cancelled() {
                "recovery_required"
            } else {
                "error"
            };
            service.library.conn()?.execute("UPDATE curation_publications SET state=?1,error=?2,updated=?3 WHERE id=?4 AND state NOT IN ('saved','undone')",params![state,message,now(),id])?;
            service.library.conn()?.execute("UPDATE curation_tracks SET state=?1,error=?2,updated=?3 WHERE track_id=?4 AND active_publication_id=?5",params![state,message,now(),track,id])?;
            result.failed += 1;
        } else {
            result.processed += 1;
        }
    }
    let events = {
        let conn = service.library.conn()?;
        let mut query=conn.prepare("SELECT id,track_id,revision,state FROM curation_publications WHERE notified=0 AND state IN ('saved','undone','error','recovery_required') ORDER BY created LIMIT 200")?;
        query
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, u64>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    for (id, track, revision, state) in events {
        service.emit(CurationEvent {
            track_id: Some(track),
            revision,
            state,
        });
        service.library.conn()?.execute(
            "UPDATE curation_publications SET notified=1 WHERE id=?1",
            [id],
        )?;
    }
    result.pending = super::outbox::pending(service, 0, 1)?.total;
    Ok(result)
}
fn native(
    _service: &CurationService,
    _publication: &Publication,
    _cancel: &CancellationToken,
) -> Result<()> {
    #[cfg(feature = "autotag")]
    {
        super::native::publish(_service, _publication, _cancel)
    }
    #[cfg(not(feature = "autotag"))]
    {
        anyhow::bail!("Native Curate mappings require a build with native tagging support")
    }
}
pub(super) fn retry(
    service: &CurationService,
    track: &str,
    revision: u64,
    cancel: &CancellationToken,
) -> Result<CurationTrackState> {
    let head = head(&service.library, track)?;
    ensure!(
        head.revision == revision,
        "Curate draft changed; reload before Retry"
    );
    if head.revision == head.acknowledged && head.active.is_none() {
        return Ok(public(head));
    }
    if let Some(active) = head.active {
        #[cfg(feature = "autotag")]
        {
            let job: Option<String> = service.library.conn()?.query_row("SELECT o.job_id FROM curation_publications p JOIN metadata_edit_overrides o ON o.batch_id=p.child_batch_id JOIN jobs j ON j.id=o.job_id JOIN tracks t ON t.id=o.track_id WHERE p.id=?1 AND o.state IN ('queued','claimed') AND j.state='attention' AND t.current_job=j.id",[&active],|row|row.get(0)).optional()?;
            if let Some(job) = job {
                service.library.retry(&job, None)?;
            }
        }
        service.library.conn()?.execute("UPDATE curation_publications SET state='recovery_required',error=NULL WHERE id=?1 AND state IN ('error','recovery_required','waiting_incoming')",[active])?;
        service.library.conn()?.execute(
            "UPDATE curation_tracks SET state='recovery_required',error=NULL WHERE track_id=?1",
            [track],
        )?;
    } else {
        service.library.conn()?.execute(
            "UPDATE curation_tracks SET state='pending',error=NULL WHERE track_id=?1",
            [track],
        )?;
    }
    service.flush_pending(200, cancel)?;
    service.wake();
    Ok(public(super::storage::head(&service.library, track)?))
}
pub(super) fn undo(
    service: &CurationService,
    id: &str,
    cancel: &CancellationToken,
) -> Result<CurationPublication> {
    undo_with(service, id, cancel, |_| Ok(()))
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum UndoBoundary {
    IntentSaved,
    #[cfg(feature = "autotag")]
    ChildLinked,
}
pub(super) fn undo_with(
    service: &CurationService,
    id: &str,
    cancel: &CancellationToken,
    mut boundary: impl FnMut(UndoBoundary) -> Result<()>,
) -> Result<CurationPublication> {
    let _lease = service.publication_lease()?;
    cancel.check()?;
    let original = load(service, id)?;
    ensure!(
        original.public.summary.can_undo,
        "Curate publication is no longer the current owned revision"
    );

    #[cfg(feature = "autotag")]
    if original.public.summary.native_required {
        crate::MetadataEditService::new(
            service.library.clone(),
            service
                .runtime
                .clone()
                .context("Native tagging runtime unavailable")?,
        )
        .verify_curation_undo_source(
            original
                .public
                .child_batch_id
                .as_deref()
                .context("Missing native Curate child")?,
            cancel,
        )?;
    }
    let track = &original.public.summary.track_id;
    let before = head(&service.library, track)?;
    let revision = before.revision + 1;
    let undo_id = uuid::Uuid::new_v4().to_string();
    let desired = json(&original.public.before)?;
    let actual = json(&before.committed)?;
    let predecessor: Option<String> = service.library.conn()?.query_row(
        "SELECT predecessor_id FROM curation_publications WHERE id=?1",
        [id],
        |row| row.get(0),
    )?;
    let mappings = json(&super::mapping::owned(service, predecessor.clone())?)?;
    let restored_taxonomy: u64 = service.library.conn()?.query_row("SELECT COALESCE((SELECT prior.taxonomy_revision FROM curation_publications current LEFT JOIN curation_publications prior ON prior.id=current.predecessor_id WHERE current.id=?1),0)",[id],|row|row.get(0))?;
    let mut conn = service.library.conn()?;
    let tx = conn.transaction()?;
    ensure!(tx.execute("UPDATE curation_tracks SET revision=?1,desired_json=?2,active_publication_id=?3,state='saving' WHERE track_id=?4 AND revision=?5 AND acknowledged_revision=?5 AND active_publication_id IS NULL AND last_publication_id=?6",params![revision,desired,undo_id,track,before.revision,id])?==1,"Later Curate edits block Undo");
    tx.execute("INSERT INTO curation_publications(id,track_id,revision,before_revision,before_json,after_json,taxonomy_revision,mappings_json,native_required,state,undo_of,created,updated,input_json,input_proof_json)VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?11,'saving',?9,?10,?10,?12,?13)",params![undo_id,track,revision,before.acknowledged,actual,desired,restored_taxonomy,mappings,id,now(),original.public.summary.native_required,json(&before.current_input)?,before.input_proof])?;
    if !original.public.summary.native_required {
        commit_tx(&tx, &undo_id)?;
    }
    tx.execute(
        "UPDATE curation_publications SET predecessor_id=?1 WHERE id=?2",
        params![predecessor, undo_id],
    )?;
    tx.commit()?;
    drop(conn);
    boundary(UndoBoundary::IntentSaved)?;
    if original.public.summary.native_required {
        #[cfg(feature = "autotag")]
        if let Err(error) = super::native::undo_with(
            service,
            &load(service, &undo_id)?,
            original
                .public
                .child_batch_id
                .as_deref()
                .context("Missing native Curate child")?,
            cancel,
            || boundary(UndoBoundary::ChildLinked),
        ) {
            let message = format!("{error:#}");
            service.library.conn()?.execute("UPDATE curation_publications SET state='recovery_required',error=?1 WHERE id=?2 AND state NOT IN ('saved','undone')",params![message,undo_id])?;
            service.library.conn()?.execute("UPDATE curation_tracks SET state='recovery_required',error=?1 WHERE active_publication_id=?2",params![message,undo_id])?;
            return Err(error);
        }
        #[cfg(not(feature = "autotag"))]
        anyhow::bail!("Native Curate Undo requires native tagging support");
    }
    service.emit(CurationEvent {
        track_id: Some(track.clone()),
        revision,
        state: "undone".into(),
    });
    Ok(load(service, &undo_id)?.public)
}
