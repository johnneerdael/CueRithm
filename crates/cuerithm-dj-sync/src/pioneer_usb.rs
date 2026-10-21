//! Removable Pioneer/AlphaTheta library export. Build under a staging root, then
//! publish the returned relative paths with the caller's backup/rollback journal.
pub(crate) mod analysis;
pub(crate) mod compatibility;
mod database;
mod ownership;
mod paths;
mod pdb;
use crate::{SyncLibrary, SyncOptions, SyncTrack, UsbCompatibility};
use anyhow::{Context, Result, ensure};
use cuerithm_media::MediaRuntime;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

struct TrackRecord {
    track: SyncTrack,
    id: u32,
    path: String,
    analysis_path: String,
    file_size: u64,
    file_type: u16,
    artist: u32,
    album: u32,
    genre: u32,
    label: u32,
    key: u32,
    artwork: u32,
}
#[derive(Default)]
struct Names {
    artist: BTreeMap<String, u32>,
    album: BTreeMap<String, u32>,
    genre: BTreeMap<String, u32>,
    label: BTreeMap<String, u32>,
    key: BTreeMap<String, u32>,
}
fn name_id(names: &mut BTreeMap<String, u32>, name: &str) -> u32 {
    if name.is_empty() {
        return 0;
    }
    let next = names.values().copied().max().unwrap_or(0) + 1;
    *names.entry(name.into()).or_insert(next)
}

