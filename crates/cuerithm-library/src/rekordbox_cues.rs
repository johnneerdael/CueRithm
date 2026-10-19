//! Translate Cuerithm's published cues into rekordbox cue rows, following the reference exporter's
//! slot, colour and loop rules.
use camelot_core::lexicon_publication::{PublishedCue, PublishedType};
use cuerithm_rekordbox::CueRow;
use serde::{Deserialize, Serialize};

/// Where exported cues go in rekordbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CueDestination {
    /// Memory cues stay memory cues; the rest become hot cues in their saved slots.
    Default,
    HotCue,
    MemoryCue,
    /// Every cue becomes both a hot cue and a memory cue.
    Both,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CueExportOptions {
    pub destination: CueDestination,
    /// Map colours rekordbox has no entry for to the nearest one it has.
    pub nearest_colors: bool,
}
impl Default for CueExportOptions {
    fn default() -> Self {
        Self {
            destination: CueDestination::Default,
            nearest_colors: true,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CueExport {
    pub rows: Vec<CueRow>,
    /// Cues rekordbox cannot hold: more than eight hot cues, more than ten memory cues, or a slot below -1.
    pub skipped: usize,
}

/// Hot cue A-H are stored as kinds 1, 2, 3, 5, 6, 7, 8 and 9; kind 4 is the load/loop point.
const HOT_KINDS: [i64; 8] = [1, 2, 3, 5, 6, 7, 8, 9];
const MAX_MEMORY_CUES: usize = 10;

fn hot_table_index(color: &str, nearest: bool) -> Option<i64> {
    let exact = match color {
        "blue" => 1,
        "blue_light" => 6,
        "aqua" => 10,
        "aqua_dark" => 15,
        "teal" => 18,
        "green" => 21,
        "lime" => 25,
        "yellow" => 30,
        "yellow_dark" => 33,
        "orange" => 36,
        "red" => 41,
        "magenta_red" => 46,
        "magenta" => 52,
        "violet" => 56,
        "violet_light" => 60,
        "blue_violet" => 62,
        "black" => 63,
        "white" => 64,
        _ => {
            return nearest
                .then(|| match color {
                    "blue_dark" => Some(1),
                    "magenta_dark" => Some(52),
                    "red_dark" | "red_light" | "red_orange" => Some(41),
                    "beige" => Some(36),
                    "green_light" | "green_dark" => Some(21),
                    "grey_light" => Some(64),
                    "grey_dark" => Some(63),
                    _ => None,
                })
                .flatten();
        }
    };
    Some(exact)
}
fn memory_color(color: &str, nearest: bool) -> Option<i64> {
    let exact = match color {
        "magenta" => 0,
        "red" => 1,
        "orange" => 2,
        "yellow" => 3,
        "green" => 4,
        "aqua" => 5,
        "blue" => 6,
        "violet" => 7,
        _ => {
            return nearest
                .then(|| match color {
                    "magenta_dark" | "magenta_red" => Some(0),
                    "red_dark" | "red_light" | "red_orange" => Some(1),
                    "beige" => Some(2),
                    "yellow_dark" => Some(3),
                    "lime" | "green_light" | "green_dark" | "teal" => Some(4),
                    "aqua_dark" | "blue_light" | "blue_dark" | "blue_violet" => Some(5),
                    "violet_light" => Some(7),
                    _ => None,
                })
                .flatten();
        }
    };
    Some(exact)
}
/// `(Color, ColorTableIndex)` for a cue; hot cues use the colour table, memory cues the eight-colour palette.
fn colors(cue: &PublishedCue, nearest: bool, hot: bool) -> (Option<i64>, Option<i64>) {
    let name = cue.color.as_str().filter(|c| !c.is_empty());
    let (mut color, mut index) = (Some(-1), Some(0));
    if cue.kind == PublishedType::Loop && name.is_none() {
        return (Some(255), Some(0));
    }
    if let Some(name) = name {
        if hot {
            if let Some(found) = hot_table_index(name, nearest) {
                index = Some(found);
            }
        } else {
            index = None;
            color = memory_color(name, nearest).or(Some(-1));
        }
    }
    (color, index)
}
fn original_number(cue: &PublishedCue) -> Option<i64> {
    cue.data
        .as_ref()
        .and_then(|d| d["rekordbox"]["originalNum"].as_i64())
}

/// Rows for one track. `bpm` is the track's saved tempo, used for loop lengths in beats.
pub fn rekordbox_cue_rows(cues: &[PublishedCue], bpm: f64, options: CueExportOptions) -> CueExport {
    let mut skipped = 0;
    // Stored slot for every cue; None for a cue that cannot be exported.
    let mut slots: Vec<Option<i64>> = vec![None; cues.len()];
    let mut highest = cues.iter().filter_map(original_number).max().unwrap_or(-1);
    for (index, cue) in cues.iter().enumerate() {
        let original = original_number(cue);
        let mut slot = match options.destination {
            CueDestination::Default => match original {
                Some(number) => number,
                None => {
                    if cues
                        .iter()
                        .any(|c| original_number(c) == Some(i64::from(cue.position)))
                    {
                        highest += 1;
                        highest
                    } else {
                        i64::from(cue.position)
                    }
                }
            },
            _ => i64::from(cue.position),
        };
        if slot < -1 {
            skipped += 1;
            continue;
        }
        slot = if options.destination == CueDestination::MemoryCue || slot == -1 {
            0
        } else {
            slot + if slot <= 2 { 1 } else { 2 }
        };
        slots[index] = Some(slot);
    }
    // Slots beyond the eight hot cues move to a free hot slot, or are dropped when none is left.
    let mut used: Vec<i64> = slots
        .iter()
        .flatten()
        .copied()
        .filter(|s| HOT_KINDS.contains(s))
        .collect();
    for slot in slots.iter_mut() {
        let Some(value) = *slot else { continue };
        if value == 0 || value == 4 || HOT_KINDS.contains(&value) {
            continue;
        }
        match HOT_KINDS.iter().find(|k| !used.contains(k)) {
            Some(free) => {
                used.push(*free);
                *slot = Some(*free);
            }
            None => {
                *slot = None;
                skipped += 1;
            }
        }
    }
    let mut rows = vec![];
    let mut load_point_taken = false;
    let mut hot_active_loop_taken = false;
    let mut memory_cues = 0;
    for (index, cue) in cues.iter().enumerate() {
        let Some(mut slot) = slots[index] else {
            continue;
        };
        let is_active_loop = cue.kind == PublishedType::Loop && cue.active_loop == Some(true);
        if is_active_loop
            && slot == 0
            && !load_point_taken
            && options.destination != CueDestination::Both
        {
            slot = 4;
            load_point_taken = true;
        }
        let hot = slot != 0 && slot != 4;
        let (color, color_table_index) = colors(cue, options.nearest_colors, hot);
        if !hot && memory_cues >= MAX_MEMORY_CUES {
            skipped += 1;
            continue;
        }
        let in_msec = (cue.start_time * 1000.).floor().max(0.) as i64;
        let mut out_msec = -1;
        let mut active = false;
        let mut beat_loop_size = None;
        if cue.kind == PublishedType::Loop {
            let end = cue.end_time.unwrap_or(cue.start_time);
            out_msec = (end * 1000.).floor() as i64;
            if cue.start_time < 0. {
                out_msec -= (cue.start_time * 1000.).floor() as i64;
            }
            if !hot_active_loop_taken {
                active = is_active_loop;
            }
            if hot && active {
                hot_active_loop_taken = true;
            }
            if bpm > 0. {
                let beats = ((end - cue.start_time) / (60. / bpm)).round().max(1.) as i64;
                beat_loop_size = Some(65536 * beats + 1);
            }
        }
        if slot == 4 {
            active = false;
        }
        let row = CueRow {
            in_msec,
            out_msec,
            kind: slot,
            active_loop: active,
            beat_loop_size,
            color,
            color_table_index,
            comment: Some(cue.name.clone()).filter(|n| !n.is_empty()),
        };
        if !hot {
            memory_cues += 1;
        }
        rows.push(row.clone());
        if options.destination == CueDestination::Both {
            let (color, index) = colors(cue, options.nearest_colors, false);
            let mut memory = CueRow {
                kind: 0,
                color,
                color_table_index: index,
                ..row
            };
            if is_active_loop && !load_point_taken {
                memory.kind = 4;
                memory.active_loop = false;
                load_point_taken = true;
            }
            rows.push(memory);
        } else if is_active_loop && hot && !load_point_taken {
            // A hot active loop also leaves the load/loop point that rekordbox expects.
            let (color, index) = colors(cue, options.nearest_colors, false);
            rows.push(CueRow {
                kind: 4,
                active_loop: false,
                color,
                color_table_index: index,
                ..row
            });
            load_point_taken = true;
        }
    }
    CueExport { rows, skipped }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn cue(name: &str, start: f64, position: u32, color: &str) -> PublishedCue {
        PublishedCue {
            name: name.into(),
            start_time: start,
            kind: PublishedType::Cue,
            position,
            color: json!(color),
            end_time: None,
            active_loop: None,
            data: None,
        }
    }
    fn memory(name: &str, start: f64, color: &str) -> PublishedCue {
        PublishedCue {
            data: Some(json!({"rekordbox": {"originalNum": -1}})),
            ..cue(name, start, 0, color)
        }
    }
    fn active_loop(name: &str, start: f64, end: f64, position: u32) -> PublishedCue {
        PublishedCue {
            kind: PublishedType::Loop,
            end_time: Some(end),
            active_loop: Some(true),
            ..cue(name, start, position, "magenta")
        }
    }

    #[test]
    fn the_factory_cues_land_in_hot_slots_with_table_colors() {
        let factory = [
            ("Start", "yellow"),
            ("Drop", "red"),
            ("Breakdown", "blue"),
            ("Drop #2", "red"),
            ("Breakdown #2", "blue"),
            ("End / Fade-out", "violet"),
        ];
        let cues: Vec<_> = factory
            .iter()
            .enumerate()
            .map(|(i, (name, color))| cue(name, 10. + i as f64 * 30.25, i as u32, color))
            .collect();
        let export = rekordbox_cue_rows(&cues, 128., Default::default());
        assert_eq!(export.skipped, 0);
        // Slots A, B, C, D, E, F are stored as 1, 2, 3, 5, 6, 7 because 4 is the load/loop point.
        assert_eq!(
            export.rows.iter().map(|r| r.kind).collect::<Vec<_>>(),
            [1, 2, 3, 5, 6, 7]
        );
        assert_eq!(
            export
                .rows
                .iter()
                .map(|r| r.color_table_index)
                .collect::<Vec<_>>(),
            [Some(30), Some(41), Some(1), Some(41), Some(1), Some(56)]
        );
        assert!(
            export
                .rows
                .iter()
                .all(|r| r.color == Some(-1) && r.out_msec == -1 && !r.active_loop)
        );
        assert_eq!(export.rows[1].in_msec, 40_250);
        assert_eq!(export.rows[1].comment.as_deref(), Some("Drop"));
    }

    #[test]
    fn memory_cues_use_the_eight_color_palette_and_are_capped_at_ten() {
        let mut cues = vec![memory("Mem", 5., "red"), memory("Dark", 6., "blue_dark")];
        let nearest = rekordbox_cue_rows(&cues, 128., Default::default());
        assert_eq!(
            (
                nearest.rows[0].kind,
                nearest.rows[0].color,
                nearest.rows[0].color_table_index
            ),
            (0, Some(1), None)
        );
        assert_eq!(nearest.rows[1].color, Some(5));
        let exact = rekordbox_cue_rows(
            &cues,
            128.,
            CueExportOptions {
                nearest_colors: false,
                ..Default::default()
            },
        );
        assert_eq!(exact.rows[1].color, Some(-1));
        cues = (0..12).map(|i| memory("m", i as f64, "green")).collect();
        let capped = rekordbox_cue_rows(&cues, 128., Default::default());
        assert_eq!((capped.rows.len(), capped.skipped), (10, 2));
    }

    #[test]
    fn a_hot_active_loop_keeps_its_loop_and_adds_one_load_point() {
        let cues = [
            cue("Start", 1., 0, "yellow"),
            active_loop("S.O.S.", 60., 68., 6),
        ];
        let export = rekordbox_cue_rows(&cues, 120., Default::default());
        let hot = &export.rows[1];
        // Slot 6 is stored as 8; 8 seconds at 120 BPM is 16 beats.
        assert_eq!(
            (hot.kind, hot.active_loop, hot.in_msec, hot.out_msec),
            (8, true, 60_000, 68_000)
        );
        assert_eq!(hot.beat_loop_size, Some(65536 * 16 + 1));
        assert_eq!(hot.color_table_index, Some(52));
        let load = &export.rows[2];
        assert_eq!(
            (load.kind, load.active_loop, load.out_msec),
            (4, false, 68_000)
        );
        assert_eq!((load.color, load.color_table_index), (Some(0), None));
        assert_eq!(export.rows.len(), 3);
    }

    #[test]
    fn a_memory_active_loop_becomes_the_load_point_once_and_loops_without_color_get_the_default() {
        let mut first = active_loop("A", 10., 12., 0);
        first.data = Some(json!({"rekordbox": {"originalNum": -1}}));
        let mut second = first.clone();
        second.start_time = 20.;
        second.end_time = Some(22.);
        let export = rekordbox_cue_rows(&[first, second], 120., Default::default());
        assert_eq!(
            (export.rows[0].kind, export.rows[0].active_loop),
            (4, false)
        );
        assert_eq!((export.rows[1].kind, export.rows[1].active_loop), (0, true));
        let mut plain = active_loop("B", 1., 3., 1);
        plain.color = serde_json::Value::Null;
        let row = &rekordbox_cue_rows(&[plain], 120., Default::default()).rows[0];
        assert_eq!((row.color, row.color_table_index), (Some(255), Some(0)));
    }

    #[test]
    fn a_loop_that_starts_before_the_track_is_clamped_and_extended_by_the_offset() {
        let mut early = active_loop("Early", -0.5, 1.5, 0);
        early.active_loop = Some(false);
        let row = &rekordbox_cue_rows(&[early], 120., Default::default()).rows[0];
        assert_eq!((row.in_msec, row.out_msec), (0, 2000));
        assert_eq!(row.beat_loop_size, Some(65536 * 4 + 1));
    }

    #[test]
    fn extra_hot_cues_take_free_slots_or_are_dropped() {
        let cues: Vec<_> = (0..10).map(|i| cue("c", i as f64, i, "red")).collect();
        let export = rekordbox_cue_rows(&cues, 128., Default::default());
        assert_eq!(export.rows.len(), 8);
        assert_eq!(export.skipped, 2);
        let mut kinds: Vec<_> = export.rows.iter().map(|r| r.kind).collect();
        kinds.sort_unstable();
        assert_eq!(kinds, [1, 2, 3, 5, 6, 7, 8, 9]);
        // Slots beyond the eighth move into slots the saved cues did not use.
        let sparse = rekordbox_cue_rows(
            &[cue("a", 1., 0, "red"), cue("b", 2., 9, "red")],
            128.,
            Default::default(),
        );
        assert_eq!(
            sparse.rows.iter().map(|r| r.kind).collect::<Vec<_>>(),
            [1, 2]
        );
    }

    #[test]
    fn destinations_choose_hot_memory_or_both() {
        let cues = [cue("a", 1., 0, "red"), active_loop("loop", 5., 9., 1)];
        let memory_only = rekordbox_cue_rows(
            &cues,
            120.,
            CueExportOptions {
                destination: CueDestination::MemoryCue,
                ..Default::default()
            },
        );
        assert_eq!(
            memory_only.rows.iter().map(|r| r.kind).collect::<Vec<_>>(),
            [0, 4]
        );
        assert!(
            memory_only
                .rows
                .iter()
                .all(|r| r.color_table_index.is_none())
        );
        let both = rekordbox_cue_rows(
            &cues,
            120.,
            CueExportOptions {
                destination: CueDestination::Both,
                ..Default::default()
            },
        );
        assert_eq!(
            both.rows.iter().map(|r| r.kind).collect::<Vec<_>>(),
            [1, 0, 2, 4]
        );
        let hot_only = rekordbox_cue_rows(
            &[memory("m", 3., "red")],
            120.,
            CueExportOptions {
                destination: CueDestination::HotCue,
                ..Default::default()
            },
        );
        assert_eq!(hot_only.rows[0].kind, 1);
        // Saved memory cues keep their own slot numbers: a cue may not take the slot a memory cue held.
        let clash = [memory("m", 3., "red"), cue("h", 4., 0, "red")];
        assert_eq!(
            rekordbox_cue_rows(&clash, 120., Default::default()).rows[1].kind,
            1
        );
    }
}
