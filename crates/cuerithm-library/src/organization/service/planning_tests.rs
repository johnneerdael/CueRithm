use super::*;
use cuerithm_cleanup::{Metadata, MetadataField};
use cuerithm_media::CancellationToken;
use rusqlite::params;
use std::path::PathBuf;
struct Fixture {
    _temp: tempfile::TempDir,
    root: PathBuf,
    library: Library,
    track: String,
    source: PathBuf,
    destination: OrganizationRoot,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let library = Library::open(&root.join("data/library.sqlite")).unwrap();
        let source = root.join("original.flac");
        std::fs::write(&source, b"all native records and audio are opaque here").unwrap();
        let job = library
            .enqueue(
                std::slice::from_ref(&source),
                library.settings().unwrap().revision,
            )
            .unwrap()
            .remove(0);
        let track = library
            .jobs()
            .unwrap()
            .into_iter()
            .find(|j| j.id == job)
            .unwrap()
            .track_id;
        let mut native = Metadata::default();
        native
            .fields
            .insert(MetadataField::Title, vec!["Example (Club Mix)".into()]);
        native
            .fields
            .insert(MetadataField::Artist, vec!["Example Artist".into()]);
        native
            .fields
            .insert(MetadataField::Mix, vec!["Club Mix".into()]);
        library.conn().unwrap().execute("UPDATE tracks SET state='ready',title='filename fallback',native_metadata_json=?1,musical_key='8A',bpm=126.0,energy=7 WHERE id=?2",params![serde_json::to_string(&native).unwrap(),track]).unwrap();
        let managed = root.join("managed");
        std::fs::create_dir(&managed).unwrap();
        let destination = library.register_organization_root(managed).unwrap();
        Self {
            _temp: temp,
            root,
            library,
            track,
            source,
            destination,
        }
    }
    fn settings(&self) -> OrganizationSettings {
        OrganizationSettings {
            mode: OrganizationSettingMode::Move,
            destination_root_id: Some(self.destination.id.clone()),
            ..Default::default()
        }
    }
}
#[test]
fn native_preview_uses_effective_title_and_full_frozen_scope_without_mutating_files() {
    let f = Fixture::new();
    let preview = f.library.preview_organization(
        OrganizationPreviewRequest {
            scope: OrganizationScope::Tracks {
                track_ids: vec![f.track.clone()],
            },
            settings: f.settings(),
        },
        &CancellationToken::default(),
    );
    assert!(
        preview.is_ok(),
        "Expected complete frozen plan: {preview:?}"
    );
    let preview = preview.unwrap();
    assert_eq!(preview.totals.ready, 1);
    assert!(preview.can_apply);
    let page = f
        .library
        .organization_preview_page(&preview.id, 0, 100)
        .unwrap();
    assert_eq!(page.total, 1);
    assert!(
        page.rows[0]
            .after_path
            .as_ref()
            .unwrap()
            .ends_with("Example Artist - Example (Club Mix).flac")
    );
    assert!(f.source.exists());
    assert_eq!(
        std::fs::read_dir(f.root.join("managed")).unwrap().count(),
        0
    );
}

