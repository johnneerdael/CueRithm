use cuerithm_media::CancellationToken;
use cuerithm_rekordbox::{anlz::*, *};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn envelope(id: &[u8; 4], header: u32, body: &[u8]) -> Vec<u8> {
    let mut bytes = id.to_vec();
    bytes.extend(header.to_be_bytes());
    bytes.extend((12 + body.len() as u32).to_be_bytes());
    bytes.extend(body);
    bytes
}
fn authored_file(tags: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = envelope(b"PMAI", 28, &[0xaa; 16]);
    bytes.extend(tags.concat());
    let size = bytes.len() as u32;
    bytes[8..12].copy_from_slice(&size.to_be_bytes());
    bytes
}
fn authored_wave(id: &[u8; 4], seed: u8) -> Vec<u8> {
    let count = if seed == 10 { 2u32 } else { 4u32 };
    match id {
        b"PVBR" => {
            let mut payload = vec![seed; 1604];
            payload[1600..].copy_from_slice(&(u32::from(seed) * 1000).to_be_bytes());
            envelope(id, 16, &[&[0, 0, 0, 0][..], &payload].concat())
        }
        b"PWAV" | b"PWV2" => {
            let mut body = [count.to_be_bytes(), 65536u32.to_be_bytes()].concat();
            body.extend(vec![seed; count as usize]);
            envelope(id, 20, &body)
        }
        b"PWVC" => envelope(
            id,
            14,
            &[0, 0, seed, seed + 1, seed + 2, seed + 3, seed + 4, seed + 5],
        ),
        _ => {
            let stride = match id {
                b"PWV3" => 1u32,
                b"PWV4" => 6,
                b"PWV5" => 2,
                b"PWV6" | b"PWV7" => 3,
                _ => panic!("authored waveform kind"),
            };
            let mut body = [stride.to_be_bytes(), count.to_be_bytes()].concat();
            let header = if id == b"PWV6" {
                20
            } else {
                body.extend(9830400u32.to_be_bytes());
                24
            };
            body.extend(vec![seed; stride as usize * count as usize]);
            envelope(id, header, &body)
        }
    }
}
fn authored_analysis() -> Vec<u8> {
    let mut memory = envelope(b"PCPT", 28, &[0; 44]);
    memory[20..24].copy_from_slice(&65536u32.to_be_bytes());
    memory[28..32].copy_from_slice(&[1, 0, 3, 232]);
    memory[32..36].copy_from_slice(&7000u32.to_be_bytes());
    memory[40..56].fill(0xa5);
    let opaque_record = envelope(b"ZZZZ", 12, &[0x81; 9]);
    let mut list_body = vec![0; 12];
    list_body[6..8].copy_from_slice(&2u16.to_be_bytes());
    list_body.extend(&memory);
    list_body.extend(opaque_record);
    let list = envelope(b"PCOB", 24, &list_body);
    let mut grid_body = vec![0; 12];
    grid_body[4..8].copy_from_slice(&524288u32.to_be_bytes());
    let grid = envelope(b"PQTZ", 24, &grid_body);
    let opaque = envelope(b"PWV5", 24, &[0x53; 40]);
    let mut bytes = envelope(b"PMAI", 28, &[0xaa; 16]);
    bytes.extend(list);
    bytes.extend(grid);
    bytes.extend(opaque);
    let len = bytes.len() as u32;
    bytes[8..12].copy_from_slice(&len.to_be_bytes());
    bytes
}
fn cue() -> NativeCue {
    NativeCue {
        slot: NativeCueSlot::Memory,
        start: NativeTime::from_seconds(7.025).unwrap(),
        end: Some(NativeTime::from_seconds(9.025).unwrap()),
        active_loop: true,
        order_first: 0,
        order_last: 1,
        comment: "Loop".into(),
        memory_color: 3,
        color: [0x21, 255, 128, 16],
        loop_fraction: (4, 1),
    }
}
fn plan(root: &std::path::Path, bytes: &[u8]) -> NativeAssetPlan {
    let file = AnlzFile::parse(bytes, ParseLimits::default()).unwrap();
    let owned = hash(file.tags()[0].cue_records().unwrap()[0].bytes());
    NativeAssetPlan {
        source_root: root.into(),
        edits: vec![NativeAssetEdit {
            owner_content_id: "41".into(),
            relative_path: "share/track/ANLZ0000.DAT".into(),
            expected_sha256: Some(hash(bytes)),
            content: NativeAssetContent::Analysis {
                template: None,
                edits: vec![
                    NativeAnlzEdit::Cues {
                        tag_index: 0,
                        owned_record_sha256: vec![owned],
                        replacement: vec![cue()],
                    },
                    NativeAnlzEdit::Grid {
                        tag_index: 1,
                        beats: vec![
                            Beat {
                                number: 1,
                                tempo: 12800,
                                time_ms: 25,
                            },
                            Beat {
                                number: 2,
                                tempo: 12900,
                                time_ms: 494,
                            },
                        ],
                    },
                ],
            },
        }],
    }
}
fn setup() -> (tempfile::TempDir, tempfile::TempDir, Vec<u8>) {
    let source = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(source.path().join("share/track")).unwrap();
    let bytes = authored_analysis();
    std::fs::write(source.path().join("share/track/ANLZ0000.DAT"), &bytes).unwrap();
    (source, workspace, bytes)
}

