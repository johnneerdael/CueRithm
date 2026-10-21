use crate::SyncLibrary;
use anyhow::{Context, Result, ensure};
use std::collections::BTreeSet;

/// Validate the portable export before any target is opened for writing.
pub fn validate_library(library: &SyncLibrary) -> Result<()> {
    ensure!(
        library.tracks.len() <= 1_000_000 && library.playlists.len() <= 100_000,
        "Sync library exceeds limits"
    );
    let mut ids = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut handles = std::collections::HashSet::new();
    for track in &library.tracks {
        ensure!(
            track.native_timeline.is_none(),
            "Sync input must retain canonical prepared timing"
        );
        ensure!(
            !track.id.is_empty() && ids.insert(track.id.as_str()),
            "Duplicate or missing sync track identity"
        );
        ensure!(
            !track.path.components().any(|part| matches!(
                part,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )),
            "Sync audio paths must be normalized"
        );
        if track.path.exists() {
            ensure!(
                handles.insert(same_file::Handle::from_path(&track.path)?),
                "Multiple sync tracks refer to the same audio file"
            );
        }

        ensure!(
            track.path.is_absolute()
                && !track
                    .path
                    .as_os_str()
                    .as_encoded_bytes()
                    .iter()
                    .any(u8::is_ascii_control)
                && track
                    .path
                    .to_str()
                    .is_none_or(|path| !path.chars().any(char::is_control))
                && paths.insert(&track.path),
            "Duplicate or invalid sync audio path"
        );
        ensure!(
            track.bpm.is_finite() && (0. ..=1000.).contains(&track.bpm),
            "Invalid sync BPM"
        );
        ensure!(
            track.duration_seconds.is_finite() && (0. ..=604800.).contains(&track.duration_seconds),
            "Invalid sync duration"
        );
        ensure!(track.rating <= 5, "Rating exceeds five stars");
        ensure!(
            track.cues.len() <= 256 && track.beatgrid.len() <= 1_000_000,
            "Track cue/grid limit exceeded"
        );
        let mut slots = BTreeSet::new();
        for cue in &track.cues {
            ensure!(
                !cue.active_loop || cue.end_seconds.is_some(),
                "Active loop requires a loop end time"
            );
            ensure!(slots.insert(cue.index), "Duplicate sync cue slot");
            ensure!(
                cue.start_seconds.is_finite()
                    && cue.start_seconds >= 0.
                    && cue.start_seconds <= track.duration_seconds,
                "Cue falls outside the audio timeline"
            );
            ensure!(
                cue.end_seconds.is_none_or(|end| end.is_finite()
                    && end > cue.start_seconds
                    && end <= track.duration_seconds),
                "Invalid loop end time"
            );
        }
        let mut previous = None;
        for beat in &track.beatgrid {
            ensure!(
                beat.seconds.is_finite()
                    && beat.seconds >= 0.
                    && beat.seconds <= track.duration_seconds
                    && previous.is_none_or(|p| p < beat.seconds),
                "Invalid or unordered grid anchors"
            );
            ensure!(
                beat.bpm.is_finite() && beat.bpm > 0. && beat.bpm <= 1000.,
                "Invalid grid tempo"
            );
            previous = Some(beat.seconds);
        }
        for value in [
            &track.title,
            &track.artist,
            &track.album,
            &track.genre,
            &track.label,
            &track.comment,
            &track.key,
        ] {
            ensure!(
                value.len() <= 1_000_000
                    && !value
                        .chars()
                        .any(|c| c < ' ' && !matches!(c, '\n' | '\r' | '\t')),
                "Invalid sync metadata text"
            );
        }
    }
    let mut playlist_ids = BTreeSet::new();
    for playlist in &library.playlists {
        ensure!(
            !playlist.id.is_empty() && playlist_ids.insert(playlist.id.as_str()),
            "Duplicate or missing sync playlist identity"
        );
        ensure!(
            !playlist.name.trim().is_empty() && playlist.name.len() <= 4096,
            "Invalid sync playlist name"
        );
        ensure!(
            playlist
                .track_ids
                .iter()
                .all(|id| ids.contains(id.as_str())),
            "Playlist references a missing sync track"
        );
        ensure!(
            !playlist.folder || playlist.track_ids.is_empty(),
            "Folder membership must come from its child playlists"
        );
    }
    for playlist in &library.playlists {
        let mut seen = BTreeSet::from([playlist.id.as_str()]);
        let mut parent = playlist.parent_id.as_deref();
        while let Some(id) = parent {
            ensure!(seen.insert(id), "Sync playlist hierarchy contains a cycle");
            let node = library
                .playlists
                .iter()
                .find(|p| p.id == id)
                .context("Sync playlist parent is missing")?;
            ensure!(node.folder, "Sync playlist parent is not a folder");
            parent = node.parent_id.as_deref();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SyncCue, SyncPlaylist, SyncTrack};
    #[test]
    fn active_loop_requires_a_loop_end_before_native_writes() {
        let mut library = SyncLibrary {
            tracks: vec![SyncTrack {
                native_path: None,
                id: "track".into(),
                path: std::env::temp_dir().join("synthetic-active.wav"),
                duration_seconds: 10.,
                cues: vec![SyncCue {
                    start_seconds: 1.,
                    active_loop: true,
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(
            validate_library(&library)
                .unwrap_err()
                .to_string()
                .contains("Active loop requires")
        );
        library.tracks[0].cues[0].end_seconds = Some(2.);
        validate_library(&library).unwrap();
        library.tracks[0].cues[0].end_seconds = None;
        library.tracks[0].cues[0].active_loop = false;
        validate_library(&library).unwrap();
    }
    #[test]
    fn rejects_two_identities_for_one_hardlinked_file_and_parent_aliases() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let original = root.join("one.wav");
        let alias = root.join("two.wav");
        std::fs::write(&original, b"same audio").unwrap();
        std::fs::hard_link(&original, &alias).unwrap();
        let mut library = SyncLibrary {
            tracks: vec![
                SyncTrack {
                    native_path: None,
                    id: "one".into(),
                    path: original,
                    ..Default::default()
                },
                SyncTrack {
                    native_path: None,
                    id: "two".into(),
                    path: alias,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert!(validate_library(&library).is_err());
        library.tracks.pop();
        // PathBuf::join normalizes `..` when the Windows root is verbatim.
        // Append raw OS text so the guard receives an actual parent component.
        let mut unnormalized = root.into_os_string();
        let separator = std::path::MAIN_SEPARATOR;
        unnormalized.push(format!("{separator}nested{separator}..{separator}one.wav"));
        library.tracks[0].path = unnormalized.into();
        assert!(validate_library(&library).is_err());
    }
    #[test]
    fn rejects_ambiguous_tracks_bad_timing_and_cycles_before_target_writes() {
        let track = SyncTrack {
            native_path: None,
            id: "track".into(),
            path: std::env::temp_dir().join("track.wav"),
            duration_seconds: 60.,
            bpm: 120.,
            ..Default::default()
        };
        let mut library = SyncLibrary {
            tracks: vec![track.clone(), track],
            ..Default::default()
        };
        assert!(validate_library(&library).is_err());
        library.tracks.pop();
        library.tracks[0].cues.push(SyncCue {
            start_seconds: f64::NAN,
            ..Default::default()
        });
        assert!(validate_library(&library).is_err());
        library.tracks[0].cues.clear();
        library.playlists.push(SyncPlaylist {
            id: "folder".into(),
            parent_id: Some("folder".into()),
            name: "Cycle".into(),
            folder: true,
            ..Default::default()
        });
        assert!(validate_library(&library).is_err());
        library.playlists[0].parent_id = None;
        validate_library(&library).unwrap();
    }
}
