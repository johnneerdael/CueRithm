#![cfg(feature = "autotag")]
use cuerithm_cleanup::{Metadata, MetadataField};
use cuerithm_library::*;
use rusqlite::params;
#[path = "support/cleanup_fixture.rs"]
#[allow(dead_code)]
mod fixture;

// Explicit synthetic library facts; numerical audio prediction is not under test.
fn prepared() -> fixture::Fixture {
    let f = fixture::Fixture::new(4);
    let conn = rusqlite::Connection::open(&f.db).unwrap();
    for (i, (genre, bpm, energy, year)) in [
        ("House", 128., 4, 2020),
        ("House", 139.995, 6, 2021),
        ("Techno", 130., 4, 2020),
        ("Techno", 145., 6, 1999),
    ]
    .into_iter()
    .enumerate()
    {
        let title = format!("Track {i}");
        let artist = if i % 2 == 0 {
            "Earth, Wind & Fire"
        } else {
            "Artist Two"
        };
        let mut analysis = f.lib.analysis(&f.ids[i]).unwrap().unwrap();
        analysis.analysis.tempo.as_mut().unwrap().bpm = bpm;
        analysis.analysis.energy.as_mut().unwrap().level = energy;
        analysis.analysis.duration_seconds = [119.5, 240., 480., 700.][i];
        let mut info = f.rt.probe(&f.paths[i]).unwrap();
        info.tags = std::collections::BTreeMap::from([
            ("title".into(), title.clone()),
            ("artist".into(), artist.into()),
            ("genre".into(), genre.into()),
            ("date".into(), year.to_string()),
        ]);
        info.properties.insert(
            "audio.bit_rate".into(),
            [127500, 192000, 320000, 1411000][i].to_string().into(),
        );
        info.properties.insert(
            "format.size".into(),
            [1000000, 10485760, 26214400, 60000000][i]
                .to_string()
                .into(),
        );
        let mut native = Metadata::default();
        native.set(MetadataField::Title, vec![title.clone()]);
        native.set(MetadataField::Artist, vec![artist.into()]);
        native.set(MetadataField::Genre, vec![genre.into()]);
        conn.execute("UPDATE tracks SET title=?1,sort_title=lower(?1),artist=?2,album='',genre=?3,analysis_json=?4,metadata_json=?5,native_metadata_json=?6 WHERE id=?7",params![title,artist,genre,serde_json::to_string(&analysis).unwrap(),serde_json::to_string(&info).unwrap(),serde_json::to_string(&native).unwrap(),f.ids[i]]).unwrap();
    }
    f
}
fn request(dimensions: Vec<GenerationDimension>, combine: bool) -> SmartGenerationRequest {
    SmartGenerationRequest {
        target: GenerationTarget::NewFolder {
            parent_id: None,
            name: "Generated".into(),
        },
        dimensions,
        combine,
    }
}
fn field(name: &str) -> GenerationDimension {
    GenerationDimension::Field {
        field: name.into(),
        include_unobserved: false,
    }
}

