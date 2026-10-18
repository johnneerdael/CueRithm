use anyhow::Result;
use cuerithm_autotag::{
    AutotagConfig, CatalogMatch, ProviderBackend, ProviderId, TagInput, Tagger,
};
use cuerithm_media::MediaInfo;
use std::sync::{
    Arc, Barrier,
    atomic::{AtomicUsize, Ordering},
};
struct Slow {
    id: ProviderId,
    active: AtomicUsize,
    peak: AtomicUsize,
}
impl ProviderBackend for Slow {
    fn id(&self) -> ProviderId {
        self.id
    }
    fn candidates(&self, _: &TagInput, _: &AutotagConfig) -> Result<Vec<CatalogMatch>> {
        let n = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(n, Ordering::SeqCst);
        std::thread::sleep(std::time::Duration::from_millis(30));
        self.active.fetch_sub(1, Ordering::SeqCst);
        Ok(vec![])
    }
}
#[test]
fn traxsource_limit_is_one_and_beatport_respects_configured_network_concurrency() {
    for (provider, expected) in [(ProviderId::Traxsource, 1), (ProviderId::Beatport, 2)] {
        let backend = Arc::new(Slow {
            id: provider,
            active: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
        });
        let tagger = Tagger::with_backends(vec![backend.clone()]).unwrap();
        let mut config = AutotagConfig::default();
        config.options.platforms = vec![provider.as_str().into()];
        config.options.threads = 2;
        let input = TagInput {
            path: "fixture.flac".into(),
            source_sha256: "0".repeat(64),
            duration_seconds: 10.,
            media: MediaInfo {
                codec: "flac".into(),
                sample_rate: 44100,
                channels: 2,
                bits_per_sample: Some(16),
                properties: Default::default(),
                tags: Default::default(),
            },
        };
        let barrier = Barrier::new(6);
        std::thread::scope(|scope| {
            for _ in 0..6 {
                scope.spawn(|| {
                    barrier.wait();
                    tagger.match_track(&input, &config).unwrap();
                });
            }
        });
        assert_eq!(backend.peak.load(Ordering::SeqCst), expected);
    }
}
