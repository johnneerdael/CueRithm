use super::*;
use crate::PreparationSettings;
use camelot_core::{
    AnalysisConfig, KeyEstimate, Mode, MusicalKey, TempoEstimate,
    energy::TrackEnergy,
    lexicon_cues::{StructuralCue, StructuralCueResult},
    prepared::PreparedMediaIdentity,
};
use cuerithm_media::MediaRuntime;
use std::path::{Path, PathBuf};
struct Fixture {
    _temp: tempfile::TempDir,
    lib: Library,
    source: PathBuf,
    runtime: MediaRuntime,
    run: JobRun,
    audio: PreparedAudio,
    result: FileAnalysis,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let folder = temp.path().canonicalize().unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let source = folder.join("source.flac");
        std::fs::copy(
            root.join("crates/cuerithm-media/tests/fixtures/metadata.flac"),
            &source,
        )
        .unwrap();
        let lib = Library::open(&folder.join("data/library.sqlite")).unwrap();
        let settings = PreparationSettings {
            tagging: Some(cuerithm_autotag::AutotagConfig::default()),
            ..Default::default()
        };
        let revision = lib.save_settings(settings).unwrap();
        lib.enqueue(std::slice::from_ref(&source), revision)
            .unwrap();
        let run = lib.claim_next().unwrap().unwrap();
        let runtime = MediaRuntime::from_bundle(&root.join("tools/media-runtime/bin")).unwrap();
        let audio = runtime.decode(&source, lib.data_dir()).unwrap();
        let mut analysis =
            camelot_core::analyze_pcm(&vec![0.; 5512], 11025, &AnalysisConfig::default()).unwrap();
        analysis.duration_seconds = audio.clock.duration_seconds();
        analysis.models = run.snapshot.models.clone();
        analysis.key = Some(KeyEstimate {
            key: MusicalKey::new(0, Mode::Major).unwrap(),
            camelot: "8B".into(),
            margin: 0.5,
            features: None,
            candidates: vec![],
        });
        analysis.tempo = Some(TempoEstimate {
            bpm: 120.,
            strength: None,
            candidates: vec![],
            beats_seconds: vec![0., 0.25],
            downbeats_seconds: None,
            downbeat_time: Some(0.),
            method: "fixture".into(),
        });
        analysis.energy = Some(TrackEnergy {
            level: 6,
            method: "fixture".into(),
        });
        analysis.lexicon_cues = Some(StructuralCueResult {
            cues: vec![StructuralCue {
                time_seconds: 0.,
                name: "Start".into(),
                color: "Red".into(),
            }],
            emergency_loop: None,
        });
        let result = FileAnalysis {
            publication: None,
            analysis,
            complete: true,
            verified_integrity: None,
            source_sample_rate: 44100,
            beat_sample_rate: None,
            neural_key_sample_rate: None,
            openkeyscan_sample_rate: None,
            source_timing: None,
            elapsed_seconds: 918.25,
            media: Some(PreparedMediaIdentity {
                export_timing: None,
                cue_genre: None,
                decoder: audio.decoder_identity.clone(),
                codec: audio.info.codec.clone(),
                source_sha256: audio.source_sha256.clone(),
                pcm_sha256: audio.pcm_sha256().unwrap(),
                clock: audio.clock.clone(),
            }),
        };
        lib.cache_analysis(&run, "fixture-engine", &result).unwrap();
        lib.fail_for_run(&run, "matching", "fixture requires solver")
            .unwrap();
        Self {
            _temp: temp,
            lib,
            source,
            runtime,
            run,
            audio,
            result,
        }
    }
    fn solver_settings(&self) -> PreparationSettings {
        let mut settings = self.run.snapshot.settings.clone();
        let solver = &mut settings.tagging.as_mut().unwrap().solver;
        solver.endpoint = Some("http://127.0.0.1:8191".into());
        solver.timeout_seconds = 41;
        settings
    }
}
#[test]
fn solver_only_retry_captures_new_transport_and_reuses_validated_original_analysis() {
    let f = Fixture::new();
    let original = serde_json::to_string(&f.result).unwrap();
    f.lib
        .conn()
        .unwrap()
        .execute(
            "INSERT INTO preparation_analysis VALUES(?1,?2,'fixture-engine',?3,?4)",
            params![
                "unrelated-source",
                f.run.settings_revision,
                f.audio.decoder_identity,
                original
            ],
        )
        .unwrap();
    let revision = f.lib.save_settings(f.solver_settings()).unwrap();
    assert_ne!(revision, f.run.settings_revision);
    let next = f.lib.retry(&f.run.id, Some(revision.clone())).unwrap();
    let run = f.lib.claim_next().unwrap().unwrap();
    assert_eq!(run.id, next);
    assert_eq!(run.settings_revision, revision);
    assert_eq!(
        run.snapshot
            .settings
            .tagging
            .as_ref()
            .unwrap()
            .solver
            .endpoint
            .as_deref(),
        Some("http://127.0.0.1:8191")
    );
    assert_eq!(
        run.snapshot
            .settings
            .tagging
            .as_ref()
            .unwrap()
            .solver
            .timeout_seconds,
        41
    );
    let audio = f.runtime.decode(&f.source, f.lib.data_dir()).unwrap();
    let cached = f
        .lib
        .cached_analysis(&run, "fixture-engine", &audio)
        .unwrap()
        .expect("solver-only retry must reuse completed analysis under its new transport revision");
    assert_eq!(serde_json::to_string(&cached).unwrap(), original);
    assert_eq!(f.lib.conn().unwrap().query_row("SELECT COUNT(*) FROM preparation_analysis WHERE source_hash='unrelated-source' AND settings_revision=?1",[&revision],|r|r.get::<_,u64>(0)).unwrap(),0);
    for field in ["source", "models", "decoder", "frames", "pcm"] {
        let mut changed: serde_json::Value = serde_json::from_str(&original).unwrap();
        match field {
            "source" => changed["media"]["source_sha256"] = serde_json::json!("other"),
            "models" => changed["analysis"]["models"] = serde_json::json!({"unexpected":"model"}),
            "decoder" => changed["media"]["decoder"] = serde_json::json!("other"),
            "frames" => {
                changed["media"]["clock"]["frames"] = serde_json::json!(audio.clock.frames + 1)
            }
            "pcm" => changed["media"]["pcm_sha256"] = serde_json::json!("other"),
            _ => unreachable!(),
        };
        f.lib.conn().unwrap().execute("UPDATE preparation_analysis SET analysis_json=?1 WHERE source_hash=?2 AND settings_revision=?3",params![changed.to_string(),run.source_sha256,revision]).unwrap();
        assert!(
            f.lib
                .cached_analysis(&run, "fixture-engine", &audio)
                .unwrap()
                .is_none(),
            "aliased cache must retain {field} verification"
        );
    }
    assert!(
        f.lib
            .cached_analysis(&run, "different-engine", &audio)
            .unwrap()
            .is_none()
    );
}
#[test]
fn solver_retry_does_not_reuse_analysis_after_engine_or_source_changes() {
    for changed_source in [false, true] {
        let f = Fixture::new();
        let mut settings = f.solver_settings();
        if changed_source {
            std::fs::copy(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../cuerithm-media/tests/fixtures/covered.flac"),
                &f.source,
            )
            .unwrap();
            assert_ne!(
                cuerithm_media::file_sha256(&f.source).unwrap(),
                f.run.source_sha256
            );
        } else {
            settings.engine.sample_rate = 22050;
        }
        let revision = f.lib.save_settings(settings).unwrap();
        f.lib.retry(&f.run.id, Some(revision)).unwrap();
        let run = f.lib.claim_next().unwrap().unwrap();
        let audio = f.runtime.decode(&f.source, f.lib.data_dir()).unwrap();
        assert!(
            f.lib
                .cached_analysis(&run, "fixture-engine", &audio)
                .unwrap()
                .is_none()
        );
        assert_eq!(f.lib.conn().unwrap().query_row("SELECT COUNT(*) FROM preparation_analysis WHERE source_hash=?1 AND settings_revision=?2",params![run.source_sha256,run.settings_revision],|r|r.get::<_,u64>(0)).unwrap(),0);
    }
}

