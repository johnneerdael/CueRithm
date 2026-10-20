use super::*;

fn catalog(root: &Path, pioneer: &str) -> BTreeMap<String, (String, String)> {
    let path = root.join(pioneer).join("rekordbox/exportLibrary.db");
    let db = database::prepare(&path, &path).unwrap();
    db.prepare("SELECT title,path,analysisDataFilePath FROM content")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?))))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}
fn assert_catalog_assets(root: &Path, pioneer: &str) {
    let records = catalog(root, pioneer);
    let mut paths = HashSet::new();
    for (audio, analysis) in records.values() {
        assert!(
            paths.insert(analysis),
            "Tracks must have separate analysis assets"
        );
        for suffix in ["DAT", "EXT", "2EX"] {
            let relative = format!("{}.{}", analysis.strip_suffix(".DAT").unwrap(), suffix);
            let bytes = fs::read(root.join(relative.trim_start_matches('/'))).unwrap();
            let parsed = AnlzFile::parse(&bytes, ParseLimits::default()).unwrap();
            assert_eq!(parsed.tags()[0].audio_path().unwrap(), *audio);
        }
    }
    let tables =
        pdb::read_tables(&fs::read(root.join(pioneer).join("rekordbox/export.pdb")).unwrap())
            .unwrap();
    for row in &tables[&0] {
        let audio =
            pdb::decode_string(row, pdb::read16(row, 94 + 20 * 2).unwrap() as usize).unwrap();
        let analysis =
            pdb::decode_string(row, pdb::read16(row, 94 + 14 * 2).unwrap() as usize).unwrap();
        assert!(
            records
                .values()
                .any(|value| value == &(audio.clone(), analysis.clone()))
        );
    }
}
#[test]
fn hash_collisions_get_distinct_analysis_and_remain_stable_on_repeat() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("synthetic.wav");
    audio(&source);
    let mut input = library(&source, "synthetic-usb-50442");
    let second = library(&source, "synthetic-usb-60251");
    input.tracks.extend(second.tracks);
    input.playlists.clear();
    let original = dir.path().join("original");
    write(&original, &input, &SyncOptions::default()).unwrap();
    assert_catalog_assets(&original, "PIONEER");
    let before = catalog(&original, "PIONEER");
    input.tracks.reverse();
    let repeat = dir.path().join("repeat");
    write_staged(&repeat, &original, &input, &SyncOptions::default()).unwrap();
    assert_eq!(catalog(&repeat, "PIONEER"), before);
    assert_catalog_assets(&repeat, "PIONEER");
}
#[test]
fn subset_collision_preserves_unrelated_analysis_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("synthetic.wav");
    audio(&source);
    let original = dir.path().join("original");
    write(
        &original,
        &library(&source, "synthetic-usb-50442"),
        &SyncOptions::default(),
    )
    .unwrap();
    let old = catalog(&original, "PIONEER")
        .into_values()
        .next()
        .unwrap()
        .1;
    let bytes = fs::read(original.join(old.trim_start_matches('/'))).unwrap();
    let stage = dir.path().join("stage");
    write_staged(
        &stage,
        &original,
        &library(&source, "synthetic-usb-60251"),
        &SyncOptions::default(),
    )
    .unwrap();
    assert_catalog_assets(&stage, "PIONEER");
    assert_eq!(
        fs::read(stage.join(old.trim_start_matches('/'))).unwrap(),
        bytes
    );
}
#[test]
fn orphan_drive_analysis_reserves_its_name() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("synthetic.wav");
    audio(&source);
    let original = dir.path().join("original");
    let reserved = original.join("PIONEER/USBANLZ/P654/6547057C/ANLZ0000.EXT");
    fs::create_dir_all(reserved.parent().unwrap()).unwrap();
    fs::write(&reserved, b"unreferenced device asset").unwrap();
    write(
        &original,
        &library(&source, "synthetic-usb-50442"),
        &SyncOptions::default(),
    )
    .unwrap();
    assert_eq!(fs::read(&reserved).unwrap(), b"unreferenced device asset");
    assert!(
        catalog(&original, "PIONEER")
            .values()
            .next()
            .unwrap()
            .1
            .ends_with("ANLZ0001.DAT")
    );
    assert_catalog_assets(&original, "PIONEER");
}
#[test]
fn hidden_existing_catalog_takes_precedence_over_empty_visible_directory() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("synthetic.wav");
    audio(&source);
    let original = dir.path().join("original");
    fs::create_dir_all(original.join(".PIONEER")).unwrap();
    write(
        &original,
        &library(&source, "first"),
        &SyncOptions::default(),
    )
    .unwrap();
    fs::create_dir_all(original.join("PIONEER")).unwrap();
    let stage = dir.path().join("stage");
    write_staged(
        &stage,
        &original,
        &library(&source, "second"),
        &SyncOptions::default(),
    )
    .unwrap();
    assert!(stage.join(".PIONEER/rekordbox/exportLibrary.db").is_file());
    assert_eq!(catalog(&stage, ".PIONEER").len(), 2);
    assert_catalog_assets(&stage, ".PIONEER");
}

