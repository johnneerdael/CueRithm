use cuerithm_rekordbox::anlz::*;

// Independent wire fixtures: explicit offsets/values from the format diagrams.
fn envelope(id: &[u8; 4], header: u32, body: &[u8]) -> Vec<u8> {
    let mut b = id.to_vec();
    b.extend(header.to_be_bytes());
    b.extend((12 + body.len() as u32).to_be_bytes());
    b.extend(body);
    b
}
fn file(tags: &[Vec<u8>]) -> Vec<u8> {
    let body = tags.concat();
    let mut b = envelope(b"PMAI", 28, &[0xaa; 16]);
    b.extend(&body);
    let len = b.len() as u32;
    b[8..12].copy_from_slice(&len.to_be_bytes());
    b
}
fn grid() -> Vec<u8> {
    envelope(
        b"PQTZ",
        24,
        &[
            0, 0, 0, 0, 0, 8, 0, 0, 0, 0, 0, 2, 0, 1, 0x32, 0xc8, 0, 0, 0, 83, 0, 2, 0x32, 0xc8, 0,
            0, 2, 32,
        ],
    )
}
fn legacy_entry() -> Vec<u8> {
    let mut b = envelope(b"PCPT", 28, &[0; 44]);
    b[12..16].copy_from_slice(&3u32.to_be_bytes());
    b[16..20].copy_from_slice(&4u32.to_be_bytes());
    b[20..24].copy_from_slice(&65536u32.to_be_bytes());
    b[24..28].copy_from_slice(&[255, 255, 0, 1]);
    b[28..32].copy_from_slice(&[2, 0, 3, 232]);
    b[32..36].copy_from_slice(&10000u32.to_be_bytes());
    b[36..40].copy_from_slice(&18000u32.to_be_bytes());
    b[40..56].fill(0x5a); // Unowned extension bytes must survive.
    b
}
fn extended_entry(comment: &str) -> Vec<u8> {
    let text: Vec<_> = comment
        .encode_utf16()
        .chain([0])
        .flat_map(u16::to_be_bytes)
        .collect();
    let mut body = vec![0; 32]; // Bytes 12..44, before variable comment.
    body[0..4].copy_from_slice(&8u32.to_be_bytes());
    body[4..8].copy_from_slice(&[2, 0, 3, 232]);
    body[8..12].copy_from_slice(&10000u32.to_be_bytes());
    body[12..16].copy_from_slice(&18000u32.to_be_bytes());
    body[16] = 3;
    body[17] = 1;
    body[24..28].copy_from_slice(&[0, 4, 0, 1]);
    body[28..32].copy_from_slice(&(text.len() as u32).to_be_bytes());
    body.extend(text);
    body.extend([0x21, 0xff, 0x80, 0x10]);
    body.extend([0x72; 40]);
    envelope(b"PCP2", 16, &body)
}
fn cue_list(id: &[u8; 4], entry: &[u8]) -> Vec<u8> {
    let body = if id == b"PCOB" {
        [
            &[0, 0, 0, 1, 0x12, 0x34, 0, 1, 255, 255, 255, 255][..],
            entry,
        ]
        .concat()
    } else {
        [&[0, 0, 0, 1, 0, 1, 0x56, 0x78][..], entry].concat()
    };
    envelope(id, if id == b"PCOB" { 24 } else { 20 }, &body)
}

#[test]
fn file_roundtrip_retains_unknown_headers_waveforms_and_tag_order() {
    let opaque = envelope(b"ZZZZ", 14, &[0x33; 19]);
    let wave = envelope(b"PWV5", 24, &[0x55; 40]);
    let bytes = file(&[opaque.clone(), grid(), wave.clone()]);
    let parsed = AnlzFile::parse(&bytes, ParseLimits::default()).unwrap();
    assert_eq!(parsed.tags().len(), 3);
    assert_eq!(parsed.tags()[0].bytes(), opaque);
    assert_eq!(parsed.tags()[2].bytes(), wave);
    assert_eq!(parsed.to_bytes().unwrap(), bytes);
    let beats = parsed.tags()[1].beats().unwrap();
    assert_eq!(
        beats,
        vec![
            Beat {
                number: 1,
                tempo: 13000,
                time_ms: 83
            },
            Beat {
                number: 2,
                tempo: 13000,
                time_ms: 544
            }
        ]
    );
}

