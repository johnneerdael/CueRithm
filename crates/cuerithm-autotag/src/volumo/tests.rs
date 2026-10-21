use super::*;
use crate::{
    CatalogFacts, CatalogHttpRequest, CatalogHttpResponse, CatalogIdentity, CatalogRuntime,
    CatalogTransport, MemoryCatalogState, MemoryChallengeStore, TransferDate,
};
use cuerithm_credentials::{
    CredentialService, ProviderId as CredentialProvider, Secret, state::FileStateStore,
};
use onetagger_tagger::MatchReason;
use serde_json::{Value, json};
use std::sync::Mutex;
type Calls = Arc<Mutex<Vec<(String, bool)>>>;

fn page_value() -> Value {
    let track = |id, isrc, title| json!({"id":id,"title":title,"version":"Club Mix","isrc":isrc,"duration":240123.5,"bpm":126,"keysign":"C minor","available":true,"artists":[{"name":"Artist"}],"featured_artists":[{"name":"Guest"}],"remixers":[{"name":"Remixer"}],"genre":1,"genre_id":1,"recordlabel":{"id":33,"name":"Label"},"album":{"id":22,"title":"Selected EP","catalog_number":"CAT-001"}});
    json!({"page":"/track/[trackId]","query":{"trackId":"123-signal-club-mix"},"props":{"pageProps":{"dehydratedState":{"queries":[
        {"queryKey":[{"scope":"Genres"}],"state":{"data":[{"id":1,"name":"House"}]}},
        {"queryKey":[{"scope":"Album","trackId":123}],"state":{"data":{"id":22,"title":"Selected EP","catalog_number":"CAT-001","recordlabel":{"id":33,"name":"Label"},"original_release_date":"2024-02","release_start_at":"2024-01-31T22:00:00Z","tracks_total":2,"tracks":[track(122,"USAAA2100002","Other"),track(123,"USAAA2100001","Signal")]}}}
    ]}}}})
}
fn html(value: &Value) -> String {
    let recording = json!({"@type":["MusicRecording","Product"],"@id":"https://volumo.com/track/123-signal-club-mix","url":"https://volumo.com/track/123-signal-club-mix","volumo:trackId":123,"recordingOf":{"name":"Signal"},"byArtist":[{"name":"Artist"}],"isrcCode":"USAAA2100001","genre":"House","image":"/img/size/0x0/11111111-1111-4111-8111-111111111111.webp","inAlbum":{"dateCreated":"2024-01-31T22:00:00Z"}});
    format!(
        "<script id='__NEXT_DATA__' type='application/json'>{value}</script><script type='application/ld+json'>{recording}</script>"
    )
}
#[test]
fn public_track_mapping_binds_selected_track_and_preserves_release_date_precision() {
    let matched = page::parse(&html(&page_value()), 123, Some("USAAA2100001")).unwrap();
    matched.validate().unwrap();
    assert_eq!(matched.track_id.as_deref(), Some("123"));
    assert_eq!(matched.release_id.as_deref(), Some("22"));
    assert_eq!(matched.title, "Signal");
    assert_eq!(matched.version.as_deref(), Some("Club Mix"));
    assert_eq!(matched.artists, ["Artist", "Guest"]);
    assert_eq!(matched.remixers, ["Remixer"]);
    assert_eq!(matched.album.as_deref(), Some("Selected EP"));
    assert_eq!(matched.label.as_deref(), Some("Label"));
    assert_eq!(matched.catalog_number.as_deref(), Some("CAT-001"));
    assert_eq!(matched.genres, ["House"]);
    assert_eq!(matched.track_number.as_ref().unwrap().to_string(), "2");
    assert_eq!(matched.track_total, Some(2));
    assert!((matched.duration.as_secs_f64() - 240.1235).abs() < 0.000001);
    assert_eq!(
        matched.selected_release_date(),
        Some(&TransferDate {
            year: 2024,
            month: Some(2),
            day: None
        })
    );
    assert!(matched.release_date.is_none());
    assert_eq!(matched.bpm, Some(126));
    assert_eq!(matched.key.as_deref(), Some("C minor"));
    assert!(matches!(matched.facts, Some(CatalogFacts::Source(_))));
    assert!(matched.other.is_empty());
}
#[test]
fn public_track_mapping_rejects_identifier_isrc_and_album_substitution() {
    assert!(page::parse(&html(&page_value()), 124, None).is_err());
    assert!(page::parse(&html(&page_value()), 123, Some("USAAA2109999")).is_err());
    let mut value = page_value();
    value["props"]["pageProps"]["dehydratedState"]["queries"][1]["state"]["data"]["tracks"][1]["album"]
        ["id"] = json!(999);
    assert!(page::parse(&html(&value), 123, None).is_err());
    assert!(page::parse("<html>layout changed</html>", 123, None).is_err());
}
#[test]
fn public_track_urls_strip_tracking_without_relaxing_origin_or_path_identity() {
    assert_eq!(
        page::track_url("https://volumo.com/track/123-signal?si=synthetic-ref#fragment").unwrap(),
        (123, "https://volumo.com/track/123-signal".into())
    );
    for url in [
        "https://hostile.invalid/track/123-signal?si=synthetic-ref",
        "https://volumo.com@hostile.invalid/track/123-signal",
        "https://user@volumo.com/track/123-signal",
        "http://volumo.com/track/123-signal",
        "https://volumo.com:444/track/123-signal",
        "https://volumo.com/album/123-signal",
        "https://volumo.com/track/%31%32%33-signal",
        "https://volumo.com/track/123-signal%2Fother",
        "https://volumo.com/track/123-signal/other",
    ] {
        assert!(
            page::track_url(url).is_err(),
            "unsafe track path must be rejected"
        );
    }
}
struct Fixture {
    calls: Calls,
    response_isrc: &'static str,
    scheduled: bool,
}
impl CatalogTransport for Fixture {
    fn execute(
        &self,
        r: CatalogHttpRequest,
        _: &Arc<AtomicBool>,
        _: Option<&cuerithm_credentials::Revocation>,
    ) -> Result<CatalogHttpResponse> {
        let bearer = r.headers.iter().any(|(name, value)| {
            name.eq_ignore_ascii_case("authorization") && value == "Bearer synthetic-volumo-key"
        });
        self.calls.lock().unwrap().push((r.url.clone(), bearer));
        let body = if r.url.contains("/api/pub/track_by_isrc/") {
            assert!(bearer, "only official API receives the credential");
            json!({"isrc":self.response_isrc,"status":if self.scheduled{"scheduled"}else{"live"},"url":"https://volumo.com/track/123-signal-club-mix","embed_src":"https://embed.invalid/never-requested","embed_html":"<script>never executed</script>"}).to_string()
        } else {
            assert!(!bearer, "public metadata must not receive an API key");
            html(&page_value())
        };
        Ok(CatalogHttpResponse {
            status: 200,
            headers: vec![],
            body: body.into_bytes(),
        })
    }
}
fn backend(
    response_isrc: &'static str,
    scheduled: bool,
) -> (tempfile::TempDir, VolumoBackend, Calls) {
    let temp = tempfile::tempdir().unwrap();
    let credentials = CredentialService::new(
        crate::catalog_runtime::tests::VaultFixture::default(),
        Arc::new(FileStateStore::open(temp.path()).unwrap()),
    )
    .unwrap();
    let status = credentials.status(CredentialProvider::Volumo).unwrap();
    credentials
        .save(
            &status.reference,
            Secret::new("synthetic-volumo-key".into()).unwrap(),
        )
        .unwrap();
    let calls = Arc::new(Mutex::new(vec![]));
    let runtime = CatalogRuntime::new(
        CatalogIdentity::new("test").unwrap(),
        Arc::new(MemoryCatalogState::default()),
        Arc::new(credentials),
    )
    .unwrap()
    .with_local_server("http://127.0.0.1:9")
    .unwrap()
    .with_transport(Arc::new(Fixture {
        calls: calls.clone(),
        response_isrc,
        scheduled,
    }));
    (
        temp,
        VolumoBackend::new(Arc::new(runtime), Arc::new(MemoryChallengeStore::default())).unwrap(),
        calls,
    )
}
fn input() -> TagInput {
    let mut input = crate::catalog_runtime::tests::provider_input();
    input
        .media
        .tags
        .insert("isrc".into(), "USAAA2100001".into());
    input
}
#[test]
fn official_isrc_lookup_then_public_expansion_uses_separate_auth_and_preserves_identity() {
    let (_temp, backend, calls) = backend("USAAA2100001", false);
    let config = AutotagConfig::default();
    let cancel = Arc::new(AtomicBool::new(false));
    let mut matches = backend
        .candidates_cancellable(&input(), &config, &cancel)
        .unwrap();
    assert_eq!(matches.len(), 1);
    let mut candidate = matches.remove(0);
    assert_eq!(candidate.reason, MatchReason::ISRC);
    backend
        .extend_candidate_cancellable(&mut candidate, &config, &cancel)
        .unwrap();
    assert_eq!(candidate.track_id.as_deref(), Some("123"));
    let calls = calls.lock().unwrap();
    assert!(calls[0].0.contains("/api/pub/track_by_isrc/USAAA2100001"));
    assert!(calls[0].1);
    assert!(calls.iter().skip(1).all(|(_, bearer)| !*bearer));
    assert!(!calls.iter().any(|(url, _)| url.contains("embed")));
}
#[test]
fn mismatched_isrc_and_scheduled_response_never_fetch_a_public_track() {
    for (isrc, scheduled) in [("USAAA2109999", false), ("USAAA2100001", true)] {
        let (_temp, backend, calls) = backend(isrc, scheduled);
        assert!(
            backend
                .candidates(&input(), &AutotagConfig::default())
                .is_err()
        );
        assert_eq!(calls.lock().unwrap().len(), 1);
    }
}
#[test]
fn saved_track_ids_use_public_metadata_but_still_require_setup_and_never_guess_search() {
    let (_temp, backend, calls) = backend("USAAA2100001", false);
    let mut input = input();
    input.media.tags.remove("isrc");
    input
        .media
        .tags
        .insert("VOLUMO_TRACK_ID".into(), "123".into());
    let mut config = AutotagConfig::default();
    config.options.match_by_id = true;
    let matched = backend.candidates(&input, &config).unwrap();
    assert_eq!(matched.len(), 1);
    assert_eq!(matched[0].reason, MatchReason::ID);
    assert!(
        calls
            .lock()
            .unwrap()
            .iter()
            .all(|(url, bearer)| url.ends_with("/track/123") && !bearer)
    );
    calls.lock().unwrap().clear();
    input.media.tags.remove("VOLUMO_TRACK_ID");
    let error = backend.candidates(&input, &config).unwrap_err();
    assert!(error.to_string().contains("ISRC or saved Volumo track ID"));
    assert!(calls.lock().unwrap().is_empty());
    input
        .media
        .tags
        .insert("VOLUMO_TRACK_ID".into(), "123".into());
    let runtime = CatalogRuntime::development()
        .unwrap()
        .with_transport(Arc::new(Fixture {
            calls: calls.clone(),
            response_isrc: "USAAA2100001",
            scheduled: false,
        }));
    let missing =
        VolumoBackend::new(Arc::new(runtime), Arc::new(MemoryChallengeStore::default())).unwrap();
    assert!(missing.candidates(&input, &config).is_err());
    assert!(calls.lock().unwrap().is_empty());
}
