use super::super::collections::project_import_collection_tx;
use super::super::projection::*;
use crate::*;
use cuerithm_dj_sync::import::*;
use cuerithm_media::CancellationToken;
use serde_json::json;
use std::{collections::BTreeMap, fs};

struct Reader {
    tracks: Vec<TrackObservation>,
    collections: Vec<CollectionObservation>,
}
impl SourceAdapter for Reader {
    fn provider(&self) -> SourceProvider {
        SourceProvider::Mixxx
    }
    fn inspect(&self, _: &ImportSnapshot) -> anyhow::Result<SourceProfile> {
        Ok(SourceProfile {
            provider: self.provider(),
            profile_id: "projection-fixture".into(),
            schema_fingerprint: "a".repeat(64),
            origin: BTreeMap::new(),
            fields: BTreeMap::new(),
            issues: Vec::new(),
        })
    }
    fn read(
        &self,
        snapshot: &ImportSnapshot,
        _: &ReadOptions,
        sink: &mut dyn ObservationSink,
        cancel: &CancellationToken,
    ) -> anyhow::Result<ObservationHeader> {
        for track in &self.tracks {
            cancel.check()?;
            sink.push(ObservationRecord::Track(Box::new(track.clone())))?;
        }
        for collection in &self.collections {
            sink.push(ObservationRecord::Collection(collection.clone()))?;
        }
        Ok(ObservationHeader {
            contract_version: 1,
            source: SourceKey {
                provider: self.provider(),
                library_instance_id: "projection-fixture-library".into(),
            },
            product_version: "fixture".into(),
            schema_fingerprint: "a".repeat(64),
            snapshot_hash: snapshot.snapshot_hash.clone(),
            adapter_version: "fixture".into(),
            completeness: Completeness::WholeLibrary,
            scope: json!({"kind":"all"}),
            observed_at: String::new(),
            accounting: RecordAccounting::default(),
            extensions: BTreeMap::new(),
        })
    }
}
fn inspect(
    lib: &Library,
    tracks: Vec<TrackObservation>,
    collections: Vec<CollectionObservation>,
) -> (ImportInspection, tempfile::TempDir) {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("source.bin"), b"invented source").unwrap();
    let request = ImportInspectRequest {
        provider: SourceProvider::Mixxx,
        selection: ImportSourceSelection {
            root: root.path().into(),
            artifacts: vec![ArtifactRequest::stable("source.bin")],
        },
        binding: SourceBindingChoice::Automatic,
        origin: None,
        path_mappings: Vec::new(),
        capture_file_tags: false,
        include_history: false,
        include_queue: false,
    };
    (
        lib.inspect_import_with_reader(
            request,
            &Reader {
                tracks,
                collections,
            },
            &CancellationToken::default(),
        )
        .unwrap(),
        root,
    )
}
fn observation(id: &str, title: &str) -> TrackObservation {
    TrackObservation {
        external_id: id.into(),
        locations: LocationEvidence {
            original: Some(format!("/Unavailable/{id}.flac")),
            availability: "missing".into(),
            ..Default::default()
        },
        metadata: BTreeMap::from([
            ("title".into(), Presence::Value(json!(title))),
            ("artists".into(), Presence::Value(json!(["Artist, intact"]))),
            ("album".into(), Presence::Value(json!("Source album"))),
        ]),
        key: Some(MusicalKey {
            presence: FieldPresence::Value,
            raw: Some("8A".into()),
            camelot: Some("8A".into()),
            ..Default::default()
        }),
        ..Default::default()
    }
}
fn publish(
    lib: &Library,
    inspection: &ImportInspection,
    track: &TrackObservation,
    id: &str,
    policy: &ImportPolicy,
    local: Option<&str>,
) {
    let mut conn = lib.conn().unwrap();
    let tx = conn.transaction().unwrap();
    let fields = selected_source_values(&tx, track, policy).unwrap();
    project_import_track_tx(
        &tx,
        TrackProjection {
            source_id: &inspection.source_id,
            snapshot_id: &inspection.inspection_id,
            observation: track,
            item_id: id,
            fields: &fields,
            local_track_id: local,
            resolved_path: None,
        },
    )
    .unwrap();
    Library::bump(&tx).unwrap();
    tx.commit().unwrap();
}
#[test]
fn unavailable_import_visible_without_audio_or_jobs() {
    let dir = tempfile::tempdir().unwrap();
    let lib = Library::open(&dir.path().join("library.db")).unwrap();
    let track = observation("a", "Source title");
    let (inspection, _root) = inspect(&lib, vec![track.clone()], vec![]);
    publish(
        &lib,
        &inspection,
        &track,
        "local-a",
        &ImportPolicy::default(),
        None,
    );
    let page = lib.tracks(TrackQuery::default()).unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.rows[0].title, "Source title");
    assert_eq!(page.rows[0].artist, "Artist, intact");
    assert_eq!(page.rows[0].key.as_deref(), Some("8A"));
    let state = page.rows[0].import_state.as_ref().unwrap();
    assert!(!state.playback_allowed);
    assert_eq!(state.analysis_state, "unanalysed");
    assert!(lib.analysis("local-a").unwrap().is_none());
    assert_eq!(lib.jobs().unwrap().len(), 0);
    assert_eq!(lib.cue_revision_count("local-a").unwrap(), 0);
    assert_eq!(lib.counts().unwrap().tracks, 1);
    assert_eq!(lib.counts().unwrap().incoming, 0);
    let detail = lib.imported_item("local-a").unwrap();
    assert_eq!(
        detail.observations[0].track.metadata["title"],
        Presence::Value(json!("Source title"))
    );
    assert!(!detail.playback_allowed);
    assert!(!lib.import_export_readiness("local-a").unwrap().ready);
    assert!(
        lib.dj_sync_library(DjSyncScope::Modified {
            track_ids: vec!["local-a".into()]
        })
        .unwrap_err()
        .to_string()
        .contains("imported")
    );
}
#[test]
fn unchecked_new_fields_remain_provenance_only() {
    let dir = tempfile::tempdir().unwrap();
    let lib = Library::open(&dir.path().join("library.db")).unwrap();
    let track = observation("a", "Private source title");
    let (inspection, _root) = inspect(&lib, vec![track.clone()], vec![]);
    let policy = ImportPolicy {
        selected_fields: [FieldGroup::Key].into(),
        ..Default::default()
    };
    publish(&lib, &inspection, &track, "local-a", &policy, None);
    let detail = lib.imported_item("local-a").unwrap();
    assert!(!detail.active_fields.contains_key("metadata.title"));
    assert_eq!(
        detail.observations[0].track.metadata["title"],
        Presence::Value(json!("Private source title"))
    );
    assert_ne!(lib.track("local-a").unwrap().title, "Private source title");
    assert_eq!(lib.track("local-a").unwrap().key.as_deref(), Some("8A"));
}
#[test]
fn existing_unchecked_fields_are_unchanged_and_rows_are_not_duplicated() {
    let dir = tempfile::tempdir().unwrap();
    let lib = Library::open(&dir.path().join("library.db")).unwrap();
    lib.conn().unwrap().execute("INSERT INTO tracks(id,path,title,musical_key,source_hash,state,created,updated) VALUES('existing',?1,'Local title','11A','local-hash','ready',1,1)",[crate::model::path_bytes(std::path::Path::new("/Synthetic/existing.flac"))]).unwrap();
    let track = observation("a", "Remote title");
    let (inspection, _root) = inspect(&lib, vec![track.clone()], vec![]);
    let policy = ImportPolicy {
        selected_fields: [FieldGroup::DescriptiveTags].into(),
        descriptive_fields: Some(["album".into()].into()),
        ..Default::default()
    };
    publish(
        &lib,
        &inspection,
        &track,
        "existing",
        &policy,
        Some("existing"),
    );
    let row = lib.track("existing").unwrap();
    assert_eq!(row.title, "Local title");
    assert_eq!(row.key.as_deref(), Some("11A"));
    assert_eq!(row.album, "Source album");
    assert_eq!(lib.tracks(TrackQuery::default()).unwrap().total, 1);
}
#[test]
fn imported_collections_preserve_occurrences_cue_families_and_unordered_crates() {
    let dir = tempfile::tempdir().unwrap();
    let lib = Library::open(&dir.path().join("library.db")).unwrap();
    let mut a = observation("a", "A");
    let b = observation("b", "B");
    for (id, family, slot) in [
        ("hot", CueFamily::Hot, Some(7)),
        ("memory", CueFamily::Memory, None),
    ] {
        a.cues.push(CueObservation {
            external_id: id.into(),
            family,
            source_slot: slot,
            native_start_seconds: Some(0.0),
            ..Default::default()
        });
    }
    let members = [("one", "a", 1), ("two", "b", 2), ("three", "a", 3)]
        .into_iter()
        .map(|(id, track, position)| MembershipObservation {
            external_occurrence_id: id.into(),
            track_external_id: track.into(),
            position: Some(position),
            ..Default::default()
        })
        .collect::<Vec<_>>();
    let collection = CollectionObservation {
        external_id: "p".into(),
        name: "A B A".into(),
        occurrences: members.clone(),
        ..Default::default()
    };
    let (inspection, _root) = inspect(&lib, vec![a.clone(), b.clone()], vec![collection.clone()]);
    publish(
        &lib,
        &inspection,
        &a,
        "local-a",
        &ImportPolicy::default(),
        None,
    );
    publish(
        &lib,
        &inspection,
        &b,
        "local-b",
        &ImportPolicy::default(),
        None,
    );
    {
        let conn = lib.conn().unwrap();
        project_import_collection_tx(
            &conn,
            &inspection.source_id,
            "collection-p",
            &collection,
            None,
        )
        .unwrap();
    }
    let page = lib
        .imported_collection_page("collection-p", None, 100)
        .unwrap();
    assert!(page.ordered);
    assert_eq!(
        page.rows
            .iter()
            .map(|r| r.item_id.as_deref().unwrap())
            .collect::<Vec<_>>(),
        vec!["local-a", "local-b", "local-a"]
    );
    assert_ne!(page.rows[0].occurrence_id, page.rows[2].occurrence_id);
    let detail = lib.imported_item("local-a").unwrap();
    assert_eq!(detail.active_fields["cues.hot"][0]["source_slot"], json!(7));
    assert_eq!(
        detail.active_fields["cues.memory"][0]["native_start_seconds"],
        json!(0.0)
    );
    let crate_collection = CollectionObservation {
        external_id: "crate".into(),
        kind: CollectionKind::Crate,
        name: "Unordered".into(),
        occurrences: members
            .into_iter()
            .map(|mut m| {
                m.position = None;
                m
            })
            .collect(),
        ..Default::default()
    };
    {
        let conn = lib.conn().unwrap();
        project_import_collection_tx(
            &conn,
            &inspection.source_id,
            "collection-crate",
            &crate_collection,
            None,
        )
        .unwrap();
    }
    assert!(
        !lib.imported_collection_page("collection-crate", None, 100)
            .unwrap()
            .ordered
    );
}