#[test]
fn stages_owned_cues_and_variable_grid_preserving_unknown_records_and_waveforms() {
    let (source, workspace, before) = setup();
    let staged = stage_native_assets(
        &plan(source.path(), &before),
        workspace.path(),
        &CancellationToken::default(),
    )
    .unwrap();
    let proof = &staged.manifest[0];
    assert_eq!(proof.before_sha256, Some(hash(&before)));
    assert!(proof.changed);
    let after = std::fs::read(proof.staged_path.as_ref().unwrap()).unwrap();
    assert_eq!(proof.after_sha256, hash(&after));
    assert_eq!(
        std::fs::read(source.path().join(&proof.relative_path)).unwrap(),
        before
    );
    let parsed = AnlzFile::parse(&after, ParseLimits::default()).unwrap();
    let original = AnlzFile::parse(&before, ParseLimits::default()).unwrap();
    let records = parsed.tags()[0].cue_records().unwrap();
    assert_eq!(
        records[1].bytes(),
        original.tags()[0].cue_records().unwrap()[1].bytes()
    );
    assert_eq!(&records[0].bytes()[40..56], &[0xa5; 16]);
    let edited = AnlzTag::legacy_cues(
        0,
        &[LegacyCue {
            hot_cue: 0,
            cue_type: 2,
            time_ms: 7025,
            loop_time_ms: 9025,
            status: 4,
            order_first: 0,
            order_last: 1,
        }],
    )
    .unwrap();
    assert_eq!(
        &records[0].bytes()[12..40],
        &edited.cue_records().unwrap()[0].bytes()[12..40]
    );
    assert_eq!(parsed.tags()[1].beats().unwrap()[1].tempo, 12900);
    assert_eq!(parsed.tags()[2].bytes(), original.tags()[2].bytes());
    assert_eq!(&after[12..28], &[0xaa; 16]);
}

