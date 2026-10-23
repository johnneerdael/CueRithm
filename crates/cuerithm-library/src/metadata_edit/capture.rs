use super::*;
use crate::organization::{Authority, Fingerprint, NativePath, capture_regular_source};
use anyhow::{Context, Result, ensure};
use cuerithm_media::CancellationToken;
use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Display {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Capture {
    pub public: MetadataEditTrack,
    pub path: NativePath,
    pub parent: Authority,
    pub fingerprint: Fingerprint,
    pub job: Option<String>,
    pub attempt: Option<u32>,
    pub display: Display,
}
type TrackSnapshot = (
    Option<Vec<u8>>,
    Option<String>,
    String,
    String,
    bool,
    u64,
    u64,
    Option<String>,
    Option<u32>,
    Option<String>,
    Option<f64>,
    Option<u8>,
    Option<f64>,
);
type RevisionSnapshot = (
    Option<Vec<u8>>,
    Option<String>,
    String,
    bool,
    u64,
    u64,
    Option<String>,
    Option<u32>,
);
impl MetadataEditService {
    pub(super) fn capture(&self, id: &str, cancel: &CancellationToken) -> Result<Capture> {
        cancel.check()?;
        let (
            path,
            hash,
            title,
            state,
            archived,
            path_revision,
            metadata_revision,
            job,
            attempt,
            key,
            bpm,
            energy,
            duration,
        ): TrackSnapshot = {
            let conn = self.library.conn()?;
            conn.query_row("SELECT t.path,t.source_hash,t.title,t.state,t.archived,t.path_revision,t.metadata_revision,t.current_job,j.attempts,t.musical_key,t.bpm,t.energy,t.duration FROM tracks t LEFT JOIN jobs j ON j.id=t.current_job WHERE t.id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?,r.get(9)?,r.get(10)?,r.get(11)?,r.get(12)?)))?
        };
        let path =
            path.context("Local file is not linked; relocate the track before editing metadata")?;
        let hash =
            hash.context("File identity is unknown; analyze the track before editing metadata")?;
        ensure!(
            !archived && matches!(state.as_str(), "ready" | "incoming"),
            "Choose an active Ready or Incoming track"
        );
        let path = crate::model::bytes_path(&path)?;
        ensure!(
            !path.starts_with(self.library.data_dir().canonicalize()?),
            "Application artifacts cannot be edited"
        );
        let (parent, fingerprint) = capture_regular_source(&path)?;
        ensure!(
            fingerprint.sha256 == hash,
            "Source changed; refresh preparation before editing"
        );
        let info = self.runtime.probe_cancellable(&path, cancel)?;
        let native = cuerithm_autotag::read_metadata_snapshot(&path, &info)?;
        cancel.check()?;
        let (_, after) = capture_regular_source(&path)?;
        ensure!(
            after.identity == fingerprint.identity && after.sha256 == fingerprint.sha256,
            "Source changed while reading metadata"
        );
        let display = {
            let conn = self.library.conn()?;
            conn.query_row(
                "SELECT title,artist,album,genre FROM tracks WHERE id=?1",
                [id],
                |r| {
                    Ok(Display {
                        title: r.get(0)?,
                        artist: r.get(1)?,
                        album: r.get(2)?,
                        genre: r.get(3)?,
                    })
                },
            )?
        };
        let capture = Capture {
            display,
            public: MetadataEditTrack {
                track_id: id.into(),
                title,
                display_path: path.to_string_lossy().into_owned(),
                state,
                source_sha256: hash,
                path_revision,
                metadata_revision,
                native,
                measured: serde_json::json!({"key":key,"bpm":bpm,"energy":energy,"duration_seconds":duration}),
            },
            path: NativePath::new(&path),
            parent,
            fingerprint,
            job,
            attempt,
        };
        Self::validate_capture(&*self.library.conn()?, &capture, None)?;
        Ok(capture)
    }
    pub(super) fn validate_capture(
        conn: &rusqlite::Connection,
        capture: &Capture,
        after: Option<&str>,
    ) -> Result<()> {
        let current:RevisionSnapshot=conn.query_row("SELECT t.path,t.source_hash,t.state,t.archived,t.path_revision,t.metadata_revision,t.current_job,j.attempts FROM tracks t LEFT JOIN jobs j ON j.id=t.current_job WHERE t.id=?1",[&capture.public.track_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?)))?;
        ensure!(
            current.0.as_deref() == Some(capture.path.0.as_slice())
                && current.1.as_deref() == Some(after.unwrap_or(&capture.fingerprint.sha256))
                && current.2 == capture.public.state
                && !current.3
                && current.4 == capture.public.path_revision
                && current.5 == capture.public.metadata_revision + u64::from(after.is_some())
                && current.6 == capture.job
                && current.7 == capture.attempt,
            "Metadata/source/path/preparation changed; preview again"
        );
        Ok(())
    }
    pub(super) fn verify_source(capture: &Capture) -> Result<()> {
        let (parent, fingerprint) = capture_regular_source(&capture.path.path()?)?;
        ensure!(
            parent.ancestors == capture.parent.ancestors
                && fingerprint.identity == capture.fingerprint.identity
                && fingerprint.sha256 == capture.fingerprint.sha256,
            "Source native identity or bytes changed; preview again"
        );
        Ok(())
    }
    pub(super) fn capture_ids(
        &self,
        ids: &[String],
        cancel: &CancellationToken,
    ) -> Result<Vec<Capture>> {
        ensure!(
            !ids.is_empty() && ids.len() <= 100_000,
            "Choose 1 to 100000 tracks"
        );
        let mut seen = std::collections::BTreeSet::new();
        let mut out = vec![];
        let mut bytes = 0usize;
        for id in ids {
            if seen.insert(id) {
                let capture = self.capture(id, cancel)?;
                bytes = bytes.saturating_add(bounded_json(&capture)?.len());
                ensure!(
                    bytes <= MAX_METADATA_JSON,
                    "Complete metadata snapshot exceeds64MiB; narrow selection"
                );
                out.push(capture);
            }
        }
        Ok(out)
    }
}
