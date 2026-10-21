//! Volumo's documented ISRC resolver and identity-bound public track metadata.
use crate::{
    AutotagConfig, BackendFailure, CatalogAuth, CatalogError, CatalogErrorKind, CatalogMatch,
    CatalogRuntime, CatalogService, ProviderBackend, ProviderFailureKind, ProviderId, TagInput,
};
use anyhow::{Context, Result, ensure};
use cuerithm_credentials::{
    CredentialRef, Presence, ProviderId as CredentialProvider, VaultState, Verification,
};
use onetagger_selected_providers::http::{CatalogHttp, ChallengeStore};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
mod page;
#[cfg(test)]
mod tests;

pub struct VolumoBackend {
    runtime: Arc<CatalogRuntime>,
    store: Arc<dyn ChallengeStore>,
    pages: CatalogHttp,
}
impl VolumoBackend {
    pub fn new(runtime: Arc<CatalogRuntime>, store: Arc<dyn ChallengeStore>) -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(runtime.identity().user_agent())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let gate = runtime.clone();
        let pages = CatalogHttp::new(client, "volumo", store.clone()).with_start_gate(Arc::new(
            move |cancel| gate.wait_start(CatalogService::Volumo, cancel),
        ));
        Ok(Self {
            runtime,
            store,
            pages,
        })
    }
    fn credential(&self) -> Result<CredentialRef> {
        let status = self.runtime.credential_status(CredentialProvider::Volumo)?;
        ensure!(
            status.presence == Presence::Present && status.vault == VaultState::Ready,
            "Volumo requires an available personal API token"
        );
        let kind = match status.verification {
            Verification::Invalid => Some(CatalogErrorKind::InvalidCredential),
            Verification::EmailUnverified => Some(CatalogErrorKind::EmailUnverified),
            Verification::Restricted => Some(CatalogErrorKind::Restricted),
            Verification::QuotaExhausted => Some(CatalogErrorKind::QuotaExhausted),
            _ => None,
        };
        if let Some(kind) = kind {
            return Err(CatalogError::new(CatalogService::Volumo, kind, None, None).into());
        }
        self.check_credential(&status.reference)?;
        Ok(status.reference)
    }
    fn check_credential(&self, reference: &CredentialRef) -> Result<()> {
        self.runtime.with_credential(reference, |_, revocation| {
            ensure!(!revocation.is_revoked(), "Volumo API token changed");
            Ok(())
        })
    }
    fn page(&self, url: &str, config: &AutotagConfig, cancel: &Arc<AtomicBool>) -> Result<String> {
        page::track_url(url)?;
        if self.store.requires_solver("volumo")? {
            return self.pages.get_text(url, &config.solver, cancel);
        }
        match self
            .runtime
            .get_text(CatalogService::Volumo, url, &[], CatalogAuth::None, cancel)
        {
            Err(error) if cloudflare(&error) => {
                self.store.mark_requires_solver("volumo")?;
                self.pages.get_text(url, &config.solver, cancel)
            }
            other => other,
        }
    }
}
impl ProviderBackend for VolumoBackend {
    fn id(&self) -> ProviderId {
        ProviderId::Volumo
    }
    fn candidates(&self, input: &TagInput, config: &AutotagConfig) -> Result<Vec<CatalogMatch>> {
        self.candidates_cancellable(input, config, &Arc::new(AtomicBool::new(false)))
    }
    fn stage_timeout(&self, config: &AutotagConfig) -> Duration {
        let solver = if config
            .solver
            .endpoint
            .as_ref()
            .is_some_and(|endpoint| !endpoint.trim().is_empty())
        {
            2 * (config.solver.timeout_seconds.min(300) + 10) + 1
        } else {
            0
        };
        // At most one official resolver and one public page per operation.
        Duration::from_secs(190 + solver)
    }
    fn candidates_cancellable(
        &self,
        input: &TagInput,
        config: &AutotagConfig,
        cancel: &Arc<AtomicBool>,
    ) -> Result<Vec<CatalogMatch>> {
        ensure!(!cancel.load(Ordering::Relaxed), "Volumo lookup cancelled");
        let credential = self.credential()?;
        let isrc = source_value(input, &["isrc"])?
            .map(|value| normalize_isrc(&value))
            .transpose()?;
        let known_id = if config.options.match_by_id {
            source_value(input, &["volumo_track_id"])?
                .map(|id| page::numeric_id(&id))
                .transpose()?
        } else {
            None
        };
        let (id, url, reason) = if let Some(id) = known_id {
            (
                id,
                format!("https://volumo.com/track/{id}"),
                onetagger_tagger::MatchReason::ID,
            )
        } else if let Some(isrc) = &isrc {
            let response = self.runtime.get_json(
                CatalogService::Volumo,
                &format!("https://volumo.com/api/pub/track_by_isrc/{isrc}"),
                &[],
                CatalogAuth::Credential(credential.clone()),
                cancel,
            );
            let body = match response {
                Ok(body) => body,
                Err(error)
                    if error
                        .downcast_ref::<CatalogError>()
                        .is_some_and(|error| error.kind == CatalogErrorKind::NotFound) =>
                {
                    return Ok(vec![]);
                }
                Err(error) => {
                    if cloudflare(&error) {
                        self.store.mark_requires_solver("volumo")?;
                    }
                    return Err(error);
                }
            };
            let returned = normalize_isrc(
                body["isrc"]
                    .as_str()
                    .context("Volumo resolver has no ISRC proof")?,
            )?;
            ensure!(
                &returned == isrc,
                "Volumo resolver returned another ISRC recording"
            );
            match body["status"].as_str() {
                Some("live") => {}
                Some("scheduled") => {
                    return Err(BackendFailure::new(
                        ProviderFailureKind::Unavailable,
                        "Volumo track is scheduled and its public metadata is not live yet",
                    )
                    .into());
                }
                _ => anyhow::bail!("Invalid Volumo resolver status"),
            }
            let url = body["url"]
                .as_str()
                .context("Volumo resolver has no track URL")?;
            let (id, url) = page::track_url(url)?;
            (id, url, onetagger_tagger::MatchReason::ISRC)
        } else {
            return Err(BackendFailure::new(ProviderFailureKind::Unavailable,"Volumo's documented metadata API needs an ISRC or saved Volumo track ID; public artist/title search is not available in this adapter").into());
        };
        let mut matched = page::parse(&self.page(&url, config, cancel)?, id, isrc.as_deref())?;
        ensure!(!cancel.load(Ordering::Relaxed), "Volumo lookup cancelled");
        self.check_credential(&credential)?;
        matched.accuracy = 1.;
        matched.reason = reason;
        Ok(vec![matched])
    }
    fn extend_candidate_cancellable(
        &self,
        matched: &mut CatalogMatch,
        config: &AutotagConfig,
        cancel: &Arc<AtomicBool>,
    ) -> Result<()> {
        matched.validate()?;
        ensure!(
            matched.platform == "volumo",
            "Wrong Volumo candidate provider"
        );
        let credential = self.credential()?;
        let id = page::numeric_id(
            matched
                .track_id
                .as_deref()
                .context("Missing Volumo track identity")?,
        )?;
        let (url_id, url) = page::track_url(&matched.url)?;
        ensure!(id == url_id, "Selected Volumo URL changed identity");
        let mut expanded = page::parse(
            &self.page(&url, config, cancel)?,
            id,
            matched.isrc.as_deref(),
        )?;
        expanded.validate_extension_from(matched)?;
        ensure!(
            !cancel.load(Ordering::Relaxed),
            "Volumo expansion cancelled"
        );
        self.check_credential(&credential)?;
        expanded.accuracy = matched.accuracy;
        expanded.reason = matched.reason;
        *matched = expanded;
        Ok(())
    }
}
fn source_value(input: &TagInput, aliases: &[&str]) -> Result<Option<String>> {
    let mut values = input
        .media
        .tags
        .iter()
        .filter(|(key, _)| {
            aliases
                .iter()
                .any(|alias| crate::candidate::native_identity_key_matches(key, alias))
        })
        .map(|(_, value)| value.trim())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    values.sort_unstable();
    values.dedup();
    ensure!(values.len() <= 1, "Conflicting Volumo source identities");
    Ok(values.first().map(|value| (*value).to_owned()))
}
fn normalize_isrc(value: &str) -> Result<String> {
    let value = value.replace('-', "").to_ascii_uppercase();
    ensure!(
        value.len() == 12
            && value.as_bytes()[..2].iter().all(u8::is_ascii_alphabetic)
            && value.as_bytes()[2..5].iter().all(u8::is_ascii_alphanumeric)
            && value.as_bytes()[5..].iter().all(u8::is_ascii_digit),
        "Invalid Volumo ISRC selector"
    );
    Ok(value)
}
fn cloudflare(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<CatalogError>()
        .is_some_and(|error| error.kind == CatalogErrorKind::CloudflareProtected)
}