#[test]
fn rejects_stale_ownership_traversal_duplicates_and_missing_sidecars() {
    let (source, workspace, before) = setup();
    let cancel = CancellationToken::default();
    let original = plan(source.path(), &before);
    let mut stale = original.clone();
    stale.edits[0].expected_sha256 = Some("0".repeat(64));
    assert!(stage_native_assets(&stale, workspace.path(), &cancel).is_err());
    for path in [
        "../escape.DAT",
        "/escape.DAT",
        "share\\escape.DAT",
        "share/../escape.DAT",
        "share/master.db-wal",
    ] {
        let mut bad = original.clone();
        bad.edits[0].relative_path = PathBuf::from(path);
        assert!(
            stage_native_assets(&bad, workspace.path(), &cancel).is_err(),
            "{path}"
        );
    }
    let mut duplicate = original.clone();
    duplicate.edits.push(duplicate.edits[0].clone());
    assert!(stage_native_assets(&duplicate, workspace.path(), &cancel).is_err());
    let mut bad = original.clone();
    if let NativeAssetContent::Analysis { edits, .. } = &mut bad.edits[0].content
        && let NativeAnlzEdit::Cues {
            owned_record_sha256,
            ..
        } = &mut edits[0]
    {
        owned_record_sha256[0] = "0".repeat(64);
    }
    assert!(stage_native_assets(&bad, workspace.path(), &cancel).is_err());
    std::fs::write(source.path().join("share/track/ANLZ0000.EXT"), &before).unwrap();
    assert!(stage_native_assets(&original, workspace.path(), &cancel).is_err());
}

#[test]
fn new_analysis_artwork_and_noop_files_have_publication_proofs() {
    let (source, workspace, before) = setup();
    let mut p = plan(source.path(), &before);
    if let NativeAssetContent::Analysis { edits, .. } = &mut p.edits[0].content {
        edits.clear();
    }
    let created =
        new_native_analysis("/Music/source.wav", &[], &[cue()], NativeAnalysisKind::Dat).unwrap();
    p.edits.push(NativeAssetEdit {
        owner_content_id: "42".into(),
        relative_path: "share/new/ANLZ0000.DAT".into(),
        expected_sha256: None,
        content: NativeAssetContent::Analysis {
            template: Some(created.clone()),
            edits: vec![],
        },
    });
    p.edits.push(NativeAssetEdit {
        owner_content_id: "42".into(),
        relative_path: "Artwork/42.jpg".into(),
        expected_sha256: None,
        content: NativeAssetContent::Artwork {
            jpeg: vec![255, 216, 255, 224, 0, 2, 255, 217],
        },
    });
    let staged = stage_native_assets(&p, workspace.path(), &CancellationToken::default()).unwrap();
    assert!(!staged.manifest[0].changed);
    assert!(staged.manifest[0].staged_path.is_none());
    assert_eq!(staged.manifest[1].before_sha256, None);
    assert_eq!(
        std::fs::read(staged.manifest[1].staged_path.as_ref().unwrap()).unwrap(),
        created
    );
    assert!(!source.path().join("share/new/ANLZ0000.DAT").exists());
    assert!(!source.path().join("Artwork/42.jpg").exists());

    let mut wrong_owner = p.clone();
    let mut sidecar = wrong_owner.edits[1].clone();
    sidecar.relative_path = "share/new/ANLZ0000.EXT".into();
    sidecar.owner_content_id = "43".into();
    wrong_owner.edits.push(sidecar);
    assert!(
        stage_native_assets(
            &wrong_owner,
            workspace.path(),
            &CancellationToken::default()
        )
        .is_err()
    );
}

#[test]
fn cancellation_and_invalid_clocks_leave_no_staged_files() {
    let (source, workspace, before) = setup();
    let cancel = CancellationToken::default();
    cancel.cancel();
    assert!(stage_native_assets(&plan(source.path(), &before), workspace.path(), &cancel).is_err());
    assert_eq!(std::fs::read_dir(workspace.path()).unwrap().count(), 0);
    for seconds in [
        f64::NAN,
        f64::INFINITY,
        -0.001,
        f64::from(u32::MAX) / 1000. + 1.,
    ] {
        assert!(NativeTime::from_seconds(seconds).is_err());
    }
    assert_eq!(
        NativeTime::from_seconds(1.2345).unwrap().milliseconds(),
        1234
    );
}

