//! CueRithm hot cues and saved loops as Mixxx `cues` rows (`CueDAO` contract).
use crate::SyncTrack;
use std::collections::BTreeSet;

/// `cues.type` values this adapter writes and replaces.
pub(super) const HOT_CUE: i64 = 1;
pub(super) const LOOP: i64 = 4;
/// Mixxx offers hot cue slots `0..36`.
const MAX_SLOT: u8 = 35;

/// Mixxx's default hot cue palette; a cue without a usable colour takes its slot's.
pub(super) const HOTCUE_PALETTE: [u32; 8] = [
    0xC50A08, 0x32BE44, 0x42D4F4, 0xF8D200, 0x0044FF, 0xAF00CC, 0xFCA6D7, 0xF2F2FF,
];

pub(super) fn slot_color(index: u8) -> u32 {
    HOTCUE_PALETTE[usize::from(index) % HOTCUE_PALETTE.len()]
}

/// One `cues` row. `position` and `length` are Mixxx engine sample positions:
/// frames times two output channels, stored as REAL.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct CueRow {
    pub kind: i64,
    pub position: f64,
    pub length: f64,
    pub hotcue: i64,
    pub label: String,
    pub color: i64,
}

fn describe(track: &SyncTrack) -> String {
    if track.title.is_empty() {
        track.id.clone()
    } else {
        track.title.clone()
    }
}

