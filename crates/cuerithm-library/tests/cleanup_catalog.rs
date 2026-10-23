#![cfg(feature = "autotag")]
use cuerithm_cleanup::{
    ArtistRules, CanonicalMapping, CleanupConfig, ContributorRole, GenreRules, MetadataField,
};
use cuerithm_library::{CleanupService, Library};
// This shared fixture also contains manual-workspace helpers.
#[allow(dead_code)]
#[path = "support/cleanup_fixture.rs"]
mod fixture;
use fixture::Fixture;
fn seed(f: &Fixture, index: usize, values: &[(MetadataField, Vec<&str>)]) {
    use cuerithm_autotag::{
        AnalysisTags, AutotagConfig, FieldChange, MatchStatus, TagInput, TagMatchOutcome,
        TagWritePlan, read_metadata,
    };
    let path = &f.paths[index];
    let cache = f.lib.data_dir().join("seed");
    let audio = f.rt.decode(path, &cache).unwrap();
    let before = read_metadata(path, &audio.info).unwrap();
    let input = TagInput {
        path: path.clone(),
        source_sha256: audio.source_sha256.clone(),
        media: audio.info.clone(),
        duration_seconds: audio.clock.duration_seconds(),
    };
    let mut cfg = AutotagConfig::default();
    cfg.options.tags.clear();
    cfg.write_energy = false;
    let outcome = TagMatchOutcome {
        status: MatchStatus::NoMatch,
        matches: vec![],
        attempted: vec![],
        warnings: vec![],
    };
    let changes = values
        .iter()
        .map(|(field, values)| FieldChange {
            field: *field,
            before: before.fields.get(field).cloned().unwrap_or_default(),
            after: values.iter().map(|s| s.to_string()).collect(),
        })
        .collect();
    let staged = TagWritePlan::build(input, &outcome, &cfg, &AnalysisTags::default())
        .unwrap()
        .with_metadata_edits(changes)
        .unwrap()
        .stage_prepared(&f.rt, &audio, &cache)
        .unwrap();
    std::fs::copy(&staged.path, path).unwrap();
    f.lib
        .enqueue(
            std::slice::from_ref(path),
            f.lib.settings().unwrap().revision,
        )
        .unwrap();
    let run = f.lib.claim_next().unwrap().unwrap();
    f.lib
        .apply_analysis_for_run(&run, &fixture::result(), &f.rt.probe(path).unwrap())
        .unwrap();
}
#[test]
fn catalogs_preserve_contributor_boundaries_and_count_unique_tracks_and_roles() {
    let f = Fixture::new(2);
    seed(
        &f,
        0,
        &[
            (MetadataField::Artist, vec!["Alias", "Earth, Wind & Fire"]),
            (MetadataField::Producer, vec!["Alias"]),
            (MetadataField::Genre, vec!["Deep House", "House"]),
        ],
    );
    seed(
        &f,
        1,
        &[
            (MetadataField::Artist, vec!["Alias"]),
            (MetadataField::Genre, vec!["House"]),
        ],
    );
    let config = CleanupConfig {
        genres: GenreRules {
            enabled: true,
            protected_values: ["House".into()].into(),
            ..Default::default()
        },
        artists: ArtistRules {
            enabled: true,
            roles: [ContributorRole::Artist, ContributorRole::Producer].into(),
            mappings: vec![CanonicalMapping {
                from: "Alias".into(),
                to: Some("Canonical".into()),
            }],
            protected_values: ["Earth, Wind & Fire".into()].into(),
        },
        ..Default::default()
    };
    let before: Vec<_> = f
        .ids
        .iter()
        .map(|id| f.lib.cue_revision_count(id).unwrap())
        .collect();
    let catalog = f.lib.cleanup_catalog(&config).unwrap();
    assert!(catalog.unavailable_tracks.is_empty());
    assert_eq!(
        catalog
            .genres
            .iter()
            .find(|v| v.value == "House")
            .unwrap()
            .track_count,
        2
    );
    assert!(
        catalog
            .genres
            .iter()
            .find(|v| v.value == "House")
            .unwrap()
            .protected
    );
    assert_eq!(
        catalog
            .genres
            .iter()
            .find(|v| v.value == "Deep House")
            .unwrap()
            .track_count,
        1
    );
    let alias = catalog.artists.iter().find(|v| v.value == "Alias").unwrap();
    assert_eq!(alias.track_count, 2);
    assert_eq!(alias.roles[&ContributorRole::Artist], 2);
    assert_eq!(alias.roles[&ContributorRole::Producer], 1);
    assert!(
        catalog
            .artists
            .iter()
            .find(|v| v.value == "Earth, Wind & Fire")
            .unwrap()
            .protected
    );
    assert!(!catalog.artists.iter().any(|v| v.value == "Earth"));
    for (i, id) in f.ids.iter().enumerate() {
        assert_eq!(f.lib.cue_revision_count(id).unwrap(), before[i]);
    }
    let count: i64 = rusqlite::Connection::open(&f.db)
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM tracks WHERE native_metadata_json IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 2);
}
#[test]
fn missing_legacy_sources_are_reported_without_inventing_catalog_values() {
    let f = Fixture::new(1);
    std::fs::remove_file(&f.paths[0]).unwrap();
    let catalog = f.lib.cleanup_catalog(&CleanupConfig::default()).unwrap();
    assert_eq!(catalog.unavailable_tracks.len(), 1);
    assert!(catalog.genres.is_empty());
    assert!(catalog.artists.is_empty());
    assert_eq!(f.lib.cue_revision_count(&f.ids[0]).unwrap(), 1);
}