#[test]
fn edits_are_staged_lossless_and_idempotent() {
    let opaque = envelope(b"ZZZZ", 12, &[7; 13]);
    let original = file(&[opaque.clone(), grid()]);
    let mut parsed = AnlzFile::parse(&original, ParseLimits::default()).unwrap();
    let replacement = AnlzTag::grid(&[Beat {
        number: 1,
        tempo: 12835,
        time_ms: 82,
    }])
    .unwrap();
    parsed.replace(1, replacement.clone()).unwrap();
    let edited = parsed.to_bytes().unwrap();
    assert_ne!(edited, original);
    assert_eq!(parsed.tags()[0].bytes(), opaque);
    assert_eq!(&edited[12..28], &[0xaa; 16]);
    let mut again = AnlzFile::parse(&edited, ParseLimits::default()).unwrap();
    again.replace(1, replacement).unwrap();
    assert_eq!(again.to_bytes().unwrap(), edited);
    assert!(again.replace(0, AnlzTag::grid(&[]).unwrap()).is_err());
}

#[test]
fn cues_keep_native_units_flags_colors_comments_and_opaque_bytes() {
    let label = format!("{}🎧", "é".repeat(150)); // >255 bytes; length is u32.
    let legacy = legacy_entry();
    let extended = extended_entry(&label);
    let bytes = file(&[cue_list(b"PCOB", &legacy), cue_list(b"PCO2", &extended)]);
    let parsed = AnlzFile::parse(&bytes, ParseLimits::default()).unwrap();
    let old = parsed.tags()[0].cues().unwrap();
    assert_eq!(old.kind, 1);
    let cue = &old.entries[0];
    assert_eq!(
        (cue.hot_cue, cue.cue_type, cue.time_ms, cue.loop_time_ms),
        (3, 2, 10000, 18000)
    );
    assert_eq!(cue.legacy.as_ref().unwrap().status, 4);
    assert_eq!(cue.raw, legacy);
    let new = parsed.tags()[1].cues().unwrap();
    let details = new.entries[0].extended.as_ref().unwrap();
    assert_eq!(details.comment.as_deref(), Some(label.as_str()));
    assert_eq!(details.color, Some([0x21, 0xff, 0x80, 0x10]));
    assert_eq!(details.memory_color, Some(3));
    assert_eq!(details.loop_fraction, Some((4, 1)));
    assert_eq!(new.entries[0].raw, extended);
    assert_eq!(parsed.to_bytes().unwrap(), bytes);
}

#[test]
fn valid_short_extended_cues_remain_readable_and_lossless() {
    for length in [28, 36, 40, 44] {
        let mut entry = extended_entry("hello");
        entry.truncate(length);
        entry[8..12].copy_from_slice(&(length as u32).to_be_bytes());
        if length >= 44 {
            entry[40..44].fill(0);
        }
        let bytes = file(&[cue_list(b"PCO2", &entry)]);
        let parsed = AnlzFile::parse(&bytes, ParseLimits::default()).unwrap();
        let cues = parsed.tags()[0].cues().unwrap();
        let details = cues.entries[0].extended.as_ref().unwrap();
        assert!(details.comment.is_none());
        assert!(details.color.is_none());
        assert_eq!(parsed.to_bytes().unwrap(), bytes);
    }
}

#[test]
fn authored_cue_builders_match_native_wire_fields() {
    let legacy = AnlzTag::legacy_cues(
        1,
        &[LegacyCue {
            hot_cue: 3,
            cue_type: 2,
            time_ms: 10000,
            loop_time_ms: 18000,
            status: 4,
            order_first: 65535,
            order_last: 1,
        }],
    )
    .unwrap();
    let mut expected = legacy_entry();
    expected[40..56].fill(0);
    let mut list = cue_list(b"PCOB", &expected);
    list[16..18].fill(0);
    assert_eq!(legacy.bytes(), list);
    let extended = AnlzTag::extended_cues(
        1,
        &[ExtendedCue {
            hot_cue: 8,
            cue_type: 2,
            time_ms: 10000,
            loop_time_ms: 18000,
            memory_color: 3,
            loop_fraction: (4, 1),
            comment: "Drop 🎧".into(),
            color: [0, 255, 128, 16],
        }],
    )
    .unwrap();
    let mut expected = extended_entry("Drop 🎧");
    let color = 44 + 2 * ("Drop 🎧".encode_utf16().count() + 1);
    expected[color] = 0;
    expected[color + 4..].fill(0);
    let mut list = cue_list(b"PCO2", &expected);
    list[18..20].fill(0);
    assert_eq!(extended.bytes(), list);
}

