//! Complete bounded outcome scopes for automation; the UI's recent-jobs limit is not a report.
use crate::{JobSummary, Library};
use anyhow::{Result, ensure};
use rusqlite::OptionalExtension;
impl Library {
    /// Resolve durable logical track ownership even after an attempt was superseded.
    pub fn automation_track_ids_for_jobs(&self, ids: &[String]) -> Result<Vec<String>> {
        ensure!(ids.len() <= 100_000, "Automation job scope exceeds limit");
        let conn = self.conn()?;
        let mut query = conn.prepare("SELECT track_id FROM jobs WHERE id=?1")?;
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for id in ids {
            let track: String = query.query_row([id], |r| r.get(0))?;
            if seen.insert(track.clone()) {
                out.push(track);
            }
        }
        Ok(out)
    }
    /// Selected reports never scan or fail on unrelated library jobs.
    pub fn automation_jobs_for_tracks(&self, ids: &[String]) -> Result<Vec<JobSummary>> {
        ensure!(ids.len() <= 100_000, "Automation track scope exceeds limit");
        let conn = self.conn()?;
        let mut query=conn.prepare("SELECT j.id,j.track_id,t.title,j.state,j.stage,j.error,j.attempts FROM tracks t JOIN jobs j ON j.id=t.current_job WHERE t.id=?1 AND j.state!='superseded'")?;
        let mut out = Vec::new();
        for id in ids {
            if let Some(row) = query
                .query_row([id], |r| {
                    Ok(JobSummary {
                        id: r.get(0)?,
                        track_id: r.get(1)?,
                        title: r.get(2)?,
                        state: r.get(3)?,
                        stage: r.get(4)?,
                        error: r.get(5)?,
                        attempts: r.get(6)?,
                    })
                })
                .optional()?
            {
                out.push(row);
            }
        }
        Ok(out)
    }
    pub fn automation_jobs(&self) -> Result<Vec<JobSummary>> {
        let conn = self.conn()?;
        let mut query=conn.prepare("SELECT j.id,j.track_id,t.title,j.state,j.stage,j.error,j.attempts FROM jobs j JOIN tracks t ON t.current_job=j.id WHERE j.state!='superseded' ORDER BY j.created DESC,j.id LIMIT 100001")?;
        let rows = query
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
            .collect::<rusqlite::Result<Vec<_>>>()?;
        ensure!(
            rows.len() <= 100_000,
            "Automation job scope exceeds 100000 current jobs; narrow the library scope"
        );
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automation_follows_a_superseded_attempt_to_its_current_logical_track() {
        let temporary = tempfile::tempdir().unwrap();
        let library = Library::open(&temporary.path().join("library.sqlite")).unwrap();
        let source = temporary.path().join("source.flac");
        std::fs::write(&source, b"initial source bytes").unwrap();
        let old = library
            .enqueue(
                std::slice::from_ref(&source),
                library.settings().unwrap().revision,
            )
            .unwrap()
            .remove(0);
        let run = library.claim_next().unwrap().unwrap();
        std::fs::write(&source, b"replacement source bytes").unwrap();
        let next = library.retry_for_run(&run).unwrap();
        assert_ne!(old, next);
        let tracks = library
            .automation_track_ids_for_jobs(std::slice::from_ref(&old))
            .unwrap();
        assert_eq!(tracks, [run.track_id]);
        let jobs = library.automation_jobs_for_tracks(&tracks).unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].id, next);
        assert_eq!(jobs[0].state, "queued");
        assert!(
            !library
                .automation_jobs()
                .unwrap()
                .iter()
                .any(|job| job.id == old)
        );
    }

    #[test]
    fn automation_selected_scope_excludes_unrelated_current_jobs() {
        let temporary = tempfile::tempdir().unwrap();
        let library = Library::open(&temporary.path().join("library.sqlite")).unwrap();
        let paths = [
            temporary.path().join("first.flac"),
            temporary.path().join("second.flac"),
        ];
        for path in &paths {
            std::fs::write(path, b"synthetic source").unwrap();
        }
        let ids = library
            .enqueue(&paths, library.settings().unwrap().revision)
            .unwrap();
        let tracks = library.automation_track_ids_for_jobs(&ids[..1]).unwrap();
        let jobs = library.automation_jobs_for_tracks(&tracks).unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].id, ids[0]);
        assert_eq!(library.automation_jobs().unwrap().len(), 2);
    }
}