/// Build a complete device library in `staging_root`; preserve settings from that root.
pub fn write(
    staging_root: &Path,
    library: &SyncLibrary,
    options: &SyncOptions,
) -> Result<Vec<PathBuf>> {
    write_staged(staging_root, staging_root, library, options)
}
/// Read configuration from `original_root` and write only beneath `staging_root`.
pub fn write_staged(
    staging_root: &Path,
    original_root: &Path,
    library: &SyncLibrary,
    options: &SyncOptions,
) -> Result<Vec<PathBuf>> {
    Ok(write_staged_with_runtime(staging_root, original_root, library, options, None)?.files)
}
#[derive(Debug)]
pub struct UsbExportReport {
    pub files: Vec<PathBuf>,
    pub warnings: Vec<String>,
    pub ownership: crate::ownership::PlaylistOwnershipReport,
}
pub fn write_staged_with_runtime(
    staging_root: &Path,
    original_root: &Path,
    library: &SyncLibrary,
    options: &SyncOptions,
    runtime: Option<&MediaRuntime>,
) -> Result<UsbExportReport> {
    write_staged_owned(
        staging_root,
        original_root,
        library,
        options,
        runtime,
        &Default::default(),
    )
}
pub fn write_staged_owned(
    staging_root: &Path,
    original_root: &Path,
    library: &SyncLibrary,
    options: &SyncOptions,
    runtime: Option<&MediaRuntime>,
    ownership: &crate::ownership::PlaylistOwnershipRequest,
) -> Result<UsbExportReport> {
    ensure!(
        !options.skip_beatgrids,
        "Pioneer USB export requires beatgrids for a complete device library"
    );
    let pioneer = paths::pioneer_directory(original_root);
    let database_dir = Path::new(pioneer).join("rekordbox");
    fs::create_dir_all(staging_root.join(&database_dir))?;
    let original_db = original_root.join(&database_dir).join("exportLibrary.db");
    let had_original_db = original_db.is_file();
    let original_pdb = original_root.join(&database_dir).join("export.pdb");
    let original_tables = if original_pdb.is_file() {
        Some(pdb::read_tables(&fs::read(&original_pdb)?)?)
    } else {
        None
    };
    let stage_db = staging_root.join(&database_dir).join("exportLibrary.db");
    let mut db = database::prepare(&original_db, &stage_db)?;
    if !had_original_db && let Some(tables) = &original_tables {
        database::import_pdb(&mut db, tables)?;
    }
    let mut files = Vec::new();
    let mut warnings = Vec::new();
    let mut records = Vec::new();
    let mut names = database::names(&db)?;
    let mut ids = HashMap::new();
    let mut paths = HashSet::new();
    let old_ids = database::track_ids(&db)?;
    let mut analysis_paths =
        paths::AnalysisPaths::new(original_root, pioneer, &db, original_tables.as_ref())?;
    let mut next = old_ids.values().copied().max().unwrap_or(0) + 1;
    let mut next_artwork: u32 =
        db.query_row("SELECT COALESCE(MAX(image_id),0)+1 FROM image", [], |r| {
            r.get(0)
        })?;
    for source_track in &library.tracks {
        let mut track = source_track.clone();
        ensure!(
            ids.insert(track.id.clone(), 0).is_none(),
            "Duplicate Pioneer USB source track ID"
        );
        ensure!(track.path.is_file(), "Pioneer USB source audio is missing");
        ensure!(
            track.sample_rate > 0
                && track.duration_seconds.is_finite()
                && track.duration_seconds > 0.0,
            "Pioneer USB requires analyzed sample rate and duration"
        );
        ensure!(
            track.bpm.is_finite() && track.bpm > 0.0 && !track.beatgrid.is_empty(),
            "Pioneer USB requires an analyzed beatgrid and BPM"
        );
        let mut extension = track
            .path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default()
            .to_lowercase();
        ensure!(
            extension != "m4p",
            "DRM-protected audio cannot be exported to Pioneer USB"
        );
        let issues = if !matches!(options.usb_cdj_compatibility, UsbCompatibility::Off) {
            compatibility::inspect(&track, runtime)?
        } else {
            Vec::new()
        };
        let fix = matches!(options.usb_cdj_compatibility, UsbCompatibility::AutoFix)
            && issues.iter().any(|i| i.fixable);
        for issue in &issues {
            warnings.push(format!(
                "Track {}: {}{}",
                track.id,
                issue.message,
                if fix && issue.fixable {
                    "; exported copy converted to stereo PCM WAV"
                } else {
                    ""
                }
            ));
        }
        if fix {
            extension = "wav".into();
        }
        let file_type = match extension.as_str() {
            "mp3" => 1,
            "m4a" => 4,
            "flac" => 5,
            "wav" => 11,
            "aif" | "aiff" => 12,
            _ => anyhow::bail!("Pioneer USB requires MP3, M4A, FLAC, WAV or AIFF audio"),
        };
        let hash = format!("{:x}", Sha256::digest(track.id.as_bytes()));
        let path = format!(
            "/Contents/CueRithm/{}/{}.{}",
            &hash[..2],
            &hash[..24],
            extension
        );
        ensure!(
            paths.insert(path.clone()),
            "Duplicate Pioneer USB media destination"
        );
        let relative = PathBuf::from(path.trim_start_matches('/'));
        fs::create_dir_all(staging_root.join(&relative).parent().unwrap())?;
        if fix {
            compatibility::fix_copy(
                runtime.context("CDJ AutoFix requires the bundled media runtime")?,
                &mut track,
                &staging_root.join(&relative),
            )?;
        } else if track.path != staging_root.join(&relative) {
            fs::copy(&track.path, staging_root.join(&relative))
                .context("Copy Pioneer USB audio")?;
        }
        track.path = staging_root.join(&relative);
        track = crate::timing::export_track(&track, crate::DjSoftware::PioneerUsb)?;
        files.push(relative);
        let old = old_ids.get(&path).or_else(|| {
            old_ids
                .iter()
                .find(|(old, _)| {
                    old.rsplit_once('.').map(|p| p.0) == path.rsplit_once('.').map(|p| p.0)
                })
                .map(|(_, id)| id)
        });
        let id = if let Some(id) = old {
            *id
        } else {
            let id = next;
            next = next.checked_add(1).context("USB track ID overflow")?;
            id
        };
        ids.insert(track.id.clone(), id);
        let analysis_path = analysis_paths.allocate(id, &hash)?;
        let artwork = if options.add_album_art {
            if let Some(bytes) = &track.artwork {
                let artwork = database::artwork_id(&db, id, &mut next_artwork)?;
                write_artwork(staging_root, pioneer, artwork, bytes, &mut files)?;
                artwork
            } else {
                0
            }
        } else {
            0
        };
        let mut record = TrackRecord {
            track: track.clone(),
            id,
            path,
            analysis_path,
            file_size: fs::metadata(&track.path)?.len(),
            file_type,
            artist: name_id(&mut names.artist, &track.artist),
            album: name_id(&mut names.album, &track.album),
            genre: name_id(&mut names.genre, &track.genre),
            label: name_id(&mut names.label, &track.label),
            key: name_id(&mut names.key, &track.key),
            artwork,
        };
        // Metadata containers and actual copied audio always reference the same device path.
        analysis::write(staging_root, &mut record, options, &mut files, runtime)?;
        records.push(record);
    }
    let ownership = database::write(
        &mut db,
        library,
        &records,
        &names,
        &ids,
        pioneer,
        database::WriteOptions { options, ownership },
    )?;
    for relative in database::referenced_assets(&db)? {
        let relative = PathBuf::from(relative.trim_start_matches('/'));
        ensure!(
            relative
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_))),
            "Unsafe existing USB asset path"
        );
        let staged = staging_root.join(&relative);
        if !staged.is_file() {
            let original = original_root.join(&relative);
            if relative.extension().is_some_and(|s| s == "2EX") && !original.is_file() {
                continue;
            }
            ensure!(
                original.is_file(),
                "Referenced existing USB media/analysis/artwork asset is missing"
            );
            fs::create_dir_all(staged.parent().unwrap())?;
            fs::copy(original, &staged)?;
        }
        files.push(relative);
    }
    let mut tables = database::pdb_tables(&db, &records)?;
    if let Some(original) = original_tables {
        let changed: HashSet<_> = records.iter().map(|r| r.id).collect();
        if let (Some(old), Some(current)) = (original.get(&0), tables.get_mut(&0)) {
            let raw: HashMap<_, _> = old
                .iter()
                .map(|r| Ok((pdb::read32(r, 72)?, r)))
                .collect::<Result<_>>()?;
            for row in current {
                let id = pdb::read32(row, 72)?;
                if !changed.contains(&id)
                    && let Some(old) = raw.get(&id)
                {
                    *row = (*old).clone();
                }
            }
        }
        tables.extend(
            original
                .into_iter()
                .filter(|(kind, _)| matches!(*kind,9..=12|14..=19)),
        );
    }
    let total: u32 = db.query_row("SELECT COUNT(*) FROM content", [], |r| r.get(0))?;
    database::default_pdb_config(&db, &mut tables, total)?;
    let pdb_path = database_dir.join("export.pdb");
    fs::write(staging_root.join(&pdb_path), pdb::build(&tables)?)?;
    files.push(pdb_path);
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode=DELETE;")?;
    let check: String = db.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    ensure!(
        check == "ok",
        "Pioneer USB encrypted database integrity check failed"
    );
    drop(db);
    files.push(database_dir.join("exportLibrary.db"));
    files.sort();
    files.dedup();
    Ok(UsbExportReport {
        files,
        warnings,
        ownership,
    })
}
fn color_id(color: Option<u32>, nearest: bool) -> u8 {
    let Some(color) = color else {
        return 0;
    };
    let rgb = [(color >> 16) as u8, (color >> 8) as u8, color as u8];
    const PALETTE: [[u8; 3]; 8] = [
        [255, 0, 255],
        [255, 0, 23],
        [255, 94, 0],
        [255, 140, 0],
        [51, 255, 0],
        [0, 196, 255],
        [0, 0, 255],
        [77, 0, 255],
    ];
    if !nearest {
        return PALETTE
            .iter()
            .position(|p| *p == rgb)
            .map_or(0, |n| n as u8 + 1);
    }
    PALETTE
        .iter()
        .enumerate()
        .min_by_key(|(_, p)| {
            p.iter()
                .zip(rgb)
                .map(|(a, b)| (i32::from(*a) - i32::from(b)).pow(2))
                .sum::<i32>()
        })
        .map_or(0, |(n, _)| n as u8 + 1)
}

