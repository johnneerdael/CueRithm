use cuerithm_library::playback::{CacheSource, Player};
use cuerithm_media::MediaRuntime;
use rodio::Source;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

fn audio(d: &Path) -> Arc<cuerithm_media::PreparedAudio> {
    let root = std::env::var_os("CUERITHM_MEDIA_RUNTIME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/media-runtime/bin")
        });
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../camelot-source-analysis/tests/fixtures/pulses64-stereo16.flac");
    Arc::new(
        MediaRuntime::from_bundle(&root)
            .unwrap()
            .decode(&path, d)
            .unwrap(),
    )
}
#[test]
fn cached_source_seek_and_eof_use_exact_playable_frames() {
    let d = tempfile::tempdir().unwrap();
    let audio = audio(d.path());
    let mut source = CacheSource::open(audio.clone()).unwrap();
    assert_eq!(source.channels(), 2);
    assert_eq!(source.sample_rate(), 44100);
    source.try_seek(Duration::from_secs_f64(10.)).unwrap();
    assert_eq!(
        source.next(),
        Some(audio.read_frames(441000, 1).unwrap()[0])
    );
    source
        .try_seek(Duration::from_secs_f64(audio.clock.duration_seconds()))
        .unwrap();
    assert!(source.next().is_none());
}
#[test]
fn paused_seek_load_and_device_errors_are_reported_without_changing_track_identity() {
    let d = tempfile::tempdir().unwrap();
    let audio = audio(d.path());
    let mut player =
        Player::with_output_factory(|| anyhow::bail!("Test output device is unavailable")).unwrap();
    assert!(player.play().is_err());
    player.load("track-a".into(), audio).unwrap();
    player.seek(2.).unwrap();
    assert_eq!(player.status().position_seconds, 2.);
    assert_eq!(player.status().track_id.as_deref(), Some("track-a"));
    assert!(player.seek(f64::NAN).is_err());
    assert!(player.seek(-1.).is_err());
    assert!(player.seek(65.).is_err());
    assert!(player.play().is_err());
    assert!(player.status().error.unwrap().contains("unavailable"));
    assert!(!player.status().playing);
    player.pause().unwrap();
    assert_eq!(player.status().track_id.as_deref(), Some("track-a"));
}

#[test]
#[ignore = "requires a real audio output device"]
fn native_output_starts_seeks_and_pauses() {
    let d = tempfile::tempdir().unwrap();
    let mut player = Player::new().unwrap();
    player
        .load("native-fixture".into(), audio(d.path()))
        .unwrap();
    player.set_volume(0.03).unwrap();
    player.play().unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let status = player.status();
    assert!(status.playing && status.position_seconds > 0., "{status:?}");
    player.seek(10.).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    player.pause().unwrap();
    assert!(!player.status().playing);
    assert!(player.status().position_seconds >= 10.);
}
