use super::{
    store::{Capture, candidate_public, digest},
    *,
};
use crate::{Library, MetadataArtworkAsset, MetadataEditService, PreparationService, model::now};
use anyhow::{Context, Result, ensure};
use cuerithm_autotag::{ArtworkIdentity, CatalogMatch, MetadataPatch, Tagger};
use cuerithm_media::{CancellationToken, MediaRuntime};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Preview {
    pub public: MatchReviewPreview,
    pub capture: Capture,
    pub request: MatchReviewRequest,
    pub selected: Option<CatalogMatch>,
    pub patch: MetadataPatch,
    pub artwork_asset: Option<MetadataArtworkAsset>,
    pub artwork_target: Option<ArtworkIdentity>,
}
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Decision {
    pub capture: Capture,
    pub request: MatchReviewRequest,
    pub selected: Option<CatalogMatch>,
    pub patch: MetadataPatch,
    pub artwork_asset: Option<MetadataArtworkAsset>,
    pub artwork_target: Option<ArtworkIdentity>,
    pub preview_id: String,
}
impl Library {
    pub(crate) fn preview_match_review_with(
        &self,
        tagger: &Tagger,
        runtime: &MediaRuntime,
        request: MatchReviewRequest,
        cancel: &CancellationToken,
    ) -> Result<MatchReviewPreview> {
        cancel.check()?;
        let capture = self
            .load_review_capture(&request.track_id)?
            .context("No retained candidate generation; requery explicitly")?;
        ensure!(
            request.generation == capture.public.generation,
            "Candidate generation changed; discard the old selection and requery"
        );
        self.validate_review_capture(&capture)?;
        let mut blockers = vec![];
        if capture.track_state == "incoming" {
            let job = capture
                .public
                .job_id
                .as_deref()
                .context("Incoming has no current preparation job")?;
            let attention: bool = self.conn()?.query_row(
                "SELECT state='attention' FROM jobs WHERE id=?1",
                [job],
                |r| r.get(0),
            )?;
            if !attention {
                blockers.push(
                    "Preparation is running or queued; wait for its result before accepting review"
                        .into(),
                );
            }
        }
        let selected = match request.action {
            MatchReviewAction::KeepExistingOnce => {
                ensure!(
                    request.candidate_id.is_none() && request.fields.is_empty(),
                    "Keep existing once cannot include candidate field changes"
                );
                if capture.track_state != "incoming" {
                    blockers.push("Keep existing once resolves Incoming exceptions; Ready tracks use explicit metadata changes".into());
                }
                None
            }
            MatchReviewAction::UseSelected => {
                let id = request
                    .candidate_id
                    .as_deref()
                    .context("Choose a retained candidate")?;
                let candidate =
                    capture.candidates.iter().find(|c| c.id == id).context(
                        "Candidate does not belong to this source/configuration generation",
                    )?;
                let full = candidate.matched.as_ref().context(
                    "Complete candidate is unavailable; explicitly requery before selecting it",
                )?;
                {
                    let lookup_config = self.current_catalog_lookup_config(&capture.config)?;
                    let access = self.review_access_for_lookup(&lookup_config)?;
                    if let Some(blocked) = access
                        .iter()
                        .find(|a| a.id == full.track.platform && !a.available)
                    {
                        blockers.push(
                            blocked
                                .reason
                                .clone()
                                .unwrap_or_else(|| "Provider is unavailable".into()),
                        );
                        Some(full.clone())
                    } else {
                        Some(tagger.expand_review_candidate(
                            &capture.input()?,
                            full,
                            &lookup_config,
                            &cancel.shared_flag(),
                        )?)
                    }
                }
            }
        };
        cancel.check()?;
        self.validate_review_capture(&capture)?;
        let editor = MetadataEditService::new(self.clone(), runtime.clone());
        let native = editor
            .snapshot_cancellable(std::slice::from_ref(&request.track_id), cancel)?
            .tracks
            .into_iter()
            .next()
            .context("Missing native metadata source")?
            .native;
        let (fields, mut patch, field_blockers) = super::fields::make_fields(
            &native,
            selected.as_ref().map(|c| &c.track),
            &request,
            capture.config.options.short_title,
        )?;
        blockers.extend(field_blockers);
        if request.action == MatchReviewAction::UseSelected {
            for required in &capture.config.required_tags {
                let name = serde_json::to_value(required)?
                    .as_str()
                    .context("Invalid required field")?
                    .to_owned();
                if let Some(field) = fields
                    .iter()
                    .find(|field| field.field.eq_ignore_ascii_case(&name))
                {
                    if field.after.iter().all(|value| value.trim().is_empty()) {
                        blockers.push(format!("Required {} is missing after this review; choose a value or correct its required-field policy",field.label));
                    }
                } else if *required != cuerithm_autotag::SupportedTag::AlbumArt {
                    let mut single = capture.config.clone();
                    single.required_tags = vec![*required];
                    if cuerithm_autotag::validate_review_required_fields(
                        &capture.path.path()?,
                        &capture.input()?.media,
                        &single,
                    )
                    .is_err()
                    {
                        blockers.push(format!("Required {name} cannot be supplied by the selected review fields; correct its required-field policy before accepting"));
                    }
                }
            }
        }
        patch.allow_id3_upgrade = request.allow_id3_upgrade;
        let requires_id3_upgrade = native
            .capabilities
            .get("id3_version")
            .is_some_and(|v| v != "2.4")
            && (fields
                .iter()
                .any(|f| f.action != MatchReviewFieldAction::Keep && f.after.len() > 1)
                || matches!(patch.release_date, cuerithm_autotag::ValueEdit::Set { .. })
                || matches!(patch.original_date, cuerithm_autotag::ValueEdit::Set { .. }));
        if requires_id3_upgrade && !request.allow_id3_upgrade {
            blockers.push("These ordered values/date precision require ID3v2.4; explicitly allow conversion and preview again".into());
        }
        let candidate = selected
            .as_ref()
            .map(|selected| {
                candidate_public(
                    request.candidate_id.clone().unwrap(),
                    selected,
                    &capture.input()?,
                    &capture.config,
                    true,
                )
            })
            .transpose()?;
        if let Some(candidate) = &candidate
            && !candidate.identity_conflicts.is_empty()
            && !request.acknowledge_conflicts
        {
            blockers.push(format!(
                "Explicitly acknowledge the selected recording differences: {}",
                candidate.identity_conflicts.join(", ")
            ));
        }
        let art_action = request
            .fields
            .iter()
            .find(|f| f.field.eq_ignore_ascii_case("albumArt"))
            .map(|f| f.action)
            .unwrap_or(MatchReviewFieldAction::Keep);
        let art_url = candidate.as_ref().and_then(|c| c.artwork_url.clone());
        let mut asset: Option<MetadataArtworkAsset> = None;
        let mut artwork_target: Option<ArtworkIdentity> = None;
        let mut art_reason: Option<String> = None;
        if art_action != MatchReviewFieldAction::Keep {
            if art_action != MatchReviewFieldAction::Replace {
                art_reason=Some("Artwork supports Keep or Replace; choose exact additional covers in the metadata editor".into());
            } else if art_url.is_none() {
                art_reason = Some("The selected candidate has no supported artwork URL".into());
            } else {
                let cached:Option<(String,String)>=self.conn()?.query_row("SELECT url_digest,asset_json FROM preparation_review_artwork WHERE generation=?1 AND candidate_id=?2",params![request.generation,request.candidate_id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
                if let Some((url_hash, json)) = cached {
                    ensure!(
                        url_hash == digest(&art_url)?,
                        "Candidate artwork URL changed; request a new preview"
                    );
                    asset = Some(serde_json::from_str(&json)?);
                } else {
                    art_reason=Some("Load the bounded artwork preview before replacing the embedded front cover".into());
                }
                let fronts = native
                    .artwork
                    .iter()
                    .filter(|p| p.identity.kind.is_none() || p.identity.kind == Some(3))
                    .collect::<Vec<_>>();
                if fronts.len() > 1 {
                    art_reason=Some("Multiple front covers exist; choose an exact artwork record in the metadata editor".into());
                } else {
                    artwork_target = fronts.first().map(|p| p.identity.clone());
                }
            }
        }
        if let Some(reason) = &art_reason {
            blockers.push(reason.clone());
        }
        if request.action == MatchReviewAction::UseSelected
            && capture
                .config
                .required_tags
                .contains(&cuerithm_autotag::SupportedTag::AlbumArt)
            && native.artwork.is_empty()
            && asset.is_none()
        {
            blockers.push("Required artwork is absent; load and explicitly replace the front cover before accepting".into());
        }

        let public = MatchReviewPreview {
            id: uuid::Uuid::new_v4().to_string(),
            revision: digest(&(
                &capture.public,
                &capture.path,
                &capture.parent,
                &capture.fingerprint,
                capture.path_revision,
                capture.metadata_revision,
                &request,
                &selected,
                &patch,
                &asset,
                &artwork_target,
            ))?,
            track_id: request.track_id.clone(),
            job_id: capture.public.job_id.clone(),
            attempt: capture.public.attempt,
            generation: request.generation.clone(),
            action: request.action,
            candidate,
            fields,
            artwork: MatchReviewArtwork {
                existing: !native.artwork.is_empty(),
                candidate_url: art_url,
                action: art_action,
                reason: art_reason,
            },
            requires_id3_upgrade,
            can_apply: blockers.is_empty(),
            blockers,
        };
        let preview = Preview {
            public: public.clone(),
            capture,
            request,
            selected,
            patch,
            artwork_asset: asset,
            artwork_target,
        };
        let payload = serde_json::to_string(&preview)?;
        ensure!(
            payload.len() <= 8 * 1024 * 1024,
            "Review preview exceeds bounded storage"
        );
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        Self::validate_review_capture_tx(&tx, &preview.capture)?;
        ensure!(
            tx.query_row(
                "SELECT generation=?2 FROM preparation_match_generations WHERE track_id=?1",
                params![public.track_id, public.generation],
                |r| r.get::<_, bool>(0)
            )?,
            "Catalog query changed while previewing"
        );
        tx.execute("INSERT INTO preparation_match_previews(id,track_id,generation,revision,payload_json,state,created) VALUES(?1,?2,?3,?4,?5,'preview',?6)",params![public.id,public.track_id,public.generation,public.revision,payload,now()])?;
        tx.commit()?;
        Ok(public)
    }
    pub(super) fn load_match_preview(&self, id: &str, revision: &str) -> Result<Preview> {
        let (state, saved, json): (String, String, String) = self.conn()?.query_row(
            "SELECT state,revision,payload_json FROM preparation_match_previews WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        ensure!(
            state == "preview" && saved == revision,
            "Match review preview changed or was already consumed"
        );
        ensure!(
            json.len() <= 8 * 1024 * 1024,
            "Review preview exceeds storage limit"
        );
        let preview: Preview = serde_json::from_str(&json)?;
        ensure!(
            preview.public.revision == revision,
            "Review preview identity differs"
        );
        Ok(preview)
    }
    pub(crate) fn apply_match_review_with(
        &self,
        runtime: &MediaRuntime,
        id: &str,
        revision: &str,
        cancel: &CancellationToken,
    ) -> Result<MatchReviewApplyResult> {
        cancel.check()?;
        let preview = self.load_match_preview(id, revision)?;
        ensure!(
            preview.public.can_apply,
            "Resolve the review blockers before accepting it"
        );
        self.validate_review_capture(&preview.capture)?;
        if preview.capture.track_state == "ready" {
            ensure!(
                preview.public.action == MatchReviewAction::UseSelected,
                "Ready review accepts explicit metadata changes only"
            );
            let mut conn = self.conn()?;
            let tx = conn.transaction()?;
            Self::validate_review_capture_tx(&tx, &preview.capture)?;
            ensure!(
                tx.query_row(
                    "SELECT generation=?2 FROM preparation_match_generations WHERE track_id=?1",
                    params![preview.public.track_id, preview.public.generation],
                    |r| r.get::<_, bool>(0)
                )?,
                "Candidate generation changed before accepting review"
            );
            ensure!(tx.execute("UPDATE preparation_match_previews SET state='delegated' WHERE id=?1 AND revision=?2 AND state='preview'",params![id,revision])?==1,"Review was already accepted");
            tx.commit()?;
            drop(conn);
            let patch = self.materialize_match_patch(
                runtime,
                &preview.patch,
                preview.artwork_asset.as_ref(),
                preview.artwork_target.as_ref(),
            )?;
            if patch.is_noop() {
                return Ok(MatchReviewApplyResult {
                    state: "unchanged".into(),
                    job_id: None,
                    metadata_batch: None,
                });
            }
            let batch = MetadataEditService::new(self.clone(), runtime.clone())
                .apply_reviewed_metadata_patch(
                    &preview.public.track_id,
                    &preview.public_source_hash(),
                    preview.capture.path_revision,
                    preview.capture.metadata_revision,
                    patch,
                    cancel,
                )?;
            self.conn()?.execute(
                "UPDATE preparation_match_previews SET state='applied' WHERE id=?1",
                [id],
            )?;
            return Ok(MatchReviewApplyResult {
                state: batch.state.clone(),
                job_id: None,
                metadata_batch: Some(batch),
            });
        }
        let _organization = self
            .inner
            .organization_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Organization gate interrupted"))?;
        let _tag = self
            .inner
            .tag_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Publication gate interrupted"))?;
        self.validate_review_capture(&preview.capture)?;
        let decision = Decision {
            capture: preview.capture.clone(),
            request: preview.request.clone(),
            selected: preview.selected.clone(),
            patch: preview.patch.clone(),
            artwork_asset: preview.artwork_asset.clone(),
            artwork_target: preview.artwork_target.clone(),
            preview_id: id.into(),
        };
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        Self::validate_review_capture_tx(&tx, &preview.capture)?;
        ensure!(
            tx.query_row(
                "SELECT generation=?2 FROM preparation_match_generations WHERE track_id=?1",
                params![preview.public.track_id, preview.public.generation],
                |r| r.get::<_, bool>(0)
            )?,
            "Candidate generation changed before accepting review"
        );
        ensure!(tx.execute("UPDATE preparation_match_previews SET state='applied' WHERE id=?1 AND revision=?2 AND state='preview'",params![id,revision])?==1,"Review was already accepted");
        let job = preview
            .public
            .job_id
            .as_deref()
            .context("Incoming has no preparation job")?;
        let job = Self::queue_preparation_edit_tx(
            &tx,
            &preview.public.track_id,
            job,
            preview.public.attempt,
            &preview.capture.public.source_sha256,
        )?;
        tx.execute(
            "UPDATE jobs SET settings_revision=?1 WHERE id=?2 AND state='queued'",
            params![preview.capture.public.settings_revision, job],
        )?;
        tx.execute("UPDATE metadata_edit_overrides SET settings_revision=?1 WHERE job_id=?2 AND state IN ('queued','claimed') AND source_hash=?3",params![preview.capture.public.settings_revision,job,preview.capture.public.source_sha256])?;
        super::worker::store_decision_tx(&tx, &job, &decision)?;
        Self::bump(&tx)?;
        tx.commit()?;
        Ok(MatchReviewApplyResult {
            state: "queued".into(),
            job_id: Some(job),
            metadata_batch: None,
        })
    }
    pub(super) fn materialize_match_patch(
        &self,
        runtime: &MediaRuntime,
        patch: &MetadataPatch,
        asset: Option<&MetadataArtworkAsset>,
        target: Option<&ArtworkIdentity>,
    ) -> Result<MetadataPatch> {
        let mut patch = patch.clone();
        if let Some(asset) = asset {
            let bytes =
                MetadataEditService::new(self.clone(), runtime.clone()).artwork_bytes(&asset.id)?;
            patch.artwork.push(cuerithm_autotag::ArtworkEdit {
                target: target.cloned(),
                replacement: Some(cuerithm_autotag::ArtworkValue {
                    kind: target.and_then(|t| t.kind).or(Some(3)),
                    description: target.and_then(|t| t.description.clone()),
                    bytes,
                }),
            });
        }
        patch.validate()?;
        Ok(patch)
    }
}
impl Preview {
    fn public_source_hash(&self) -> String {
        self.capture.public.source_sha256.clone()
    }
}
impl PreparationService {
    pub fn preview_match_review(
        &self,
        request: MatchReviewRequest,
        cancel: &CancellationToken,
    ) -> Result<MatchReviewPreview> {
        let (library, runtime, tagger) = self.match_review_context();
        library.preview_match_review_with(tagger, runtime, request, cancel)
    }
    pub fn apply_match_review(
        &self,
        id: &str,
        revision: &str,
        cancel: &CancellationToken,
    ) -> Result<MatchReviewApplyResult> {
        let (library, runtime, _) = self.match_review_context();
        let result = library.apply_match_review_with(runtime, id, revision, cancel)?;
        self.wake();
        Ok(result)
    }
    pub fn requery_matches(
        &self,
        track_id: &str,
        generation: &str,
        cancel: &CancellationToken,
    ) -> Result<MatchReviewState> {
        let (library, runtime, tagger) = self.match_review_context();
        library.requery_matches_with(tagger, runtime, track_id, generation, cancel)
    }
    pub fn review_artwork_asset(
        &self,
        preview_id: &str,
        revision: &str,
        cancel: &CancellationToken,
    ) -> Result<MetadataArtworkAsset> {
        let (library, runtime, _) = self.match_review_context();
        library.review_artwork_asset_with(
            runtime,
            preview_id,
            revision,
            cancel,
            cuerithm_autotag::fetch_catalog_artwork,
        )
    }
}

impl Library {
    pub(crate) fn requery_matches_with(
        &self,
        tagger: &Tagger,
        runtime: &MediaRuntime,
        track: &str,
        generation: &str,
        cancel: &CancellationToken,
    ) -> Result<MatchReviewState> {
        cancel.check()?;
        let state = self
            .track_match_review(track)?
            .context("Track is unavailable for catalog review")?;
        ensure!(
            state.generation == generation,
            "Catalog generation changed; reload before Requery"
        );
        ensure!(
            state.can_requery,
            "{}",
            state
                .reason
                .as_deref()
                .unwrap_or("Catalog providers are unavailable")
        );
        let before = self
            .load_review_capture(track)?
            .context("Missing review source capture")?;
        self.validate_review_capture(&before)?;
        let snapshot = self.current_preparation_snapshot_for_path(&before.path.path()?)?;
        let mut config = snapshot
            .settings
            .tagging
            .clone()
            .context("Configure catalog providers before Requery")?;
        config.options.skip_tagged = false;
        config.validate()?;
        if let Some(job) = before.public.job_id.as_deref() {
            ensure!(
                !self.conn()?.query_row("SELECT EXISTS(SELECT 1 FROM preparation_checkpoints WHERE job_id=?1 AND state!='complete' AND (state!='prepared' OR batch_id IS NOT NULL))",[job],|r|r.get::<_,bool>(0))?,
                "Prepared organization evidence must finish recovery before a new catalog query"
            );
        }
        let mut pending = before.clone();
        pending.public.generation = uuid::Uuid::new_v4().to_string();
        pending.public.candidates.clear();
        pending.candidates.clear();
        pending.public.status = "unavailable".into();
        pending.public.reason =
            Some("Catalog query in progress or interrupted; Requery again to replace it".into());
        pending.config = config.clone();
        pending.public.settings_revision = snapshot.revision.clone();
        {
            let mut conn = self.conn()?;
            let tx = conn.transaction()?;
            Self::validate_review_capture_tx(&tx, &before)?;
            ensure!(tx.execute("UPDATE preparation_match_generations SET generation=?1,settings_revision=?2,payload_json=?3 WHERE track_id=?4 AND generation=?5",params![pending.public.generation,snapshot.revision,serde_json::to_string(&pending)?,track,generation])?==1,"Another query replaced this generation");
            tx.execute("UPDATE preparation_match_previews SET state='invalidated' WHERE track_id=?1 AND state='preview'",[track])?;
            tx.commit()?;
        }
        let audio = runtime.decode(&before.path.path()?, &self.data_dir().join("cache/media"))?;
        cancel.check()?;
        ensure!(
            audio.source_sha256 == before.public.source_sha256,
            "Source changed before catalog query"
        );
        let mut input = cuerithm_autotag::TagInput {
            path: before.path.path()?,
            source_sha256: audio.source_sha256.clone(),
            media: audio.info.clone(),
            duration_seconds: audio.clock.duration_seconds(),
        };
        if let Some(job) = before.public.job_id.as_deref()
            && let Some(patch) =
                self.metadata_override_for_query(track, job, &input.source_sha256)?
        {
            super::effective::project_query(&mut input, &patch);
        }
        let outcome = match tagger.match_track_cancellable(&input, &config, &cancel.shared_flag()) {
            Ok(outcome) => outcome,
            Err(error) => {
                cancel.check()?;
                if let Some(error) = error.downcast_ref::<cuerithm_autotag::CatalogLookupError>() {
                    cuerithm_autotag::TagMatchOutcome {
                        status: error.outcome.status,
                        matches: error.outcome.matches.clone(),
                        attempted: error.outcome.attempted.clone(),
                        warnings: error
                            .outcome
                            .warnings
                            .iter()
                            .map(|w| cuerithm_autotag::ProviderFailure {
                                provider: w.provider,
                                kind: w.kind,
                                message: w.message.clone(),
                            })
                            .collect(),
                    }
                } else {
                    return Err(error);
                }
            }
        };
        cancel.check()?;
        self.validate_review_capture(&before)?;
        let mut capture_snapshot = snapshot.clone();
        capture_snapshot.settings.tagging = Some(config);
        let run = crate::JobRun {
            id: before.public.job_id.clone().unwrap_or_default(),
            track_id: track.into(),
            attempt: before.public.attempt,
            path: before.path.path()?,
            source_sha256: before.public.source_sha256.clone(),
            settings_revision: snapshot.revision.clone(),
            snapshot: capture_snapshot,
        };
        let mut capture = self.capture_match_review(&run, &input, &outcome)?;
        capture.public.job_id = before.public.job_id.clone();
        capture.job_settings_revision = before.job_settings_revision.clone();
        {
            let mut conn = self.conn()?;
            let tx = conn.transaction()?;
            Self::validate_review_capture_tx(&tx, &before)?;
            ensure!(
                tx.query_row(
                    "SELECT generation=?2 FROM preparation_match_generations WHERE track_id=?1",
                    params![track, pending.public.generation],
                    |r| r.get::<_, bool>(0)
                )?,
                "A newer query replaced this result"
            );
            Self::store_match_review_tx(&tx, &run, &capture)?;
            tx.commit()?;
        }
        self.track_match_review(track)?
            .context("Review source changed after catalog query")
    }
}

impl Library {
    pub(super) fn review_artwork_asset_with(
        &self,
        runtime: &MediaRuntime,
        id: &str,
        revision: &str,
        cancel: &CancellationToken,
        fetch: impl FnOnce(cuerithm_autotag::ProviderId, &str, &CancellationToken) -> Result<Vec<u8>>,
    ) -> Result<MetadataArtworkAsset> {
        cancel.check()?;
        let preview = self.load_match_preview(id, revision)?;
        self.validate_review_capture(&preview.capture)?;
        ensure!(
            preview.request.action == MatchReviewAction::UseSelected,
            "Choose a catalog candidate before loading artwork"
        );
        let candidate = preview
            .public
            .candidate
            .as_ref()
            .context("Selected candidate is missing")?;
        let url = candidate
            .artwork_url
            .as_deref()
            .context("Selected candidate has no supported artwork URL")?;
        let candidate_id = preview
            .request
            .candidate_id
            .as_deref()
            .context("Selected candidate token is missing")?;
        let url_hash = digest(&Some(url.to_owned()))?;
        if let Some((saved,json))=self.conn()?.query_row("SELECT url_digest,asset_json FROM preparation_review_artwork WHERE generation=?1 AND candidate_id=?2",params![preview.public.generation,candidate_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).optional()?{ensure!(saved==url_hash,"Artwork changed; request a fresh catalog generation");return Ok(serde_json::from_str(&json)?)}
        let provider = match candidate.provider.as_str() {
            "beatport" => cuerithm_autotag::ProviderId::Beatport,
            "traxsource" => cuerithm_autotag::ProviderId::Traxsource,
            _ => anyhow::bail!("Unsupported artwork provider"),
        };
        let bytes = fetch(provider, url, cancel)?;
        cancel.check()?;
        self.validate_review_capture(&preview.capture)?;
        let asset =
            MetadataEditService::new(self.clone(), runtime.clone()).import_artwork(bytes)?;
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        Self::validate_review_capture_tx(&tx, &preview.capture)?;
        ensure!(
            tx.query_row(
                "SELECT generation=?2 FROM preparation_match_generations WHERE track_id=?1",
                params![preview.public.track_id, preview.public.generation],
                |r| r.get::<_, bool>(0)
            )?,
            "Catalog generation changed while loading artwork"
        );
        ensure!(tx.query_row("SELECT state='preview' AND revision=?2 FROM preparation_match_previews WHERE id=?1",params![id,revision],|r|r.get::<_,bool>(0))?,"Review was accepted or invalidated while loading artwork");
        tx.execute("INSERT OR IGNORE INTO preparation_review_artwork(generation,candidate_id,url_digest,asset_json) VALUES(?1,?2,?3,?4)",params![preview.public.generation,candidate_id,url_hash,serde_json::to_string(&asset)?])?;
        let(saved,json):(String,String)=tx.query_row("SELECT url_digest,asset_json FROM preparation_review_artwork WHERE generation=?1 AND candidate_id=?2",params![preview.public.generation,candidate_id],|r|Ok((r.get(0)?,r.get(1)?)))?;
        ensure!(
            saved == url_hash,
            "Artwork URL changed while loading preview"
        );
        tx.commit()?;
        Ok(serde_json::from_str(&json)?)
    }
}