fn write_artwork(
    root: &Path,
    pioneer: &str,
    id: u32,
    bytes: &[u8],
    files: &mut Vec<PathBuf>,
) -> Result<()> {
    ensure!(
        bytes.len() <= 10 * 1024 * 1024,
        "USB cover image exceeds 10MB"
    );
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode()?;
    let dir = Path::new(pioneer)
        .join("Artwork")
        .join(format!("{:05}", id / 20 + 1));
    fs::create_dir_all(root.join(&dir))?;
    for (size, suffix) in [(80, ""), (240, "_m")] {
        let thumbnail = image.resize(size, size, image::imageops::FilterType::Lanczos3);
        for prefix in ['a', 'b'] {
            let path = dir.join(format!("{prefix}{id}{suffix}.jpg"));
            thumbnail.save_with_format(root.join(&path), image::ImageFormat::Jpeg)?;
            files.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SyncBeat, SyncCue, SyncPlaylist, UsbCueDestination};
    use cuerithm_rekordbox::anlz::{AnlzFile, AnlzTag, ParseLimits};
    mod path_tests;
    #[test]
    fn windows_owned_audio_usb_copy_accepts_unmapped_host_source_and_keeps_device_references() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let output = root.join("usb");
        fs::create_dir(&output).unwrap();
        let source_dir = tempfile::tempdir().unwrap();
        #[cfg(unix)]
        let filename = "CON.wav";
        #[cfg(not(unix))]
        let filename = "unmapped.wav";
        let source = source_dir.path().canonicalize().unwrap().join(filename);
        audio(&source);
        let original = fs::read(&source).unwrap();
        let target:crate::SyncTarget=serde_json::from_value(serde_json::json!({"software":"pioneer_usb","path":output,"source_root":null,"environment":{"kind":"windows_compatibility","prefix":root,"installation":null,"mappings":[{"windows_root":"C:\\","host_root":output}]}})).unwrap();
        let mut input = library(&source, "unmapped");
        input.tracks[0].native_path = Some(PathBuf::from(r"C:\forged.wav"));
        let journals = root.join("journals");
        let preview =
            crate::prepare_sync(&journals, target, &input, &SyncOptions::default()).unwrap();
        crate::apply_sync(&journals, &preview.id).unwrap();
        let dbpath = output.join("PIONEER/rekordbox/exportLibrary.db");
        let db = database::prepare(&dbpath, &dbpath).unwrap();
        let native: String = db
            .query_row("SELECT path FROM content", [], |row| row.get(0))
            .unwrap();
        drop(db);
        assert!(native.starts_with("/Contents/CueRithm/"));
        assert!(!native.contains(source_dir.path().to_str().unwrap()));
        assert!(!native.contains("forged"));
        assert_eq!(
            fs::read(output.join(native.trim_start_matches('/'))).unwrap(),
            original
        );
        let analysis = preview
            .files
            .iter()
            .find(|file| {
                file.target
                    .extension()
                    .is_some_and(|extension| extension == "DAT")
            })
            .unwrap();
        let parsed =
            AnlzFile::parse(&fs::read(&analysis.target).unwrap(), ParseLimits::default()).unwrap();
        assert_eq!(
            parsed
                .tags()
                .iter()
                .find(|tag| tag.fourcc() == *b"PPTH")
                .unwrap()
                .audio_path()
                .unwrap(),
            native
        );
        crate::restore_sync(&journals, &preview.id).unwrap();
        assert!(!output.join(native.trim_start_matches('/')).exists());
        assert_eq!(fs::read(source).unwrap(), original);
    }
    #[test]
    fn encrypted_catalog_uses_observed_native_sqlcipher_v4_profile() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("catalog.db");
        let db = database::prepare(&path, &path).unwrap();
        for (pragma, expected) in [
            ("cipher_page_size", "4096"),
            ("kdf_iter", "256000"),
            ("cipher_hmac_algorithm", "HMAC_SHA512"),
            ("cipher_kdf_algorithm", "PBKDF2_HMAC_SHA512"),
        ] {
            let value: String = db
                .pragma_query_value(None, pragma, |row| row.get(0))
                .unwrap();
            assert_eq!(value, expected, "{pragma}");
        }
    }
    fn audio(path: &Path) {
        let rate = 48000_u32;
        let count = rate * 2;
        let size = count * 2;
        let mut b = b"RIFF".to_vec();
        b.extend((size + 36).to_le_bytes());
        b.extend(b"WAVEfmt ");
        b.extend(16_u32.to_le_bytes());
        b.extend(1_u16.to_le_bytes());
        b.extend(1_u16.to_le_bytes());
        b.extend(rate.to_le_bytes());
        b.extend((rate * 2).to_le_bytes());
        b.extend(2_u16.to_le_bytes());
        b.extend(16_u16.to_le_bytes());
        b.extend(b"data");
        b.extend(size.to_le_bytes());
        for n in 0..count {
            let sample = ((n as f64 * std::f64::consts::TAU * 220.0 / f64::from(rate)).sin()
                * 16000.0) as i16;
            b.extend(sample.to_le_bytes());
        }
        fs::write(path, b).unwrap();
    }
    fn library(path: &Path, id: &str) -> SyncLibrary {
        SyncLibrary {
            tracks: vec![SyncTrack {
                native_path: None,
                id: id.into(),
                path: path.into(),
                title: format!("Synthetic {id}"),
                artist: "Artist é".into(),
                album: "Album".into(),
                genre: "Test".into(),
                label: "Label".into(),
                key: "8A".into(),
                bpm: 120.0,
                duration_seconds: 2.0,
                sample_rate: 48000,
                rating: 4,
                beatgrid: vec![SyncBeat {
                    seconds: 0.125,
                    bpm: 120.0,
                    beat: 1,
                }],
                cues: vec![
                    SyncCue {
                        index: 2,
                        name: "Départ".into(),
                        start_seconds: 0.25,
                        color: Some(0xff0000),
                        ..Default::default()
                    },
                    SyncCue {
                        active_loop: false,
                        index: 0,
                        name: "Loop".into(),
                        start_seconds: 0.5,
                        end_seconds: Some(1.0),
                        color: Some(0x00ff00),
                    },
                ],
                ..Default::default()
            }],
            playlists: vec![
                SyncPlaylist {
                    id: "folder".into(),
                    name: "Sets".into(),
                    folder: true,
                    ..Default::default()
                },
                SyncPlaylist {
                    id: "set".into(),
                    parent_id: Some("folder".into()),
                    name: format!("Set {id}"),
                    track_ids: vec![id.into()],
                    ..Default::default()
                },
            ],
        }
    }
    #[test]
    fn active_loops_agree_across_usb_catalogs_and_preserve_native_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("synthetic.wav");
        audio(&source);
        let mut input = library(&source, "active");
        input.tracks[0].cues[1].active_loop = true;
        input.tracks[0].cues.push(SyncCue {
            index: 3,
            start_seconds: 1.,
            end_seconds: Some(1.5),
            ..Default::default()
        });
        let output = dir.path().join("staged");
        let report = write_staged_with_runtime(
            &output,
            &output,
            &input,
            &SyncOptions {
                usb_cue_destination: UsbCueDestination::Both,
                ..Default::default()
            },
            None,
        )
        .unwrap();
        let dat = report
            .files
            .iter()
            .find(|p| p.extension().is_some_and(|e| e == "DAT"))
            .unwrap();
        let parsed =
            AnlzFile::parse(&fs::read(output.join(dat)).unwrap(), ParseLimits::default()).unwrap();
        let lists: Vec<_> = parsed
            .tags()
            .iter()
            .filter(|t| t.fourcc() == *b"PCOB")
            .map(|t| t.cues().unwrap())
            .collect();
        let hot = lists.iter().find(|c| c.kind == 1).unwrap();
        assert!(
            hot.entries
                .iter()
                .all(|c| c.legacy.as_ref().unwrap().status == 0)
        );
        let memory = lists.iter().find(|c| c.kind == 0).unwrap();
        assert_eq!(
            memory
                .entries
                .iter()
                .map(|c| (c.time_ms, c.legacy.as_ref().unwrap().status))
                .collect::<Vec<_>>(),
            [(1000, 0), (500, 4), (250, 0)]
        );
        let database_path = output.join("PIONEER/rekordbox/exportLibrary.db");
        let db = database::prepare(&database_path, &database_path).unwrap();
        db.execute_batch(
            "ALTER TABLE content ADD COLUMN native_note TEXT;
             UPDATE content SET native_note='preserve native metadata';",
        )
        .unwrap();
        drop(db);
        // Re-export through the real writer: the active loop must agree across
        // both catalogs while an unknown native content field survives.
        write(
            &output,
            &input,
            &SyncOptions {
                usb_cue_destination: UsbCueDestination::Both,
                ..Default::default()
            },
        )
        .unwrap();
        let db = database::prepare(&database_path, &database_path).unwrap();
        let states = db
            .prepare("SELECT inUsec,kind,isActiveLoop FROM cue ORDER BY inUsec,kind")
            .unwrap()
            .query_map([], |row| {
                Ok((
                    row.get::<_, u32>(0)?,
                    row.get::<_, u32>(1)?,
                    row.get::<_, u32>(2)?,
                ))
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(
            states,
            [
                (250000, 0, 0),
                (250000, 3, 0),
                (500000, 0, 1),
                (500000, 1, 1),
                (1000000, 0, 0),
                (1000000, 4, 0),
            ]
        );
        assert_eq!(
            db.query_row("SELECT native_note FROM content", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap(),
            "preserve native metadata"
        );
    }
    #[test]
    fn optional_numeric_metadata_agrees_across_usb_catalogs_and_survives_unknown_sources() {
        use cuerithm_smartlists::FieldValue;
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("synthetic.wav");
        audio(&source);
        let mut input = library(&source, "metadata");
        let output = dir.path().join("usb");
        let supplied = [
            ("bitrate".into(), FieldValue::Number(320.9)),
            ("trackNumber".into(), FieldValue::Number(7.)),
        ]
        .into();
        for attributes in [
            supplied,
            Default::default(),
            [
                ("bitrate".into(), FieldValue::Null),
                ("trackNumber".into(), FieldValue::Null),
            ]
            .into(),
        ] {
            input.tracks[0].attributes = attributes;
            write(&output, &input, &SyncOptions::default()).unwrap();
            let db_path = output.join("PIONEER/rekordbox/exportLibrary.db");
            let db = database::prepare(&db_path, &db_path).unwrap();
            assert_eq!(
                db.query_row("SELECT bitrate,trackNo FROM content", [], |row| {
                    Ok((row.get::<_, u32>(0)?, row.get::<_, u32>(1)?))
                })
                .unwrap(),
                (320, 7)
            );
            let tables =
                pdb::read_tables(&fs::read(output.join("PIONEER/rekordbox/export.pdb")).unwrap())
                    .unwrap();
            assert_eq!(pdb::read32(&tables[&0][0], 48).unwrap(), 320);
            assert_eq!(pdb::read32(&tables[&0][0], 52).unwrap(), 7);
        }
    }
    #[test]
    fn invalid_optional_numeric_metadata_does_not_commit_usb_catalog_changes() {
        use cuerithm_smartlists::FieldValue;
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("synthetic.wav");
        audio(&source);
        let input = library(&source, "metadata");
        let output = dir.path().join("usb");
        write(&output, &input, &SyncOptions::default()).unwrap();
        let db_path = output.join("PIONEER/rekordbox/exportLibrary.db");
        for (field, value) in [
            ("bitrate", FieldValue::Number(-1.)),
            ("bitrate", FieldValue::Number(f64::INFINITY)),
            ("bitrate", FieldValue::Number(4294967296.)),
            ("bitrate", FieldValue::Text("320".into())),
            ("trackNumber", FieldValue::Number(7.5)),
            ("trackNumber", FieldValue::Number(4294967296.)),
            ("trackNumber", FieldValue::Boolean(true)),
        ] {
            let mut invalid = input.clone();
            invalid.tracks[0].title = "must not commit".into();
            invalid.tracks[0].attributes.insert(field.into(), value);
            assert!(
                write(&output, &invalid, &SyncOptions::default()).is_err(),
                "{field}"
            );
            let db = database::prepare(&db_path, &db_path).unwrap();
            assert_eq!(
                db.query_row("SELECT title FROM content", [], |row| row
                    .get::<_, String>(0))
                    .unwrap(),
                "Synthetic metadata"
            );
        }
    }
    #[test]
    fn autofix_catalog_bitrate_matches_the_stereo_pcm_copy() {
        use cuerithm_smartlists::FieldValue;
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let runtime = MediaRuntime::from_bundle(&workspace.join("tools/media-runtime/bin"))
            .expect("Sealed runtime required for compatibility verification");
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("low-rate.wav");
        audio(&source);
        let mut bytes = fs::read(&source).unwrap();
        bytes[24..28].copy_from_slice(&16000_u32.to_le_bytes());
        bytes[28..32].copy_from_slice(&32000_u32.to_le_bytes());
        fs::write(&source, &bytes).unwrap();
        let mut input = library(&source, "converted");
        input.tracks[0].sample_rate = 16000;
        input.tracks[0].duration_seconds = 6.;
        input.tracks[0]
            .attributes
            .insert("bitrate".into(), FieldValue::Number(256.));
        let output = dir.path().join("usb");
        write_staged_with_runtime(
            &output,
            &output,
            &input,
            &SyncOptions {
                usb_cdj_compatibility: UsbCompatibility::AutoFix,
                ..Default::default()
            },
            Some(&runtime),
        )
        .unwrap();
        let db_path = output.join("PIONEER/rekordbox/exportLibrary.db");
        let db = database::prepare(&db_path, &db_path).unwrap();
        assert_eq!(
            db.query_row("SELECT samplingRate,bitrate FROM content", [], |row| {
                Ok((row.get::<_, u32>(0)?, row.get::<_, u32>(1)?))
            })
            .unwrap(),
            (44100, 1411)
        );
        let tables =
            pdb::read_tables(&fs::read(output.join("PIONEER/rekordbox/export.pdb")).unwrap())
                .unwrap();
        assert_eq!(pdb::read32(&tables[&0][0], 48).unwrap(), 1411);
        assert_eq!(input.tracks[0].attribute_number("bitrate"), Some(256.));
        assert_eq!(fs::read(source).unwrap(), bytes);
    }
    #[test]
    fn exports_playable_media_encrypted_database_and_complete_analysis_assets() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("synthetic.wav");
        audio(&source);
        let output = std::env::var_os("CUERITHM_USB_FIXTURE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| dir.path().join("usb"));
        let library = library(&source, "first");
        let options = SyncOptions {
            usb_cue_destination: UsbCueDestination::Both,
            extra_cue_markers: true,
            ..Default::default()
        };
        let files = write(&output, &library, &options).unwrap();
        assert!(
            files
                .iter()
                .any(|p| p.extension().is_some_and(|e| e == "pdb"))
        );
        for p in &files {
            assert!(
                output.join(p).is_file(),
                "Missing manifest asset: {}",
                p.display()
            );
        }
        let media = files.iter().find(|p| p.starts_with("Contents")).unwrap();
        assert_eq!(
            fs::read(output.join(media)).unwrap(),
            fs::read(&source).unwrap()
        );
        let db_path = output.join("PIONEER/rekordbox/exportLibrary.db");
        assert_ne!(&fs::read(&db_path).unwrap()[..16], b"SQLite format 3\0");
        let db = database::prepare(&db_path, &db_path).unwrap();
        let metadata: (String, u32, u32) = db
            .query_row("SELECT title,bpmx100,rating FROM content", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap();
        assert_eq!(metadata, ("Synthetic first".into(), 12000, 4));
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM cue", [], |r| r.get::<_, u32>(0))
                .unwrap(),
            4
        );
        let dat = files
            .iter()
            .find(|p| p.extension().is_some_and(|e| e == "DAT"))
            .unwrap();
        let parsed =
            AnlzFile::parse(&fs::read(output.join(dat)).unwrap(), ParseLimits::default()).unwrap();
        let types: Vec<_> = parsed.tags().iter().map(AnlzTag::fourcc).collect();
        for expected in [*b"PPTH", *b"PVBR", *b"PQTZ", *b"PWAV", *b"PWV2", *b"PCOB"] {
            assert!(types.contains(&expected));
        }
        assert_eq!(
            parsed.tags()[0].audio_path().unwrap(),
            format!("/{}", media.to_str().unwrap())
        );
        let grid = parsed
            .tags()
            .iter()
            .find(|t| t.fourcc() == *b"PQTZ")
            .unwrap()
            .beats()
            .unwrap();
        assert_eq!(grid[0].time_ms, 125);
        assert_eq!(grid.len(), 4);
        let ext = files
            .iter()
            .find(|p| p.extension().is_some_and(|e| e == "EXT"))
            .unwrap();
        let parsed =
            AnlzFile::parse(&fs::read(output.join(ext)).unwrap(), ParseLimits::default()).unwrap();
        for expected in [*b"PWV3", *b"PWV4", *b"PWV5", *b"PQT2", *b"PCO2"] {
            assert!(parsed.tags().iter().any(|t| t.fourcc() == expected));
        }
        let cues = parsed
            .tags()
            .iter()
            .filter(|t| t.fourcc() == *b"PCO2")
            .map(|t| t.cues().unwrap().entries.len())
            .sum::<usize>();
        assert_eq!(cues, 4);
        let modern = files
            .iter()
            .find(|p| p.extension().is_some_and(|e| e == "2EX"))
            .unwrap();
        let parsed = AnlzFile::parse(
            &fs::read(output.join(modern)).unwrap(),
            ParseLimits::default(),
        )
        .unwrap();
        let color = parsed
            .tags()
            .iter()
            .find(|t| t.fourcc() == *b"PWVC")
            .unwrap();
        assert_eq!(color.bytes().len(), 20);
        assert_eq!(&color.bytes()[14..], &[0, 134, 0, 177, 0, 156]);
        let raw = fs::read(output.join("PIONEER/rekordbox/export.pdb")).unwrap();
        let config = pdb::config(&raw).unwrap();
        assert_eq!(config[&16].len(), 27);
        assert_eq!(config[&19].len(), 1);
    }
    #[test]
    fn subset_sync_preserves_unrelated_tracks_playlists_and_device_configuration() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("synthetic.wav");
        audio(&source);
        let original = dir.path().join("original");
        write(
            &original,
            &library(&source, "first"),
            &SyncOptions::default(),
        )
        .unwrap();
        let db_path = original.join("PIONEER/rekordbox/exportLibrary.db");
        let db = database::prepare(&db_path, &db_path).unwrap();
        db.execute(
            "UPDATE property SET deviceName='My USB',backGroundColorType=7",
            [],
        )
        .unwrap();
        drop(db);
        let staged = dir.path().join("stage");
        let files = write_staged(
            &staged,
            &original,
            &library(&source, "second"),
            &SyncOptions::default(),
        )
        .unwrap();
        let db_path = staged.join("PIONEER/rekordbox/exportLibrary.db");
        let db = database::prepare(&db_path, &db_path).unwrap();
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM content", [], |r| r.get::<_, u32>(0))
                .unwrap(),
            2
        );
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM playlist WHERE attribute=0", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            2
        );
        let settings: (String, u32) = db
            .query_row(
                "SELECT deviceName,backGroundColorType FROM property",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(settings, ("My USB".into(), 7));
        assert_eq!(
            files.iter().filter(|p| p.starts_with("Contents")).count(),
            2
        );
    }
    #[test]
    fn merges_existing_pdb_only_catalog_without_migration() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("synthetic.wav");
        audio(&source);
        let original = dir.path().join("original");
        write(
            &original,
            &library(&source, "first"),
            &SyncOptions::default(),
        )
        .unwrap();
        fs::remove_file(original.join("PIONEER/rekordbox/exportLibrary.db")).unwrap();
        let staged = dir.path().join("stage");
        write_staged(
            &staged,
            &original,
            &library(&source, "second"),
            &SyncOptions::default(),
        )
        .unwrap();
        let db_path = staged.join("PIONEER/rekordbox/exportLibrary.db");
        let db = database::prepare(&db_path, &db_path).unwrap();
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM content", [], |r| r.get::<_, u32>(0))
                .unwrap(),
            2
        );
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM playlist WHERE attribute=0", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            2
        );
        let artist: String = db
            .query_row("SELECT name FROM artist", [], |r| r.get(0))
            .unwrap();
        assert_eq!(artist, "Artist é");
    }
    #[test]
    fn detect_reports_wav_header_issues_and_autofix_needs_runtime() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("synthetic.wav");
        audio(&source);
        let mut data = fs::read(&source).unwrap();
        data[20..22].copy_from_slice(&3_u16.to_le_bytes());
        fs::write(&source, data).unwrap();
        let issues = compatibility::inspect(&library(&source, "float").tracks[0], None).unwrap();
        assert!(
            issues
                .iter()
                .any(|i| i.fixable && i.message.contains("floating-point"))
        );
        let options = SyncOptions {
            usb_cdj_compatibility: UsbCompatibility::AutoFix,
            ..Default::default()
        };
        let error = write(
            &dir.path().join("output"),
            &library(&source, "float"),
            &options,
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("requires the bundled media runtime")
        );
    }
    #[test]
    #[ignore = "requires an explicit hash-verified test media runtime"]
    fn autofix_preserves_stereo_cues_and_source_bytes() {
        let runtime = MediaRuntime::from_bundle(Path::new(
            &std::env::var_os("CUERITHM_MEDIA_RUNTIME").expect("Set the test runtime path"),
        ))
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("float.wav");
        let frames = 48000_u32 * 2;
        let payload = frames * 8;
        let mut bytes = b"RIFF".to_vec();
        bytes.extend((payload + 36).to_le_bytes());
        bytes.extend(b"WAVEfmt ");
        bytes.extend(16_u32.to_le_bytes());
        bytes.extend(3_u16.to_le_bytes());
        bytes.extend(2_u16.to_le_bytes());
        bytes.extend(48000_u32.to_le_bytes());
        bytes.extend(384000_u32.to_le_bytes());
        bytes.extend(8_u16.to_le_bytes());
        bytes.extend(32_u16.to_le_bytes());
        bytes.extend(b"data");
        bytes.extend(payload.to_le_bytes());
        for n in 0..frames {
            let phase = n as f32 * std::f32::consts::TAU * 220.0 / 48000.0;
            bytes.extend((phase.sin() * 0.5).to_le_bytes());
            bytes.extend((phase.cos() * 0.5).to_le_bytes());
        }
        fs::write(&source, &bytes).unwrap();
        let mut lib = library(&source, "float");
        lib.tracks[0].color = Some(0xff0000);
        let output = dir.path().join("output");
        let options = SyncOptions {
            usb_cdj_compatibility: UsbCompatibility::AutoFix,
            ..Default::default()
        };
        let report =
            write_staged_with_runtime(&output, &output, &lib, &options, Some(&runtime)).unwrap();
        assert!(
            report
                .warnings
                .iter()
                .any(|w| w.contains("converted to stereo PCM WAV"))
        );
        assert_eq!(fs::read(&source).unwrap(), bytes);
        let wav = report
            .files
            .iter()
            .find(|p| p.starts_with("Contents"))
            .unwrap();
        let fixed = fs::read(output.join(wav)).unwrap();
        assert_eq!(u16::from_le_bytes(fixed[20..22].try_into().unwrap()), 1);
        assert_eq!(u16::from_le_bytes(fixed[22..24].try_into().unwrap()), 2);
        assert_eq!(u32::from_le_bytes(fixed[24..28].try_into().unwrap()), 44100);
        assert_ne!(&fixed[44..46], &fixed[46..48]);
        let db_path = output.join("PIONEER/rekordbox/exportLibrary.db");
        let db = database::prepare(&db_path, &db_path).unwrap();
        assert_eq!(
            db.query_row("SELECT samplingRate FROM content", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            44100
        );
        assert_eq!(
            db.query_row("SELECT inUsec FROM cue WHERE kind=3", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            250000
        );
    }
    #[test]
    fn pdb_rejects_corrupt_offsets_and_row_overflow() {
        assert!(pdb::config(&[0; 20]).is_err());
        let mut tables = pdb::Tables::new();
        tables.insert(0, vec![vec![0; 4096]]);
        assert!(pdb::build(&tables).is_err());
    }
    #[test]
    fn real_mp3_profile_moves_cues_grid_and_detail_frames_once_and_preserves_audio() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let runtime = MediaRuntime::from_bundle(&workspace.join("tools/media-runtime/bin"))
            .expect("Sealed runtime required for compressed timing verification");
        let dir = tempfile::tempdir().unwrap();
        tar::Archive::new(flate2::read::GzDecoder::new(
            fs::File::open(workspace.join("tools/fixtures/media.tar.gz")).unwrap(),
        ))
        .unpack(dir.path().join("fixtures"))
        .unwrap();
        let source = dir.path().join("fixtures/pulses.mp3");
        let audio = runtime.decode(&source, &dir.path().join("decode")).unwrap();
        let profile = audio.export_timing.clone().expect("Measured MP3 profile");
        let mut input = library(&source, "timing");
        let track = &mut input.tracks[0];
        track.sample_rate = audio.info.sample_rate;
        track.duration_seconds = audio.clock.duration_seconds();
        track.export_timing = Some(profile.clone());
        track.cues = vec![
            crate::SyncCue {
                index: 0,
                start_seconds: 0.001,
                name: "Cue".into(),
                ..Default::default()
            },
            crate::SyncCue {
                index: 1,
                start_seconds: 0.01,
                end_seconds: Some(0.02),
                name: "Loop".into(),
                ..Default::default()
            },
        ];
        track.beatgrid = vec![crate::SyncBeat {
            seconds: 0.001,
            bpm: 120.,
            beat: 1,
        }];
        let offset = crate::timing::offset_seconds(track, crate::DjSoftware::PioneerUsb).unwrap();
        let canonical = serde_json::to_vec(&input).unwrap();
        let original = fs::read(&source).unwrap();
        let one = dir.path().join("one");
        let report =
            write_staged_with_runtime(&one, &one, &input, &SyncOptions::default(), Some(&runtime))
                .unwrap();
        let dat = report
            .files
            .iter()
            .find(|p| p.extension().is_some_and(|e| e == "DAT"))
            .unwrap();
        let parsed =
            AnlzFile::parse(&fs::read(one.join(dat)).unwrap(), ParseLimits::default()).unwrap();
        let grid = parsed
            .tags()
            .iter()
            .find(|t| t.fourcc() == *b"PQTZ")
            .unwrap()
            .beats()
            .unwrap();
        assert_eq!(
            grid[0].time_ms,
            ((0.001 + offset) * 1000. + 1e-6).floor() as u32
        );
        let pvbr = parsed
            .tags()
            .iter()
            .find(|t| t.fourcc() == *b"PVBR")
            .unwrap()
            .bytes();
        assert_eq!(
            u32::from_be_bytes(pvbr[pvbr.len() - 4..].try_into().unwrap()),
            profile
                .decoded_sample_count
                .and_then(|n| u32::try_from(n).ok())
                .unwrap_or(0)
        );
        let ext = report
            .files
            .iter()
            .find(|p| p.extension().is_some_and(|e| e == "EXT"))
            .unwrap();
        let parsed =
            AnlzFile::parse(&fs::read(one.join(ext)).unwrap(), ParseLimits::default()).unwrap();
        let cue = parsed
            .tags()
            .iter()
            .filter(|t| t.fourcc() == *b"PCO2")
            .flat_map(|t| t.cues().unwrap().entries)
            .find(|c| c.hot_cue == 1)
            .unwrap();
        assert_eq!(
            cue.time_ms,
            ((0.001 + offset) * 1000. + 1e-6).floor() as u32
        );
        let detail = parsed
            .tags()
            .iter()
            .find(|t| t.fourcc() == *b"PWV3")
            .unwrap()
            .bytes();
        let count = u32::from_be_bytes(detail[16..20].try_into().unwrap());
        assert_eq!(
            count,
            (audio.frames / 294) as u32 + (150. * offset).round() as u32
        );
        let two = dir.path().join("two");
        let again =
            write_staged_with_runtime(&two, &one, &input, &SyncOptions::default(), Some(&runtime))
                .unwrap();
        for file in report.files.iter().filter(|p| {
            p.extension()
                .is_some_and(|e| matches!(e.to_str(), Some("DAT" | "EXT" | "2EX")))
        }) {
            assert!(again.files.contains(file));
            assert_eq!(
                fs::read(one.join(file)).unwrap(),
                fs::read(two.join(file)).unwrap()
            );
        }
        assert_eq!(serde_json::to_vec(&input).unwrap(), canonical);
        assert_eq!(fs::read(&source).unwrap(), original);
    }
}