#[test]
fn catalog_reservations_do_not_depend_on_assets_being_present() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("synthetic.wav");
    audio(&source);
    let original = dir.path().join("original");
    write(
        &original,
        &library(&source, "synthetic-usb-50442"),
        &SyncOptions::default(),
    )
    .unwrap();
    let tables =
        pdb::read_tables(&fs::read(original.join("PIONEER/rekordbox/export.pdb")).unwrap())
            .unwrap();
    fs::remove_dir_all(original.join("PIONEER/USBANLZ")).unwrap();
    let empty = rusqlite::Connection::open_in_memory().unwrap();
    empty
        .execute_batch(
            "CREATE TABLE content(content_id INTEGER,path TEXT,analysisDataFilePath TEXT)",
        )
        .unwrap();
    let hash = format!("{:x}", Sha256::digest(b"synthetic-usb-60251"));
    let mut paths = paths::AnalysisPaths::new(&original, "PIONEER", &empty, Some(&tables)).unwrap();
    assert!(paths.allocate(2, &hash).unwrap().ends_with("ANLZ0001.DAT"));
    let (media, analysis) = catalog(&original, "PIONEER").into_values().next().unwrap();
    empty
        .execute(
            "INSERT INTO content VALUES(1,?1,?2)",
            rusqlite::params![media, analysis.trim_start_matches('/').replace('/', "\\")],
        )
        .unwrap();
    let mut paths = paths::AnalysisPaths::new(&original, "PIONEER", &empty, None).unwrap();
    assert!(paths.allocate(2, &hash).unwrap().ends_with("ANLZ0001.DAT"));
}
#[test]
fn mismatched_previous_asset_is_not_reused_for_the_same_content() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("synthetic.wav");
    audio(&source);
    let original = dir.path().join("original");
    let input = library(&source, "first");
    write(&original, &input, &SyncOptions::default()).unwrap();
    let analysis = catalog(&original, "PIONEER")
        .into_values()
        .next()
        .unwrap()
        .1;
    let corrupt = format!("{}.EXT", analysis.strip_suffix(".DAT").unwrap());
    fs::write(
        original.join(corrupt.trim_start_matches('/')),
        b"foreign analysis asset",
    )
    .unwrap();
    let stage = dir.path().join("stage");
    write_staged(&stage, &original, &input, &SyncOptions::default()).unwrap();
    assert_ne!(
        catalog(&stage, "PIONEER").into_values().next().unwrap().1,
        analysis
    );
    assert_catalog_assets(&stage, "PIONEER");
    assert_eq!(
        fs::read(original.join(corrupt.trim_start_matches('/'))).unwrap(),
        b"foreign analysis asset"
    );
}
