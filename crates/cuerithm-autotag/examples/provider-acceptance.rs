//! Explicit development smoke harness. Never run by builds, CI or the installed application.
//! Reads only the key file passed on the command line, keeps it in memory, emits no values.
use anyhow::{Context, Result, ensure};
use cuerithm_autotag::*;
use cuerithm_credentials::{
    CredentialService, ProviderId as CredentialProvider, Secret, Vault, VaultError,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{Arc, Mutex, atomic::AtomicBool},
};

#[derive(Default)]
struct MemoryVault(Mutex<BTreeMap<CredentialProvider, String>>);
impl Vault for MemoryVault {
    fn contains(&self, p: CredentialProvider) -> Result<bool, VaultError> {
        Ok(self
            .0
            .lock()
            .map_err(|_| VaultError::Unavailable)?
            .contains_key(&p))
    }
    fn load(&self, p: CredentialProvider) -> Result<Option<Secret>, VaultError> {
        self.0
            .lock()
            .map_err(|_| VaultError::Unavailable)?
            .get(&p)
            .map(|s| Secret::new(s.clone()).map_err(|_| VaultError::Corrupt))
            .transpose()
    }
    fn store(&self, p: CredentialProvider, s: &Secret) -> Result<(), VaultError> {
        self.0
            .lock()
            .map_err(|_| VaultError::Unavailable)?
            .insert(p, s.expose().into());
        Ok(())
    }
    fn remove(&self, p: CredentialProvider) -> Result<(), VaultError> {
        self.0
            .lock()
            .map_err(|_| VaultError::Unavailable)?
            .remove(&p);
        Ok(())
    }
}
fn key_file(path: &Path) -> Result<Secret> {
    let bytes = std::fs::read(path)?;
    ensure!(bytes.len() <= 16384, "Credential input exceeds bound");
    let text = std::str::from_utf8(&bytes)?.trim();
    let keys = [
        "api_key",
        "api_token",
        "personal_token",
        "token",
        "SONOVAULT_API_KEY",
        "AUDD_API_TOKEN",
        "DISCOGS_TOKEN",
        "VOLUMO_API_TOKEN",
        "VOLUMO_TOKEN",
        "API_KEY",
        "APIKEY",
    ];
    let value = if let Ok(object) = serde_json::from_str::<Value>(text) {
        keys.iter()
            .find_map(|k| object.get(*k).and_then(Value::as_str))
            .context("Expected a single provider token field")?
            .to_string()
    } else {
        let lines = text
            .lines()
            .filter(|s| !s.trim().is_empty() && !s.trim_start().starts_with('#'))
            .collect::<Vec<_>>();
        ensure!(
            lines.len() == 1,
            "Expected one opaque token or a supported provider token field"
        );
        if let Some((name, value)) = lines[0].split_once('=') {
            ensure!(
                keys.contains(&name.trim()),
                "Unknown credential input field"
            );
            value.trim().trim_matches(['\'', '"']).to_owned()
        } else {
            lines[0].to_owned()
        }
    };
    Secret::new(value).map_err(|_| anyhow::anyhow!("Provider key format is invalid"))
}
struct VolumoBudget {
    native: Arc<dyn CatalogTransport>,
    api: std::sync::atomic::AtomicUsize,
    api_status: std::sync::atomic::AtomicUsize,
    public: std::sync::atomic::AtomicUsize,
}
impl CatalogTransport for VolumoBudget {
    fn execute(
        &self,
        request: CatalogHttpRequest,
        cancel: &Arc<AtomicBool>,
        revocation: Option<&cuerithm_credentials::Revocation>,
    ) -> Result<CatalogHttpResponse> {
        let url = reqwest::Url::parse(&request.url)?;
        ensure!(
            request.service == CatalogService::Volumo
                && request.method == CatalogMethod::Get
                && url.scheme() == "https"
                && url.host_str() == Some("volumo.com")
                && url.port_or_known_default() == Some(443),
            "Acceptance transport refuses an unapproved request"
        );
        let api_request = url.path().starts_with("/api/pub/");
        if api_request {
            ensure!(
                url.path() == "/api/pub/track_by_isrc/GBKQU2599555",
                "Acceptance permits only the chosen public recording lookup"
            );
            self.api
                .fetch_update(
                    std::sync::atomic::Ordering::SeqCst,
                    std::sync::atomic::Ordering::SeqCst,
                    |count| (count < 1).then_some(count + 1),
                )
                .map_err(|_| {
                    anyhow::anyhow!("The sole authorized Volumo API attempt was already reserved")
                })?;
        } else {
            ensure!(
                url.path().starts_with("/track/") || url.path().starts_with("/img/"),
                "Acceptance refuses unrelated public pages"
            );
            ensure!(
                !request
                    .headers
                    .iter()
                    .any(|(name, _)| name.eq_ignore_ascii_case("Authorization")
                        || name.eq_ignore_ascii_case("Cookie")),
                "Public acceptance requests cannot carry credentials"
            );
            self.public
                .fetch_update(
                    std::sync::atomic::Ordering::SeqCst,
                    std::sync::atomic::Ordering::SeqCst,
                    |count| (count < 3).then_some(count + 1),
                )
                .map_err(|_| {
                    anyhow::anyhow!("Public metadata acceptance request budget exhausted")
                })?;
        }
        let response = self.native.execute(request, cancel, revocation)?;
        if api_request {
            self.api_status.store(
                response.status as usize,
                std::sync::atomic::Ordering::SeqCst,
            );
        }
        Ok(response)
    }
}
fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    ensure!(args.len() >= 2, "Choose an explicit acceptance mode");
    let cancel = Arc::new(AtomicBool::new(false));
    match args[1].as_str() {
        "volumo-public-catalog" => {
            ensure!(
                args.len() == 4,
                "volumo-public-catalog KEY_FILE ONE_ATTEMPT_MARKER"
            );
            let _marker = std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&args[3])
                .context("This Volumo acceptance attempt was already reserved")?;
            let temporary = tempfile::tempdir()?;
            let service = CredentialService::new(
                MemoryVault::default(),
                Arc::new(
                    cuerithm_credentials::state::FileStateStore::open(temporary.path())
                        .map_err(|_| anyhow::anyhow!("Temporary credential state unavailable"))?,
                ),
            )?;
            let reference = service.status(CredentialProvider::Volumo)?.reference;
            service.save(&reference, key_file(Path::new(&args[2]))?)?;
            let budget = Arc::new(VolumoBudget {
                native: CatalogRuntime::native_http_transport(),
                api: Default::default(),
                api_status: Default::default(),
                public: Default::default(),
            });
            let runtime = Arc::new(
                CatalogRuntime::new(
                    CatalogIdentity::new("development")?,
                    Arc::new(MemoryCatalogState::default()),
                    Arc::new(service),
                )?
                .with_transport(budget.clone()),
            );
            let backend = VolumoBackend::new(
                runtime,
                Arc::new(onetagger_selected_providers::http::MemoryChallengeStore::default()),
            )?;
            let input = TagInput {
                path: "public-fixture.flac".into(),
                source_sha256: String::new(),
                duration_seconds: 0.,
                media: cuerithm_media::MediaInfo {
                    codec: "flac".into(),
                    sample_rate: 44100,
                    channels: 2,
                    bits_per_sample: Some(16),
                    properties: Default::default(),
                    tags: [("isrc".into(), "GBKQU2599555".into())].into(),
                },
            };
            let config = AutotagConfig::default();
            let matches = match backend.candidates_cancellable(&input, &config, &cancel) {
                Ok(matches) => matches,
                Err(error) => {
                    let typed = error.downcast_ref::<CatalogError>();
                    println!(
                        "{}",
                        json!({"service":"volumo","status":"error","api_request_count":budget.api.load(std::sync::atomic::Ordering::SeqCst),"api_http_status":budget.api_status.load(std::sync::atomic::Ordering::SeqCst),"public_metadata_request_count":budget.public.load(std::sync::atomic::Ordering::SeqCst),"error_kind":typed.map(|e|e.kind),"http_status":typed.and_then(|e|e.status)})
                    );
                    return Err(anyhow::anyhow!(
                        "Volumo metadata acceptance failed; see sanitized result"
                    ));
                }
            };
            ensure!(
                matches.len() <= 10,
                "Acceptance returned too many candidates"
            );
            let candidate = matches.first();
            if let Some(candidate) = candidate {
                candidate.validate()?;
            }
            println!(
                "{}",
                json!({"service":"volumo","api_request_count":budget.api.load(std::sync::atomic::Ordering::SeqCst),"api_http_status":budget.api_status.load(std::sync::atomic::Ordering::SeqCst),"public_metadata_request_count":budget.public.load(std::sync::atomic::Ordering::SeqCst),"candidate_count":matches.len(),"has_title":candidate.is_some_and(|m|!m.title.is_empty()),"has_artists":candidate.is_some_and(|m|!m.artists.is_empty()),"matches_isrc":candidate.and_then(|m|m.isrc.as_deref())==Some("GBKQU2599555"),"has_release":candidate.is_some_and(|m|m.release_id.is_some()),"has_artwork":candidate.is_some_and(|m|m.art.is_some())})
            );
        }
        "audd-public-demo" => {
            ensure!(
                args.len() == 4,
                "audd-public-demo KEY_FILE ONE_ATTEMPT_MARKER"
            );
            let token = key_file(Path::new(&args[2]))?;
            // Reserve the sole authorized request before transport; failures do not silently spend another.
            let _marker = std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&args[3])
                .context("This public-demo trial has already been attempted")?;
            let runtime = CatalogRuntime::development()?;
            let body = runtime.post_form(
                CatalogService::AudD,
                "https://api.audd.io/",
                &[
                    ("api_token".into(), token.expose().into()),
                    ("url".into(), "https://audd.tech/example.mp3".into()),
                    ("return".into(), "musicbrainz".into()),
                ],
                CatalogAuth::None,
                &cancel,
            )?;
            println!(
                "{}",
                json!({"service":"audd","request_count":1,"api_success":body["status"]=="success","recognized":body.get("result").is_some_and(|v|!v.is_null()),"has_title":body["result"]["title"].is_string(),"has_artist":body["result"]["artist"].is_string(),"has_musicbrainz":body["result"].get("musicbrainz").is_some(),"error_code":body["error"]["error_code"].as_u64()})
            );
        }
        "credential-probe" => {
            ensure!(
                args.len() == 4,
                "credential-probe sonovault|discogs KEY_FILE"
            );
            let provider = match args[2].as_str() {
                "sonovault" => CredentialProvider::SonoVault,
                "discogs" => CredentialProvider::Discogs,
                _ => anyhow::bail!("This harness probes only metadata credentials"),
            };
            let temporary = tempfile::tempdir()?;
            let service = CredentialService::new(
                MemoryVault::default(),
                Arc::new(
                    cuerithm_credentials::state::FileStateStore::open(temporary.path())
                        .map_err(|_| anyhow::anyhow!("Temporary credential state unavailable"))?,
                ),
            )?;
            let expected = service.status(provider)?.reference;
            let saved = service.save(&expected, key_file(Path::new(&args[3]))?)?;
            let runtime = CatalogRuntime::new(
                CatalogIdentity::new("development")?,
                Arc::new(MemoryCatalogState::default()),
                Arc::new(service),
            )?;
            let probe = runtime.test_connection(&saved.reference, &cancel)?;
            println!("{}", json!({"service":args[2],"probe":probe}));
        }
        "public-catalog" => {
            ensure!(args.len() == 3, "public-catalog PROVIDER");
            let provider = ProviderId::parse(&args[2]).context("Unknown provider")?;
            ensure!(
                !matches!(provider, ProviderId::Sonovault | ProviderId::Bpmsupreme),
                "Use the explicit credential-probe for account services"
            );
            let input = TagInput {
                path: "public-fixture.flac".into(),
                source_sha256: String::new(),
                duration_seconds: 320.,
                media: cuerithm_media::MediaInfo {
                    codec: "flac".into(),
                    sample_rate: 44100,
                    channels: 2,
                    bits_per_sample: Some(16),
                    tags: BTreeMap::from([
                        ("artist".into(), "Daft Punk".into()),
                        ("title".into(), "One More Time".into()),
                    ]),
                    properties: BTreeMap::new(),
                },
            };
            let mut config = AutotagConfig::default();
            config.options.platforms = vec![provider.as_str().into()];
            config.options.match_duration = false;
            let tagger = Tagger::new()?;
            match tagger.match_track(&input, &config) {
                Ok(out) => println!(
                    "{}",
                    json!({"service":args[2],"status":out.status,"candidates":out.matches.len(),"warnings":out.warnings})
                ),
                Err(e) => {
                    let kind = e
                        .downcast_ref::<CatalogLookupError>()
                        .map(|f| json!({"status":f.outcome.status,"warnings":f.outcome.warnings}))
                        .unwrap_or(json!({"status":"error"}));
                    println!("{}", json!({"service":args[2],"result":kind}));
                }
            }
        }
        "identify-public-demo" => {
            ensure!(
                args.len() == 5,
                "identify-public-demo AUDIO_FILE BUNDLED_RUNTIME CACHE_DIR"
            );
            let media = cuerithm_media::MediaRuntime::from_bundle(Path::new(&args[3]))?;
            let audio = media.decode(Path::new(&args[2]), Path::new(&args[4]))?;
            let config = RecognitionConfig {
                enabled: true,
                fingerprint_consent: true,
                ..Default::default()
            };
            let result = recognize_prepared(
                Arc::new(CatalogRuntime::development()?),
                &audio,
                &config,
                &cancel,
            )?;
            let found = result.matches;
            println!(
                "{}",
                json!({"service":"acoustid_musicbrainz","candidates":found.len(),"editions_truncated":result.editions_truncated,"typed_facts":found.iter().all(|m|m.facts.is_some()),"release_bound":found.iter().filter(|m|m.track.release_id.is_some()).count()})
            );
        }
        "fingerprint-synthetic" => {
            let samples = (0..11025 * 30)
                .map(|n| {
                    let t = n as f64 / 11025.;
                    ((t * std::f64::consts::TAU * 440.).sin() * 10000.
                        + (t * std::f64::consts::TAU * (120. + 10. * t)).sin() * 7000.)
                        as i16
                })
                .collect::<Vec<_>>();
            println!("{}", chromaprint_pcm(&samples, 11025, 1)?);
        }
        _ => anyhow::bail!("Unknown acceptance mode"),
    }
    Ok(())
}