#[test]
fn source_pcm_changes_regenerate_only_known_waveforms_and_retain_opaque_extensions() {
    for (extension, kinds) in [
        ("DAT", [*b"PVBR", *b"PWAV", *b"PWV2"]),
        ("EXT", [*b"PWV3", *b"PWV4", *b"PWV5"]),
        ("2EX", [*b"PWV6", *b"PWV7", *b"PWVC"]),
    ] {
        let source = tempfile::tempdir().unwrap();
        let workspace = tempfile::tempdir().unwrap();
        let mut old_wave = authored_wave(&kinds[0], 10);
        let old_header = u32::from_be_bytes(old_wave[4..8].try_into().unwrap()) as usize;
        old_wave.splice(old_header..old_header, [0x91, 0x92, 0x93]);
        old_wave[4..8].copy_from_slice(&((old_header + 3) as u32).to_be_bytes());
        old_wave.extend([0xde, 0xad]);
        let len = old_wave.len() as u32;
        old_wave[8..12].copy_from_slice(&len.to_be_bytes());
        let protected = vec![
            envelope(b"ZZZZ", 12, &[0x55; 9]),
            AnlzTag::path("/Music/original.wav")
                .unwrap()
                .bytes()
                .to_vec(),
            AnlzTag::grid(&[Beat {
                number: 1,
                tempo: 12300,
                time_ms: 0,
            }])
            .unwrap()
            .bytes()
            .to_vec(),
            AnlzTag::legacy_cues(
                0,
                &[LegacyCue {
                    hot_cue: 0,
                    cue_type: 1,
                    time_ms: 1234,
                    loop_time_ms: 0,
                    status: 0,
                    order_first: u16::MAX,
                    order_last: u16::MAX,
                }],
            )
            .unwrap()
            .bytes()
            .to_vec(),
        ];
        let mut tags = vec![old_wave];
        tags.extend(protected.clone());
        let before = authored_file(&tags);
        let mut generated_tags = vec![
            AnlzTag::path("/Music/new.wav").unwrap().bytes().to_vec(),
            AnlzTag::grid(&[]).unwrap().bytes().to_vec(),
            AnlzTag::legacy_cues(0, &[]).unwrap().bytes().to_vec(),
        ];
        generated_tags.extend(kinds.iter().map(|kind| authored_wave(kind, 20)));
        let full_generated = authored_file(&generated_tags);
        let template = native_waveform_template(&full_generated).unwrap();
        let template_file = AnlzFile::parse(&template, ParseLimits::default()).unwrap();
        assert_eq!(
            template_file
                .tags()
                .iter()
                .map(AnlzTag::fourcc)
                .collect::<Vec<_>>(),
            kinds
        );
        let relative = PathBuf::from(format!("ANLZ0000.{extension}"));
        std::fs::write(source.path().join(&relative), &before).unwrap();
        let mut plan = NativeAssetPlan {
            source_root: source.path().into(),
            edits: vec![NativeAssetEdit {
                owner_content_id: "17".into(),
                relative_path: relative.clone(),
                expected_sha256: Some(hash(&before)),
                content: NativeAssetContent::Analysis {
                    template: None,
                    edits: vec![NativeAnlzEdit::Waveforms {
                        template: template.clone(),
                    }],
                },
            }],
        };
        let staged =
            stage_native_assets(&plan, workspace.path(), &CancellationToken::default()).unwrap();
        let bytes = std::fs::read(staged.manifest[0].staged_path.as_ref().unwrap()).unwrap();
        let after = AnlzFile::parse(&bytes, ParseLimits::default()).unwrap();
        assert_eq!(
            &after.tags()[0].bytes()[old_header..old_header + 3],
            &[0x91, 0x92, 0x93]
        );
        assert!(after.tags()[0].bytes().ends_with(&[0xde, 0xad]));
        let generated_first =
            AnlzTag::parse(&authored_wave(&kinds[0], 20), ParseLimits::default()).unwrap();
        assert_eq!(
            &after.tags()[0].bytes()[old_header + 3..after.tags()[0].bytes().len() - 2],
            &generated_first.bytes()[generated_first.header_len()..]
        );
        for (index, protected) in protected.iter().enumerate() {
            assert_eq!(after.tags()[index + 1].bytes(), protected);
        }
        assert_eq!(after.tags()[5].bytes(), authored_wave(&kinds[1], 20));
        assert_eq!(after.tags()[6].bytes(), authored_wave(&kinds[2], 20));
        assert_eq!(&bytes[12..28], &[0xaa; 16]);
        assert_eq!(
            std::fs::read(source.path().join(&relative)).unwrap(),
            before
        );
        std::fs::write(source.path().join(&relative), &bytes).unwrap();
        plan.edits[0].expected_sha256 = Some(hash(&bytes));
        let repeated =
            stage_native_assets(&plan, workspace.path(), &CancellationToken::default()).unwrap();
        assert!(!repeated.manifest[0].changed);
        if let NativeAssetContent::Analysis { edits, .. } = &mut plan.edits[0].content {
            edits[0] = NativeAnlzEdit::Waveforms {
                template: full_generated,
            };
        }
        assert!(
            stage_native_assets(&plan, workspace.path(), &CancellationToken::default()).is_err(),
            "cue/path/grid template injection must fail"
        );
        let reject = |bad_template| {
            let mut invalid = plan.clone();
            if let NativeAssetContent::Analysis { edits, .. } = &mut invalid.edits[0].content {
                *edits = vec![NativeAnlzEdit::Waveforms {
                    template: bad_template,
                }];
            }
            assert!(
                stage_native_assets(&invalid, workspace.path(), &CancellationToken::default())
                    .is_err()
            );
        };
        reject(authored_file(&[
            authored_wave(&kinds[0], 20),
            authored_wave(&kinds[0], 20),
            authored_wave(&kinds[1], 20),
        ]));
        let wrong = if extension == "DAT" {
            [*b"PWV3", *b"PWV4", *b"PWV5"]
        } else {
            [*b"PVBR", *b"PWAV", *b"PWV2"]
        };
        reject(authored_file(
            &wrong
                .iter()
                .map(|id| authored_wave(id, 20))
                .collect::<Vec<_>>(),
        ));
        let mut malformed = authored_wave(&kinds[0], 20);
        malformed.push(0xff);
        let size = malformed.len() as u32;
        malformed[8..12].copy_from_slice(&size.to_be_bytes());
        assert!(
            native_waveform_template(&authored_file(&[
                malformed,
                authored_wave(&kinds[1], 20),
                authored_wave(&kinds[2], 20)
            ]))
            .is_err()
        );
    }
}