#[test]
fn observed_values_create_dynamic_definitions_and_repeat_without_overwriting() {
    let f = prepared();
    let bytes = f
        .paths
        .iter()
        .map(|p| std::fs::read(p).unwrap())
        .collect::<Vec<_>>();
    let before = f
        .ids
        .iter()
        .map(|id| serde_json::to_value(f.lib.analysis(id).unwrap()).unwrap())
        .collect::<Vec<_>>();
    let preview = f
        .lib
        .preview_smart_generation(request(vec![field("genre")], false))
        .unwrap();
    assert_eq!(
        preview
            .items
            .iter()
            .map(|i| (i.name.as_str(), i.track_count))
            .collect::<Vec<_>>(),
        vec![("House", 2), ("Techno", 2)]
    );
    assert_eq!(preview.new_nodes, 3);
    let result = f
        .lib
        .apply_smart_generation(&preview.id, preview.stamp.playlist_revision)
        .unwrap();
    f.lib.refresh_smart_lists(1_792_500_000_000).unwrap();
    for item in &preview.items {
        assert_eq!(
            f.lib.playlist_members(&item.id).unwrap().len(),
            item.track_count
        );
    }
    let again = f
        .lib
        .preview_smart_generation(SmartGenerationRequest {
            target: GenerationTarget::Folder {
                id: preview.folder_id.unwrap(),
            },
            dimensions: vec![field("genre")],
            combine: false,
        })
        .unwrap();
    assert_eq!(again.new_nodes, 0);
    assert!(again.items.iter().all(|i| !i.is_new));
    let repeated = f
        .lib
        .apply_smart_generation(&again.id, again.stamp.playlist_revision)
        .unwrap();
    assert!(repeated.edit_id.is_none());
    assert_eq!(f.lib.playlist_state().unwrap().revision, result.revision);
    for (i, id) in f.ids.iter().enumerate() {
        assert_eq!(std::fs::read(&f.paths[i]).unwrap(), bytes[i]);
        assert_eq!(
            serde_json::to_value(f.lib.analysis(id).unwrap()).unwrap(),
            before[i]
        );
        assert_eq!(f.lib.cue_revision_count(id).unwrap(), 1);
    }
}

#[test]
fn cartesian_and_empty_dimensions_keep_source_combination_behavior() {
    let f = prepared();
    let preview = f
        .lib
        .preview_smart_generation(request(vec![field("genre"), field("energy")], true))
        .unwrap();
    assert_eq!(
        preview
            .items
            .iter()
            .map(|i| (i.name.as_str(), i.track_count))
            .collect::<Vec<_>>(),
        vec![
            ("House 04 Energy", 1),
            ("House 06 Energy", 1),
            ("Techno 04 Energy", 1),
            ("Techno 06 Energy", 1)
        ]
    );
    let partial = f
        .lib
        .preview_smart_generation(request(vec![field("album"), field("genre")], true))
        .unwrap();
    assert_eq!(partial.items.len(), 2);
    let empty = f
        .lib
        .preview_smart_generation(request(vec![field("album")], false))
        .unwrap();
    assert!(empty.items.is_empty());
    assert_eq!(empty.new_nodes, 0);
    assert!(empty.folder_id.is_none());
}

#[test]
fn fractional_bpm_boundaries_cover_every_observed_track_at_comparison_precision() {
    let f = prepared();
    let preview = f
        .lib
        .preview_smart_generation(request(vec![GenerationDimension::BpmRanges], false))
        .unwrap();
    let result = f
        .lib
        .apply_smart_generation(&preview.id, preview.stamp.playlist_revision)
        .unwrap();
    f.lib.refresh_smart_lists(1_792_500_000_000).unwrap();
    let mut found = std::collections::BTreeSet::new();
    for item in &preview.items {
        found.extend(f.lib.playlist_members(&item.id).unwrap());
    }
    assert_eq!(
        found,
        f.ids
            .iter()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>()
    );
    f.lib
        .undo_playlist_edit(result.edit_id.as_ref().unwrap(), result.revision)
        .unwrap();
}