#[test]
fn verified_reuse_remains_available_for_another_metadata_only_revision() {
    let f = Fixture::new();
    let original = f.run.source_sha256.clone();
    let mut run = f.run;
    replace_metadata_fixture(&f.lib, &f.runtime, &f.source, "metadata revision one");
    let audio = f.runtime.decode(&f.source, f.lib.data_dir()).unwrap();
    run.source_sha256 = audio.source_sha256.clone();
    f.lib.conn().unwrap().execute("INSERT INTO preparation_reuse(job_id,source_hash,analysis_source_hash,analysis_settings_revision) VALUES(?1,?2,?3,?4)",params![run.id,run.source_sha256,original,run.settings_revision]).unwrap();
    assert_eq!(
        f.lib
            .cached_analysis(&run, "fixture-engine", &audio)
            .unwrap()
            .unwrap()
            .elapsed_seconds,
        918.25
    );
    let previous = run.source_sha256.clone();
    replace_metadata_fixture(&f.lib, &f.runtime, &f.source, "metadata revision two");
    let audio = f.runtime.decode(&f.source, f.lib.data_dir()).unwrap();
    run.source_sha256 = audio.source_sha256.clone();
    f.lib
        .conn()
        .unwrap()
        .execute(
            "UPDATE preparation_reuse SET source_hash=?1,analysis_source_hash=?2 WHERE job_id=?3",
            params![run.source_sha256, previous, run.id],
        )
        .unwrap();
    let again=f.lib.cached_analysis(&run,"fixture-engine",&audio).unwrap().expect("a second protected metadata-only edit must retain the previously verified measured result");
    assert_eq!(again.elapsed_seconds, 918.25);
}

fn replace_metadata_fixture(lib: &Library, runtime: &MediaRuntime, source: &Path, title: &str) {
    use cuerithm_autotag::*;
    let audio = runtime.decode(source, lib.data_dir()).unwrap();
    let input = TagInput {
        path: source.to_owned(),
        source_sha256: audio.source_sha256.clone(),
        media: audio.info.clone(),
        duration_seconds: audio.clock.duration_seconds(),
    };
    let mut config = AutotagConfig {
        no_match: NoMatchPolicy::KeepExisting,
        ..Default::default()
    };
    config.options.tags.clear();
    config.required_tags.clear();
    let outcome = TagMatchOutcome {
        status: MatchStatus::NoMatch,
        matches: vec![],
        attempted: vec![],
        warnings: vec![],
    };
    let mut patch = MetadataPatch::default();
    patch.text.insert(
        MetadataTextField::Title,
        TextEdit::Set {
            values: vec![title.into()],
        },
    );
    let staged = TagWritePlan::build(input, &outcome, &config, &AnalysisTags::default())
        .unwrap()
        .with_editor_patch(patch)
        .unwrap()
        .stage_prepared(runtime, &audio, lib.data_dir())
        .unwrap();
    std::fs::copy(&staged.path, source).unwrap();
}
