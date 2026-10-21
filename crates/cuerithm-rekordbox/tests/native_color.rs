use cuerithm_rekordbox::*;

#[test]
fn colored_memory_has_null_sql_index_and_native_memory_byte() {
    let color = project_native_cue_color("red", None, NativeCueSlot::Memory, false, false).unwrap();
    assert_eq!(
        color.sql,
        NativeCueColor {
            color: 1,
            color_table_index: None
        }
    );
    assert_eq!(color.memory_color, 2);
    assert!(!color.converted);
}

#[test]
fn uncolored_points_loops_and_actual_black_remain_distinct() {
    for slot in [NativeCueSlot::Memory, NativeCueSlot::Hot(0)] {
        let point = project_native_cue_color("", None, slot, false, false).unwrap();
        assert_eq!(
            point.sql,
            NativeCueColor {
                color: -1,
                color_table_index: Some(0)
            }
        );
        assert_eq!(point.rgba, [0; 4]);
        assert_eq!(point.memory_color, 0);
        let saved_loop = project_native_cue_color("none", None, slot, true, false).unwrap();
        assert_eq!(
            saved_loop.sql,
            NativeCueColor {
                color: 255,
                color_table_index: Some(0)
            }
        );
    }
    let black =
        project_native_cue_color("black", None, NativeCueSlot::Hot(0), false, false).unwrap();
    assert_eq!(black.sql.color_table_index, Some(63));
    assert_eq!(black.rgba, [0; 4]);
    let rgb_black =
        project_native_cue_color("", Some(0), NativeCueSlot::Hot(0), false, false).unwrap();
    assert_eq!(rgb_black, black);
}

#[test]
fn all_exact_hot_palette_colors_round_trip_without_conversion() {
    assert_eq!(NATIVE_HOT_CUE_RGB.len(), 64);
    let unique = std::collections::BTreeSet::from(NATIVE_HOT_CUE_RGB);
    assert_eq!(unique.len(), 64);
    for (index, rgb) in NATIVE_HOT_CUE_RGB.iter().copied().enumerate() {
        let color =
            project_native_cue_color("", Some(rgb), NativeCueSlot::Hot(7), false, false).unwrap();
        assert_eq!(color.sql.color_table_index, Some((index + 1) as i64));
        assert_eq!(color.sql.color, -1);
        assert_eq!(
            color.rgba,
            [0, (rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8]
        );
        assert!(!color.converted);
        let hex = format!("#{rgb:06X}");
        assert_eq!(
            project_native_cue_color(&hex, None, NativeCueSlot::Hot(0), false, false).unwrap(),
            color
        );
    }
    assert_eq!(
        project_native_cue_color("#ff0000", None, NativeCueSlot::Hot(0), false, false)
            .unwrap()
            .sql
            .color_table_index,
        Some(42)
    );
}

#[test]
fn semantic_hot_labels_project_to_the_named_native_indices() {
    for (label, index) in [
        ("Blue", 1),
        ("BlueLight", 6),
        ("Aqua", 10),
        ("AquaDark", 15),
        ("Teal", 18),
        ("Green", 21),
        ("Lime", 25),
        ("Yellow", 30),
        ("YellowDark", 33),
        ("Orange", 36),
        ("Red", 41),
        ("MagentaRed", 46),
        ("Magenta", 52),
        ("Violet", 56),
        ("VioletLight", 60),
        ("BlueViolet", 62),
        ("Black", 63),
        ("White", 64),
    ] {
        let color =
            project_native_cue_color(label, Some(0x123456), NativeCueSlot::Hot(0), true, false)
                .unwrap();
        assert_eq!(
            color.sql,
            NativeCueColor {
                color: -1,
                color_table_index: Some(index)
            }
        );
        assert!(!color.converted);
        let rgb = NATIVE_HOT_CUE_RGB[(index - 1) as usize];
        assert_eq!(
            color.rgba,
            [0, (rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8]
        );
    }
}

#[test]
fn arbitrary_colors_and_unsupported_memory_names_require_explicit_conversion() {
    assert!(
        project_native_cue_color("", Some(0x123456), NativeCueSlot::Hot(0), false, false).is_err()
    );
    let nearest =
        project_native_cue_color("", Some(0x123456), NativeCueSlot::Hot(0), false, true).unwrap();
    assert!(nearest.converted);
    assert_ne!(nearest.sql.color_table_index, Some(0));
    assert!(project_native_cue_color("teal", None, NativeCueSlot::Memory, false, false).is_err());
    let memory =
        project_native_cue_color("teal", None, NativeCueSlot::Memory, false, true).unwrap();
    assert!(memory.converted);
    assert_eq!(memory.sql.color_table_index, None);
    assert!((1..=8).contains(&memory.memory_color));
    assert!(
        project_native_cue_color("not-a-color", None, NativeCueSlot::Hot(0), false, true).is_err()
    );
    assert!(
        project_native_cue_color("", Some(0x1_000000), NativeCueSlot::Hot(0), false, true).is_err()
    );
    assert!(project_native_cue_color("", None, NativeCueSlot::Hot(8), false, false).is_err());
}

#[test]
fn nearest_color_ties_choose_the_first_native_palette_index() {
    // Exactly halfway between the first two native blue entries.
    let color =
        project_native_cue_color("", Some(0x000eff), NativeCueSlot::Hot(0), false, true).unwrap();
    assert_eq!(color.sql.color_table_index, Some(1));
    assert_eq!(color.rgba, [0, 0, 0, 255]);
    assert!(color.converted);
}

#[test]
fn all_memory_representatives_round_trip_and_colored_hot_loops_keep_color_minus_one() {
    for (index, rgb) in NATIVE_MEMORY_CUE_RGB.iter().copied().enumerate() {
        let color =
            project_native_cue_color("", Some(rgb), NativeCueSlot::Memory, true, false).unwrap();
        assert_eq!(
            color.sql,
            NativeCueColor {
                color: index as i64,
                color_table_index: None
            }
        );
        assert_eq!(color.memory_color, (index + 1) as u8);
        assert!(!color.converted);
    }
    for (label, rgb) in [("red", None), ("", Some(0xff0000))] {
        let colored =
            project_native_cue_color(label, rgb, NativeCueSlot::Hot(0), true, false).unwrap();
        assert_eq!(colored.sql.color, -1);
        assert!(colored.sql.color_table_index.is_some_and(|index| index > 0));
    }
    assert_eq!(
        project_native_cue_color("", None, NativeCueSlot::Hot(0), true, false)
            .unwrap()
            .sql
            .color,
        255
    );
}