/// Rows for every usable cue, ordered by slot. A cue without `end_seconds` is a hot
/// cue, one with it a saved loop in that slot. Skipped cues and moved starts are
/// reported through `warnings`; Mixxx's loader would discard zero-length loops,
/// black hot cues at the start and repeated slots, so none of those are written.
pub(super) fn plan_cues(track: &SyncTrack, warnings: &mut Vec<String>) -> Vec<CueRow> {
    if track.cues.is_empty() {
        return Vec::new();
    }
    let name = describe(track);
    if track.sample_rate == 0 {
        warnings.push(format!(
            "Cues of '{name}' were skipped: the track has no sample rate"
        ));
        return Vec::new();
    }
    let rate = f64::from(track.sample_rate);
    let mut rows = Vec::new();
    let mut seen = BTreeSet::new();
    let mut moved = 0usize;
    for cue in &track.cues {
        let slot = cue.index;
        let skip = |reason: &str, warnings: &mut Vec<String>| {
            warnings.push(format!("Cue {slot} of '{name}' was skipped: {reason}"));
        };
        if slot > MAX_SLOT {
            skip("Mixxx has hot cue slots 0 to 35", warnings);
            continue;
        }
        if !seen.insert(slot) {
            skip("another cue already uses this slot", warnings);
            continue;
        }
        if !cue.start_seconds.is_finite() || cue.end_seconds.is_some_and(|end| !end.is_finite()) {
            skip("its time is not a number", warnings);
            continue;
        }
        let mut start = (cue.start_seconds * rate).round();
        if start < 0.0 {
            start = 0.0;
            moved += 1;
        }
        let length = match cue.end_seconds {
            None => 0.0,
            Some(end) => {
                let end = (end * rate).round();
                if end <= 0.0 {
                    skip("the loop ends before the first sample", warnings);
                    continue;
                }
                let length = end - start;
                if length < 1.0 {
                    skip("the loop is shorter than one frame", warnings);
                    continue;
                }
                length
            }
        };
        let color = cue
            .color
            .map(|rgb| rgb & 0x00FF_FFFF)
            .filter(|rgb| *rgb != 0)
            .unwrap_or_else(|| slot_color(slot));
        rows.push(CueRow {
            kind: if cue.end_seconds.is_some() {
                LOOP
            } else {
                HOT_CUE
            },
            position: start * 2.0,
            length: length * 2.0,
            hotcue: i64::from(slot),
            label: cue.name.clone(),
            color: i64::from(color),
        });
    }
    if moved > 0 {
        warnings.push(format!(
            "{moved} cue(s) of '{name}' start before Mixxx's first sample and were moved to the start"
        ));
    }
    rows.sort_by_key(|row| row.hotcue);
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SyncCue;

    fn cue(index: u8, start: f64, end: Option<f64>, color: Option<u32>) -> SyncCue {
        SyncCue {
            index,
            name: format!("cue {index}"),
            start_seconds: start,
            end_seconds: end,
            color,
            ..SyncCue::default()
        }
    }
    fn track(cues: Vec<SyncCue>) -> SyncTrack {
        SyncTrack {
            id: "t1".into(),
            title: "Song".into(),
            sample_rate: 44_100,
            cues,
            ..SyncTrack::default()
        }
    }
    fn plan(cues: Vec<SyncCue>) -> (Vec<CueRow>, Vec<String>) {
        let mut warnings = Vec::new();
        (plan_cues(&track(cues), &mut warnings), warnings)
    }

    #[test]
    fn hot_cue_position_is_frames_times_two() {
        let (rows, warnings) = plan(vec![cue(0, 1.5, None, Some(0x123456))]);
        assert!(warnings.is_empty());
        assert_eq!(
            rows,
            vec![CueRow {
                kind: HOT_CUE,
                position: 132_300.0, // 1.5 s * 44100 frames * 2 channels
                length: 0.0,
                hotcue: 0,
                label: "cue 0".into(),
                color: 0x123456,
            }]
        );
    }

    #[test]
    fn loop_has_length_and_hotcue_slot() {
        let (rows, _) = plan(vec![cue(3, 2.0, Some(4.0), None)]);
        assert_eq!(rows[0].kind, LOOP);
        assert_eq!(rows[0].position, 176_400.0);
        assert_eq!(rows[0].length, 176_400.0);
        assert_eq!(rows[0].hotcue, 3);
    }

    #[test]
    fn missing_or_black_color_takes_slot_palette() {
        let (rows, _) = plan(vec![
            cue(0, 1.0, None, None),
            cue(9, 2.0, None, Some(0)),
            cue(2, 3.0, None, Some(0xFF000000)), // alpha only is still black
        ]);
        let by_slot = |slot: i64| rows.iter().find(|r| r.hotcue == slot).unwrap().color;
        assert_eq!(by_slot(0), 0xC50A08);
        assert_eq!(
            by_slot(9),
            0x32BE44,
            "slot 9 wraps to the second palette colour"
        );
        assert_eq!(by_slot(2), 0x42D4F4);
    }

    #[test]
    fn exact_rgb_is_kept_whatever_the_palette_is() {
        let (rows, _) = plan(vec![cue(0, 1.0, None, Some(0x010203))]);
        assert_eq!(rows[0].color, 0x010203);
        assert!(HOTCUE_PALETTE.iter().all(|c| *c != 0x010203));
    }

    #[test]
    fn slot_36_is_skipped_with_warning() {
        let (rows, warnings) = plan(vec![cue(35, 1.0, None, None), cue(36, 2.0, None, None)]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].hotcue, 35);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("Cue 36") && warnings[0].contains("Song"));
    }

    #[test]
    fn duplicate_slot_keeps_first() {
        let (rows, warnings) = plan(vec![cue(1, 1.0, None, None), cue(1, 9.0, None, None)]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].position, 88_200.0);
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn zero_length_loop_is_skipped() {
        let (rows, warnings) = plan(vec![cue(0, 1.0, Some(1.0), None)]);
        assert!(rows.is_empty());
        assert!(warnings[0].contains("shorter than one frame"));
        // Half a frame rounds to the same frame.
        let (rows, _) = plan(vec![cue(0, 1.0, Some(1.0 + 0.4 / 44_100.0), None)]);
        assert!(rows.is_empty());
    }

    #[test]
    fn start_before_zero_is_moved_to_the_first_frame_with_one_warning() {
        let (rows, warnings) = plan(vec![
            cue(0, -0.02, None, None),
            cue(1, -0.01, Some(0.5), None),
        ]);
        assert_eq!(rows[0].position, 0.0);
        assert_eq!(rows[1].position, 0.0);
        assert_eq!(
            rows[1].length, 44_100.0,
            "the loop now runs from the first frame to 0.5 s"
        );
        assert_eq!(warnings.len(), 1, "one summary per track: {warnings:?}");
        assert!(warnings[0].contains("2 cue(s)"));
    }

    #[test]
    fn loop_ending_before_zero_is_skipped() {
        let (rows, warnings) = plan(vec![cue(0, -0.05, Some(-0.01), None)]);
        assert!(rows.is_empty());
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("ends before the first sample"))
        );
    }

    #[test]
    fn non_finite_times_and_missing_sample_rate_are_skipped() {
        let (rows, warnings) = plan(vec![
            cue(0, f64::NAN, None, None),
            cue(1, 1.0, Some(f64::INFINITY), None),
        ]);
        assert!(rows.is_empty());
        assert_eq!(warnings.len(), 2);
        let mut silent = track(vec![cue(0, 1.0, None, None)]);
        silent.sample_rate = 0;
        let mut warnings = Vec::new();
        assert!(plan_cues(&silent, &mut warnings).is_empty());
        assert!(warnings[0].contains("no sample rate"));
    }

    #[test]
    fn rows_pass_the_loader_discard_rules_and_sort_by_slot() {
        let (rows, _) = plan(vec![
            cue(5, 3.0, Some(5.0), None),
            cue(0, 0.0, None, Some(0)),
            cue(2, 1.0, None, None),
        ]);
        assert_eq!(rows.iter().map(|r| r.hotcue).collect::<Vec<_>>(), [0, 2, 5]);
        for row in &rows {
            assert!(row.kind != LOOP || row.length > 0.0, "no zero-length loops");
            assert!(
                !(row.kind == HOT_CUE && row.position == 0.0 && row.color == 0),
                "no black hot cue at position 0"
            );
        }
    }
}