#[test]
fn embedding_sidecars_stay_opaque_and_relocation_changes_only_the_path_chunk() {
    let source = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let before =
        new_native_analysis("/Music/previous.flac", &[], &[], NativeAnalysisKind::Dat).unwrap();
    std::fs::write(source.path().join("ANLZ0000.DAT"), &before).unwrap();
    let embedding = [
        0x81, 0xa9, b'e', b'm', b'b', b'e', b'd', b'd', b'i', b'n', b'g', 0x8b, 0xc4, 0x04, 0x5a,
        0x00, 0xff, 0x7f,
    ];
    std::fs::write(source.path().join("ANLZ0000.3EX"), embedding).unwrap();
    let mut p = NativeAssetPlan {
        source_root: source.path().into(),
        edits: vec![NativeAssetEdit {
            owner_content_id: "10".into(),
            relative_path: "ANLZ0000.DAT".into(),
            expected_sha256: Some(hash(&before)),
            content: NativeAssetContent::Analysis {
                template: None,
                edits: vec![NativeAnlzEdit::AudioPath {
                    tag_index: 0,
                    path: "/Music/new 🎧.flac".into(),
                }],
            },
        }],
    };
    assert!(
        stage_native_assets(&p, workspace.path(), &CancellationToken::default()).is_err(),
        "opaque companions must also be captured"
    );
    p.edits.push(NativeAssetEdit {
        owner_content_id: "10".into(),
        relative_path: "ANLZ0000.3EX".into(),
        expected_sha256: Some(hash(&embedding)),
        content: NativeAssetContent::OpaqueExisting,
    });
    let staged = stage_native_assets(&p, workspace.path(), &CancellationToken::default()).unwrap();
    assert!(!staged.manifest[1].changed);
    assert_eq!(staged.manifest[1].after_sha256, hash(&embedding));
    assert!(staged.manifest[1].staged_path.is_none());
    let bytes = std::fs::read(staged.manifest[0].staged_path.as_ref().unwrap()).unwrap();
    let after = AnlzFile::parse(&bytes, ParseLimits::default()).unwrap();
    assert_eq!(after.tags()[0].audio_path().unwrap(), "/Music/new 🎧.flac");
    let old = AnlzFile::parse(&before, ParseLimits::default()).unwrap();
    for (before, after) in old.tags()[1..].iter().zip(&after.tags()[1..]) {
        assert_eq!(before.bytes(), after.bytes());
    }
    assert_eq!(
        std::fs::read(source.path().join("ANLZ0000.3EX")).unwrap(),
        embedding
    );
}

