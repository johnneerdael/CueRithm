//! Application-scoped provider setup, separate from portable preparation settings.
mod global_state;
mod model;
mod native;
use crate::Library;
use anyhow::{Result, ensure};
use cuerithm_autotag::{
    CatalogError, CatalogErrorKind, CatalogIdentity, CatalogProbe, CatalogRuntime, CatalogService,
    ProviderId,
};
use cuerithm_credentials::{
    CredentialRef, CredentialStatus, Presence, ProviderId as CredentialProviderId, Secret,
    VaultState, Verification,
};
use cuerithm_media::CancellationToken;
use model::reason;
pub use model::*;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct ProviderSetupService {
    library: Library,
    runtime: Arc<CatalogRuntime>,
    global_state: Option<Arc<global_state::ApplicationCatalogState>>,
    probes: Arc<Mutex<BTreeMap<CredentialProviderId, CatalogProbe>>>,
}
impl Library {
    /// Construct once with the actual packaged release. Share clones with UI and headless work.
    pub fn provider_runtime(&self, release: &str) -> Result<ProviderSetupService> {
        let mut slot = self
            .inner
            .catalog_runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("Catalog runtime interrupted"))?;
        if let Some(runtime) = slot.as_ref().and_then(std::sync::Weak::upgrade) {
            ensure!(
                runtime.identity().user_agent() == CatalogIdentity::new(release)?.user_agent(),
                "Catalog application identity changed"
            );
            return Ok(ProviderSetupService {
                library: self.clone(),
                runtime: runtime.clone(),
                global_state: global_state::state_for(&runtime)?,
                probes: Arc::new(Mutex::new(BTreeMap::new())),
            });
        }
        let (runtime, global_state) = global_state::runtime(release)?;
        *slot = Some(Arc::downgrade(&runtime));
        Ok(ProviderSetupService {
            library: self.clone(),
            runtime,
            global_state: Some(global_state),
            probes: Arc::new(Mutex::new(BTreeMap::new())),
        })
    }
    pub fn shared_catalog_runtime(&self) -> Result<Option<Arc<CatalogRuntime>>> {
        Ok(self
            .inner
            .catalog_runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("Catalog runtime interrupted"))?
            .as_ref()
            .and_then(std::sync::Weak::upgrade))
    }
    /// Trusted fixture/client injection. Never creates or reads a native vault.
    pub fn provider_runtime_with(
        &self,
        runtime: Arc<CatalogRuntime>,
    ) -> Result<ProviderSetupService> {
        let mut slot = self
            .inner
            .catalog_runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("Catalog runtime interrupted"))?;
        if let Some(current) = slot.as_ref().and_then(std::sync::Weak::upgrade) {
            ensure!(
                Arc::ptr_eq(&current, &runtime),
                "This library already has a different catalog authority"
            );
        } else {
            *slot = Some(Arc::downgrade(&runtime));
        }
        let global_state = global_state::state_for(&runtime)?;
        Ok(ProviderSetupService {
            library: self.clone(),
            runtime,
            global_state,
            probes: Arc::new(Mutex::new(BTreeMap::new())),
        })
    }
}
fn credential_provider(provider: ProviderId) -> Option<CredentialProviderId> {
    Some(match provider {
        ProviderId::Sonovault => CredentialProviderId::SonoVault,
        ProviderId::Discogs => CredentialProviderId::Discogs,
        ProviderId::Bpmsupreme => CredentialProviderId::BpmSupreme,
        _ => return None,
    })
}
fn catalog_provider(provider: CredentialProviderId) -> Result<ProviderId> {
    Ok(match provider {
        CredentialProviderId::SonoVault => ProviderId::Sonovault,
        CredentialProviderId::Discogs => ProviderId::Discogs,
        CredentialProviderId::BpmSupreme => ProviderId::Bpmsupreme,
        _ => anyhow::bail!("This credential is not an enabled metadata provider"),
    })
}
fn service(provider: ProviderId) -> CatalogService {
    match provider {
        ProviderId::Beatport => CatalogService::Beatport,
        ProviderId::Traxsource => CatalogService::Traxsource,
        ProviderId::Musicbrainz => CatalogService::MusicBrainz,
        ProviderId::Sonovault => CatalogService::SonoVault,
        ProviderId::Discogs => CatalogService::Discogs,
        ProviderId::Itunes => CatalogService::ITunes,
        ProviderId::Bpmsupreme => CatalogService::BpmSupreme,
        ProviderId::Bandcamp => CatalogService::Bandcamp,
        ProviderId::Deezer => CatalogService::Deezer,
    }
}
impl ProviderSetupService {
    fn rate_projection(&self, service: &str) -> Result<crate::catalog_state::RateProjection> {
        match &self.global_state {
            Some(state) => state.projection(service),
            None => self.library.catalog_rate_projection(service),
        }
    }

