//! Optional personal-key SonoVault metadata, using only the shared runtime.
use crate::catalog_runtime::{CatalogAuth, CatalogError, CatalogErrorKind, CatalogService};
use crate::{
    AutotagConfig, CatalogFacts, CatalogMatch, CatalogRuntime, CatalogTrack, ProviderBackend,
    ProviderId, SourceCatalogFacts, SourceCatalogIdentity, SourceCatalogIdentityKind, TagInput,
    TransferDate,
};
use anyhow::{Context, Result, ensure};
use cuerithm_credentials::{
    CredentialRef, Presence, ProviderId as CredentialProviderId, VaultState,
};
use serde_json::Value;
use std::sync::{Arc, atomic::AtomicBool};
mod links;
pub use links::SonoVaultLookup;
pub struct SonoVaultBackend {
    runtime: Arc<CatalogRuntime>,
}
impl SonoVaultBackend {
    pub fn new(runtime: Arc<CatalogRuntime>) -> Self {
        Self { runtime }
    }
    fn credential(&self) -> Result<CredentialRef> {
        let s = self
            .runtime
            .credential_status(CredentialProviderId::SonoVault)?;
        ensure!(
            s.presence == Presence::Present && s.vault == VaultState::Ready,
            "SonoVault requires an available personal API key"
        );
        let blocked = match s.verification {
            cuerithm_credentials::Verification::Invalid => {
                Some(CatalogErrorKind::InvalidCredential)
            }
            cuerithm_credentials::Verification::EmailUnverified => {
                Some(CatalogErrorKind::EmailUnverified)
            }
            cuerithm_credentials::Verification::Restricted => Some(CatalogErrorKind::Restricted),
            cuerithm_credentials::Verification::QuotaExhausted => {
                Some(CatalogErrorKind::QuotaExhausted)
            }
            _ => None,
        };
        if let Some(kind) = blocked {
            return Err(CatalogError::new(CatalogService::SonoVault, kind, None, None).into());
        }
        self.runtime.with_credential(&s.reference, |key, _| {
            ensure!(
                !key.starts_with("svk_test_"),
                "SonoVault test keys provide mock data and cannot identify music"
            );
            Ok(())
        })?;
        Ok(s.reference)
    }
    fn get(
        &self,
        url: &str,
        query: &[(String, String)],
        cancel: &Arc<AtomicBool>,
    ) -> Result<Value> {
        self.runtime.get_json(
            CatalogService::SonoVault,
            url,
            query,
            CatalogAuth::Credential(self.credential()?),
            cancel,
        )
    }
}
impl ProviderBackend for SonoVaultBackend {
    fn id(&self) -> ProviderId {
        ProviderId::Sonovault
    }
    fn candidates(&self, i: &TagInput, c: &AutotagConfig) -> Result<Vec<CatalogMatch>> {
        self.candidates_cancellable(i, c, &Arc::new(AtomicBool::new(false)))
    }
    fn candidates_cancellable(
        &self,
        i: &TagInput,
        c: &AutotagConfig,
        cancel: &Arc<AtomicBool>,
    ) -> Result<Vec<CatalogMatch>> {
        let info = i.audio_info(c)?;
        if let Some(isrc) = info.isrc.as_ref().filter(|v| valid_isrc(v)) {
            match self.get(
                &format!(
                    "https://api.sonovault.now/v1/tracks/isrc/{}",
                    isrc.to_ascii_uppercase()
                ),
                &[],
                cancel,
            ) {
                Ok(v) => {
                    let mut m = parse_track(&v)?;
                    ensure!(
                        m.isrc
                            .as_ref()
                            .is_some_and(|v| v.eq_ignore_ascii_case(isrc)),
                        "SonoVault returned another recording identity"
                    );
                    m.reason = onetagger_tagger::MatchReason::ISRC;
                    m.accuracy = 1.;
                    return Ok(vec![m]);
                }
                Err(e)
                    if e.downcast_ref::<CatalogError>()
                        .is_some_and(|e| e.kind == CatalogErrorKind::NotFound) => {}
                Err(e) => return Err(e),
            }
        }
        if c.options.match_by_id
            && let Some(lookup) = links::known_source_lookup(i)?
        {
            return Ok(self
                .resolve_platform_lookup(&lookup, cancel)?
                .into_iter()
                .collect());
        }
        let Some(title) = info.title.as_ref().filter(|s| !s.trim().is_empty()) else {
            return Ok(vec![]);
        };
        if info.artists.is_empty() {
            return Ok(vec![]);
        }
        let v = self.get(
            "https://api.sonovault.now/v1/tracks/search",
            &[
                ("artist".into(), info.artists.join(", ")),
                ("title".into(), title.clone()),
                ("limit".into(), "20".into()),
            ],
            cancel,
        )?;
        let rows = v
            .get("results")
            .and_then(Value::as_array)
            .context("Malformed SonoVault search response")?;
        ensure!(rows.len() <= 100, "SonoVault search response exceeds limit");
        let matches = rows.iter().map(parse_track).collect::<Result<Vec<_>>>()?;
        Ok(matched(&info, &c.options, matches))
    }
    fn extend_candidate_cancellable(
        &self,
        m: &mut CatalogMatch,
        _: &AutotagConfig,
        cancel: &Arc<AtomicBool>,
    ) -> Result<()> {
        m.validate()?;
        ensure!(m.track.platform == "sonovault", "Wrong provider identity");
        let id = m.track_id.as_ref().context("Missing recording identity")?;
        numeric_id(id)?;
        let v = self.get(
            &format!("https://api.sonovault.now/v1/tracks/{id}"),
            &[],
            cancel,
        )?;
        let mut next = parse_track(&v)?;
        self.enrich_platform_links(&mut next, cancel)?;
        next.validate_extension_from(m)?;
        next.accuracy = m.accuracy;
        next.reason = m.reason;
        *m = next;
        Ok(())
    }
}
pub(crate) fn matched(
    info: &onetagger_tagger::AudioFileInfo,
    config: &onetagger_tagger::TaggerConfig,
    matches: Vec<CatalogMatch>,
) -> Vec<CatalogMatch> {
    let tracks = matches.iter().map(|m| m.track.clone()).collect();
    onetagger_tagger::MatchingUtils::match_track(info, &tracks, config, true)
        .into_iter()
        .filter_map(|hit| {
            matches
                .iter()
                .find(|m| {
                    m.track.track_id == hit.track.track_id
                        && m.track.release_id == hit.track.release_id
                })
                .cloned()
                .map(|mut m| {
                    m.accuracy = hit.accuracy;
                    m.reason = hit.reason;
                    m
                })
        })
        .collect()
}
pub(crate) fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty() && v.len() <= 4096)
        .map(str::to_owned)
}
pub(crate) fn strings(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(64)
        .filter_map(|v| {
            v.as_str()
                .filter(|s| !s.is_empty() && s.len() <= 4096)
                .map(str::to_owned)
        })
        .collect()
}
pub(crate) fn numeric_id(v: &str) -> Result<()> {
    ensure!(
        !v.is_empty()
            && v.len() <= 20
            && v.bytes().all(|c| c.is_ascii_digit())
            && v.parse::<u64>().is_ok_and(|n| n > 0),
        "Invalid catalog resource identity"
    );
    Ok(())
}
pub(crate) fn date(v: &str, precision: Option<&str>) -> Result<TransferDate> {
    let parts = v.split('-').collect::<Vec<_>>();
    ensure!(
        !parts.is_empty() && parts.len() <= 3,
        "Invalid catalog date"
    );
    let year = parts[0].parse::<i32>()?;
    let mut month = parts
        .get(1)
        .map(|s| s.parse::<u8>())
        .transpose()?
        .filter(|n| *n != 0);
    let mut day = parts
        .get(2)
        .map(|s| s.parse::<u8>())
        .transpose()?
        .filter(|n| *n != 0);
    match precision {
        Some("year") => {
            month = None;
            day = None
        }
        Some("month") => day = None,
        Some("day") | None => {}
        _ => anyhow::bail!("Invalid catalog date precision"),
    };
    let d = TransferDate { year, month, day };
    crate::validate_date(&d)?;
    Ok(d)
}
fn valid_isrc(v: &str) -> bool {
    v.len() == 12 && v.bytes().all(|c| c.is_ascii_alphanumeric())
}
fn parse_track(v: &Value) -> Result<CatalogMatch> {
    let id = v
        .get("id")
        .and_then(Value::as_u64)
        .filter(|v| *v > 0)
        .context("Missing SonoVault recording identity")?
        .to_string();
    let title = text(v, "title").context("Missing SonoVault recording title")?;
    let credits = v
        .get("artists")
        .and_then(Value::as_array)
        .context("Malformed SonoVault artist credits")?;
    ensure!(credits.len() <= 64, "Too many SonoVault artist credits");
    let artists = credits
        .iter()
        .filter(|a| a.get("is_remixer").and_then(Value::as_bool) != Some(true))
        .filter_map(|a| text(a, "name"))
        .collect::<Vec<_>>();
    ensure!(!artists.is_empty(), "Missing SonoVault artist credits");
    let release = v
        .get("releases")
        .and_then(Value::as_array)
        .and_then(|r| r.first());
    let release_id = release
        .and_then(|r| r.get("id"))
        .and_then(Value::as_u64)
        .filter(|v| *v > 0)
        .map(|v| v.to_string());
    let release_date = release
        .and_then(|r| text(r, "release_date"))
        .map(|s| date(&s, None))
        .transpose()?;
    let mut track = CatalogTrack {
        platform: "sonovault".into(),
        title,
        artists,
        remixers: credits
            .iter()
            .filter(|a| a.get("is_remixer").and_then(Value::as_bool) == Some(true))
            .filter_map(|a| text(a, "name"))
            .collect(),
        track_id: Some(id.clone()),
        release_id: release_id.clone(),
        album: release.and_then(|r| text(r, "title")),
        label: release
            .and_then(|r| r.get("label"))
            .and_then(|l| text(l, "name")),
        isrc: text(v, "isrc"),
        genres: strings(v, "genre"),
        styles: strings(v, "subgenre"),
        url: "https://sonovault.now".into(),
        ..Default::default()
    };
    if let Some(seconds) = v.get("duration").and_then(Value::as_u64) {
        ensure!(seconds <= 86400, "Invalid SonoVault duration");
        track.duration = std::time::Duration::from_secs(seconds);
    }
    let mut m = CatalogMatch::new(0., track);
    m.facts = Some(CatalogFacts::Source(SourceCatalogFacts {
        schema_version: 1,
        provider: "sonovault".into(),
        release_id,
        release_date,
        identities: vec![SourceCatalogIdentity {
            source: "sonovault".into(),
            kind: SourceCatalogIdentityKind::Recording,
            id,
        }],
        ..Default::default()
    }));
    m.validate()?;
    Ok(m)
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn recording_projection_preserves_credits_and_never_imports_provider_dsp() {
        let m=parse_track(&json!({"id":123,"title":"Signal (Dub)","artists":[{"id":1,"name":"Credit","is_primary":true,"is_remixer":false},{"id":2,"name":"Guest","is_primary":false,"is_remixer":false},{"id":3,"name":"Remixer","is_primary":false,"is_remixer":true}],"releases":[{"id":2,"title":"Album","label":{"id":3,"name":"Label"},"release_date":"2021-07-01"}],"isrc":"USAAA2100001","duration":null,"bpm":174,"key_musical":"Am","genre":["House"],"subgenre":["Deep House"]})).unwrap();
        m.validate().unwrap();
        assert_eq!(m.track_id.as_deref(), Some("123"));
        assert_eq!(m.artists, vec!["Credit", "Guest"]);
        assert_eq!(m.remixers, vec!["Remixer"]);
        assert!(m.bpm.is_none() && m.key.is_none());
        assert!(m.track_number.is_none());
        assert_eq!(m.duration, std::time::Duration::ZERO);
    }

    #[test]
    fn personal_search_and_selected_recording_refresh_preserve_owned_facts() {
        use crate::catalog_runtime::*;
        use cuerithm_credentials::*;
        struct Fixture;
        impl CatalogTransport for Fixture {
            fn execute(
                &self,
                r: CatalogHttpRequest,
                _: &Arc<AtomicBool>,
                _: Option<&Revocation>,
            ) -> Result<CatalogHttpResponse> {
                assert!(
                    r.headers
                        .iter()
                        .any(|(k, v)| k == "x-api-key" && v == "svk_live_fixture")
                );
                let track = json!({"id":123,"title":"Signal (Dub)","artists":[{"name":"Credit"}],"releases":[],"isrc":"USAAA2100001","duration":300,"genre":["House"],"subgenre":[]});
                let body = if r.url.ends_with("/tracks/links?id=123") {
                    json!({"track_id":123,"title":"Signal (Dub)","isrc":"USAAA2100001","links":[]})
                } else if r.url.contains("/tracks/search?") {
                    json!({"results":[track],"next_cursor":null})
                } else {
                    assert!(r.url.ends_with("/tracks/123"));
                    track
                };
                Ok(CatalogHttpResponse {
                    status: 200,
                    body: serde_json::to_vec(&body).unwrap(),
                    headers: vec![],
                })
            }
        }
        let temp = tempfile::tempdir().unwrap();
        let service = CredentialService::new(
            crate::catalog_runtime::tests::VaultFixture::default(),
            Arc::new(state::FileStateStore::open(temp.path()).unwrap()),
        )
        .unwrap();
        let old = service
            .status(cuerithm_credentials::ProviderId::SonoVault)
            .unwrap();
        service
            .save(
                &old.reference,
                Secret::new("svk_live_fixture".into()).unwrap(),
            )
            .unwrap();
        let runtime = CatalogRuntime::new(
            CatalogIdentity::new("test").unwrap(),
            Arc::new(MemoryCatalogState::default()),
            Arc::new(service),
        )
        .unwrap()
        .with_local_server("http://127.0.0.1:9")
        .unwrap()
        .with_transport(Arc::new(Fixture));
        let backend = SonoVaultBackend::new(Arc::new(runtime));
        let mut m = backend
            .candidates(
                &crate::catalog_runtime::tests::provider_input(),
                &AutotagConfig::default(),
            )
            .unwrap()
            .remove(0);
        backend
            .extend_candidate_cancellable(
                &mut m,
                &AutotagConfig::default(),
                &Arc::new(AtomicBool::new(false)),
            )
            .unwrap();
        assert_eq!(m.track_id.as_deref(), Some("123"));
        assert!(matches!(m.facts, Some(crate::CatalogFacts::Source(_))));
    }

    #[test]
    fn known_invalid_personal_key_blocks_before_automatic_metadata_request() {
        use crate::catalog_runtime::*;
        use cuerithm_credentials::*;
        struct Never;
        impl CatalogTransport for Never {
            fn execute(
                &self,
                _: CatalogHttpRequest,
                _: &Arc<AtomicBool>,
                _: Option<&Revocation>,
            ) -> Result<CatalogHttpResponse> {
                panic!("Known invalid personal key must not be retried for each track")
            }
        }
        let temp = tempfile::tempdir().unwrap();
        let service = CredentialService::new(
            crate::catalog_runtime::tests::VaultFixture::default(),
            Arc::new(state::FileStateStore::open(temp.path()).unwrap()),
        )
        .unwrap();
        let old = service
            .status(cuerithm_credentials::ProviderId::SonoVault)
            .unwrap();
        let saved = service
            .save(
                &old.reference,
                Secret::new("svk_live_fixture".into()).unwrap(),
            )
            .unwrap();
        service
            .record_verification(&saved.reference, Verification::Invalid)
            .unwrap();
        let r = CatalogRuntime::new(
            CatalogIdentity::new("test").unwrap(),
            Arc::new(MemoryCatalogState::default()),
            Arc::new(service),
        )
        .unwrap()
        .with_transport(Arc::new(Never));
        let backend = SonoVaultBackend::new(Arc::new(r));
        assert!(
            backend
                .candidates(
                    &crate::catalog_runtime::tests::provider_input(),
                    &AutotagConfig::default()
                )
                .is_err()
        );
    }
}