fn import_unresolved(f: &Fixture, id: &str) {
    use cuerithm_library::{
        AnalysisStatus, EncodedPath, PathFlavor, PortableLocator, PortableTrack,
    };
    f.lib
        .import_portable_tracks(&[PortableTrack {
            id: id.into(),
            locator: PortableLocator {
                id: format!("locator:{id}"),
                flavor: PathFlavor::WindowsDrive,
                original: EncodedPath::utf8(format!("D:/Music/{id}.flac")),
                source_root: None,
                components: None,
            },
            source_sha256: None,
            title: id.into(),
            artist: "Unavailable Artist".into(),
            album: String::new(),
            genre: "Unavailable Genre".into(),
            metadata: Default::default(),
            key: None,
            bpm: None,
            energy: None,
            duration: None,
            archived: false,
            origin: "lexicon".into(),
            analysis_status: AnalysisStatus::Imported,
            musical: None,
        }])
        .unwrap();
    rusqlite::Connection::open(&f.db).unwrap().execute("UPDATE tracks SET native_metadata_json=?1 WHERE id=?2",rusqlite::params![serde_json::json!({"fields":{"artist":["Unavailable Artist"],"genre":["Unavailable Genre"]},"custom_tags":[]}).to_string(),id]).unwrap();
}
#[test]
fn catalog_reports_unmapped_and_unknown_hash_tracks_without_losing_mapped_values() {
    let f = Fixture::new(1);
    seed(
        &f,
        0,
        &[
            (MetadataField::Artist, vec!["Mapped Artist"]),
            (MetadataField::Genre, vec!["Mapped Genre"]),
        ],
    );
    import_unresolved(&f, "unmapped");
    import_unresolved(&f, "unknown-hash");
    let unknown_path = f.dir.path().join("unknown-hash.flac");
    std::fs::copy(&f.paths[0], &unknown_path).unwrap();
    f.lib
        .bind_portable_track("unknown-hash", &unknown_path, None)
        .unwrap();
    let hash = cuerithm_media::file_sha256(&f.paths[0]).unwrap();
    let jobs = f.lib.jobs().unwrap().len();
    let catalog = f.lib.cleanup_catalog(&CleanupConfig::default()).unwrap();
    assert_eq!(catalog.unavailable_tracks.len(), 2);
    assert!(catalog.unavailable_tracks.iter().any(|failure| failure.track_id == "unmapped" && failure.message.contains("not linked")));
    assert!(
        catalog
            .unavailable_tracks
            .iter()
            .any(|failure| failure.track_id == "unknown-hash"
                && failure.message.contains("hash is unknown"))
    );
    assert_eq!(catalog.genres.len(), 1);
    assert_eq!(catalog.genres[0].value, "Mapped Genre");
    assert_eq!(catalog.genres[0].track_count, 1);
    assert_eq!(
        catalog
            .artists
            .iter()
            .find(|artist| artist.value == "Mapped Artist")
            .unwrap()
            .track_count,
        1
    );
    assert!(
        !catalog
            .artists
            .iter()
            .any(|artist| artist.value == "Unavailable Artist")
    );
    assert_eq!(f.lib.jobs().unwrap().len(), jobs);
    assert_eq!(cuerithm_media::file_sha256(&f.paths[0]).unwrap(), hash);
    assert!(f.lib.local_track_binding("unmapped").unwrap().is_none());
}
#[test]
fn cleanup_preview_reports_nullable_bindings_per_track_and_keeps_mapped_proposals() {
    let f = Fixture::new(1);
    import_unresolved(&f, "unmapped");
    import_unresolved(&f, "unknown-hash");
    let unknown_path = f.dir.path().join("unknown-hash.flac");
    std::fs::copy(&f.paths[0], &unknown_path).unwrap();
    f.lib
        .bind_portable_track("unknown-hash", &unknown_path, None)
        .unwrap();
    let hash = cuerithm_media::file_sha256(&f.paths[0]).unwrap();
    let preview = f
        .service()
        .preview_cleanup(cuerithm_library::CleanupScope::AllReady, fixture::config())
        .unwrap();
    assert_eq!(preview.failed.len(), 2);
    assert!(preview.failed.iter().any(|failure| failure.track_id == "unmapped" && failure.message.contains("not linked")));
    assert!(
        preview
            .failed
            .iter()
            .any(|failure| failure.track_id == "unknown-hash"
                && failure.message.contains("hash is unknown"))
    );
    assert!(
        preview
            .proposals
            .iter()
            .any(|proposal| proposal.track_id == f.ids[0])
    );
    assert_eq!(cuerithm_media::file_sha256(&f.paths[0]).unwrap(), hash);
}