#[test]
fn generation_previews_are_guarded_and_mid_transaction_failure_leaves_no_partial_tree() {
    let f = prepared();
    let req = || request(vec![field("genre"), field("energy")], true);
    let old = f.lib.preview_smart_generation(req()).unwrap();
    let conn = rusqlite::Connection::open(&f.db).unwrap();
    conn.execute(
        "UPDATE tracks SET title='New title' WHERE id=?1",
        [&f.ids[0]],
    )
    .unwrap();
    assert!(
        f.lib
            .apply_smart_generation(&old.id, old.stamp.playlist_revision)
            .is_err()
    );
    let preview = f.lib.preview_smart_generation(req()).unwrap();
    conn.execute_batch("CREATE TABLE injected_counter(n INTEGER);INSERT INTO injected_counter VALUES(0);CREATE TRIGGER fail_generation BEFORE INSERT ON playlists BEGIN UPDATE injected_counter SET n=n+1;SELECT CASE WHEN (SELECT n FROM injected_counter)=3 THEN RAISE(ABORT,'injected') END;END;").unwrap();
    let before = f.lib.playlist_state().unwrap();
    assert!(
        f.lib
            .apply_smart_generation(&preview.id, preview.stamp.playlist_revision)
            .is_err()
    );
    assert_eq!(f.lib.playlist_state().unwrap(), before);
    assert_eq!(
        conn.query_row("SELECT n FROM injected_counter", [], |r| r.get::<_, u64>(0))
            .unwrap(),
        0
    );
    conn.execute_batch("DROP TRIGGER fail_generation").unwrap();
    f.lib
        .apply_smart_generation(&preview.id, preview.stamp.playlist_revision)
        .unwrap();
    assert!(
        f.lib
            .apply_smart_generation(&preview.id, preview.stamp.playlist_revision)
            .is_err()
    );
}

