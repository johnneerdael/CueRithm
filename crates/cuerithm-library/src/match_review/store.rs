use super::*;
use crate::{
    JobRun, Library,
    model::{now, path_bytes},
    organization::{Authority, Fingerprint, NativePath},
};
use anyhow::{Context, Result, ensure};
use cuerithm_autotag::{AutotagConfig, CatalogMatch, TagInput, TagMatchOutcome};
use cuerithm_media::MediaInfo;
use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
pub(super) const MAX_BYTES: usize = 2 * 1024 * 1024;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SavedCandidate {
    pub id: String,
    pub matched: Option<CatalogMatch>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Capture {
    pub public: MatchReviewState,
    pub path: NativePath,
    pub parent: Authority,
    pub fingerprint: Fingerprint,
    pub path_revision: u64,
    pub metadata_revision: u64,
    pub track_state: String,
    pub job_settings_revision: String,
    pub config: AutotagConfig,
    pub media: Option<MediaInfo>,
    pub candidates: Vec<SavedCandidate>,
}
pub(super) fn digest(value: &impl Serialize) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(value)?)))
}
fn text(value: &str) -> String {
    value.chars().take(256).collect()
}
fn tag(media: &MediaInfo, key: &str) -> Option<String> {
    media
        .tags
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .map(|(_, v)| text(v))
        .filter(|s| !s.trim().is_empty())
}
pub(super) fn candidate_public(
    id: String,
    matched: &CatalogMatch,
    input: &TagInput,
    config: &AutotagConfig,
    selectable: bool,
) -> Result<MatchReviewCandidate> {
    let track = &matched.track;
    matched.validate()?;
    let provider = cuerithm_autotag::ProviderId::parse(&track.platform)
        .context("Unrecognized candidate provider")?;
    Ok(MatchReviewCandidate {
        id,
        provider: track.platform.clone(),
        track_id: track.track_id.as_deref().map(text),
        release_id: track.release_id.as_deref().map(text),
        title: text(&track.title),
        version: track.version.as_deref().map(text),
        artists: track.artists.iter().take(16).map(|v| text(v)).collect(),
        album: track.album.as_deref().map(text),
        isrc: track.isrc.as_deref().map(text),
        duration_seconds: (!track.duration.is_zero()).then_some(track.duration.as_secs_f64()),
        similarity: matched.accuracy,
        identity_conflicts: cuerithm_autotag::Tagger::review_match_conflicts(
            input, matched, config,
        )?,
        source_url: cuerithm_autotag::review_catalog_url(provider, &track.url, false),
        artwork_url: matched
            .musicbrainz()
            .and_then(|facts| facts.cover.as_ref())
            .map(|cover| {
                format!(
                    "https://coverartarchive.org/release/{}/{}",
                    cover.release_id,
                    cover.image_id.as_deref().unwrap_or("front")
                )
            })
            .or_else(|| {
                track
                    .art
                    .as_deref()
                    .and_then(|url| cuerithm_autotag::review_catalog_url(provider, url, true))
            }),
        facts: matched.facts.clone(),
        selectable,
        reason: (!selectable).then(|| {
            "Complete candidate exceeds the evidence budget; requery before selecting it".into()
        }),
    })
}
impl Capture {
    pub(super) fn input(&self) -> Result<TagInput> {
        Ok(TagInput {
            path: self.path.path()?,
            source_sha256: self.public.source_sha256.clone(),
            media: self.media.clone().context(
                "Complete query metadata is unavailable; requery before choosing a candidate",
            )?,
            duration_seconds: self.public.source.duration_seconds,
        })
    }
}
impl Library {
    pub(crate) fn capture_match_review(
        &self,
        run: &JobRun,
        input: &TagInput,
        outcome: &TagMatchOutcome,
    ) -> Result<Capture> {
        let config = run
            .snapshot
            .settings
            .tagging
            .as_ref()
            .context("Missing tagging configuration")?
            .clone();
        let (path, path_revision, metadata_revision, state): (Vec<u8>, u64, u64, String) =
            self.conn()?.query_row(
                "SELECT path,path_revision,metadata_revision,state FROM tracks WHERE id=?1",
                [&run.track_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )?;
        ensure!(
            path == path_bytes(&run.path),
            "Source path changed before candidate capture"
        );
        let (parent, fingerprint) = crate::organization::capture_regular_source(&run.path)?;
        ensure!(
            fingerprint.sha256 == run.source_sha256,
            "Source changed before candidate capture"
        );
        for candidate in &outcome.matches {
            candidate.validate()?;
            if candidate.recognition.is_some() {
                ensure!(
                    cuerithm_autotag::Tagger::review_candidate_provider_enabled(
                        input, candidate, &config
                    )?,
                    "Recognition candidate does not belong to this consented source"
                );
            }
        }
        let info = input.audio_info(&config)?;
        let generation = uuid::Uuid::new_v4().to_string();
        let mut capture = Capture {
            public: MatchReviewState {
                track_id: run.track_id.clone(),
                job_id: Some(run.id.clone()),
                attempt: run.attempt,
                generation: generation.clone(),
                source_sha256: run.source_sha256.clone(),
                settings_revision: run.settings_revision.clone(),
                status: serde_json::to_value(outcome.status)?
                    .as_str()
                    .context("Invalid match status")?
                    .into(),
                track_state: state.clone(),
                can_keep_existing_once: false,
                source: MatchReviewSource {
                    title: tag(&input.media, "title"),
                    artists: info.artists.iter().take(16).map(|a| text(a)).collect(),
                    album: tag(&input.media, "album"),
                    version: tag(&input.media, "version").or_else(|| tag(&input.media, "mix")),
                    duration_seconds: input.duration_seconds,
                    has_artwork: cuerithm_autotag::has_embedded_artwork(&run.path, &input.media)
                        .unwrap_or(false),
                },
                candidates: vec![],
                truncated: outcome.matches.len() > 10,
                can_requery: true,
                reason: None,
                providers: vec![],
                warnings: outcome
                    .warnings
                    .iter()
                    .map(|w| MatchReviewWarning {
                        provider: w.provider.as_str().into(),
                        kind: w.kind,
                        message: w.message.chars().take(2048).collect(),
                    })
                    .collect(),
            },
            path: NativePath(path),
            parent,
            fingerprint,
            path_revision,
            metadata_revision,
            track_state: state,
            job_settings_revision: run.settings_revision.clone(),
            config: config.clone(),
            media: Some(input.media.clone()),
            candidates: vec![],
        };
        if serde_json::to_vec(&capture)?.len() > MAX_BYTES / 2 {
            capture.media = None;
            capture.public.truncated = true;
        }
        let mut used = serde_json::to_vec(&capture)?.len();
        for (index, matched) in outcome.matches.iter().take(10).enumerate() {
            let id = digest(&(&generation, index, matched))?;
            let bytes = serde_json::to_vec(matched)?.len();
            let mut public = candidate_public(id.clone(), matched, input, &config, true)?;
            let public_bytes = serde_json::to_vec(&public)?.len();
            let selectable = capture.media.is_some()
                && used
                    .saturating_add(bytes)
                    .saturating_add(public_bytes)
                    .saturating_add(128)
                    < MAX_BYTES;
            if selectable {
                used = used
                    .saturating_add(bytes)
                    .saturating_add(public_bytes)
                    .saturating_add(128);
            } else {
                capture.public.truncated = true;
                public.facts = None;
                public.selectable = false;
                public.reason = Some(
                    "Complete candidate exceeds the evidence budget; requery before selecting it"
                        .into(),
                );
                used = used
                    .saturating_add(serde_json::to_vec(&public)?.len())
                    .saturating_add(128);
            }
            capture.public.candidates.push(public);
            capture.candidates.push(SavedCandidate {
                id,
                matched: selectable.then(|| matched.clone()),
            });
        }
        ensure!(
            serde_json::to_vec(&capture)?.len() <= MAX_BYTES,
            "Review capture exceeded its bounded storage"
        );
        Ok(capture)
    }
    pub(crate) fn store_match_review_tx(
        tx: &Transaction<'_>,
        run: &JobRun,
        capture: &Capture,
    ) -> Result<()> {
        let current: (Vec<u8>, u64, u64) = tx.query_row(
            "SELECT path,path_revision,metadata_revision FROM tracks WHERE id=?1",
            [&run.track_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        ensure!(
            current
                == (
                    capture.path.0.clone(),
                    capture.path_revision,
                    capture.metadata_revision
                ),
            "Track changed while catalog evidence was captured"
        );
        tx.execute(
            "DELETE FROM preparation_match_previews WHERE track_id=?1",
            [&run.track_id],
        )?;
        tx.execute("INSERT INTO preparation_match_generations(track_id,job_id,generation,attempt,source_hash,settings_revision,payload_json,created) VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(track_id) DO UPDATE SET job_id=excluded.job_id,generation=excluded.generation,attempt=excluded.attempt,source_hash=excluded.source_hash,settings_revision=excluded.settings_revision,payload_json=excluded.payload_json,created=excluded.created",params![capture.public.track_id,capture.public.job_id,capture.public.generation,capture.public.attempt,capture.public.source_sha256,capture.public.settings_revision,serde_json::to_string(capture)?,now()])?;
        Ok(())
    }
    pub(super) fn load_review_capture(&self, track: &str) -> Result<Option<Capture>> {
        let json: Option<String> = self
            .conn()?
            .query_row(
                "SELECT payload_json FROM preparation_match_generations WHERE track_id=?1",
                [track],
                |r| r.get(0),
            )
            .optional()?;
        json.map(|json| {
            ensure!(
                json.len() <= MAX_BYTES,
                "Review evidence exceeds storage limit"
            );
            Ok(serde_json::from_str(&json)?)
        })
        .transpose()
    }
    pub(super) fn validate_review_capture_tx(
        conn: &rusqlite::Connection,
        capture: &Capture,
    ) -> Result<()> {
        let (path,hash,pr,mr,state,archived,job,attempt,revision):(Vec<u8>,String,u64,u64,String,bool,Option<String>,u32,String)=conn.query_row("SELECT t.path,t.source_hash,t.path_revision,t.metadata_revision,t.state,t.archived,t.current_job,COALESCE(j.attempts,0),COALESCE(j.settings_revision,'') FROM tracks t LEFT JOIN jobs j ON j.id=t.current_job WHERE t.id=?1",[&capture.public.track_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?)))?;
        ensure!(
            path == capture.path.0
                && hash == capture.public.source_sha256
                && pr == capture.path_revision
                && mr == capture.metadata_revision
                && state == capture.track_state
                && !archived
                && job == capture.public.job_id
                && attempt == capture.public.attempt
                && revision == capture.job_settings_revision,
            "Review source, metadata, settings or execution attempt changed; reload and requery"
        );
        Ok(())
    }
    pub(super) fn validate_review_capture(&self, capture: &Capture) -> Result<()> {
        Self::validate_review_capture_tx(&*self.conn()?, capture)?;
        let (parent, fingerprint) =
            crate::organization::capture_regular_source(&capture.path.path()?)?;
        ensure!(
            parent.path == capture.parent.path
                && parent.ancestors == capture.parent.ancestors
                && fingerprint.identity == capture.fingerprint.identity
                && fingerprint.sha256 == capture.fingerprint.sha256
                && fingerprint.bytes == capture.fingerprint.bytes,
            "Native source changed after catalog review; reload and requery"
        );
        Ok(())
    }
    /// Transport and privacy permission are live machine settings, never accepted metadata policy.
    pub(crate) fn current_catalog_lookup_config(
        &self,
        captured: &AutotagConfig,
    ) -> Result<AutotagConfig> {
        let live = self.settings()?.settings.tagging.unwrap_or_default();
        let solver = live.solver;
        let mut current = captured.clone();
        current.solver.endpoint = solver.endpoint;
        current.solver.timeout_seconds = solver.timeout_seconds;
        current.recognition.fingerprint_consent &= live.recognition.fingerprint_consent;
        current.recognition.enabled &=
            live.recognition.enabled && current.recognition.fingerprint_consent;
        Ok(current)
    }
    pub(crate) fn validate_catalog_run(&self, job: &JobRun) -> Result<()> {
        let current: bool = self.conn()?.query_row(
            "SELECT EXISTS(SELECT 1 FROM jobs j JOIN tracks t ON t.id=j.track_id WHERE j.id=?1 AND j.track_id=?2 AND j.state='running' AND j.attempts=?3 AND j.settings_revision=?4 AND j.source_hash=?5 AND t.current_job=j.id AND t.archived=0 AND t.source_hash=j.source_hash AND t.path=?6)",
            params![job.id, job.track_id, job.attempt, job.settings_revision, job.source_sha256, path_bytes(&job.path)],
            |row| row.get(0),
        )?;
        ensure!(current, "Preparation was superseded before catalog lookup");
        Ok(())
    }
    pub(crate) fn recognition_request_guard(
        &self,
        job: &JobRun,
        captured: &AutotagConfig,
    ) -> cuerithm_autotag::CatalogRequestGuard {
        let library = self.clone();
        let job = job.clone();
        let captured = captured.clone();
        std::sync::Arc::new(move || {
            library.validate_catalog_run(&job)?;
            let config = library.current_catalog_lookup_config(&captured)?;
            ensure!(
                config.recognition.enabled && config.recognition.fingerprint_consent,
                "Fingerprint transmission consent was revoked or identification is disabled"
            );
            Ok(())
        })
    }
    pub(super) fn recognized_review_access(
        &self,
        capture: &Capture,
    ) -> Result<Option<MatchReviewProviderAccess>> {
        if !capture
            .candidates
            .iter()
            .filter_map(|candidate| candidate.matched.as_ref())
            .any(|candidate| candidate.recognition.is_some())
        {
            return Ok(None);
        }
        let input = capture.input()?;
        let mut access_config = self.current_catalog_lookup_config(&capture.config)?;
        let recognized = capture
            .candidates
            .iter()
            .filter_map(|candidate| candidate.matched.as_ref())
            .any(|candidate| {
                candidate.recognition.is_some()
                    && cuerithm_autotag::Tagger::review_candidate_provider_enabled(
                        &input,
                        candidate,
                        &access_config,
                    )
                    .unwrap_or(false)
            });
        if !recognized {
            return Ok(None);
        }
        access_config.options.platforms = vec!["musicbrainz".into()];
        Ok(self.review_access_for_lookup(&access_config)?.pop())
    }
    pub(super) fn review_access(
        &self,
        config: Option<&AutotagConfig>,
    ) -> Result<Vec<MatchReviewProviderAccess>> {
        let Some(config) = config else {
            return Ok(vec![]);
        };
        self.review_access_for_lookup(&self.current_catalog_lookup_config(config)?)
    }
    pub(super) fn review_access_for_lookup(
        &self,
        config: &AutotagConfig,
    ) -> Result<Vec<MatchReviewProviderAccess>> {
        use cuerithm_autotag::ChallengeStore;
        let has_solver = config.solver.api_url()?.is_some();
        let setup = self
            .shared_catalog_runtime()?
            .map(|runtime| self.provider_runtime_with(runtime))
            .transpose()?;
        config.options.platforms.iter().map(|id|{
            let requires_solver=self.requires_solver(id)?;
            let provider=cuerithm_autotag::ProviderId::parse(id);
            let configured=provider.map(|provider|setup.as_ref().map(|setup|setup.status(provider)).transpose()).transpose()?.flatten();
            let needs_native_setup=matches!(provider,Some(cuerithm_autotag::ProviderId::Sonovault|cuerithm_autotag::ProviderId::Bpmsupreme));
            let capability=configured.as_ref().map_or(!needs_native_setup,|status|status.can_use);
            let available=provider.is_some()&&capability&&(!requires_solver||has_solver);
            let reason=if requires_solver&&!has_solver {
                Some("Unavailable: configure a FlareSolverr-compatible solver in Automatic tagging settings".into())
            }else if !capability {
                Some(configured.and_then(|status|status.reason.map(|reason|reason.message)).unwrap_or_else(||"Set up this optional provider in Automatic tagging settings".into()))
            }else{None};
            Ok(MatchReviewProviderAccess{id:id.clone(),available,requires_solver,reason})
        }).collect()
    }
    pub(super) fn initial_review_capture(&self, track: &str) -> Result<Capture> {
        type InitialRow = (
            Option<Vec<u8>>,
            Option<String>,
            String,
            bool,
            Option<String>,
            u32,
            String,
            Option<String>,
            Option<f64>,
        );
        let(path,hash,state,archived,job,attempt,job_revision,metadata,duration):InitialRow=self.conn()?.query_row("SELECT t.path,t.source_hash,t.state,t.archived,t.current_job,COALESCE(j.attempts,0),COALESCE(j.settings_revision,''),t.metadata_json,t.duration FROM tracks t LEFT JOIN jobs j ON j.id=t.current_job WHERE t.id=?1",[track],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?)))?;
        let path =
            path.context("Local file is not linked; relocate the track before reviewing matches")?;
        let hash =
            hash.context("File identity is unknown; analyze the track before reviewing matches")?;
        ensure!(
            !archived,
            "Restore this track before reviewing catalog matches"
        );
        let native_path = NativePath(path);
        let (parent, fingerprint) =
            crate::organization::capture_regular_source(&native_path.path()?)?;
        ensure!(
            fingerprint.sha256 == hash,
            "Source bytes changed; refresh the track before catalog review"
        );
        let snapshot = match self.current_preparation_snapshot_for_path(&native_path.path()?) {
            Ok(snapshot) => snapshot,
            // A disconnected current preset must not hide an existing Incoming
            // Keep-once capability. This placeholder never starts a query; the
            // live capability projection below reports the connection blocker.
            Err(_) if !job_revision.is_empty() => self.settings_revision(&job_revision)?,
            Err(_) => self.settings()?,
        };
        let config = snapshot.settings.tagging.clone().unwrap_or_default();
        let media: Option<MediaInfo> = metadata.and_then(|s| serde_json::from_str(&s).ok());
        let (pr, mr) = self.conn()?.query_row(
            "SELECT path_revision,metadata_revision FROM tracks WHERE id=?1",
            [track],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Ok(Capture {
            public: MatchReviewState {
                track_id: track.into(),
                job_id: job,
                attempt,
                generation: uuid::Uuid::new_v4().to_string(),
                source_sha256: hash,
                settings_revision: snapshot.revision,
                status: "no_match".into(),
                track_state: state.clone(),
                can_keep_existing_once: false,
                source: MatchReviewSource {
                    title: media.as_ref().and_then(|m| tag(m, "title")),
                    artists: media
                        .as_ref()
                        .and_then(|m| tag(m, "artist"))
                        .into_iter()
                        .collect(),
                    album: media.as_ref().and_then(|m| tag(m, "album")),
                    version: media.as_ref().and_then(|m| tag(m, "version")),
                    duration_seconds: duration.unwrap_or(0.),
                    has_artwork: false,
                },
                candidates: vec![],
                truncated: false,
                can_requery: false,
                reason: Some(
                    "No retained catalog query; explicitly Requery to search configured providers"
                        .into(),
                ),
                providers: vec![],
                warnings: vec![],
            },
            path: native_path,
            parent,
            fingerprint,
            path_revision: pr,
            metadata_revision: mr,
            track_state: state,
            job_settings_revision: job_revision,
            config,
            media,
            candidates: vec![],
        })
    }
    pub fn track_match_review(&self, track_id: &str) -> Result<Option<MatchReviewState>> {
        let capture = match self.load_review_capture(track_id)? {
            Some(capture) if self.validate_review_capture(&capture).is_ok() => capture,
            _ => {
                let capture = self.initial_review_capture(track_id)?;
                let mut conn = self.conn()?;
                let tx = conn.transaction()?;
                Self::validate_review_capture_tx(&tx, &capture)?;
                tx.execute(
                    "DELETE FROM preparation_match_previews WHERE track_id=?1",
                    [track_id],
                )?;
                tx.execute("INSERT INTO preparation_match_generations(track_id,job_id,generation,attempt,source_hash,settings_revision,payload_json,created) VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(track_id) DO UPDATE SET job_id=excluded.job_id,generation=excluded.generation,attempt=excluded.attempt,source_hash=excluded.source_hash,settings_revision=excluded.settings_revision,payload_json=excluded.payload_json,created=excluded.created",params![track_id,capture.public.job_id,capture.public.generation,capture.public.attempt,capture.public.source_sha256,capture.public.settings_revision,serde_json::to_string(&capture)?,now()])?;
                tx.commit()?;
                capture
            }
        };
        let current = self.current_preparation_snapshot_for_path(&capture.path.path()?);
        let recognized_access = self.recognized_review_access(&capture)?;
        let mut public = capture.public;
        let attention = public
            .job_id
            .as_deref()
            .map(|job| {
                self.conn()?
                    .query_row(
                        "SELECT state='attention' FROM jobs WHERE id=?1",
                        [job],
                        |r| r.get::<_, bool>(0),
                    )
                    .map_err(anyhow::Error::from)
            })
            .transpose()?
            .unwrap_or(false);
        public.can_keep_existing_once = public.track_state == "incoming" && attention;
        let current_error = match current {
            Ok(snapshot) => {
                public.providers = self.review_access(snapshot.settings.tagging.as_ref())?;
                None
            }
            Err(error) => {
                public.providers = self.review_access(Some(&capture.config))?;
                Some(format!(
                    "Current preparation preset is unavailable: {error:#}"
                ))
            }
        };
        public.can_requery = current_error.is_none()
            && (public.track_state == "ready" || attention)
            && public.providers.iter().any(|p| p.available);
        if let Some(access) = recognized_access
            && !public
                .providers
                .iter()
                .any(|provider| provider.id == access.id)
        {
            public.providers.push(access);
        }
        if let Some(error) = current_error {
            public.reason = Some(error);
        } else if public.providers.is_empty() {
            public.reason = Some(
                "Configure catalog providers in Automatic tagging settings before Requery".into(),
            );
        } else if !public.providers.iter().any(|p| p.available) {
            public.reason=Some("Configured catalog providers are unavailable; configure a FlareSolverr-compatible solver".into());
        } else if public.track_state == "incoming" && !attention {
            public.reason =
                Some("Preparation is running or queued; wait for its result before Requery".into());
        }
        Ok(Some(public))
    }
}
