//! Opt-in half of `tools/mixxx-acceptance/run.py`: writes a scratch Mixxx library with
//! the same timing export and staged writer the app uses, so an installed Mixxx can
//! then be pointed at it. Never touches a real library: the folder comes from the
//! environment and is created by the harness.
use super::*;
use crate::{
    BeatgridLock, DjSoftware, MixxxContainer, MixxxExistingTracks, MixxxOptions, SyncBeat, SyncCue,
    SyncPlaylist, SyncTrack, timing,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Cue {
    index: u8,
    name: String,
    start: f64,
    end: Option<f64>,
    color: Option<u32>,
}
#[derive(Deserialize)]
struct Entry {
    file: String,
    title: String,
    key: String,
    bpm: f64,
    first_beat: f64,
    duration: f64,
    sample_rate: u32,
    cues: Vec<Cue>,
}

#[test]
#[ignore = "driven by tools/mixxx-acceptance/run.py; set CUERITHM_MIXXX_ACCEPTANCE"]
fn write_acceptance_library() {
    let folder = PathBuf::from(
        std::env::var_os("CUERITHM_MIXXX_ACCEPTANCE").expect("CUERITHM_MIXXX_ACCEPTANCE"),
    );
    let mode = match std::env::var("CUERITHM_MIXXX_MODE").as_deref() {
        Ok("keep") => MixxxExistingTracks::Keep,
        _ => MixxxExistingTracks::Replace,
    };
    let manifest: Vec<Entry> =
        serde_json::from_slice(&std::fs::read(folder.join("music/manifest.json")).unwrap())
            .unwrap();
    let tracks: Vec<SyncTrack> = manifest
        .iter()
        .map(|entry| SyncTrack {
            id: entry.file.clone(),
            path: folder.join("music").join(&entry.file),
            title: entry.title.clone(),
            artist: "CueRithm acceptance".into(),
            album: "Acceptance".into(),
            key: entry.key.clone(),
            bpm: entry.bpm,
            duration_seconds: entry.duration,
            sample_rate: entry.sample_rate,
            rating: 4,
            beatgrid: vec![SyncBeat {
                seconds: entry.first_beat,
                bpm: entry.bpm,
                beat: 1,
            }],
            cues: entry
                .cues
                .iter()
                .map(|cue| SyncCue {
                    index: cue.index,
                    name: cue.name.clone(),
                    start_seconds: cue.start,
                    end_seconds: cue.end,
                    color: cue.color,
                    ..SyncCue::default()
                })
                .collect(),
            ..SyncTrack::default()
        })
        .collect();
    let library = SyncLibrary {
        playlists: vec![SyncPlaylist {
            id: "acceptance".into(),
            name: "CueRithm acceptance".into(),
            track_ids: tracks.iter().map(|t| t.id.clone()).collect(),
            ..SyncPlaylist::default()
        }],
        // The same decoder-alignment shift prepare_sync applies before writing.
        tracks: timing::export_library(
            &SyncLibrary {
                tracks,
                playlists: vec![],
            },
            DjSoftware::Mixxx,
        )
        .unwrap()
        .tracks,
    };
    let options = SyncOptions {
        beatgrid_lock: BeatgridLock::Lock,
        mixxx: MixxxOptions {
            existing_tracks: mode,
            container: MixxxContainer::Playlists,
            ..MixxxOptions::default()
        },
        ..SyncOptions::default()
    };
    let database = folder.join("settings").join(DATABASE_FILE);
    let staged = write_staged(
        &database,
        &database,
        &library,
        &options,
        &PlaylistOwnershipRequest::default(),
        Host::current(),
    )
    .unwrap();
    for warning in &staged.warnings {
        println!("WARNING {warning}");
    }
    println!("CHANGED {}", staged.changed);
}