fn tags(f: &fixture::Fixture, count: usize) {
    let mut conn = rusqlite::Connection::open(&f.db).unwrap();
    let tx = conn.transaction().unwrap();
    tx.execute(
        "INSERT INTO custom_tag_categories VALUES('mood','Mood')",
        [],
    )
    .unwrap();
    for i in 0..count {
        tx.execute(
            "INSERT INTO custom_tags VALUES(?1,'mood',?2)",
            params![format!("tag-{i}"), format!("Tag {i}")],
        )
        .unwrap();
    }
    tx.commit().unwrap();
}
#[test]
fn tag_and_category_generation_distinguish_observation_from_explicit_known_taxonomy() {
    let f = prepared();
    tags(&f, 3);
    let conn = rusqlite::Connection::open(&f.db).unwrap();
    for (i, tag) in [(0, "tag-0"), (2, "tag-0"), (1, "tag-1")] {
        conn.execute(
            "INSERT INTO track_custom_tags VALUES(?1,?2)",
            params![f.ids[i], tag],
        )
        .unwrap();
    }
    let observed = f
        .lib
        .preview_smart_generation(request(
            vec![GenerationDimension::Tags { tag_ids: None }],
            false,
        ))
        .unwrap();
    assert_eq!(
        observed
            .items
            .iter()
            .map(|i| (i.name.as_str(), i.track_count))
            .collect::<Vec<_>>(),
        vec![("Tag 0", 2), ("Tag 1", 1)]
    );
    let category = f
        .lib
        .preview_smart_generation(request(
            vec![GenerationDimension::Category {
                category_id: "mood".into(),
            }],
            false,
        ))
        .unwrap();
    assert_eq!(category.items.len(), 3);
    assert_eq!(category.items[2].track_count, 0);
    assert!(
        f.lib
            .preview_smart_generation(request(
                vec![GenerationDimension::Tags {
                    tag_ids: Some(vec!["deleted".into()])
                }],
                false
            ))
            .is_err()
    );
}
#[test]
fn full_domains_are_explicit_and_separate_dimensions_have_separate_folders() {
    let f = prepared();
    let all = f
        .lib
        .preview_smart_generation(request(
            vec![GenerationDimension::Field {
                field: "energy".into(),
                include_unobserved: true,
            }],
            false,
        ))
        .unwrap();
    assert_eq!(all.items.len(), 10);
    assert_eq!(all.items[0].name, "01 Energy");
    assert_eq!(all.items[9].name, "10 Energy");
    assert!(all.items.iter().all(|i| i.name != "00 Energy"));
    // Star rating keeps its zero: 0 means unrated, which is a real value.
    let stars = f
        .lib
        .preview_smart_generation(request(
            vec![GenerationDimension::Field {
                field: "rating".into(),
                include_unobserved: true,
            }],
            false,
        ))
        .unwrap();
    assert_eq!(stars.items.len(), 6);
    assert_eq!(stars.items[0].name, "0");
    assert_eq!(stars.items[5].name, "5");
    assert_eq!(all.items.iter().map(|i| i.track_count).sum::<usize>(), 4);
    let separate = f
        .lib
        .preview_smart_generation(request(vec![field("genre"), field("energy")], false))
        .unwrap();
    assert_eq!(separate.new_nodes, 7);
    assert_eq!(
        separate
            .items
            .iter()
            .map(|i| &i.parent_id)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        2
    );
    for excluded in ["playCount", "lastPlayed", "danceability", "cloudUpload"] {
        assert!(
            f.lib
                .preview_smart_generation(request(vec![field(excluded)], false))
                .is_err(),
            "{excluded}"
        );
    }
}
#[test]
fn conflicting_existing_definitions_and_duplicate_field_modes_never_get_silently_replaced() {
    let f = prepared();
    assert!(
        f.lib
            .preview_smart_generation(request(
                vec![
                    field("energy"),
                    GenerationDimension::Field {
                        field: "energy".into(),
                        include_unobserved: true
                    }
                ],
                false
            ))
            .is_err()
    );
    let preview = f
        .lib
        .preview_smart_generation(request(vec![field("genre")], false))
        .unwrap();
    f.lib
        .apply_smart_generation(&preview.id, preview.stamp.playlist_revision)
        .unwrap();
    let mut changed = preview.items[0].definition.clone();
    changed.rules[0].value = cuerithm_smartlists::RuleValue::Text("Different genre".into());
    f.lib
        .apply_playlist_edit(
            PlaylistEdit::SetDefinition {
                id: preview.items[0].id.clone(),
                definition: changed.clone(),
            },
            f.lib.playlist_state().unwrap().revision,
        )
        .unwrap();
    let before = f.lib.playlist_state().unwrap();
    assert!(
        f.lib
            .preview_smart_generation(SmartGenerationRequest {
                target: GenerationTarget::Folder {
                    id: preview.folder_id.unwrap()
                },
                dimensions: vec![field("genre")],
                combine: false
            })
            .is_err()
    );
    assert_eq!(f.lib.playlist_state().unwrap(), before);
}
#[test]
fn five_thousand_nodes_are_allowed_but_folder_overhead_and_larger_products_are_not() {
    let f = prepared();
    tags(&f, 5000);
    let created = f
        .lib
        .apply_playlist_edit(
            PlaylistEdit::Create {
                id: Some("target".into()),
                parent_id: None,
                kind: PlaylistKind::Folder,
                name: "Target".into(),
                definition: None,
            },
            f.lib.playlist_state().unwrap().revision,
        )
        .unwrap();
    assert_eq!(created.created_ids, vec!["target"]);
    let ids = (0..5000).map(|i| format!("tag-{i}")).collect::<Vec<_>>();
    let preview = f
        .lib
        .preview_smart_generation(SmartGenerationRequest {
            target: GenerationTarget::Folder {
                id: "target".into(),
            },
            dimensions: vec![GenerationDimension::Tags {
                tag_ids: Some(ids.clone()),
            }],
            combine: false,
        })
        .unwrap();
    assert_eq!(preview.new_nodes, 5000);
    let result = f
        .lib
        .apply_smart_generation(&preview.id, preview.stamp.playlist_revision)
        .unwrap();
    assert_eq!(result.created_ids.len(), 5000);
    let before = f.lib.playlist_state().unwrap();
    assert!(
        f.lib
            .preview_smart_generation(request(
                vec![GenerationDimension::Tags { tag_ids: Some(ids) }],
                false
            ))
            .is_err()
    );
    assert_eq!(f.lib.playlist_state().unwrap(), before);
    let large = request(
        vec![
            GenerationDimension::Field {
                field: "energy".into(),
                include_unobserved: true,
            },
            GenerationDimension::Field {
                field: "rating".into(),
                include_unobserved: true,
            },
            GenerationDimension::Category {
                category_id: "mood".into(),
            },
        ],
        true,
    );
    assert!(f.lib.preview_smart_generation(large).is_err());
    assert_eq!(f.lib.playlist_state().unwrap(), before);
}