    pub fn runtime(&self) -> &Arc<CatalogRuntime> {
        &self.runtime
    }
    pub fn statuses(&self) -> Result<Vec<ProviderSetupStatus>> {
        ProviderId::ALL
            .into_iter()
            .map(|provider| self.status(provider))
            .collect()
    }
    pub fn status(&self, provider: ProviderId) -> Result<ProviderSetupStatus> {
        let id = provider.as_str();
        let mut problem = None;
        let credential=credential_provider(provider).and_then(|p|match self.runtime.credential_status(p){Ok(value)=>Some(value),Err(_)=>{problem=reason("credential_authority_unavailable","System credential storage is unavailable. Public providers and offline analysis remain available.");None}});
        let present = credential
            .as_ref()
            .is_some_and(|s| s.presence == Presence::Present && s.vault == VaultState::Ready);
        let verified = present
            && credential.as_ref().is_some_and(|s| {
                s.verification == Verification::Valid
                    && (provider != ProviderId::Discogs
                        || self.runtime.is_authenticated(&s.reference).unwrap_or(false))
            });
        let mut can_use = match provider {
            ProviderId::Sonovault => present,
            ProviderId::Bpmsupreme => verified,
            _ => true,
        };
        if let Some(s) = &credential {
            if s.vault != VaultState::Ready {
                problem = reason(
                    "credential_store_unavailable",
                    "Unlock or reconnect the system credential store. Stored key presence is unknown.",
                );
            } else if s.presence == Presence::Missing && provider != ProviderId::Discogs {
                problem = reason(
                    "credential_missing",
                    "Set up this optional provider to use its metadata.",
                );
            } else {
                match s.verification {
                    Verification::Invalid => {
                        if provider != ProviderId::Discogs {
                            can_use = false;
                        }
                        problem = reason(
                            "invalid_credential",
                            "The credential was rejected. Replace it or remove it; Discogs public access remains available anonymously.",
                        );
                    }
                    Verification::EmailUnverified => {
                        can_use = false;
                        problem = reason(
                            "email_unverified",
                            "Verify your email with the provider, then test the saved connection again.",
                        );
                    }
                    Verification::Restricted => {
                        can_use = false;
                        problem = reason(
                            "account_restricted",
                            "The account or credential cannot use this metadata capability.",
                        );
                    }
                    Verification::QuotaExhausted => {
                        can_use = false;
                        problem = reason(
                            "quota_exhausted",
                            "Provider allowance is exhausted. Reset time is unavailable; check your account before retrying.",
                        );
                    }
                    Verification::RateLimited => {
                        problem = reason(
                            "rate_limited",
                            "Provider rate or quota limit reached. Requests remain bounded; wait or check your account before testing again.",
                        )
                    }
                    _ => {}
                }
            }
        }
        if let Some(credential) = &credential {
            let probe = self
                .probes
                .lock()
                .map_err(|_| anyhow::anyhow!("Provider status interrupted"))?
                .get(&credential.reference.provider)
                .filter(|probe| probe.credential.reference == credential.reference)
                .cloned();
            if let Some(probe) = probe
                && let Some(kind) = probe.kind
            {
                match kind {
                    CatalogErrorKind::MockCredential => {
                        can_use = false;
                        problem = reason(
                            "mock_credential",
                            "This is a test key. Mock responses do not establish live catalog access; use a live key for metadata.",
                        );
                    }
                    CatalogErrorKind::RateOrQuotaLimited => {
                        problem = reason(
                            "rate_or_quota_limited",
                            "The provider reported a rate or quota limit without a known monthly reset. Check your account or explicitly test again later.",
                        )
                    }
                    CatalogErrorKind::Timeout | CatalogErrorKind::Transport => {
                        problem = reason(
                            "temporary_connection_failure",
                            "The connection test did not finish. This does not prove the saved credential is invalid.",
                        )
                    }
                    _ => {}
                }
            }
        }
        let setup_kind = match provider {
            ProviderId::Sonovault => ProviderSetupKind::ApiKey,
            ProviderId::Discogs => ProviderSetupKind::PersonalToken,
            ProviderId::Bpmsupreme => ProviderSetupKind::EmailPassword,
            _ => ProviderSetupKind::None,
        };

        let (timing, budget_available) = match self.rate_projection(service(provider).as_str()) {
            Ok(value) => (value, true),
            Err(_) => {
                can_use = false;
                problem = reason(
                    "catalog_state_unavailable",
                    "Application catalog storage is unavailable. Offline analysis and library editing remain available.",
                );
                ((None, None, None, false), false)
            }
        };
        let (retry_at_ms, server_limit, _, held) = timing;
        if held && provider == ProviderId::Sonovault {
            can_use = false;
            problem = reason(
                "rate_or_quota_limited",
                "SonoVault reported a rate or quota limit without a known reset. Check your allowance, then explicitly test the saved connection before retrying.",
            );
        }
        let ceiling = match provider {
            ProviderId::Discogs => Some(if verified { 60 } else { 25 }),
            ProviderId::Sonovault => Some(20),
            _ => None,
        };
        let rate_limit_per_minute = ceiling.map(|cap| {
            server_limit
                .filter(|limit| *limit > 0)
                .map_or(cap, |limit| limit.min(cap))
        });
        let auth_mode = if provider == ProviderId::Discogs {
            if verified {
                ProviderAuthMode::Authenticated
            } else {
                ProviderAuthMode::Anonymous
            }
        } else if credential_provider(provider).is_none() {
            ProviderAuthMode::Public
        } else if can_use {
            ProviderAuthMode::Authenticated
        } else {
            ProviderAuthMode::Unavailable
        };
        let mut status = ProviderSetupStatus {
            id: id.into(),
            revision: String::new(),
            setup_kind,
            credential,
            auth_mode,
            can_use,
            can_test: present && budget_available,
            reason: problem,
            rate_limit_per_minute,
            min_request_interval_ms: (provider == ProviderId::Musicbrainz).then_some(1000),
            retry_at_ms,
            quota: None,
        };
        use sha2::{Digest, Sha256};
        status.revision = format!("{:x}", Sha256::digest(serde_json::to_vec(&status)?));
        Ok(status)
    }
    pub fn save(
        &self,
        expected: &CredentialRef,
        input: ProviderCredentialInput,
    ) -> Result<CredentialStatus> {
        catalog_provider(expected.provider)?;
        let secret = match (expected.provider, input) {
            (CredentialProviderId::SonoVault, ProviderCredentialInput::ApiKey { secret })
            | (CredentialProviderId::Discogs, ProviderCredentialInput::PersonalToken { secret }) => {
                Secret::new(secret)?
            }
            (
                CredentialProviderId::BpmSupreme,
                ProviderCredentialInput::EmailPassword { email, password },
            ) => {
                ensure!(
                    !email.trim().is_empty()
                        && email.len() <= 4096
                        && !password.is_empty()
                        && password.len() <= 4096,
                    "Enter an email and password"
                );
                Secret::structured(serde_json::to_string(
                    &serde_json::json!({"email":email.trim(),"password":password}),
                )?)?
            }
            _ => anyhow::bail!("Credential input does not match this provider's setup method"),
        };
        self.probes
            .lock()
            .map_err(|_| anyhow::anyhow!("Provider status interrupted"))?
            .remove(&expected.provider);
        self.runtime.credential_save(expected, secret)
    }
    pub fn remove(&self, provider: CredentialProviderId) -> Result<CredentialStatus> {
        catalog_provider(provider)?;
        self.probes
            .lock()
            .map_err(|_| anyhow::anyhow!("Provider status interrupted"))?
            .remove(&provider);
        self.runtime.credential_remove(provider)
    }
    pub fn test(
        &self,
        expected: &CredentialRef,
        cancel: &CancellationToken,
    ) -> Result<ProviderSetupStatus> {
        cancel.check()?;
        let provider = catalog_provider(expected.provider)?;
        let mut probe = if provider == ProviderId::Bpmsupreme {
            match cuerithm_autotag::test_pool_account(
                &self.runtime,
                expected,
                &cancel.shared_flag(),
            ) {
                Ok(()) => CatalogProbe {
                    credential: self
                        .runtime
                        .record_verification(expected, Verification::Valid)?,
                    live: true,
                    authenticated_account: true,
                    kind: None,
                    retry_at_ms: None,
                },
                Err(error) => {
                    cancel.check()?;
                    let kind = error
                        .downcast_ref::<CatalogError>()
                        .map(|error| error.kind)
                        .unwrap_or(CatalogErrorKind::InvalidResponse);
                    let verification = match kind {
                        CatalogErrorKind::InvalidCredential => Some(Verification::Invalid),
                        CatalogErrorKind::EmailUnverified => Some(Verification::EmailUnverified),
                        CatalogErrorKind::Restricted => Some(Verification::Restricted),
                        CatalogErrorKind::RateLimited | CatalogErrorKind::RateOrQuotaLimited => {
                            Some(Verification::RateLimited)
                        }
                        CatalogErrorKind::QuotaExhausted => Some(Verification::QuotaExhausted),
                        _ => None,
                    };
                    let credential = if let Some(value) = verification {
                        self.runtime.record_verification(expected, value)?
                    } else {
                        self.runtime.credential_status(expected.provider)?
                    };
                    ensure!(
                        credential.reference == *expected,
                        "Credential changed during account verification"
                    );
                    CatalogProbe {
                        credential,
                        live: false,
                        authenticated_account: false,
                        kind: Some(kind),
                        retry_at_ms: error
                            .downcast_ref::<CatalogError>()
                            .and_then(|error| error.retry_at_ms),
                    }
                }
            }
        } else {
            self.runtime
                .test_connection(expected, &cancel.shared_flag())?
        };
        if probe.kind == Some(CatalogErrorKind::MockCredential) {
            probe.credential = self
                .runtime
                .record_verification(expected, Verification::Restricted)?;
        }
        self.probes
            .lock()
            .map_err(|_| anyhow::anyhow!("Provider status interrupted"))?
            .insert(expected.provider, probe);
        cancel.check()?;
        self.status(provider)
    }
}
