use crate::{CleanupService, Library};
use camelot_core::{
    AnalysisConfig, FileAnalysis, KeyEstimate, Mode, MusicalKey, TempoEstimate,
    energy::TrackEnergy,
    lexicon_cues::{StructuralCue, StructuralCueResult},
};
use cuerithm_cleanup::{CleanupConfig, ConfiguredFix, MetadataField, SmartFix};
use cuerithm_media::MediaRuntime;
use onetagger_tag::Field;
use std::path::{Path, PathBuf};
pub fn result() -> FileAnalysis {
    let mut a =
        camelot_core::analyze_pcm(&vec![0.; 11025], 11025, &AnalysisConfig::default()).unwrap();
    a.key = Some(KeyEstimate {
        key: MusicalKey::new(0, Mode::Major).unwrap(),
        camelot: "8B".into(),
        margin: 0.5,
        features: None,
        candidates: vec![],
    });
    a.tempo = Some(TempoEstimate {
        bpm: 120.,
        strength: None,
        candidates: vec![],
        beats_seconds: vec![0., 0.5],
        downbeats_seconds: None,
        downbeat_time: Some(0.),
        method: "test".into(),
    });
    a.energy = Some(TrackEnergy {
        level: 5,
        method: "test".into(),
    });
    a.lexicon_cues = Some(StructuralCueResult {
        cues: vec![StructuralCue {
            time_seconds: 0.,
            name: "Start".into(),
            color: "Red".into(),
        }],
        emergency_loop: None,
    });
    FileAnalysis {
        selection: None,
        selected_beatgrid: None,
        publication: None,
        analysis: a,
        complete: true,
        verified_integrity: None,
        source_sample_rate: 44100,
        beat_sample_rate: None,
        neural_key_sample_rate: None,
        openkeyscan_sample_rate: None,
        source_timing: None,
        elapsed_seconds: 0.,
        media: None,
    }
}

pub struct Fixture {
    pub dir: tempfile::TempDir,
    pub db: PathBuf,
    pub lib: Library,
    pub rt: MediaRuntime,
    pub ids: Vec<String>,
    pub paths: Vec<PathBuf>,
}
impl Fixture {
    pub fn new(count: usize) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("app/db");
        let lib = Library::open(&db).unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let rt = MediaRuntime::from_bundle(&root.join("tools/media-runtime/bin")).unwrap();
        let mut ids = vec![];
        let mut paths = vec![];
        for i in 0..count {
            let path = dir.path().join(format!("track-{i}.flac"));
            std::fs::copy(
                root.join("crates/cuerithm-media/tests/fixtures/metadata.flac"),
                &path,
            )
            .unwrap();
            let mut wrapper = onetagger_tag::Tag::load_file(&path, true).unwrap();
            wrapper
                .tag_mut()
                .set_field(Field::Title, vec![format!("signal__track {i}")], true);
            wrapper
                .tag_mut()
                .set_field(Field::Genre, vec!["House".into()], true);
            wrapper.tag_mut().save_file(&path).unwrap();
            lib.enqueue(
                std::slice::from_ref(&path),
                lib.settings().unwrap().revision,
            )
            .unwrap();
            let run = lib.claim_next().unwrap().unwrap();
            let info = rt.probe(&path).unwrap();
            lib.apply_analysis_for_run(&run, &result(), &info).unwrap();
            ids.push(run.track_id);
            paths.push(path);
        }
        Self {
            dir,
            db,
            lib,
            rt,
            ids,
            paths,
        }
    }
    pub fn service(&self) -> CleanupService {
        CleanupService::new(self.lib.clone(), self.rt.clone())
    }
}
pub fn config() -> CleanupConfig {
    CleanupConfig {
        fixes: vec![
            ConfiguredFix {
                id: "spaces".into(),
                enabled: true,
                ignored_values: Default::default(),
                rule: SmartFix::ReplaceCharacters {
                    characters: "_".into(),
                },
            },
            ConfiguredFix {
                id: "garbage".into(),
                enabled: true,
                ignored_values: Default::default(),
                rule: SmartFix::RemoveGarbage {
                    field: MetadataField::Title,
                },
            },
        ],
        ..Default::default()
    }
}