#[test]
fn extended_cue_and_fractional_grid_edits_keep_producer_extensions() {
    let source = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let mut hot = cue();
    hot.slot = NativeCueSlot::Hot(7);
    hot.active_loop = false;
    let created = new_native_analysis(
        "/Music/source.m4a",
        &[],
        &[hot.clone()],
        NativeAnalysisKind::Ext,
    )
    .unwrap();
    assert_eq!(
        &created[12..28],
        &[0, 0, 0, 1, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0]
    );
    let mut file = AnlzFile::parse(&created, ParseLimits::default()).unwrap();
    let index = file
        .tags()
        .iter()
        .position(|tag| tag.fourcc() == *b"PCO2" && tag.cues().unwrap().kind == 1)
        .unwrap();
    let mut old = file.tags()[index].cue_records().unwrap()[0]
        .bytes()
        .to_vec();
    old[29] = 0x97;
    old.extend([0xbc; 11]);
    let len = old.len() as u32;
    old[8..12].copy_from_slice(&len.to_be_bytes());
    let list = file.tags()[index]
        .with_cue_records(&[AnlzTag::parse(&old, ParseLimits::default()).unwrap()])
        .unwrap();
    file.replace(index, list).unwrap();
    let before = file.to_bytes().unwrap();
    std::fs::write(source.path().join("ANLZ0000.EXT"), &before).unwrap();
    hot.comment = "Longer 🎧 label".into();
    hot.color = [7, 1, 2, 3];
    let legacy_index = file
        .tags()
        .iter()
        .position(|tag| tag.fourcc() == *b"PCOB" && tag.cues().unwrap().kind == 1)
        .unwrap();
    let legacy_hash = hash(file.tags()[legacy_index].cue_records().unwrap()[0].bytes());
    let p = NativeAssetPlan {
        source_root: source.path().into(),
        edits: vec![NativeAssetEdit {
            owner_content_id: "81".into(),
            relative_path: "ANLZ0000.EXT".into(),
            expected_sha256: Some(hash(&before)),
            content: NativeAssetContent::Analysis {
                template: None,
                edits: vec![
                    NativeAnlzEdit::Cues {
                        tag_index: index,
                        owned_record_sha256: vec![hash(&old)],
                        replacement: vec![hot.clone()],
                    },
                    NativeAnlzEdit::Cues {
                        tag_index: legacy_index,
                        owned_record_sha256: vec![legacy_hash],
                        replacement: vec![hot],
                    },
                ],
            },
        }],
    };
    let mut missing_activity_companion = p.clone();
    if let NativeAssetContent::Analysis { edits, .. } =
        &mut missing_activity_companion.edits[0].content
    {
        edits.truncate(1);
    }
    assert!(
        stage_native_assets(
            &missing_activity_companion,
            workspace.path(),
            &CancellationToken::default()
        )
        .is_err()
    );
    let staged = stage_native_assets(&p, workspace.path(), &CancellationToken::default()).unwrap();
    let bytes = std::fs::read(staged.manifest[0].staged_path.as_ref().unwrap()).unwrap();
    let edited = AnlzFile::parse(&bytes, ParseLimits::default()).unwrap();
    let record = edited.tags()[index].cue_records().unwrap()[0]
        .bytes()
        .to_vec();
    assert_eq!(record[29], 0x97);
    assert!(record.ends_with(&[0xbc; 11]));
    let details = edited.tags()[index]
        .cues()
        .unwrap()
        .entries
        .remove(0)
        .extended
        .unwrap();
    assert_eq!(details.comment.as_deref(), Some("Longer 🎧 label"));
    assert_eq!(details.color, Some([7, 1, 2, 3]));

    let beats = [
        Beat {
            number: 1,
            tempo: 12000,
            time_ms: 25,
        },
        Beat {
            number: 2,
            tempo: 12000,
            time_ms: 525,
        },
    ];
    let mut grid = AnlzTag::microsecond_grid(&beats, &[35, 35], false, 0)
        .unwrap()
        .bytes()
        .to_vec();
    grid[12..16].fill(0xd1);
    grid.extend([0xde, 0xad]);
    let len = grid.len() as u32;
    grid[8..12].copy_from_slice(&len.to_be_bytes());
    let old_grid = AnlzTag::parse(&grid, ParseLimits::default()).unwrap();
    let edited = old_grid
        .with_microsecond_grid(&beats, &[100, 200], true, 0)
        .unwrap();
    assert_eq!(edited.extended_grid().unwrap().flags, 0x02000002);
    assert_eq!(edited.extended_grid().unwrap().words, [100, 200]);
    assert_eq!(&edited.bytes()[12..16], &[0xd1; 4]);
    assert!(edited.bytes().ends_with(&[0xde, 0xad]));
    let mut unknown = grid;
    unknown[16..20].copy_from_slice(&7u32.to_be_bytes());
    assert!(
        AnlzTag::parse(&unknown, ParseLimits::default())
            .unwrap()
            .with_microsecond_grid(&beats, &[0, 0], false, 0)
            .is_err()
    );
}