#[test]
fn utf16_paths_preserve_unicode_and_reject_interior_nuls() {
    let path = "?/Music/日本語/🎧.flac";
    let tag = AnlzTag::path(path).unwrap();
    assert_eq!(tag.audio_path().unwrap(), path);
    assert!(AnlzTag::path("a\0b").is_err());
    let mut invalid = tag.bytes().to_vec();
    invalid[16..18].copy_from_slice(&0xd800u16.to_be_bytes());
    assert!(
        AnlzTag::parse(&invalid, ParseLimits::default())
            .unwrap()
            .audio_path()
            .is_err()
    );
}

#[test]
fn malformed_lengths_counts_and_budgets_fail_without_panics() {
    let good = file(&[grid()]);
    for end in 0..good.len() {
        assert!(AnlzFile::parse(&good[..end], ParseLimits::default()).is_err());
    }
    for (offset, value) in [
        (4, 0u32),
        (4, 11),
        (4, u32::MAX),
        (8, 12),
        (32, 0),
        (32, u32::MAX),
        (36, 0),
        (36, u32::MAX),
    ] {
        let mut b = good.clone();
        b[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
        assert!(
            AnlzFile::parse(&b, ParseLimits::default()).is_err(),
            "offset {offset}, value {value}"
        );
    }
    assert!(
        AnlzFile::parse(
            &good,
            ParseLimits {
                max_bytes: good.len() - 1,
                ..Default::default()
            }
        )
        .is_err()
    );
    assert!(
        AnlzFile::parse(
            &good,
            ParseLimits {
                max_tags: 0,
                ..Default::default()
            }
        )
        .is_err()
    );
    let mut b = grid();
    b[20..24].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(
        AnlzTag::parse(&b, ParseLimits::default())
            .unwrap()
            .beats()
            .is_err()
    );
    assert!(
        AnlzTag::parse(
            &grid(),
            ParseLimits {
                max_entries: 1,
                ..Default::default()
            }
        )
        .unwrap()
        .beats()
        .is_err()
    );
    let mut b = extended_entry("x");
    b[40..44].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(
        AnlzTag::parse(&cue_list(b"PCO2", &b), ParseLimits::default())
            .unwrap()
            .cues()
            .is_err()
    );
    assert!(
        AnlzTag::parse(
            &cue_list(b"PCO2", &extended_entry("long")),
            ParseLimits {
                max_string_bytes: 2,
                ..Default::default()
            }
        )
        .unwrap()
        .cues()
        .is_err()
    );
}

#[test]
fn unknown_cue_entries_are_preserved_and_not_silently_skipped() {
    let opaque = envelope(b"NEW!", 12, &[0x99; 20]);
    let tag = AnlzTag::parse(&cue_list(b"PCO2", &opaque), ParseLimits::default()).unwrap();
    assert!(tag.cues().is_err());
    assert_eq!(tag.bytes(), cue_list(b"PCO2", &opaque));
}

#[test]
fn editing_owned_payloads_preserves_tag_header_extensions_and_trailers() {
    let mut raw = grid();
    raw.splice(24..24, [0x81; 4]);
    raw.extend([0x91; 7]);
    raw[4..8].copy_from_slice(&28u32.to_be_bytes());
    let length = raw.len() as u32;
    raw[8..12].copy_from_slice(&length.to_be_bytes());
    raw[12..16].copy_from_slice(&123u32.to_be_bytes());
    let tag = AnlzTag::parse(&raw, ParseLimits::default()).unwrap();
    let edited = tag
        .with_beats(&[Beat {
            number: 1,
            tempo: 12800,
            time_ms: 100,
        }])
        .unwrap();
    assert_eq!(&edited.bytes()[12..20], &raw[12..20]);
    assert_eq!(&edited.bytes()[24..28], &[0x81; 4]);
    assert_eq!(&edited.bytes()[36..], &[0x91; 7]);
    assert_eq!(edited.beats().unwrap().len(), 1);
    assert_eq!(
        edited.with_beats(&edited.beats().unwrap()).unwrap().bytes(),
        edited.bytes()
    );

    let mut raw = AnlzTag::path("old.flac").unwrap().bytes().to_vec();
    raw.extend([0xf3; 11]);
    let length = raw.len() as u32;
    raw[8..12].copy_from_slice(&length.to_be_bytes());
    let edited = AnlzTag::parse(&raw, ParseLimits::default())
        .unwrap()
        .with_audio_path("new/🎧.mp3")
        .unwrap();
    assert_eq!(edited.audio_path().unwrap(), "new/🎧.mp3");
    assert_eq!(&edited.bytes()[edited.bytes().len() - 11..], &[0xf3; 11]);
}

#[test]
fn merging_cue_records_keeps_unowned_entries_list_fields_and_extensions() {
    let retained = extended_entry("Keep me");
    let opaque = envelope(b"NEW!", 12, &[0x55; 8]);
    let mut raw = cue_list(b"PCO2", &[retained.clone(), opaque.clone()].concat());
    raw[16..18].copy_from_slice(&2u16.to_be_bytes());
    raw.extend([0xaf; 5]);
    let length = raw.len() as u32;
    raw[8..12].copy_from_slice(&length.to_be_bytes());
    let list = AnlzTag::parse(&raw, ParseLimits::default()).unwrap();
    let mut records = list.cue_records().unwrap();
    assert_eq!(records[0].bytes(), retained);
    assert_eq!(records[1].bytes(), opaque);
    assert_eq!(list.with_cue_records(&records).unwrap().bytes(), raw);
    records.remove(0);
    let edited = list.with_cue_records(&records).unwrap();
    assert_eq!(&edited.bytes()[18..20], &[0x56, 0x78]);
    assert_eq!(edited.cue_records().unwrap()[0].bytes(), opaque);
    assert_eq!(&edited.bytes()[edited.bytes().len() - 5..], &[0xaf; 5]);
}

#[test]
fn grid_validation_and_microsecond_extension_keep_the_native_boundary() {
    assert!(
        AnlzTag::grid(&[Beat {
            number: 0,
            tempo: 12800,
            time_ms: 1
        }])
        .is_err()
    );
    assert!(
        AnlzTag::grid(&[Beat {
            number: 1,
            tempo: 0,
            time_ms: 1
        }])
        .is_err()
    );
    assert!(
        AnlzTag::grid(&[
            Beat {
                number: 1,
                tempo: 12800,
                time_ms: 2
            },
            Beat {
                number: 2,
                tempo: 12800,
                time_ms: 1
            }
        ])
        .is_err()
    );
    let beats = [
        Beat {
            number: 1,
            tempo: 13000,
            time_ms: 83,
        },
        Beat {
            number: 2,
            tempo: 13000,
            time_ms: 544,
        },
    ];
    let tag = AnlzTag::microsecond_grid(&beats, &[311, 849], false, 0).unwrap();
    let ext = tag.extended_grid().unwrap();
    assert_eq!(ext.words, vec![311, 849]);
    assert_eq!(ext.first, Some(beats[0]));
    assert_eq!(ext.last, Some(beats[1]));
    assert_eq!(ext.downbeat, 1);
    assert_eq!(ext.checksum, 26630);
    assert!(AnlzTag::microsecond_grid(&beats, &[1], false, 0).is_err());
    assert!(AnlzTag::microsecond_grid(&beats, &[1, 1000], false, 0).is_err());
}

#[test]
fn shipped_lexicon_builders_match_all_authored_reference_cases() {
    use serde_json::Value;
    fn u(v: &Value, k: &str) -> u32 {
        v[k].as_u64().unwrap().try_into().unwrap()
    }
    fn beats(v: &Value) -> Vec<Beat> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|b| Beat {
                number: u(b, "beatNumber") as u16,
                tempo: u(b, "tempo") as u16,
                time_ms: u(b, "timeMs"),
            })
            .collect()
    }
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/anlz/reference.json")).unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 30);
    for (index, c) in cases.iter().enumerate() {
        let input = &c["input"];
        let tag = match c["family"].as_str().unwrap() {
            "grid" => AnlzTag::grid(&beats(&input["beats"])),
            "extended_grid" => AnlzTag::microsecond_grid(
                &beats(&input["beats"]),
                &input["fractions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap() as u16)
                    .collect::<Vec<_>>(),
                input["variable"].as_bool().unwrap(),
                u(input, "downbeat") as usize,
            ),
            "path" => AnlzTag::path(input["path"].as_str().unwrap()),
            "legacy" => AnlzTag::legacy_cues(
                u(input, "kind"),
                &input["entries"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|e| LegacyCue {
                        hot_cue: u(e, "hotCue"),
                        cue_type: u(e, "type") as u8,
                        time_ms: u(e, "timeMs"),
                        loop_time_ms: u(e, "loopTimeMs"),
                        status: u(e, "status"),
                        order_first: u(e, "orderFirst") as u16,
                        order_last: u(e, "orderLast") as u16,
                    })
                    .collect::<Vec<_>>(),
            ),
            "extended" => AnlzTag::extended_cues(
                u(input, "kind"),
                &input["entries"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|e| ExtendedCue {
                        hot_cue: u(e, "hotCue"),
                        cue_type: u(e, "type") as u8,
                        time_ms: u(e, "timeMs"),
                        loop_time_ms: u(e, "loopTimeMs"),
                        memory_color: u(e, "colorId") as u8,
                        loop_fraction: if u(e, "loopBeats") == 0 {
                            (0, 0)
                        } else {
                            (u(e, "loopBeats") as u16, 1)
                        },
                        comment: e["label"].as_str().unwrap().into(),
                        color: [0, u(e, "r") as u8, u(e, "g") as u8, u(e, "b") as u8],
                    })
                    .collect::<Vec<_>>(),
            ),
            _ => panic!("Unknown reference family"),
        }
        .unwrap();
        let expected = c["wire"].as_str().unwrap();
        let wire: Vec<_> = (0..expected.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&expected[i..i + 2], 16).unwrap())
            .collect();
        assert_eq!(tag.bytes(), wire, "reference case {index}");
    }
}

