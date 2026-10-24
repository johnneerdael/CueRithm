use cuerithm_dj_sync::import::*;
use cuerithm_library::*;
use cuerithm_media::CancellationToken;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path};

#[derive(Default)]
struct Observations(Vec<ObservationRecord>);
impl ObservationSink for Observations {
    fn push(&mut self, record: ObservationRecord) -> anyhow::Result<()> {
        self.0.push(record);
        Ok(())
    }
    fn put_blob(&mut self, bytes: &[u8], media_type: Option<&str>) -> anyhow::Result<BlobRef> {
        Ok(BlobRef {
            sha256: format!("{:x}", Sha256::digest(bytes)),
            bytes: bytes.len() as u64,
            media_type: media_type.map(str::to_owned),
        })
    }
}
fn hashes(root: &Path, artifacts: &[ArtifactRequest]) -> BTreeMap<String, String> {
    artifacts
        .iter()
        .map(|a| {
            (
                a.id.clone(),
                format!(
                    "{:x}",
                    Sha256::digest(std::fs::read(root.join(&a.relative_path)).unwrap())
                ),
            )
        })
        .collect()
}

#[test]
fn all_off_roundtrip_is_lossless_and_has_zero_processing() {
    let fixture = tempfile::tempdir().unwrap();
    let archive = std::fs::File::open(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/fixtures/import-native-sources.tar.gz"),
    )
    .expect("Generate the authored native source fixtures");
    tar::Archive::new(flate2::read::GzDecoder::new(archive))
        .unpack(fixture.path())
        .unwrap();
    for name in [
        "rekordbox",
        "serato3",
        "serato4",
        "engine_dj",
        "traktor",
        "virtual_dj",
        "djay_pro",
        "apple_music_xml",
        "pioneer_usb",
        "m3u",
        "mixed_in_key11",
        "dj_studio",
        "mixxx",
    ] {
        let provider: SourceProvider = serde_json::from_value(serde_json::json!(name)).unwrap();
        let case = fixture.path().join(name);
        let root = case.join("source");
        let artifacts: Vec<ArtifactRequest> =
            serde_json::from_slice(&std::fs::read(case.join("selection.json")).unwrap()).unwrap();
        let before = hashes(&root, &artifacts);
        let direct = (provider == SourceProvider::Rekordbox).then(|| {
            cuerithm_dj_sync::import::readers::rekordbox::RekordboxReader::with_provisioned_key(
                cuerithm_rekordbox::DatabaseKey::new(b"generated collection key").unwrap(),
                Default::default(),
            )
        });
        let reader: &dyn SourceAdapter = direct
            .as_ref()
            .map(|r| r as &dyn SourceAdapter)
            .unwrap_or_else(|| adapter(provider));
        let request =
            SourceCaptureRequest::from_selected_root(provider, &root, artifacts.clone()).unwrap();
        let snapshot = reader
            .capture(
                &request,
                fixture.path(),
                &ImportLimits::default(),
                &CancellationToken::default(),
            )
            .unwrap();
        let mut expected = Observations::default();
        reader
            .read(
                &snapshot,
                &ReadOptions::default(),
                &mut expected,
                &CancellationToken::default(),
            )
            .unwrap();
        let db = tempfile::tempdir().unwrap();
        let library = Library::open(&db.path().join("library.db")).unwrap();
        let request = ImportInspectRequest {
            provider,
            selection: ImportSourceSelection {
                folder_inventory: false,
                root: root.clone(),
                artifacts,
            },
            binding: SourceBindingChoice::Automatic,
            origin: None,
            path_mappings: vec![],
            capture_file_tags: false,
            include_history: false,
            include_queue: false,
        };
        let inspection = if let Some(reader) = direct {
            library.inspect_rekordbox_import(request, reader, &CancellationToken::default())
        } else {
            library.inspect_import(request, &CancellationToken::default())
        }
        .unwrap();
        assert!(
            inspection.profile.fields.values().all(|field| !field.write),
            "{name}: authored fixtures must not promote native writers"
        );
        println!(
            "AUTHORED_IMPORT_PROFILE {}",
            serde_json::json!({"provider":provider,"profile_id":inspection.profile.profile_id,"schema_fingerprint":inspection.profile.schema_fingerprint,"fields":inspection.profile.fields,"tracks":inspection.counts.tracks,"collections":inspection.counts.collections,"memberships":inspection.counts.memberships,"native_app_acceptance":"unrun","native_clock_promotion":false})
        );
        let mut observed = Vec::new();
        for kind in [
            ImportRecordKind::Track,
            ImportRecordKind::Collection,
            ImportRecordKind::Membership,
            ImportRecordKind::Issue,
        ] {
            let mut cursor = None;
            loop {
                let page = library
                    .import_page(
                        &inspection.inspection_id,
                        ImportPageQuery {
                            kind,
                            limit: 1000,
                            cursor,
                        },
                    )
                    .unwrap();
                observed.extend(page.records);
                cursor = page.next_cursor;
                if cursor.is_none() {
                    break;
                }
            }
        }
        let mut expected = expected
            .0
            .into_iter()
            .map(|r| serde_json::to_string(&r).unwrap())
            .collect::<Vec<_>>();
        let mut observed = observed
            .into_iter()
            .map(|r| serde_json::to_string(&r).unwrap())
            .collect::<Vec<_>>();
        expected.sort();
        observed.sort();
        assert_eq!(observed, expected, "{name}: immutable native observations");
        let preview = library
            .preview_import(
                ImportPreviewRequest {
                    inspection_id: inspection.inspection_id.clone(),
                    ..Default::default()
                },
                &CancellationToken::default(),
            )
            .unwrap();
        assert!(!preview.processing_requested, "{name}");
        let result = library
            .apply_import(&preview.preview_id, &CancellationToken::default())
            .unwrap();
        assert!(!result.processing_requested, "{name}");
        assert_eq!(result.tracks_created, inspection.counts.tracks, "{name}");
        assert!(library.jobs().unwrap().is_empty(), "{name}");
        assert!(
            library
                .import_processing_stages(&result.operation_id, None, 1000)
                .unwrap()
                .records
                .is_empty(),
            "{name}"
        );
        for track in library.tracks(TrackQuery::default()).unwrap().rows {
            assert!(library.analysis(&track.id).unwrap().is_none(), "{name}");
        }
        assert_eq!(
            hashes(&root, &request_artifacts(&case)),
            before,
            "{name}: source bytes unchanged"
        );
    }
}
fn request_artifacts(case: &Path) -> Vec<ArtifactRequest> {
    serde_json::from_slice(&std::fs::read(case.join("selection.json")).unwrap()).unwrap()
}
#[test]
fn applied_collections_remain_browsable_with_duplicate_occurrences() {
    let root = tempfile::tempdir().unwrap();
    let db = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("set.m3u8"), "A.flac\nB.flac\nA.flac\n").unwrap();
    let library = Library::open(&db.path().join("library.db")).unwrap();
    let inspection = library
        .inspect_import(
            ImportInspectRequest {
                provider: SourceProvider::M3u,
                selection: ImportSourceSelection {
                    folder_inventory: false,
                    root: root.path().to_owned(),
                    artifacts: vec![ArtifactRequest::stable("set.m3u8")],
                },
                binding: SourceBindingChoice::Automatic,
                origin: None,
                path_mappings: vec![],
                capture_file_tags: false,
                include_history: false,
                include_queue: false,
            },
            &CancellationToken::default(),
        )
        .unwrap();
    let preview = library
        .preview_import(
            ImportPreviewRequest {
                inspection_id: inspection.inspection_id,
                ..Default::default()
            },
            &CancellationToken::default(),
        )
        .unwrap();
    library
        .apply_import(&preview.preview_id, &CancellationToken::default())
        .unwrap();
    drop(library);
    let library = Library::open(&db.path().join("library.db")).unwrap();
    let collections = library
        .imported_collection_list(ImportCollectionListQuery::default())
        .unwrap();
    assert_eq!(collections.records.len(), 1);
    assert_eq!(collections.records[0].name, "set");
    assert_eq!(collections.records[0].occurrences, 3);
    let page = library
        .imported_collection_page(&collections.records[0].id, None, 100)
        .unwrap();
    assert_eq!(page.rows.len(), 3);
    assert_eq!(page.rows[0].item_id, page.rows[2].item_id);
    assert_ne!(page.rows[0].occurrence_id, page.rows[2].occurrence_id);
    assert!(
        page.rows
            .iter()
            .all(|row| row.track.as_ref().is_some_and(|track| track
                .import_state
                .as_ref()
                .is_some_and(|state| !state.playback_allowed)))
    );
}
#[test]
fn media_loading_preserves_imported_fields() {
    let root = tempfile::tempdir().unwrap();
    let db = tempfile::tempdir().unwrap();
    let archive = std::fs::File::open(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/fixtures/import-publication-media.tar.gz"),
    )
    .unwrap();
    tar::Archive::new(flate2::read::GzDecoder::new(archive))
        .unpack(root.path())
        .unwrap();
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.path().join("manifest.json")).unwrap()).unwrap();
    let cases = manifest["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 10);
    let playlist = cases
        .iter()
        .map(|case| case["file"].as_str().unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(root.path().join("media.m3u8"), playlist).unwrap();
    let library = Library::open(&db.path().join("library.db")).unwrap();
    let inspection = library
        .inspect_import(
            ImportInspectRequest {
                provider: SourceProvider::M3u,
                selection: ImportSourceSelection {
                    folder_inventory: false,
                    root: root.path().to_owned(),
                    artifacts: vec![ArtifactRequest::stable("media.m3u8")],
                },
                binding: SourceBindingChoice::Automatic,
                origin: None,
                path_mappings: vec![],
                capture_file_tags: true,
                include_history: false,
                include_queue: false,
            },
            &CancellationToken::default(),
        )
        .unwrap();
    let preview = library
        .preview_import(
            ImportPreviewRequest {
                inspection_id: inspection.inspection_id,
                policy: ImportPolicy {
                    capture_file_tags: true,
                    ..Default::default()
                },
                ..Default::default()
            },
            &CancellationToken::default(),
        )
        .unwrap();
    library
        .apply_import(&preview.preview_id, &CancellationToken::default())
        .unwrap();
    let runtime = cuerithm_media::MediaRuntime::from_bundle(&std::path::PathBuf::from(
        std::env::var_os("CUERITHM_MEDIA_RUNTIME").unwrap(),
    ))
    .unwrap();
    let rows = library.tracks(TrackQuery::default()).unwrap().rows;
    assert_eq!(rows.len(), 10);
    for row in rows {
        let before = library.imported_item(&row.id).unwrap();
        assert!(before.playback_allowed);
        let audio = runtime
            .decode(
                &library.track_path(&row.id).unwrap(),
                &db.path().join("playback-cache"),
            )
            .unwrap();
        let waveform = cuerithm_library::waveform::waveform(&audio, 64).unwrap();
        assert_eq!(waveform.bins.len(), 64);
        let after = library.imported_item(&row.id).unwrap();
        assert_eq!(after.active_fields, before.active_fields);
        assert_eq!(after.provenance, before.provenance);
        assert!(library.analysis(&row.id).unwrap().is_none());
    }
    assert!(library.jobs().unwrap().is_empty());
}

#[test]
fn refresh_retains_unselected_and_local_only_changes() {
    let first = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let db = tempfile::tempdir().unwrap();
    let library = Library::open(&db.path().join("library.db")).unwrap();
    let write = |root: &Path, title: &str, artist: &str| {
        std::fs::write(root.join("collection.nml"),format!(r#"<NML VERSION="19"><HEAD PROGRAM="Traktor" VERSION="4.1.0"/><COLLECTION><ENTRY TITLE="{title}" ARTIST="{artist}"><LOCATION VOLUME="D:" DIR="/:Music/:" FILE="Authored.flac"/></ENTRY></COLLECTION></NML>"#)).unwrap()
    };
    let inspect = |root: &Path| {
        library
            .inspect_import(
                ImportInspectRequest {
                    provider: SourceProvider::Traktor,
                    selection: ImportSourceSelection {
                        folder_inventory: false,
                        root: root.to_owned(),
                        artifacts: vec![ArtifactRequest::stable("collection.nml")],
                    },
                    binding: SourceBindingChoice::NewLibrary,
                    origin: None,
                    path_mappings: vec![],
                    capture_file_tags: false,
                    include_history: false,
                    include_queue: false,
                },
                &CancellationToken::default(),
            )
            .unwrap()
    };
    write(first.path(), "Original title", "Original artist");
    let source = inspect(first.path());
    let preview = library
        .preview_import(
            ImportPreviewRequest {
                inspection_id: source.inspection_id,
                ..Default::default()
            },
            &CancellationToken::default(),
        )
        .unwrap();
    library
        .apply_import(&preview.preview_id, &CancellationToken::default())
        .unwrap();
    let id = library.tracks(TrackQuery::default()).unwrap().rows[0]
        .id
        .clone();
    write(other.path(), "Local title", "Original artist");
    let second = inspect(other.path());
    let page = library
        .import_page(&second.inspection_id, ImportPageQuery::default())
        .unwrap();
    let ObservationRecord::Track(track) = &page.records[0] else {
        panic!("expected native track")
    };
    let preview = library
        .preview_import(
            ImportPreviewRequest {
                inspection_id: second.inspection_id,
                target_items: BTreeMap::from([(track.external_id.clone(), id.clone())]),
                decisions: vec![ImportConflictDecision {
                    external_id: track.external_id.clone(),
                    field: "metadata.title".into(),
                    decision: ImportDecision::UseSource,
                }],
                ..Default::default()
            },
            &CancellationToken::default(),
        )
        .unwrap();
    library
        .apply_import(&preview.preview_id, &CancellationToken::default())
        .unwrap();
    write(first.path(), "Original title", "Changed unselected artist");
    let refresh = library
        .inspect_import_refresh(&source.source_id, &CancellationToken::default())
        .unwrap();
    let preview = library
        .preview_import(
            ImportPreviewRequest {
                inspection_id: refresh.inspection_id,
                policy: ImportPolicy {
                    descriptive_fields: Some(["title".into()].into()),
                    ..Default::default()
                },
                ..Default::default()
            },
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(preview.fields_changed, 0);
    assert_eq!(preview.conflicts, 0);
    library
        .apply_import(&preview.preview_id, &CancellationToken::default())
        .unwrap();
    let row = library.track(&id).unwrap();
    assert_eq!(row.title, "Local title");
    assert_eq!(row.artist, "Original artist");
    assert!(library.jobs().unwrap().is_empty());
}