#[test]
fn owned_nested_artwork_moves_with_audio_and_shared_artwork_blocks() {
    let f = Fixture::new();
    let art = f.root.join("art/cover.png");
    std::fs::create_dir(art.parent().unwrap()).unwrap();
    std::fs::write(&art, b"owned cover bytes").unwrap();
    f.library
        .conn()
        .unwrap()
        .execute(
            "INSERT INTO organization_owned_sidecars VALUES(?1,?2,?3,'current',1)",
            params![
                f.track,
                crate::model::path_bytes(&art),
                cuerithm_media::file_sha256(&art).unwrap()
            ],
        )
        .unwrap();
    let preview = f
        .library
        .preview_organization(
            OrganizationPreviewRequest {
                scope: OrganizationScope::Tracks {
                    track_ids: vec![f.track.clone()],
                },
                settings: f.settings(),
            },
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(preview.totals.ready, 1);
    let plan = f.library.organization_plan_rows(&preview.id).unwrap();
    assert_eq!(plan[0].captured.as_ref().unwrap().files.len(), 2);
    let completed = f
        .library
        .apply_organization_preview(
            &preview.id,
            &preview.revision,
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(completed.state, "complete");
    assert_eq!(
        std::fs::read(f.root.join("managed/art/cover.png")).unwrap(),
        b"owned cover bytes"
    );
    assert!(!art.exists());

    let f = Fixture::new();
    let art = f.root.join("cover.png");
    std::fs::write(&art, b"shared artwork").unwrap();
    let other = f.root.join("other.flac");
    std::fs::write(&other, b"other").unwrap();
    let job = f
        .library
        .enqueue(&[other], f.library.settings().unwrap().revision)
        .unwrap()
        .remove(0);
    let other = f
        .library
        .jobs()
        .unwrap()
        .into_iter()
        .find(|j| j.id == job)
        .unwrap()
        .track_id;
    for track in [&f.track, &other] {
        f.library
            .conn()
            .unwrap()
            .execute(
                "INSERT INTO organization_owned_sidecars VALUES(?1,?2,?3,'current',1)",
                params![
                    track,
                    crate::model::path_bytes(&art),
                    cuerithm_media::file_sha256(&art).unwrap()
                ],
            )
            .unwrap();
    }
    let preview = f
        .library
        .preview_organization(
            OrganizationPreviewRequest {
                scope: OrganizationScope::Tracks {
                    track_ids: vec![f.track.clone()],
                },
                settings: f.settings(),
            },
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(preview.totals.blocked, 1);
    assert!(f.source.exists());
    assert!(art.exists());
}

#[test]
fn smart_playlist_and_playlist_folder_scope_freezes_all_deduplicated_members() {
    let f = Fixture::new();
    {
        let conn = f.library.conn().unwrap();
        conn.execute("INSERT INTO playlists(id,kind,name,position,last_edit_id) VALUES('folder','folder','Folder',0,'fixture')",[]).unwrap();
        conn.execute("INSERT INTO playlists(id,parent_id,kind,name,position,last_edit_id) VALUES('smart','folder','smartlist','Smart',0,'fixture')",[]).unwrap();
        conn.execute("INSERT INTO playlists(id,parent_id,kind,name,position,last_edit_id) VALUES('manual','folder','playlist','Manual',1,'fixture')",[]).unwrap();
        conn.execute("UPDATE playlists SET definition_json=?1 WHERE id='smart'",[serde_json::json!({"match_all":true,"rules":[{"id":"title","field":"title","operator":"Exists","value":{"kind":"none"},"or":false}]}).to_string()]).unwrap();
        conn.execute(
            "INSERT INTO smart_playlist_members VALUES('smart',?1,0)",
            [&f.track],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO playlist_members VALUES('manual',?1,0)",
            [&f.track],
        )
        .unwrap();
    }
    for id in ["smart", "folder"] {
        let preview = f
            .library
            .preview_organization(
                OrganizationPreviewRequest {
                    scope: OrganizationScope::Playlist {
                        playlist_id: id.into(),
                    },
                    settings: f.settings(),
                },
                &CancellationToken::default(),
            )
            .unwrap();
        assert_eq!(preview.totals.total, 1);
        assert_eq!(preview.totals.ready, 1);
    }
    assert!(
        f.library
            .preview_organization(
                OrganizationPreviewRequest {
                    scope: OrganizationScope::Playlist {
                        playlist_id: "missing".into()
                    },
                    settings: f.settings()
                },
                &CancellationToken::default()
            )
            .is_err()
    );
}

#[test]
fn release_time_native_alias_precedes_old_date_and_preserves_precision() {
    let f = Fixture::new();
    let mut native = Metadata::default();
    native
        .fields
        .insert(MetadataField::Title, vec!["Title".into()]);
    f.library
        .conn()
        .unwrap()
        .execute(
            "UPDATE tracks SET native_metadata_json=?1 WHERE id=?2",
            params![serde_json::to_string(&native).unwrap(), f.track],
        )
        .unwrap();
    let media = serde_json::json!({"codec":"flac","sample_rate":44100,"channels":2,"bits_per_sample":16,"tags":{"DATE":"1990","RELEASE_TIME":"2026-10-20"}});
    f.library
        .conn()
        .unwrap()
        .execute(
            "UPDATE tracks SET metadata_json=?1 WHERE id=?2",
            params![media.to_string(), f.track],
        )
        .unwrap();
    let mut settings = f.settings();
    settings.pattern = "{year}/{release_date} - {title}".into();
    let preview = f
        .library
        .preview_organization(
            OrganizationPreviewRequest {
                scope: OrganizationScope::Tracks {
                    track_ids: vec![f.track.clone()],
                },
                settings,
            },
            &CancellationToken::default(),
        )
        .unwrap();
    let page = f
        .library
        .organization_preview_page(&preview.id, 0, 100)
        .unwrap();
    // The preview path uses the native separator, so compare components, not text.
    assert!(
        std::path::Path::new(page.rows[0].after_path.as_ref().unwrap())
            .ends_with(std::path::Path::new("2026").join("2026-10-20 - Title.flac"))
    );
}

#[test]
fn organization_transports_every_supported_container_without_rewriting_audio_or_records() {
    // Container decoding is covered by media/preparation tests. The organization
    // boundary transports each complete native file as opaque bytes.
    for extension in [
        "mp3", "aac", "m4a", "flac", "wav", "aiff", "ogg", "opus", "wma",
    ] {
        let f = Fixture::new();
        let source = f.root.join(format!("original.{extension}"));
        std::fs::rename(&f.source, &source).unwrap();
        f.library
            .conn()
            .unwrap()
            .execute(
                "UPDATE tracks SET path=?1 WHERE id=?2",
                params![crate::model::path_bytes(&source), f.track],
            )
            .unwrap();
        let before = cuerithm_media::file_sha256(&source).unwrap();
        let preview = f
            .library
            .preview_organization(
                OrganizationPreviewRequest {
                    scope: OrganizationScope::Tracks {
                        track_ids: vec![f.track.clone()],
                    },
                    settings: f.settings(),
                },
                &CancellationToken::default(),
            )
            .unwrap();
        assert_eq!(preview.totals.ready, 1, "{extension}");
        let batch = f
            .library
            .apply_organization_preview(
                &preview.id,
                &preview.revision,
                &CancellationToken::default(),
            )
            .unwrap();
        assert_eq!(batch.state, "complete", "{extension}");
        let path = f.library.track_path(&f.track).unwrap();
        assert_eq!(path.extension().unwrap(), extension);
        assert_eq!(cuerithm_media::file_sha256(&path).unwrap(), before);
        assert!(!source.exists());
    }
}