#[test]
fn replaying_the_same_asset_changes_is_a_noop_and_rejects_unowned_hot_slots() {
    let (source, workspace, before) = setup();
    let p = plan(source.path(), &before);
    let staged = stage_native_assets(&p, workspace.path(), &CancellationToken::default()).unwrap();
    let after = std::fs::read(staged.manifest[0].staged_path.as_ref().unwrap()).unwrap();
    std::fs::write(source.path().join(&p.edits[0].relative_path), &after).unwrap();
    let mut again = p;
    again.edits[0].expected_sha256 = Some(hash(&after));
    let file = AnlzFile::parse(&after, ParseLimits::default()).unwrap();
    if let NativeAssetContent::Analysis { edits, .. } = &mut again.edits[0].content
        && let NativeAnlzEdit::Cues {
            owned_record_sha256,
            ..
        } = &mut edits[0]
    {
        owned_record_sha256[0] = hash(file.tags()[0].cue_records().unwrap()[0].bytes());
    }
    let repeated =
        stage_native_assets(&again, workspace.path(), &CancellationToken::default()).unwrap();
    assert!(!repeated.manifest[0].changed);
    assert!(repeated.manifest[0].staged_path.is_none());
    let mut hot = cue();
    hot.slot = NativeCueSlot::Hot(0);
    let mut duplicate = hot.clone();
    duplicate.slot = NativeCueSlot::Hot(0);
    assert!(
        new_native_analysis(
            "/track.flac",
            &[],
            &[hot, duplicate],
            NativeAnalysisKind::Dat
        )
        .is_err()
    );
}