#[test]
fn restored_mapped_catalog_prefers_live_canonical_values_over_old_media_and_cached_tags() {
    let f = Fixture::new(1);
    seed(
        &f,
        0,
        &[
            (MetadataField::Artist, vec!["Old Artist"]),
            (MetadataField::Genre, vec!["Old Genre"]),
        ],
    );
    import_unresolved(&f, "unmapped");
    let limits = cuerithm_library::backup::ArchiveLimits::default();
    let archive = f
        .lib
        .portable_snapshot_spooled(&limits, &|_, _| Ok(()))
        .unwrap();
    let restored_db = f.dir.path().join("restored/library.sqlite");
    let restored = Library::open(&restored_db).unwrap();
    restored
        .restore_spooled(&archive, &limits, &|_, _| Ok(()))
        .unwrap();
    restored
        .bind_portable_track(&f.ids[0], &f.paths[0], None)
        .unwrap();
    let first = restored.cleanup_catalog(&CleanupConfig::default()).unwrap();
    assert!(first.genres.iter().any(|genre| genre.value == "Old Genre"));
    let before = cuerithm_media::file_sha256(&f.paths[0]).unwrap();
    let conn = rusqlite::Connection::open(&restored_db).unwrap();
    conn.execute("UPDATE tracks SET metadata_json=json_set(metadata_json,'$.artist',json(?1),'$.genre',json(?2)),native_metadata_json=?3 WHERE id=?4",rusqlite::params![serde_json::json!(["Edited Artist, With Comma"]).to_string(),serde_json::json!(["Edited Genre"]).to_string(),serde_json::json!({"fields":{"artist":["Old Artist"],"genre":["Old Genre"]},"custom_tags":[]}).to_string(),f.ids[0]]).unwrap();
    drop(conn);
    let catalog = restored.cleanup_catalog(&CleanupConfig::default()).unwrap();
    assert_eq!(catalog.unavailable_tracks.len(), 1);
    assert_eq!(catalog.unavailable_tracks[0].track_id, "unmapped");
    assert!(
        catalog
            .genres
            .iter()
            .any(|genre| genre.value == "Edited Genre" && genre.track_count == 1)
    );
    assert!(
        !catalog
            .genres
            .iter()
            .any(|genre| genre.value == "Old Genre")
    );
    assert!(
        catalog
            .artists
            .iter()
            .any(|artist| artist.value == "Edited Artist, With Comma" && artist.track_count == 1)
    );
    assert!(
        !catalog
            .artists
            .iter()
            .any(|artist| artist.value == "Old Artist")
    );
    rusqlite::Connection::open(&restored_db).unwrap().execute("UPDATE tracks SET metadata_json=json_set(metadata_json,'$.artist',json('[]'),'$.genre',json('[]')) WHERE id=?1",[&f.ids[0]]).unwrap();
    let cleared = restored.cleanup_catalog(&CleanupConfig::default()).unwrap();
    assert!(cleared.genres.is_empty());
    assert!(!cleared.artists.iter().any(|artist| matches!(
        artist.value.as_str(),
        "Old Artist" | "Edited Artist, With Comma"
    )));
    assert_eq!(cuerithm_media::file_sha256(&f.paths[0]).unwrap(), before);
    assert!(restored.jobs().unwrap().is_empty());
}
