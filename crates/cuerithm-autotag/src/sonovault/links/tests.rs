use super::*;
use crate::catalog_runtime::*;
use cuerithm_credentials::{
    CredentialService, ProviderId as CredentialProvider, Secret, state::FileStateStore,
};
use serde_json::json;
struct LinksFixture;
impl CatalogTransport for LinksFixture {
    fn execute(
        &self,
        r: CatalogHttpRequest,
        _: &Arc<AtomicBool>,
        _: Option<&cuerithm_credentials::Revocation>,
    ) -> Result<CatalogHttpResponse> {
        assert!(
            r.headers
                .iter()
                .any(|(k, v)| k == "x-api-key" && v == "svk_live_fixture")
        );
        let body = if r.url.ends_with("/v1/tracks/links?beatport_id=999") {
            json!({"track_id":123,"title":"Signal","isrc":"USAAA2100001","links":[{"source":"beatport","external_id":"999","url":"https://www.beatport.com/track/signal/999"},{"source":"spotify","external_id":"5sICkBXVmaCQk5aISGR3x0","url":"https://open.spotify.com/track/5sICkBXVmaCQk5aISGR3x0"},{"source":"youtube","external_id":"dQw4w9WgXcQ","url":"https://www.youtube.com/watch?v=dQw4w9WgXcQ"}]})
        } else {
            assert!(r.url.ends_with("/v1/tracks/123"));
            json!({"id":123,"title":"Signal","artists":[{"name":"Credit"}],"releases":[],"isrc":"USAAA2100001","duration":300,"genre":[],"subgenre":[]})
        };
        Ok(CatalogHttpResponse {
            status: 200,
            headers: vec![],
            body: serde_json::to_vec(&body).unwrap(),
        })
    }
}
#[test]
fn platform_links_lookup_binds_selector_and_keeps_platform_ids_readonly() {
    let temp = tempfile::tempdir().unwrap();
    let service = CredentialService::new(
        crate::catalog_runtime::tests::VaultFixture::default(),
        Arc::new(FileStateStore::open(temp.path()).unwrap()),
    )
    .unwrap();
    let before = service.status(CredentialProvider::SonoVault).unwrap();
    service
        .save(
            &before.reference,
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
    .with_transport(Arc::new(LinksFixture));
    let backend = SonoVaultBackend::new(Arc::new(runtime));
    let candidate = backend
        .resolve_platform_lookup(
            &SonoVaultLookup::BeatportTrackId(999),
            &Arc::new(AtomicBool::new(false)),
        )
        .unwrap()
        .unwrap();
    candidate.validate().unwrap();
    assert_eq!(candidate.track_id.as_deref(), Some("123"));
    let Some(CatalogFacts::Source(facts)) = &candidate.facts else {
        panic!("Expected source-scoped facts")
    };
    assert!(
        facts
            .identities
            .iter()
            .any(|v| v.source == "beatport" && v.id == "999")
    );
    assert!(facts.identities.iter().any(|v| v.source == "spotify"));
    assert!(
        facts
            .links
            .iter()
            .any(|v| v.source == "youtube" && v.url == "https://youtu.be/dQw4w9WgXcQ")
    );
    assert!(candidate.other.is_empty());
    assert!(!crate::provider_native_field_allowed(
        "sonovault",
        crate::SupportedTag::TrackId
    ));
    assert!(!crate::provider_native_field_allowed(
        "sonovault",
        crate::SupportedTag::BPM
    ));
    let mut input = crate::catalog_runtime::tests::provider_input();
    input
        .media
        .tags
        .insert("BEATPORT_TRACK_ID".into(), "999".into());
    let mut config = AutotagConfig::default();
    config.options.match_by_id = true;
    let matches = backend.candidates(&input, &config).unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].track_id.as_deref(), Some("123"));
    assert_eq!(matches[0].reason, onetagger_tagger::MatchReason::ID);
}

#[test]
fn platform_link_evidence_rejects_unbound_ids_bad_positions_and_url_escape() {
    let mut value = json!({"track_id":123,"title":"Signal","isrc":null,"links":[{"source":"beatport","external_id":"999","url":"https://www.beatport.com/track/signal/999"}]});
    assert!(parse_links(&value, &SonoVaultLookup::TrackId(456)).is_err());
    assert!(parse_links(&value, &SonoVaultLookup::BeatportTrackId(888)).is_err());
    value["links"][0]["url"] = json!("https://evil.invalid/track/signal/999?token=fixture-private");
    let error = parse_links(&value, &SonoVaultLookup::BeatportTrackId(999))
        .err()
        .unwrap();
    assert!(!error.to_string().contains("fixture-private"));
    assert_eq!(
        SonoVaultLookup::DiscogsReleaseTrack {
            release_id: 42,
            position: "A1".into()
        }
        .query()
        .unwrap(),
        ("discogs_id".into(), "42-A1".into())
    );
    assert!(
        SonoVaultLookup::DiscogsReleaseTrack {
            release_id: 42,
            position: String::new()
        }
        .query()
        .is_err()
    );
    let mut input = crate::catalog_runtime::tests::provider_input();
    input
        .media
        .tags
        .insert("SPOTIFY_TRACK_ID".into(), "5sICkBXVmaCQk5aISGR3x0".into());
    assert!(known_source_lookup(&input).unwrap().is_none());
}

