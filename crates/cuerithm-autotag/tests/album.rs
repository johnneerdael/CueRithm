use cuerithm_autotag::{
    AlbumHint, AutotagConfig, CatalogAlbum, CatalogMatch, CatalogTrack, ProviderBackend,
    ProviderId, TagInput, Tagger,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};
struct Provider {
    id: ProviderId,
    searches: AtomicUsize,
    extensions: AtomicUsize,
    fail_extension: bool,
}
impl ProviderBackend for Provider {
    fn id(&self) -> ProviderId {
        self.id
    }
    fn candidates(
        &self,
        _: &TagInput,
        config: &AutotagConfig,
    ) -> anyhow::Result<Vec<CatalogMatch>> {
        self.searches.fetch_add(1, Ordering::SeqCst);
        Ok(self
            .album("searched-release", config)?
            .unwrap()
            .tracks
            .into_iter()
            .map(|track| CatalogMatch::new(1., track))
            .collect())
    }
    fn album(&self, id: &str, _: &AutotagConfig) -> anyhow::Result<Option<CatalogAlbum>> {
        Ok(Some(CatalogAlbum {
            id: id.into(),
            name: "Fixture album".into(),
            tracks: vec![CatalogTrack {
                platform: self.id.as_str().into(),
                title: "Signal".into(),
                artists: vec!["Fixture Artist".into()],
                album: Some("Fixture album".into()),
                album_artists: vec!["Fixture Artist".into()],
                track_id: Some("101".into()),
                release_id: Some(id.into()),
                track_number: Some(1.into()),
                track_total: Some(1),
                duration: Duration::from_secs(300),
                ..Default::default()
            }],
        }))
    }
    fn extend(&self, _: &mut CatalogTrack, _: &AutotagConfig) -> anyhow::Result<()> {
        self.extensions.fetch_add(1, Ordering::SeqCst);
        anyhow::ensure!(!self.fail_extension, "extension must not be requested");
        Ok(())
    }
}
#[test]
fn album_candidates_can_resolve_tracks_without_repeating_catalog_search() {
    for id in [ProviderId::Beatport, ProviderId::Traxsource] {
        let provider = Arc::new(Provider {
            id,
            searches: AtomicUsize::new(0),
            extensions: AtomicUsize::new(0),
            fail_extension: false,
        });
        let tagger = Tagger::with_backends(vec![provider.clone()]).unwrap();
        let mut config = AutotagConfig::default();
        config.options.platforms = vec![id.as_str().into()];
        config.options.album_tagging = true;
        let album = tagger
            .get_album(id, "release-1", &config, &Arc::new(AtomicBool::new(false)))
            .unwrap()
            .unwrap();
        let hint = AlbumHint {
            provider: id,
            album,
        };
        let input = TagInput {
            path: "fixture.flac".into(),
            source_sha256: "0".repeat(64),
            duration_seconds: 300.,
            media: cuerithm_media::MediaInfo {
                codec: "flac".into(),
                sample_rate: 44100,
                channels: 2,
                bits_per_sample: Some(16),
                properties: Default::default(),
                tags: [
                    ("title".into(), "Signal".into()),
                    ("artist".into(), "Fixture Artist".into()),
                ]
                .into(),
            },
        };
        let result = tagger
            .match_track_with_album(
                &input,
                &config,
                &Arc::new(AtomicBool::new(false)),
                Some(&hint),
            )
            .unwrap();
        assert_eq!(result.matches.len(), 1);
        assert_eq!(
            result.matches[0].reason,
            onetagger_tagger::MatchReason::Album
        );
        assert_eq!(provider.searches.load(Ordering::SeqCst), 0);
        assert_eq!(
            provider.extensions.load(Ordering::SeqCst),
            usize::from(id == ProviderId::Beatport)
        );
        config.options.album_tagging = false;
        let _ = tagger
            .match_track_with_album(
                &input,
                &config,
                &Arc::new(AtomicBool::new(false)),
                Some(&hint),
            )
            .unwrap();
        assert_eq!(provider.searches.load(Ordering::SeqCst), 1);
        assert_eq!(
            provider.extensions.load(Ordering::SeqCst),
            1 + usize::from(id == ProviderId::Beatport)
        );
    }
}

#[test]
fn cached_traxsource_album_survives_unavailable_extension_with_optional_metadata_absent() {
    let provider = Arc::new(Provider {
        id: ProviderId::Traxsource,
        searches: AtomicUsize::new(0),
        extensions: AtomicUsize::new(0),
        fail_extension: true,
    });
    let tagger = Tagger::with_backends(vec![provider.clone()]).unwrap();
    let mut config = AutotagConfig::default();
    config.options.platforms = vec!["traxsource".into()];
    config.options.album_tagging = true;
    config.options.tags = Tagger::provider_options()
        .into_iter()
        .find(|option| option.id == "traxsource")
        .unwrap()
        .supported_tags;
    let album = tagger
        .get_album(
            ProviderId::Traxsource,
            "456",
            &config,
            &Arc::new(AtomicBool::new(false)),
        )
        .unwrap()
        .unwrap();
    let hint = AlbumHint {
        provider: ProviderId::Traxsource,
        album,
    };
    let input = TagInput {
        path: "fixture.flac".into(),
        source_sha256: "0".repeat(64),
        duration_seconds: 300.,
        media: cuerithm_media::MediaInfo {
            codec: "flac".into(),
            sample_rate: 44100,
            channels: 2,
            bits_per_sample: Some(16),
            properties: Default::default(),
            tags: [
                ("title".into(), "Signal".into()),
                ("artist".into(), "Fixture Artist".into()),
            ]
            .into(),
        },
    };
    let outcome = tagger
        .match_track_with_album(
            &input,
            &config,
            &Arc::new(AtomicBool::new(false)),
            Some(&hint),
        )
        .unwrap();
    assert_eq!(outcome.matches.len(), 1);
    assert_eq!(outcome.matches[0].track, hint.album.tracks[0]);
    assert_eq!(provider.searches.load(Ordering::SeqCst), 0);
    assert_eq!(provider.extensions.load(Ordering::SeqCst), 0);

    // Falling back to ordinary search still requires extension and surfaces failures.
    let error = tagger
        .match_track_with_album(&input, &config, &Arc::new(AtomicBool::new(false)), None)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("extension must not be requested")
    );
    assert_eq!(provider.searches.load(Ordering::SeqCst), 1);
    assert_eq!(provider.extensions.load(Ordering::SeqCst), 1);
}
