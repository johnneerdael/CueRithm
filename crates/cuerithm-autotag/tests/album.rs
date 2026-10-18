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
}
impl ProviderBackend for Provider {
    fn id(&self) -> ProviderId {
        self.id
    }
    fn candidates(&self, _: &TagInput, _: &AutotagConfig) -> anyhow::Result<Vec<CatalogMatch>> {
        self.searches.fetch_add(1, Ordering::SeqCst);
        Ok(vec![])
    }
    fn album(&self, id: &str, _: &AutotagConfig) -> anyhow::Result<Option<CatalogAlbum>> {
        Ok(Some(CatalogAlbum {
            id: id.into(),
            name: "Fixture album".into(),
            tracks: vec![CatalogTrack {
                platform: self.id.as_str().into(),
                title: "Signal".into(),
                artists: vec!["Fixture Artist".into()],
                duration: Duration::from_secs(300),
                ..Default::default()
            }],
        }))
    }
}
#[test]
fn album_candidates_can_resolve_tracks_without_repeating_catalog_search() {
    let provider = Arc::new(Provider {
        id: ProviderId::Beatport,
        searches: AtomicUsize::new(0),
    });
    let tagger = Tagger::with_backends(vec![provider.clone()]).unwrap();
    let mut config = AutotagConfig::default();
    config.options.platforms = vec!["beatport".into()];
    config.options.album_tagging = true;
    let album = tagger
        .get_album(
            ProviderId::Beatport,
            "release-1",
            &config,
            &AtomicBool::new(false),
        )
        .unwrap()
        .unwrap();
    let hint = AlbumHint {
        provider: ProviderId::Beatport,
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
        .match_track_with_album(&input, &config, &AtomicBool::new(false), Some(&hint))
        .unwrap();
    assert_eq!(result.matches.len(), 1);
    assert_eq!(
        result.matches[0].reason,
        onetagger_tagger::MatchReason::Album
    );
    assert_eq!(provider.searches.load(Ordering::SeqCst), 0);
    config.options.album_tagging = false;
    let _ = tagger
        .match_track_with_album(&input, &config, &AtomicBool::new(false), Some(&hint))
        .unwrap();
    assert_eq!(provider.searches.load(Ordering::SeqCst), 1);
}
