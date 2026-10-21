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
        "API_KEY",
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
fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    ensure!(args.len() >= 2, "Choose an explicit acceptance mode");
    let cancel = Arc::new(AtomicBool::new(false));
    match args[1].as_str() {
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