#[test]
fn harmless_case_changes_and_custom_rule_ids_reuse_the_existing_generated_identity() {
    let f = prepared();
    let preview = f
        .lib
        .preview_smart_generation(request(vec![field("genre")], false))
        .unwrap();
    f.lib
        .apply_smart_generation(&preview.id, preview.stamp.playlist_revision)
        .unwrap();
    let original = &preview.items[0];
    let mut definition = original.definition.clone();
    definition.rules[0].id = "user-rule-id".into();
    f.lib
        .apply_playlist_edit(
            PlaylistEdit::SetDefinition {
                id: original.id.clone(),
                definition: definition.clone(),
            },
            f.lib.playlist_state().unwrap().revision,
        )
        .unwrap();
    let conn = rusqlite::Connection::open(&f.db).unwrap();
    for id in &f.ids[..2] {
        let text: String = conn
            .query_row(
                "SELECT native_metadata_json FROM tracks WHERE id=?1",
                [id],
                |r| r.get(0),
            )
            .unwrap();
        let mut metadata: Metadata = serde_json::from_str(&text).unwrap();
        metadata.set(MetadataField::Genre, vec!["house".into()]);
        conn.execute(
            "UPDATE tracks SET native_metadata_json=?1 WHERE id=?2",
            params![serde_json::to_string(&metadata).unwrap(), id],
        )
        .unwrap();
    }
    let again = f
        .lib
        .preview_smart_generation(SmartGenerationRequest {
            target: GenerationTarget::Folder {
                id: preview.folder_id.unwrap(),
            },
            dimensions: vec![field("genre")],
            combine: false,
        })
        .unwrap();
    assert_eq!(again.new_nodes, 0);
    let item = again.items.iter().find(|i| i.id == original.id).unwrap();
    assert_eq!(item.name, original.name);
    assert_eq!(item.definition, definition);
    let result = f
        .lib
        .apply_smart_generation(&again.id, again.stamp.playlist_revision)
        .unwrap();
    assert!(result.edit_id.is_none());
}

#[test]
fn scalar_and_bucket_dimensions_count_the_actual_snapshot_without_splitting_contributors() {
    let f = prepared();
    for (dimension, expected) in [
        (
            field("artist"),
            vec![("Artist Two", 2), ("Earth, Wind & Fire", 2)],
        ),
        (field("year"), vec![("1999", 1), ("2020", 2), ("2021", 1)]),
        (field("key"), vec![("8B", 4)]),
        (
            GenerationDimension::Decades,
            vec![("1990s", 1), ("2020s", 3)],
        ),
        (
            GenerationDimension::BitrateBuckets,
            vec![("< 128 kbps", 1), ("192-256 kbps", 1), ("320 kbps+", 2)],
        ),
        (
            GenerationDimension::SizeBuckets,
            vec![
                ("< 5 MB", 1),
                ("10-25 MB", 1),
                ("25-50 MB", 1),
                ("50 MB+", 1),
            ],
        ),
        (
            GenerationDimension::DurationBuckets,
            vec![
                ("< 2 min", 1),
                ("4-6 min", 1),
                ("8-10 min", 1),
                ("10 min+", 1),
            ],
        ),
    ] {
        let preview = f
            .lib
            .preview_smart_generation(request(vec![dimension], false))
            .unwrap();
        assert_eq!(
            preview
                .items
                .iter()
                .map(|i| (i.name.as_str(), i.track_count))
                .collect::<Vec<_>>(),
            expected
        );
    }
}
