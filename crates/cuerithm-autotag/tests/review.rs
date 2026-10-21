use cuerithm_autotag::{
    AutotagConfig, CatalogMatch, CatalogTrack, ProviderBackend, ProviderId, TagInput, Tagger,
};
use cuerithm_media::MediaInfo;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
struct Backend {
    calls: AtomicUsize,
    active: AtomicUsize,
    max: AtomicUsize,
    change_id: bool,
}
impl ProviderBackend for Backend {
    fn id(&self) -> ProviderId {
        ProviderId::Beatport
    }
    fn candidates(&self, _: &TagInput, _: &AutotagConfig) -> anyhow::Result<Vec<CatalogMatch>> {
        Ok(vec![])
    }
    fn extend(&self, track: &mut CatalogTrack, _: &AutotagConfig) -> anyhow::Result<()> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.max.fetch_max(active, Ordering::SeqCst);
        std::thread::sleep(std::time::Duration::from_millis(20));
        track.label = Some("Expanded label".into());
        if self.change_id {
            track.track_id = Some("another recording".into());
        }
        self.active.fetch_sub(1, Ordering::SeqCst);
        Ok(())
    }
}
fn input() -> TagInput {
    TagInput {
        path: "Artist - Signal.flac".into(),
        source_sha256: "a".repeat(64),
        duration_seconds: 300.,
        media: MediaInfo {
            codec: "flac".into(),
            sample_rate: 44100,
            channels: 2,
            bits_per_sample: Some(16),
            properties: Default::default(),
            tags: [
                ("artist".into(), "Artist".into()),
                ("title".into(), "Signal".into()),
            ]
            .into(),
        },
    }
}
fn candidate() -> CatalogMatch {
    CatalogMatch::new(
        0.99,
        CatalogTrack {
            platform: "beatport".into(),
            track_id: Some("selected recording".into()),
            title: "Signal".into(),
            artists: vec!["Artist".into()],
            ..Default::default()
        },
    )
}
fn config() -> AutotagConfig {
    let mut config = AutotagConfig::default();
    config.options.platforms = vec!["beatport".into()];
    config.options.threads = 1;
    config
}
#[test]
fn selected_expansion_uses_shared_provider_gate_and_does_not_mutate_snapshot() {
    let backend = Arc::new(Backend {
        calls: AtomicUsize::new(0),
        active: AtomicUsize::new(0),
        max: AtomicUsize::new(0),
        change_id: false,
    });
    let tagger = Arc::new(Tagger::with_backends(vec![backend.clone()]).unwrap());
    let frozen = candidate();
    std::thread::scope(|scope| {
        for _ in 0..3 {
            let tagger = tagger.clone();
            let frozen = frozen.clone();
            scope.spawn(move || {
                let expanded = tagger
                    .expand_review_candidate(
                        &input(),
                        &frozen,
                        &config(),
                        &Arc::new(AtomicBool::new(false)),
                    )
                    .unwrap();
                assert_eq!(expanded.track.label.as_deref(), Some("Expanded label"));
            });
        }
    });
    assert_eq!(backend.calls.load(Ordering::SeqCst), 3);
    assert_eq!(backend.max.load(Ordering::SeqCst), 1);
    assert!(frozen.track.label.is_none());
}
#[test]
fn selected_expansion_rejects_cancellation_and_recording_substitution() {
    let backend = Arc::new(Backend {
        calls: AtomicUsize::new(0),
        active: AtomicUsize::new(0),
        max: AtomicUsize::new(0),
        change_id: true,
    });
    let tagger = Tagger::with_backends(vec![backend.clone()]).unwrap();
    assert!(
        tagger
            .expand_review_candidate(
                &input(),
                &candidate(),
                &config(),
                &Arc::new(AtomicBool::new(true))
            )
            .is_err()
    );
    assert_eq!(backend.calls.load(Ordering::SeqCst), 0);
    assert!(
        tagger
            .expand_review_candidate(
                &input(),
                &candidate(),
                &config(),
                &Arc::new(AtomicBool::new(false))
            )
            .is_err()
    );
    assert_eq!(backend.calls.load(Ordering::SeqCst), 1);
}