#[test]
fn imported_archive_roundtrip_preserves_missing_media_without_creating_jobs() {
    let dir = tempfile::tempdir().unwrap();
    let lib = Library::open(&dir.path().join("library.db")).unwrap();
    let track = observation("a", "Archived source");
    let (inspection, _root) = inspect(&lib, vec![track.clone()], vec![]);
    publish(
        &lib,
        &inspection,
        &track,
        "local-a",
        &ImportPolicy::default(),
        None,
    );
    lib.set_archived("local-a", true).unwrap();
    assert_eq!(lib.tracks(TrackQuery::default()).unwrap().total, 0);
    assert_eq!(
        lib.tracks(TrackQuery {
            view: TrackView::Archive,
            ..Default::default()
        })
        .unwrap()
        .total,
        1
    );
    lib.set_archived("local-a", false).unwrap();
    assert_eq!(lib.tracks(TrackQuery::default()).unwrap().total, 1);
    assert_eq!(lib.counts().unwrap().incoming, 0);
    assert!(lib.jobs().unwrap().is_empty());
    assert!(lib.analysis("local-a").unwrap().is_none());
}
#[test]
fn imported_collection_keeps_occurrences_for_members_not_selected_as_library_items() {
    let dir = tempfile::tempdir().unwrap();
    let lib = Library::open(&dir.path().join("library.db")).unwrap();
    let a = observation("a", "A");
    let b = observation("b", "B");
    let collection = CollectionObservation {
        external_id: "p".into(),
        name: "Partial selection".into(),
        occurrences: vec![
            MembershipObservation {
                external_occurrence_id: "one".into(),
                track_external_id: "a".into(),
                position: Some(1),
                ..Default::default()
            },
            MembershipObservation {
                external_occurrence_id: "two".into(),
                track_external_id: "b".into(),
                position: Some(2),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let (inspection, _root) = inspect(&lib, vec![a.clone(), b], vec![collection.clone()]);
    publish(
        &lib,
        &inspection,
        &a,
        "local-a",
        &ImportPolicy::default(),
        None,
    );
    project_import_collection_tx(
        &lib.conn().unwrap(),
        &inspection.source_id,
        "partial-collection",
        &collection,
        None,
    )
    .unwrap();
    let page = lib
        .imported_collection_page("partial-collection", None, 100)
        .unwrap();
    assert_eq!(page.total, 2);
    assert_eq!(page.rows.len(), 2);
    assert!(page.rows[1].item_id.is_none());
    assert!(page.rows[1].track.is_none());
}
#[test]
fn lazy_playback_and_waveform_decoding_preserve_imported_musical_values() {
    let dir = tempfile::tempdir().unwrap();
    let lib = Library::open(&dir.path().join("library.db")).unwrap();
    let path = dir.path().join("authored.wav");
    let mut writer = hound::WavWriter::create(
        &path,
        hound::WavSpec {
            channels: 2,
            sample_rate: 44100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for i in 0..44100 {
        let value = ((i as f32 * 440.0 * std::f32::consts::TAU / 44100.0).sin() * 10000.0) as i16;
        writer.write_sample(value).unwrap();
        writer.write_sample(value).unwrap();
    }
    writer.finalize().unwrap();
    let track = observation("a", "Preserved source");
    let (inspection, _root) = inspect(&lib, vec![track.clone()], vec![]);
    {
        let mut conn = lib.conn().unwrap();
        let tx = conn.transaction().unwrap();
        let fields = selected_source_values(&tx, &track, &ImportPolicy::default()).unwrap();
        project_import_track_tx(
            &tx,
            TrackProjection {
                source_id: &inspection.source_id,
                snapshot_id: &inspection.inspection_id,
                observation: &track,
                item_id: "local-a",
                fields: &fields,
                local_track_id: None,
                resolved_path: Some(&path),
            },
        )
        .unwrap();
        tx.commit().unwrap();
    }
    let before = lib.imported_item("local-a").unwrap().active_fields;
    assert!(
        lib.track("local-a")
            .unwrap()
            .import_state
            .unwrap()
            .playback_allowed
    );
    let bundle = std::env::var_os("CUERITHM_MEDIA_RUNTIME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/media-runtime/bin")
        });
    let runtime = cuerithm_media::MediaRuntime::from_bundle(&bundle).unwrap();
    let audio = runtime
        .decode(
            &lib.track_path("local-a").unwrap(),
            &dir.path().join("cache"),
        )
        .unwrap();
    let waveform = crate::waveform::waveform(&audio, 256).unwrap();
    assert_eq!(waveform.bins.len(), 256);
    assert_eq!(lib.imported_item("local-a").unwrap().active_fields, before);
    assert!(lib.analysis("local-a").unwrap().is_none());
    assert!(lib.jobs().unwrap().is_empty());
}

#[test]
fn existing_import_updates_preserve_unicode_search_and_native_artist_boundaries() {
    let dir = tempfile::tempdir().unwrap();
    let lib = Library::open(&dir.path().join("library.db")).unwrap();
    lib.conn().unwrap().execute("INSERT INTO tracks(id,path,title,source_hash,state,created,updated) VALUES('existing',?1,'Old','hash','ready',1,1)",[crate::model::path_bytes(std::path::Path::new("/Synthetic/old.flac"))]).unwrap();
    let mut track = observation("a", "ÉTÉ");
    track
        .metadata
        .insert("artists".into(), Presence::Value(json!(["BJÖRK, BAND"])));
    let (inspection, _root) = inspect(&lib, vec![track.clone()], vec![]);
    let policy = ImportPolicy {
        selected_fields: [FieldGroup::DescriptiveTags].into(),
        ..Default::default()
    };
    publish(
        &lib,
        &inspection,
        &track,
        "existing",
        &policy,
        Some("existing"),
    );
    let page = lib
        .tracks(TrackQuery {
            search: "été".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.rows[0].title, "ÉTÉ");
    assert_eq!(page.rows[0].artist, "BJÖRK, BAND");
    assert_eq!(
        lib.tracks(TrackQuery {
            search: "björk, band".into(),
            ..Default::default()
        })
        .unwrap()
        .total,
        1
    );
}
