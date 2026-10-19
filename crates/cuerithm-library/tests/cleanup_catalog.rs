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
