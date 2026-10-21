//! JSON automation in the packaged desktop executable, using the same library lease/services.
use anyhow::{Context, Result, ensure};
use cuerithm_library::{Library, OrganizationPreviewRequest, TrackQuery};
use cuerithm_media::CancellationToken;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{io::Read, path::Path};

#[derive(Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
enum Command {
    Status,
    Report {
        scope: ReportScope,
    },
    Playlist {
        track_ids: Vec<String>,
    },
    OrganizePreview {
        request: OrganizationPreviewRequest,
    },
    OrganizePage {
        preview_id: String,
        offset: usize,
        limit: usize,
    },
    OrganizeApply {
        preview_id: String,
        revision: String,
    },
    OrganizeRecover {
        batch_id: String,
    },
    OrganizeHistory,
    MetadataDraft {
        track_ids: Vec<String>,
    },
    MetadataUpdate {
        draft_id: String,
        revision: u64,
        patch: cuerithm_library::MetadataEditorPatch,
    },
    MetadataPreview {
        draft_id: String,
        revision: u64,
    },
    MetadataApply {
        preview_id: String,
    },
    MetadataRecover {
        batch_id: String,
    },
    PrepareQueue {
        paths: Vec<std::path::PathBuf>,
        timeout_seconds: u64,
    },
    RetryPreparation {
        job_ids: Vec<String>,
        timeout_seconds: u64,
    },
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ReportScope {
    AllReady,
    Selected { track_ids: Vec<String> },
    Failed,
    Review,
}

/// Exit 0: complete, 2: partial/blocked, 3: busy, 1: invalid request/failure.
pub fn run(library_path: &Path) -> i32 {
    let request = match read_request(std::io::stdin().lock()) {
        Ok(v) => v,
        Err(_) => {
            emit(
                json!({"status":"error","error":"Expected one bounded JSON automation command on stdin"}),
            );
            return 1;
        }
    };
    if !library_path.is_file() {
        emit(json!({"status":"error","error":"Choose an existing CueRithm library database"}));
        return 1;
    }
    let library = match Library::open(library_path) {
        Ok(l) => l,
        Err(e) => {
            let busy = e.to_string().contains("already open");
            emit(
                json!({"status":if busy{"busy"}else{"error"},"error":if busy{"This library is open in another process"}else{"The library could not be opened"}}),
            );
            return if busy { 3 } else { 1 };
        }
    };
    match execute(&library, request) {
        Ok((data, partial)) => {
            emit(json!({"status":if partial{"partial"}else{"complete"},"result":data}));
            if partial { 2 } else { 0 }
        }
        Err(e) => {
            emit(json!({"status":"error","error":e.to_string()}));
            1
        }
    }
}
fn emit(value: Value) {
    println!("{value}");
}
fn read_request(reader: impl Read) -> Result<Command> {
    let mut bytes = Vec::new();
    reader.take(1_048_577).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 1_048_576, "Automation request exceeds 1 MiB");
    Ok(serde_json::from_slice(&bytes)?)
}
fn selected_ids(ids: Vec<String>) -> Result<Vec<String>> {
    ensure!(
        !ids.is_empty() && ids.len() <= 10_000,
        "Choose between 1 and 10000 tracks"
    );
    let mut seen = std::collections::HashSet::new();
    ensure!(
        ids.iter()
            .all(|id| !id.is_empty() && id.len() <= 128 && seen.insert(id.clone())),
        "Invalid or duplicate track identity"
    );
    Ok(ids)
}
fn execute(library: &Library, command: Command) -> Result<(Value, bool)> {
    let cancel = CancellationToken::default();
    match command {
        Command::Status => Ok((
            json!({"counts":library.counts()?,"revision":library.revision()?,"recent_jobs":library.jobs()?,"recent_jobs_limit":1000}),
            false,
        )),
        Command::OrganizePreview { request } => {
            let result = library.preview_organization(request, &cancel)?;
            let partial = result.totals.blocked > 0 || result.totals.skipped > 0;
            Ok((serde_json::to_value(result)?, partial))
        }
        Command::OrganizePage {
            preview_id,
            offset,
            limit,
        } => {
            ensure!(
                (1..=200).contains(&limit),
                "Page size must be between 1 and 200"
            );
            Ok((
                serde_json::to_value(library.organization_preview_page(
                    &preview_id,
                    offset,
                    limit,
                )?)?,
                false,
            ))
        }
        Command::OrganizeApply {
            preview_id,
            revision,
        } => batch(library.apply_organization_preview(&preview_id, &revision, &cancel)?),
        Command::OrganizeRecover { batch_id } => {
            batch(library.recover_organization_batch(&batch_id, &cancel)?)
        }
        Command::OrganizeHistory => Ok((
            serde_json::to_value(library.organization_batches()?)?,
            false,
        )),
        Command::MetadataDraft { track_ids } => {
            let service = editor(library)?;
            Ok((
                serde_json::to_value(service.create_draft(&selected_ids(track_ids)?)?)?,
                false,
            ))
        }
        Command::MetadataUpdate {
            draft_id,
            revision,
            patch,
        } => {
            let service = editor(library)?;
            Ok((
                serde_json::to_value(service.update_draft(&draft_id, revision, patch)?)?,
                false,
            ))
        }
        Command::MetadataPreview { draft_id, revision } => {
            let service = editor(library)?;
            let preview = service.preview(&draft_id, revision, &cancel)?;
            let partial = preview.rows.iter().any(|r| r.outcome == "blocked");
            Ok((serde_json::to_value(preview)?, partial))
        }
        Command::MetadataApply { preview_id } => {
            metadata_batch(editor(library)?.apply(&preview_id, &cancel)?)
        }
        Command::MetadataRecover { batch_id } => {
            metadata_batch(editor(library)?.recover(&batch_id, &cancel)?)
        }
        Command::PrepareQueue {
            paths,
            timeout_seconds,
        } => prepare(library, paths, None, timeout_seconds),
        Command::RetryPreparation {
            job_ids,
            timeout_seconds,
        } => prepare(
            library,
            Vec::new(),
            Some(selected_ids(job_ids)?),
            timeout_seconds,
        ),
        Command::Playlist { track_ids } => {
            let mut playlist = String::from("#EXTM3U\n");
            for id in selected_ids(track_ids)? {
                let track = library.track(&id)?;
                ensure!(
                    track.state == "ready" && !track.archived,
                    "Only ready tracks can be exported"
                );
                let path = library.track_path(&id)?;
                let path = path.to_str().context("Playlist path is not Unicode")?;
                ensure!(
                    !path.contains(['\r', '\n', '\0']),
                    "Playlist path contains a line separator"
                );
                playlist.push_str(path);
                playlist.push('\n');
            }
            Ok((json!({"format":"m3u8","playlist":playlist}), false))
        }
        Command::Report { scope } => report(library, scope),
    }
}
fn batch(result: cuerithm_library::OrganizationBatch) -> Result<(Value, bool)> {
    let t = &result.totals;
    let partial = t.failed + t.blocked + t.skipped + t.recovery_required + t.cancelled > 0;
    Ok((serde_json::to_value(result)?, partial))
}
fn editor(library: &Library) -> Result<cuerithm_library::MetadataEditService> {
    let (media, _) = cuerithm_desktop_lib::automation_context(library)?;
    Ok(cuerithm_library::MetadataEditService::new(
        library.clone(),
        media,
    ))
}
fn metadata_batch(result: cuerithm_library::MetadataEditBatch) -> Result<(Value, bool)> {
    let partial = !matches!(result.state.as_str(), "saved" | "undone");
    Ok((serde_json::to_value(result)?, partial))
}
fn pending_scope(ids: &[String], jobs: &[cuerithm_library::JobSummary]) -> (bool, Vec<String>) {
    let current = jobs
        .iter()
        .map(|j| j.track_id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let missing = ids
        .iter()
        .filter(|id| !current.contains(id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let pending = jobs
        .iter()
        .any(|j| matches!(j.state.as_str(), "pending" | "queued" | "running"));
    (pending, missing)
}
fn prepare(
    library: &Library,
    paths: Vec<std::path::PathBuf>,
    retry: Option<Vec<String>>,
    timeout: u64,
) -> Result<(Value, bool)> {
    ensure!(
        (1..=86400).contains(&timeout),
        "Preparation timeout must be between 1 and 86400 seconds"
    );
    ensure!(
        paths.len() <= 10000,
        "Preparation path scope exceeds 10000 entries"
    );
    let (media, catalog) = cuerithm_desktop_lib::automation_context(library)?;
    let mut accepted = Vec::new();
    let mut failures = Vec::new();
    let service;
    if let Some(ids) = retry {
        service = cuerithm_library::PreparationService::start_with_catalog_runtime(
            library.clone(),
            media,
            catalog,
        )?;
        // A later bad ID cannot hide an earlier accepted retry.
        for id in ids {
            match library.retry(&id, None) {
                Ok(id) => accepted.push(id),
                Err(e) => failures.push(json!({"job_id":id,"error":e.to_string()})),
            }
        }
    } else {
        let before = library.automation_jobs()?;
        accepted.extend(
            before
                .iter()
                .filter(|j| matches!(j.state.as_str(), "pending" | "running" | "queued"))
                .map(|j| j.id.clone()),
        );
        service = cuerithm_library::PreparationService::start_with_catalog_runtime(
            library.clone(),
            media,
            catalog,
        )?;
        for path in paths {
            match service.enqueue_paths(std::slice::from_ref(&path)) {
                Ok(ids) => accepted.extend(ids),
                Err(e) => failures.push(json!({"path":path,"error":e.to_string()})),
            }
        }
        // A folder discovery can durably accept some files before encountering an error.
        let before_ids = before
            .iter()
            .map(|j| j.id.as_str())
            .collect::<std::collections::HashSet<_>>();
        accepted.extend(
            library
                .automation_jobs()?
                .into_iter()
                .filter(|j| !before_ids.contains(j.id.as_str()))
                .map(|j| j.id),
        );
    }
    accepted.sort();
    accepted.dedup();
    let tracks = library.automation_track_ids_for_jobs(&accepted)?;
    let started = std::time::Instant::now();
    let mut timed_out = false;
    loop {
        let jobs = library.automation_jobs_for_tracks(&tracks)?;
        let (pending, missing) = pending_scope(&tracks, &jobs);
        if !pending || !missing.is_empty() {
            break;
        }
        if started.elapsed() >= std::time::Duration::from_secs(timeout) {
            timed_out = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    service.shutdown();
    let jobs = library.automation_jobs_for_tracks(&tracks)?;
    let (_, missing) = pending_scope(&tracks, &jobs);
    let partial = timed_out
        || !failures.is_empty()
        || !missing.is_empty()
        || jobs.iter().any(|j| j.state != "completed");
    Ok((
        json!({"timed_out":timed_out,"track_ids":tracks,"jobs":jobs,"unresolved_track_ids":missing,"failures":failures}),
        partial,
    ))
}
fn report(library: &Library, scope: ReportScope) -> Result<(Value, bool)> {
    let (ids, jobs) = match scope {
        ReportScope::Selected { track_ids } => {
            let ids = selected_ids(track_ids)?;
            let jobs = library.automation_jobs_for_tracks(&ids)?;
            (ids, jobs)
        }
        ReportScope::Failed | ReportScope::Review => {
            let jobs = library.automation_jobs()?;
            let review = matches!(scope, ReportScope::Review);
            let mut ids = Vec::new();
            let mut seen = std::collections::HashSet::new();
            for j in &jobs {
                let include = if review {
                    j.state == "attention" && library.track_match_review(&j.track_id)?.is_some()
                } else {
                    j.state == "attention"
                };
                if include && seen.insert(j.track_id.clone()) {
                    ids.push(j.track_id.clone());
                }
            }
            (ids, jobs)
        }
        ReportScope::AllReady => {
            let mut q = TrackQuery {
                limit: 200,
                ..Default::default()
            };
            let mut ids = Vec::new();
            loop {
                let page = library.tracks(q.clone())?;
                q.expected_revision = Some(page.revision);
                ids.extend(page.rows.into_iter().map(|t| t.id));
                ensure!(
                    ids.len() <= 100_000,
                    "Report scope exceeds 100000 tracks; select tracks"
                );
                q.cursor = page.next_cursor;
                if q.cursor.is_none() {
                    break;
                }
            }
            let jobs = library.automation_jobs_for_tracks(&ids)?;
            (ids, jobs)
        }
    };
    let mut outcomes_by_track = std::collections::HashMap::<&str, Vec<Value>>::new();
    for j in &jobs {
        outcomes_by_track
            .entry(j.track_id.as_str())
            .or_default()
            .push(json!({"job_id":j.id,"state":j.state,"stage":j.stage,"attempts":j.attempts}));
    }
    let mut rows = Vec::new();
    let mut partial = false;
    for id in ids {
        let track = library.track(&id)?;
        let outcomes = outcomes_by_track.remove(id.as_str()).unwrap_or_default();
        partial |= track.state != "ready";
        // Deliberate export allowlist: no raw provider payload, credentials, platform IDs or notes.
        rows.push(json!({"track_id":id,"title":track.title,"artist":track.artist,"state":track.state,"jobs":outcomes}));
    }
    Ok((json!({"schema_version":1,"rows":rows}), partial))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_command_rejects_unknown_keys_and_duplicate_scope() {
        assert!(
            read_request(br#"{"command":"status","secret":"not-accepted"}"#.as_slice()).is_err()
        );
        assert!(selected_ids(vec!["a".into(), "a".into()]).is_err());
        assert!(read_request(vec![b' '; 1_048_577].as_slice()).is_err());
    }
    #[test]
    fn preparation_waits_for_current_attempt_on_the_same_logical_track() {
        let ids = vec!["track".into()];
        let jobs = vec![cuerithm_library::JobSummary {
            id: "new-attempt".into(),
            track_id: "track".into(),
            title: String::new(),
            state: "running".into(),
            stage: "analysis".into(),
            error: None,
            attempts: 1,
        }];
        assert_eq!(pending_scope(&ids, &jobs), (true, vec![]));
        assert_eq!(pending_scope(&ids, &[]), (false, ids));
    }
    #[test]
    fn report_uses_owned_library_and_export_allowlist() {
        let dir = tempfile::tempdir().unwrap();
        let l = Library::open(&dir.path().join("library.db")).unwrap();
        let (value, partial) = execute(
            &l,
            Command::Report {
                scope: ReportScope::AllReady,
            },
        )
        .unwrap();
        assert_eq!(value["schema_version"], 1);
        assert!(value["rows"].as_array().unwrap().is_empty());
        assert!(!partial);
        assert!(Library::open(&dir.path().join("library.db")).is_err());
    }
}