#[test]
fn a_constant_grid_counts_bars_forward_and_the_lead_in_backward() {
    use cuerithm_rekordbox::anlz::beat_grid;
    // 120 BPM is a beat every 500 ms; the first beat is at 1.25 s, so two beats precede it.
    let beats = beat_grid(120., 1.25, 4.);
    let times: Vec<_> = beats.iter().map(|b| (b.number, b.time_ms)).collect();
    assert_eq!(
        times,
        [
            (3, 250),
            (4, 750),
            (1, 1250),
            (2, 1750),
            (3, 2250),
            (4, 2750),
            (1, 3250),
            (2, 3750)
        ]
    );
    assert!(beats.iter().all(|b| b.tempo == 12000));
    // Fractional tempos are floored to hundredths, as rekordbox stores them.
    assert_eq!(beat_grid(128.345, 0., 2.)[0].tempo, 12834);
    for (bpm, first, duration) in [
        (0., 0., 10.),
        (700., 0., 10.),
        (120., 0., 3600.),
        (120., f64::NAN, 10.),
        (f64::NAN, 0., 10.),
        (120., 0., -1.),
    ] {
        assert!(
            beat_grid(bpm, first, duration).is_empty(),
            "{bpm} {first} {duration}"
        );
    }
}

#[test]
fn a_placeholder_analysis_file_parses_and_carries_the_path_and_grid() {
    use cuerithm_rekordbox::anlz::{AnlzFile, ParseLimits, beat_grid, placeholder_analysis};
    let beats = beat_grid(126., 0.2, 30.);
    let bytes = placeholder_analysis("Artist - Title (Club Mix).flac", &beats).unwrap();
    let file = AnlzFile::parse(&bytes, ParseLimits::default()).unwrap();
    let kinds: Vec<_> = file
        .tags()
        .iter()
        .map(|t| String::from_utf8_lossy(&t.fourcc()).into_owned())
        .collect();
    assert_eq!(
        kinds,
        ["PPTH", "PVBR", "PQTZ", "PWAV", "PWV2", "PCOB", "PCOB"]
    );
    assert_eq!(
        file.tags()[0].audio_path().unwrap(),
        "?/Artist - Title (Club Mix).flac"
    );
    assert_eq!(file.tags()[2].beats().unwrap(), beats);
    // Without a grid there is no PQTZ at all, rather than an empty one.
    let bare = AnlzFile::parse(
        &placeholder_analysis("x.mp3", &[]).unwrap(),
        ParseLimits::default(),
    )
    .unwrap();
    assert_eq!(bare.tags().len(), 6);
    // The same input always gives the same bytes.
    assert_eq!(
        bytes,
        placeholder_analysis("Artist - Title (Club Mix).flac", &beats).unwrap()
    );
    assert!(placeholder_analysis("", &beats).is_err());
}
