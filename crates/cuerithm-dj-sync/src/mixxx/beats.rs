//! CueRithm's sparse tempo anchors as Mixxx's `beats` blob.
use super::proto;
use crate::SyncTrack;
use anyhow::{Result, ensure};

pub(super) const BEAT_GRID: &str = "BeatGrid-2.0";
pub(super) const BEAT_MAP: &str = "BeatMap-1.0";

/// What to store in `library.beats`, `beats_version` and `bpm`.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct BeatPlan {
    pub version: &'static str,
    pub blob: Vec<u8>,
    pub bpm: f64,
}

/// Anchors this close to the first anchor's grid count as the same constant tempo.
const GRID_TOLERANCE_SECONDS: f64 = 0.0005;
const SAME_BPM: f64 = 1e-6;

fn frame(seconds: f64, sample_rate: f64) -> Result<i32> {
    let frames = (seconds * sample_rate).round();
    ensure!(
        frames.is_finite() && frames.abs() <= f64::from(i32::MAX),
        "Beat position {seconds} s is outside what Mixxx stores"
    );
    Ok(frames as i32)
}

/// `None` when the track has no grid. A constant grid is `BeatGrid-2.0` anchored at
/// the first anchor; any tempo or phase change is a dense `BeatMap-1.0` that keeps
/// every anchor's frame exactly. Frames are at the file's own sample rate.
pub(super) fn plan_beats(track: &SyncTrack) -> Result<Option<BeatPlan>> {
    let anchors = &track.beatgrid;
    let Some(first) = anchors.first() else {
        return Ok(None);
    };
    ensure!(
        track.sample_rate > 0,
        "Track has no sample rate for its beat grid"
    );
    let rate = f64::from(track.sample_rate);
    for (index, anchor) in anchors.iter().enumerate() {
        ensure!(
            anchor.seconds.is_finite() && anchor.bpm.is_finite() && anchor.bpm > 0.0,
            "Beat grid anchor {index} has an invalid time or tempo"
        );
        if index > 0 {
            ensure!(
                anchor.seconds > anchors[index - 1].seconds,
                "Beat grid anchors must be in increasing time order"
            );
        }
    }
    let beat_length = 60.0 / first.bpm;
    let constant = anchors.iter().skip(1).all(|anchor| {
        let beats_away = (anchor.seconds - first.seconds) / beat_length;
        (anchor.bpm - first.bpm).abs() <= SAME_BPM
            && (beats_away - beats_away.round()).abs() * beat_length <= GRID_TOLERANCE_SECONDS
    });
    if constant {
        return Ok(Some(BeatPlan {
            version: BEAT_GRID,
            blob: proto::beat_grid(first.bpm, frame(first.seconds, rate)?),
            bpm: first.bpm,
        }));
    }

    let mut frames: Vec<i32> = Vec::new();
    for (index, anchor) in anchors.iter().enumerate() {
        let step = 60.0 / anchor.bpm;
        let end = anchors.get(index + 1).map_or(
            track.duration_seconds.max(anchor.seconds + 2.0 * step),
            |next| next.seconds,
        );
        let anchor_frame = frame(anchor.seconds, rate)?;
        ensure!(
            frames.last().is_none_or(|last| anchor_frame > *last),
            "Beat grid anchors collapse onto the same Mixxx frame"
        );
        frames.push(anchor_frame);
        for k in 1_u32.. {
            let seconds = anchor.seconds + f64::from(k) * step;
            if seconds >= end {
                break;
            }
            let next = frame(seconds, rate)?;
            // A beat that would land on or before the previous frame, or on the
            // next anchor's own frame, adds nothing.
            let next_anchor_frame = anchors.get(index + 1).map(|n| frame(n.seconds, rate));
            if frames.last().is_some_and(|last| next <= *last)
                || next_anchor_frame.is_some_and(|n| n.is_ok_and(|n| next >= n))
            {
                continue;
            }
            frames.push(next);
        }
    }
    ensure!(frames.len() >= 2, "Beat grid has fewer than two beats");
    let span = f64::from(frames[frames.len() - 1] - frames[0]) / rate;
    ensure!(span > 0.0, "Beat grid spans no time");
    Ok(Some(BeatPlan {
        version: BEAT_MAP,
        blob: proto::beat_map(&frames),
        bpm: (frames.len() - 1) as f64 * 60.0 / span,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SyncBeat;

    fn track(rate: u32, duration: f64, anchors: &[(f64, f64)]) -> SyncTrack {
        SyncTrack {
            sample_rate: rate,
            duration_seconds: duration,
            beatgrid: anchors
                .iter()
                .enumerate()
                .map(|(index, (seconds, bpm))| SyncBeat {
                    seconds: *seconds,
                    bpm: *bpm,
                    beat: index as u32,
                })
                .collect(),
            ..SyncTrack::default()
        }
    }

    #[test]
    fn constant_grid_uses_beatgrid_2_at_first_anchor_in_source_frames() {
        let plan = plan_beats(&track(48_000, 200.0, &[(0.5, 128.0)]))
            .unwrap()
            .unwrap();
        assert_eq!(plan.version, "BeatGrid-2.0");
        assert_eq!(
            proto::decode::beat_grid(&plan.blob).unwrap(),
            (128.0, 24_000)
        );
        assert_eq!(plan.bpm, 128.0);
    }

    #[test]
    fn repeated_equal_bpm_anchors_on_grid_stay_constant() {
        // Eight and sixty-four beats after the first anchor at 128 BPM.
        let beat = 60.0 / 128.0;
        let plan = plan_beats(&track(
            44_100,
            200.0,
            &[
                (0.5, 128.0),
                (0.5 + 8.0 * beat, 128.0),
                (0.5 + 64.0 * beat, 128.0),
            ],
        ))
        .unwrap()
        .unwrap();
        assert_eq!(plan.version, BEAT_GRID);
        assert_eq!(
            proto::decode::beat_grid(&plan.blob).unwrap(),
            (128.0, 22_050)
        );
    }

    #[test]
    fn tempo_change_builds_beatmap_with_every_anchor() {
        let plan = plan_beats(&track(44_100, 20.0, &[(0.5, 120.0), (10.5, 128.0)]))
            .unwrap()
            .unwrap();
        assert_eq!(plan.version, BEAT_MAP);
        let frames = proto::decode::beat_map(&plan.blob).unwrap();
        assert!(
            frames.windows(2).all(|pair| pair[0] < pair[1]),
            "strictly increasing"
        );
        assert!(frames.contains(&22_050), "first anchor kept exactly");
        assert!(frames.contains(&463_050), "second anchor kept exactly");
        // 20 beats of 0.5 s from 0.5 s up to 10.5 s, then 21 beats of 0.46875 s.
        assert_eq!(frames.len(), 41);
        assert!((plan.bpm - 123.87).abs() < 0.05, "{}", plan.bpm);
    }

    #[test]
    fn phase_jump_at_equal_bpm_builds_beatmap() {
        // 50 ms off the first anchor's grid is far beyond the 0.5 ms tolerance.
        let plan = plan_beats(&track(44_100, 30.0, &[(0.5, 120.0), (10.55, 120.0)]))
            .unwrap()
            .unwrap();
        assert_eq!(plan.version, BEAT_MAP);
        let frames = proto::decode::beat_map(&plan.blob).unwrap();
        assert!(frames.contains(&22_050) && frames.contains(&465_255));
    }

    #[test]
    fn empty_grid_is_none() {
        assert_eq!(plan_beats(&track(44_100, 10.0, &[])).unwrap(), None);
    }

    #[test]
    fn invalid_input_is_rejected() {
        for bpm in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(
                plan_beats(&track(44_100, 10.0, &[(0.5, bpm)])).is_err(),
                "{bpm}"
            );
        }
        assert!(plan_beats(&track(44_100, 10.0, &[(f64::NAN, 120.0)])).is_err());
        assert!(plan_beats(&track(0, 10.0, &[(0.5, 120.0)])).is_err());
        assert!(
            plan_beats(&track(44_100, 10.0, &[(5.0, 120.0), (1.0, 120.0)])).is_err(),
            "anchors out of order"
        );
        assert!(
            plan_beats(&track(44_100, 10.0, &[(1.0, 120.0), (1.00001, 130.0)])).is_err(),
            "two anchors on one frame"
        );
        assert!(
            plan_beats(&track(44_100, 1e9, &[(1e9, 120.0)])).is_err(),
            "beyond i32 frames"
        );
    }

    #[test]
    fn grid_before_the_first_sample_is_kept_as_a_negative_frame() {
        let plan = plan_beats(&track(44_100, 10.0, &[(-0.01, 120.0)]))
            .unwrap()
            .unwrap();
        assert_eq!(proto::decode::beat_grid(&plan.blob).unwrap().1, -441);
    }
}