struct IsrcResponses {
    links: Value,
    detail: Value,
    calls: Arc<std::sync::atomic::AtomicUsize>,
}
impl CatalogTransport for IsrcResponses {
    fn execute(
        &self,
        r: CatalogHttpRequest,
        _: &Arc<AtomicBool>,
        _: Option<&cuerithm_credentials::Revocation>,
    ) -> Result<CatalogHttpResponse> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let body = if r.url.ends_with("/tracks/links?isrc=USAAA2100001") {
            &self.links
        } else {
            assert!(r.url.ends_with("/tracks/123"));
            &self.detail
        };
        Ok(CatalogHttpResponse {
            status: 200,
            headers: vec![],
            body: serde_json::to_vec(body).unwrap(),
        })
    }
}
fn isrc_backend(
    links_isrc: Option<Value>,
    detail_isrc: Option<Value>,
) -> (
    tempfile::TempDir,
    SonoVaultBackend,
    Arc<std::sync::atomic::AtomicUsize>,
) {
    let temp = tempfile::tempdir().unwrap();
    let service = CredentialService::new(
        crate::catalog_runtime::tests::VaultFixture::default(),
        Arc::new(FileStateStore::open(temp.path()).unwrap()),
    )
    .unwrap();
    let before = service.status(CredentialProvider::SonoVault).unwrap();
    service
        .save(
            &before.reference,
            Secret::new("svk_live_fixture".into()).unwrap(),
        )
        .unwrap();
    let mut links = json!({"track_id":123,"title":"Signal","links":[]});
    let mut detail = json!({"id":123,"title":"Signal","artists":[{"name":"Credit"}],"releases":[],"duration":300,"genre":[],"subgenre":[]});
    if let Some(value) = links_isrc {
        links["isrc"] = value;
    }
    if let Some(value) = detail_isrc {
        detail["isrc"] = value;
    }
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let runtime = CatalogRuntime::new(
        CatalogIdentity::new("test").unwrap(),
        Arc::new(MemoryCatalogState::default()),
        Arc::new(service),
    )
    .unwrap()
    .with_local_server("http://127.0.0.1:9")
    .unwrap()
    .with_transport(Arc::new(IsrcResponses {
        links,
        detail,
        calls: calls.clone(),
    }));
    (temp, SonoVaultBackend::new(Arc::new(runtime)), calls)
}
#[test]
fn isrc_link_lookup_rejects_another_isrc_before_detail_request() {
    let (_temp, backend, calls) =
        isrc_backend(Some(json!("USAAA2100002")), Some(json!("USAAA2100001")));
    let result = backend.resolve_platform_lookup(
        &SonoVaultLookup::Isrc("us-aaa-21-00001".into()),
        &Arc::new(AtomicBool::new(false)),
    );
    assert!(
        result.is_err(),
        "A matching internal track ID cannot prove a different ISRC"
    );
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
}
#[test]
fn isrc_link_lookup_rejects_missing_or_null_isrc_before_detail_request() {
    for missing in [None, Some(Value::Null)] {
        let (_temp, backend, calls) = isrc_backend(missing, Some(json!("USAAA2100001")));
        assert!(
            backend
                .resolve_platform_lookup(
                    &SonoVaultLookup::Isrc("USAAA2100001".into()),
                    &Arc::new(AtomicBool::new(false))
                )
                .is_err()
        );
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
}
#[test]
fn isrc_link_lookup_retains_proof_until_recording_detail_is_verified() {
    for detail in [None, Some(Value::Null), Some(json!("USAAA2100002"))] {
        let (_temp, backend, calls) = isrc_backend(Some(json!("us-aaa-21-00001")), detail);
        assert!(
            backend
                .resolve_platform_lookup(
                    &SonoVaultLookup::Isrc("USAAA2100001".into()),
                    &Arc::new(AtomicBool::new(false))
                )
                .is_err(),
            "A matching internal track ID cannot replace requested-ISRC proof"
        );
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
    }
    let (_temp, backend, calls) =
        isrc_backend(Some(json!("us-aaa-21-00001")), Some(json!("USAAA2100001")));
    let candidate = backend
        .resolve_platform_lookup(
            &SonoVaultLookup::Isrc("us-aaa-21-00001".into()),
            &Arc::new(AtomicBool::new(false)),
        )
        .unwrap()
        .unwrap();
    assert_eq!(candidate.reason, onetagger_tagger::MatchReason::ISRC);
    assert_eq!(candidate.isrc.as_deref(), Some("USAAA2100001"));
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
}
