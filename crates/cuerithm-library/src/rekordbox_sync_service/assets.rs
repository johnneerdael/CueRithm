use super::*;
use cuerithm_rekordbox::anlz::{AnlzFile, ParseLimits};

pub(super) struct Bundle {
    pub analysis_path: Option<String>,
    pub artwork_path: Option<String>,
    pub sources: Vec<AssetSource>,
    pub publications: Vec<FilePublication>,
}
fn relative(path: &str) -> Result<PathBuf> {
    let text = path.replace('\\', "/");
    ensure!(
        !text.starts_with("//") && !text.contains(':'),
        "Invalid relative native asset reference"
    );
    let path = PathBuf::from(text.trim_start_matches('/'));
    ensure!(
        !path.as_os_str().is_empty()
            && path
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_))),
        "Invalid native asset reference"
    );
    Ok(path)
}
fn asset_relative_resolved(
    target: &RekordboxSyncTarget,
    paths: &TargetPathResolver<'_>,
    path: &str,
) -> Result<PathBuf> {
    if path.as_bytes().get(1) == Some(&b':') || path.starts_with("\\\\") || path.starts_with("//") {
        let mapped = paths.to_host(Path::new(path))?;
        let root = target.asset_root.canonicalize()?;
        let mut physical = None;
        let mut logical_root = None;
        // Compare physical roots while retaining native share-relative spelling,
        // including PIONEER paths through contained directory aliases. Resolve an
        // absent output suffix through its nearest existing ancestor.
        for ancestor in mapped.ancestors() {
            let canonical = match ancestor.canonicalize() {
                Ok(canonical) => canonical,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error).context("Native asset reference is unavailable"),
            };
            if physical.is_none() {
                physical = Some(canonical.join(mapped.strip_prefix(ancestor)?));
            }
            if canonical == root {
                logical_root = Some(ancestor);
                break;
            }
        }
        ensure!(
            physical.is_some_and(|physical| physical.starts_with(&root)),
            "Native asset reference escaped selected share directory"
        );
        let relative = mapped.strip_prefix(
            logical_root.context("Native asset reference escaped selected share directory")?,
        )?;
        ensure!(
            !relative.as_os_str().is_empty()
                && relative
                    .components()
                    .all(|part| matches!(part, std::path::Component::Normal(_))),
            "Invalid native asset reference"
        );
        return Ok(relative.to_owned());
    }
    relative(path)
}
// Native image paths resolve under the selected share directory.
fn artwork_relative_resolved(
    target: &RekordboxSyncTarget,
    paths: &TargetPathResolver<'_>,
    path: &str,
) -> Result<PathBuf> {
    let relative = asset_relative_resolved(target, paths, path)?;
    ensure!(
        relative.starts_with("PIONEER/Artwork"),
        "Invalid native artwork resource reference"
    );
    Ok(relative)
}
fn source(root: &Path, path: &Path) -> Result<PathBuf> {
    let joined = root.join(path);
    let resolved = joined
        .canonicalize()
        .context("Native analysis resource is missing")?;
    ensure!(
        resolved.starts_with(root.canonicalize()?)
            && !fs::symlink_metadata(&joined)?.file_type().is_symlink(),
        "Native resource escaped its asset root"
    );
    Ok(resolved)
}
fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    fs::create_dir_all(path.parent().context("Staged asset parent missing")?)?;
    let mut f = File::options().write(true).create_new(true).open(path)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    Ok(())
}
fn publication(
    library: &Library,
    id: &str,
) -> Result<camelot_core::lexicon_publication::Publication> {
    let connection = library.conn()?;
    let (analysis,latest):(String,Option<String>)=connection.query_row("SELECT analysis_json,(SELECT json FROM cue_revisions WHERE track_id=tracks.id ORDER BY created DESC,rowid DESC LIMIT 1) FROM tracks WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?)))?;
    let analysis: Value = serde_json::from_str(&analysis)?;
    let latest = latest
        .map(|v| serde_json::from_str::<Value>(&v))
        .transpose()?;
    let publication = latest
        .as_ref()
        .filter(|v| v["schema"] == "lexicon-publication-v1")
        .unwrap_or(&analysis["publication"]);
    Ok(serde_json::from_value(publication.clone())?)
}
pub(super) fn cue_observation(library: &Library, id: &str) -> Result<Value> {
    Ok(serde_json::to_value(publication(library, id)?.cues)?)
}
pub(super) struct CueProjection {
    pub cues: Vec<NativeCue>,
    pub colors: Vec<NativeCueColor>,
    pub converted: usize,
}
pub(super) fn cues(
    library: &Library,
    track: &SyncTrack,
    allow_nearest: bool,
) -> Result<CueProjection> {
    let publication = publication(library, &track.id)?;
    ensure!(
        publication.cues.len() == track.cues.len(),
        "Saved native cue publication changed"
    );
    let mut colors = vec![];
    let mut converted = 0;
    let cues = publication
        .cues
        .iter()
        .zip(&track.cues)
        .enumerate()
        .map(|(i, (saved, cue))| {
            let memory = saved
                .data
                .as_ref()
                .is_some_and(|data| data["rekordbox"]["originalNum"] == -1);
            ensure!(
                memory || cue.index < 8,
                "Saved hot cue position exceeds rekordbox's eight slots"
            );
            let slot = if memory {
                NativeCueSlot::Memory
            } else {
                NativeCueSlot::Hot(cue.index)
            };
            let color = project_native_cue_color(
                saved.color.as_str().unwrap_or(""),
                cue.color,
                slot,
                cue.end_seconds.is_some(),
                allow_nearest,
            )?;
            converted += usize::from(color.converted);
            colors.push(color.sql);
            Ok(NativeCue {
                slot,
                start: NativeTime::from_seconds(cue.start_seconds)?,
                end: cue.end_seconds.map(NativeTime::from_seconds).transpose()?,
                active_loop: saved.active_loop.unwrap_or(false),
                order_first: u16::try_from(i)?,
                order_last: u16::try_from(i)?,
                comment: cue.name.clone(),
                memory_color: color.memory_color,
                color: color.rgba,
                loop_fraction: (0, 0),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(CueProjection {
        cues,
        colors,
        converted,
    })
}
fn entry_owned(
    entry: &cuerithm_rekordbox::anlz::CueEntry,
    remote: &ImportedTrack,
    owned: &BTreeSet<String>,
) -> bool {
    remote.cues.iter().any(|cue| {
        owned.contains(&cue.id)
            && entry.hot_cue
                == if cue.memory {
                    0
                } else {
                    u32::from(cue.slot.unwrap_or(255)) + 1
                }
            && cue
                .start_seconds
                .and_then(|v| NativeTime::from_seconds(v).ok())
                .is_some_and(|v| v.millis() == entry.time_ms)
            && match cue.end_seconds {
                None => entry.cue_type == 1,
                Some(end) => NativeTime::from_seconds(end)
                    .is_ok_and(|end| entry.cue_type == 2 && end.millis() == entry.loop_time_ms),
            }
    })
}
#[allow(clippy::too_many_arguments)]
pub(super) fn build(
    library: &Library,
    target: &RekordboxSyncTarget,
    paths: &TargetPathResolver<'_>,
    id: &str,
    track: &SyncTrack,
    native_audio_path: &Path,
    artwork_source_path: &Path,
    remote: Option<&ImportedTrack>,
    native_cues: &[NativeCue],
    owned: &BTreeSet<String>,
    write_cues: bool,
    write_grid: bool,
    rewrite_audio_reference: bool,
    regenerate_waveforms: bool,
    write_artwork: bool,
    clear_artwork: bool,
    runtime: Option<&cuerithm_media::MediaRuntime>,
    cancel: &CancellationToken,
) -> Result<Bundle> {
    let physical_audio_path = native_audio_path;
    let native_audio_reference =
        audio_reference_resolved(paths, native_audio_path, remote, rewrite_audio_reference)?;
    let native_audio_path = native_audio_reference.as_path();
    let workspace = root(library)
        .join("operations")
        .join(id)
        .join("build")
        .join(digest(&track.id)?);
    fs::create_dir_all(&workspace)?;
    let component = &digest(&track.id)?[..24];
    let new_relative = PathBuf::from(format!(
        "PIONEER/USBANLZ/CUERITHM/{id}/{component}/ANLZ0000.DAT"
    ));
    let mut result = Bundle {
        analysis_path: None,
        artwork_path: None,
        sources: vec![],
        publications: vec![],
    };
    if write_cues || write_grid || rewrite_audio_reference || remote.is_none() {
        if let Some(old) = remote
            .and_then(|r| r.source.analysis_path.as_deref())
            .filter(|p| !p.is_empty())
        {
            let old = asset_relative_resolved(target, paths, old)?;
            let regenerated = if regenerate_waveforms {
                ensure!(
                    ["DAT", "EXT", "2EX"].iter().all(|extension| target
                        .asset_root
                        .join(old.with_extension(extension))
                        .is_file()),
                    "Native analysis bundle is incomplete; analyze the track in rekordbox before replacing its source"
                );
                Some(cuerithm_dj_sync::native_rekordbox_analysis(
                    track,
                    native_cues,
                    runtime
                        .context("Updated native waveforms require the bundled media runtime")?,
                    cancel,
                )?)
            } else {
                None
            };
            let replacement_grid = write_grid
                .then(|| cuerithm_dj_sync::native_rekordbox_grid(track))
                .transpose()?;
            let mut primary_grid = None;
            let mut edits = vec![];
            for extension in ["DAT", "EXT", "2EX", "3EX"] {
                let relative_path = old.with_extension(extension);
                if !target.asset_root.join(&relative_path).exists() {
                    continue;
                }
                let path = source(&target.asset_root, &relative_path)?;
                ensure_not_native_file(&target.database, &path)?;
                let hash = file_sha256_cancellable(&path, cancel)?;
                result.sources.push(AssetSource {
                    path: path.clone(),
                    hash: hash.clone(),
                });
                let content = if extension == "3EX" {
                    NativeAssetContent::OpaqueExisting
                } else {
                    let file = AnlzFile::parse(&fs::read(&path)?, ParseLimits::default())?;
                    let mut changes = vec![];
                    for (index, tag) in file.tags().iter().enumerate() {
                        match &tag.fourcc() {
                            b"PPTH"
                                if tag.audio_path()?.as_str()
                                    != native_audio_path
                                        .to_str()
                                        .context("Native audio path is not Unicode")?
                                    && (rewrite_audio_reference
                                        || !same_native_audio_resolved(
                                            paths,
                                            &tag.audio_path()?,
                                            physical_audio_path,
                                        )) =>
                            {
                                changes.push(NativeAnlzEdit::AudioPath {
                                    tag_index: index,
                                    path: native_audio_path.to_str().unwrap().into(),
                                })
                            }
                            b"PCOB" | b"PCO2" if write_cues => {
                                let list = tag.cues()?;
                                let proofs = list
                                    .entries
                                    .iter()
                                    .filter(|entry| entry_owned(entry, remote.unwrap(), owned))
                                    .map(|entry| format!("{:x}", Sha256::digest(&entry.raw)))
                                    .collect();
                                let replacement = native_cues
                                    .iter()
                                    .filter(|cue| {
                                        u32::from(matches!(cue.slot, NativeCueSlot::Hot(_)))
                                            == list.kind
                                    })
                                    .cloned()
                                    .collect();
                                changes.push(NativeAnlzEdit::Cues {
                                    tag_index: index,
                                    owned_record_sha256: proofs,
                                    replacement,
                                });
                            }
                            b"PQTZ" if write_grid => {
                                if extension == "DAT" {
                                    ensure!(
                                        primary_grid.is_none(),
                                        "Ambiguous native primary beatgrid"
                                    );
                                    primary_grid = Some(tag.beats()?);
                                }
                                changes.push(NativeAnlzEdit::Grid {
                                    tag_index: index,
                                    beats: replacement_grid.as_ref().unwrap().beats.clone(),
                                });
                            }
                            b"PQT2" if write_grid => {
                                let primary = primary_grid.as_ref().context(
                                    "Native extended beatgrid has no companion DAT grid",
                                )?;
                                tag.native_microsecond_grid(primary)?;
                                let replacement = replacement_grid.as_ref().unwrap();
                                changes.push(NativeAnlzEdit::NativeMicrosecondGrid {
                                    tag_index: index,
                                    primary: primary.clone(),
                                    beats: replacement.beats.clone(),
                                    fractions: replacement.fractions.clone(),
                                    variable: replacement.variable,
                                });
                            }
                            _ => {}
                        }
                    }
                    if let Some(generated) = &regenerated {
                        let bytes = match extension {
                            "DAT" => &generated.dat,
                            "EXT" => &generated.ext,
                            "2EX" => &generated.two_ex,
                            _ => unreachable!(),
                        };
                        changes.push(NativeAnlzEdit::Waveforms {
                            template: native_waveform_template(bytes)?,
                        });
                    }
                    NativeAssetContent::Analysis {
                        template: None,
                        edits: changes,
                    }
                };
                edits.push(NativeAssetEdit {
                    owner_content_id: component.into(),
                    relative_path,
                    expected_sha256: Some(hash),
                    content,
                });
            }
            ensure!(
                edits
                    .iter()
                    .any(|e| e.relative_path.extension().is_some_and(|x| x == "DAT")),
                "Native DAT resource is missing"
            );
            let staged = stage_native_assets(
                &NativeAssetPlan {
                    source_root: target.asset_root.clone(),
                    edits,
                },
                &workspace,
                cancel,
            )?;
            if staged.manifest.iter().any(|proof| proof.changed) {
                for proof in &staged.manifest {
                    let extension = proof
                        .relative_path
                        .extension()
                        .context("Native resource extension missing")?;
                    let destination = new_relative.with_extension(extension);
                    let staged_path =
                        workspace.join(format!("ANLZ0000.{}", extension.to_string_lossy()));
                    let input = proof
                        .staged_path
                        .clone()
                        .unwrap_or(target.asset_root.join(&proof.relative_path));
                    fs::copy(&input, &staged_path)?;
                    File::options()
                        .read(true)
                        .write(true)
                        .open(&staged_path)?
                        .sync_all()?;
                    ensure!(
                        file_sha256_cancellable(&staged_path, cancel)? == proof.after_sha256,
                        "Versioned native resource proof mismatch"
                    );
                    result.publications.push(FilePublication {
                        target: target.asset_root.join(&destination),
                        before_sha256: None,
                        staged: Some(staged_path),
                    });
                }
                result.analysis_path = Some(format!(
                    "/{}",
                    new_relative
                        .to_str()
                        .context("Native resource path is not Unicode")?
                ));
            }
        } else {
            ensure!(
                write_cues || remote.is_none_or(|r| r.cues.is_empty()),
                "Native analysis is missing; explicitly review replacement of the listed native cues before generating a new bundle"
            );
            let runtime =
                runtime.context("New rekordbox tracks require the bundled media runtime")?;
            let generated =
                cuerithm_dj_sync::native_rekordbox_analysis(track, native_cues, runtime, cancel)?;
            for (extension, bytes) in [
                ("DAT", generated.dat),
                ("EXT", generated.ext),
                ("2EX", generated.two_ex),
            ] {
                let mut file = AnlzFile::parse(&bytes, ParseLimits::default())?;
                for index in 0..file.tags().len() {
                    let tag = &file.tags()[index];
                    let replacement = if tag.fourcc() == *b"PPTH" {
                        Some(
                            tag.with_audio_path(
                                native_audio_path
                                    .to_str()
                                    .context("Native audio path is not Unicode")?,
                            )?,
                        )
                    } else if !write_grid && tag.fourcc() == *b"PQTZ" {
                        Some(tag.with_beats(&[])?)
                    } else {
                        None
                    };
                    if let Some(tag) = replacement {
                        file.replace(index, tag)?;
                    }
                }
                let bytes = file.to_bytes()?;
                let staged = workspace.join(format!("ANLZ0000.{extension}"));
                write(&staged, &bytes)?;
                result.publications.push(FilePublication {
                    target: target
                        .asset_root
                        .join(new_relative.with_extension(extension)),
                    before_sha256: None,
                    staged: Some(staged),
                });
            }
            result.analysis_path = Some(format!(
                "/{}",
                new_relative
                    .to_str()
                    .context("Native resource path is not Unicode")?
            ));
        }
    }
    if clear_artwork {
        result.artwork_path = Some(String::new());
    } else if write_artwork {
        let runtime = runtime.context("Artwork export requires the bundled media runtime")?;
        if let Some(art) =
            cuerithm_dj_sync::native_rekordbox_artwork(artwork_source_path, runtime, cancel)?
        {
            let mut same = false;
            if let Some(old) = remote
                .and_then(|r| r.source.artwork_path.as_deref())
                .filter(|p| !p.is_empty())
            {
                let old = artwork_relative_resolved(target, paths, old)?;
                let mut equal = true;
                for (suffix, bytes) in [("", &art.main), ("_m", &art.medium), ("_s", &art.small)] {
                    let relative = artwork_sibling(&old, suffix)?;
                    if !target.asset_root.join(&relative).exists() {
                        equal = false;
                        continue;
                    }
                    let path = source(&target.asset_root, &relative)?;
                    ensure_not_native_file(&target.database, &path)?;
                    let hash = file_sha256_cancellable(&path, cancel)?;
                    equal &= hash == format!("{:x}", Sha256::digest(bytes));
                    result.sources.push(AssetSource { path, hash });
                }
                same = equal;
            }
            if !same {
                let main = PathBuf::from(format!("PIONEER/Artwork/CUERITHM/{id}/{component}.jpg"));
                for (suffix, bytes) in [("", art.main), ("_m", art.medium), ("_s", art.small)] {
                    let relative = artwork_sibling(&main, suffix)?;
                    let staged = workspace.join(format!("artwork{suffix}.jpg"));
                    write(&staged, &bytes)?;
                    result.publications.push(FilePublication {
                        target: target.asset_root.join(relative),
                        before_sha256: None,
                        staged: Some(staged),
                    });
                }
                result.artwork_path = Some(format!("/{}", main.to_str().unwrap()));
            }
        }
    }
    Ok(result)
}
fn artwork_sibling(path: &Path, suffix: &str) -> Result<PathBuf> {
    let stem = path
        .file_stem()
        .and_then(|v| v.to_str())
        .context("Invalid native artwork filename")?;
    let extension = path
        .extension()
        .and_then(|v| v.to_str())
        .context("Invalid native artwork extension")?;
    Ok(path.with_file_name(format!("{stem}{suffix}.{extension}")))
}
pub(super) fn add_publications(
    transaction: &FileTransaction,
    publications: &[FilePublication],
) -> Result<()> {
    let mut directories = BTreeSet::new();
    for item in publications {
        ensure!(
            file_hash(&item.target)?.is_none(),
            "Immutable native destination already exists"
        );
        let mut parent = item.target.parent();
        while let Some(path) = parent {
            if path.exists() {
                break;
            }
            directories.insert(path.to_owned());
            parent = path.parent();
        }
    }
    let directories = directories
        .into_iter()
        .map(|target| {
            Ok(DirectoryChange {
                before_manifest: directory_manifest(&target)?,
                target,
                after_exists: true,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    transaction.add_batch(publications, &directories)
}

pub(super) fn grid_observation_resolved(
    target: &RekordboxSyncTarget,
    paths: &TargetPathResolver<'_>,
    path: &str,
) -> Result<Value> {
    let file = source(
        &target.asset_root,
        &asset_relative_resolved(target, paths, path)?,
    )?;
    ensure_not_native_file(&target.database, &file)?;
    let parsed = AnlzFile::parse(&fs::read(file)?, ParseLimits::default())?;
    let tag = parsed
        .tags()
        .iter()
        .find(|t| t.fourcc() == *b"PQTZ")
        .context("Native grid observation is unavailable")?;
    Ok(serde_json::to_value(tag.beats()?)?)
}
pub(super) fn artwork_observation_resolved(
    target: &RekordboxSyncTarget,
    paths: &TargetPathResolver<'_>,
    path: &str,
    cancel: &CancellationToken,
) -> Result<Value> {
    let file = source(
        &target.asset_root,
        &artwork_relative_resolved(target, paths, path)?,
    )?;
    ensure_not_native_file(&target.database, &file)?;
    Ok(json!(file_sha256_cancellable(&file, cancel)?))
}

#[cfg(test)]
fn asset_relative(target: &RekordboxSyncTarget, path: &str) -> Result<PathBuf> {
    let paths = target.environment.resolver()?;
    let relative = asset_relative_resolved(target, &paths, path)?;
    paths.verify()?;
    Ok(relative)
}
#[cfg(test)]
fn artwork_relative(target: &RekordboxSyncTarget, path: &str) -> Result<PathBuf> {
    let paths = target.environment.resolver()?;
    let relative = artwork_relative_resolved(target, &paths, path)?;
    paths.verify()?;
    Ok(relative)
}
#[cfg(test)]
pub(super) fn artwork_observation(
    target: &RekordboxSyncTarget,
    path: &str,
    cancel: &CancellationToken,
) -> Result<Value> {
    let paths = target.environment.resolver()?;
    let value = artwork_observation_resolved(target, &paths, path, cancel)?;
    paths.verify()?;
    Ok(value)
}
#[cfg(test)]
mod environment_tests {
    use super::*;
    #[test]
    fn windows_asset_references_resolve_only_inside_selected_share() {
        let dir = tempfile::tempdir().unwrap();
        let host = dir.path().canonicalize().unwrap();
        let share = host.join("share");
        fs::create_dir(&share).unwrap();
        fs::write(share.join("old.DAT"), b"authored asset").unwrap();
        let target = RekordboxSyncTarget {
            id: "authored".into(),
            installation: TargetInstallation {
                root: host.clone(),
                executable: host.join("rekordbox.exe"),
                version: "7.1".into(),
                major: 7,
                version_source: "explicit_version_hint".into(),
            },
            database: host.join("master.db"),
            options_file: host.join("options.json"),
            asset_root: share.clone(),
            environment: TargetEnvironment::WindowsCompatibility {
                prefix: host.clone(),
                installation: None,
                mappings: vec![DriveMapping {
                    windows_root: "C:\\".into(),
                    host_root: host,
                }],
            },
        };
        assert_eq!(
            asset_relative(&target, r"C:\share\old.DAT").unwrap(),
            Path::new("old.DAT")
        );
        assert_eq!(
            artwork_relative(&target, r"\PIONEER\Artwork\cover.jpg").unwrap(),
            Path::new("PIONEER/Artwork/cover.jpg")
        );
        assert_eq!(
            asset_relative(&target, r"C:\share\PIONEER\USBANLZ\future\ANLZ0000.DAT").unwrap(),
            Path::new("PIONEER/USBANLZ/future/ANLZ0000.DAT")
        );
        assert!(asset_relative(&target, r"C:\outside.DAT").is_err());
        assert!(asset_relative(&target, r"\\server\share\old.DAT").is_err());
        assert!(asset_relative(&target, r"PIONEER\..\outside.DAT").is_err());
        assert!(artwork_relative(&target, "/PIONEER/USBANLZ/old.DAT").is_err());
        assert_eq!(
            source(
                &share,
                &asset_relative(&target, r"C:\share\old.DAT").unwrap()
            )
            .unwrap(),
            share.join("old.DAT")
        );
    }
    #[cfg(unix)]
    #[test]
    fn mapped_asset_alias_uses_physical_containment_and_preserves_logical_share_path() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let host = dir.path().canonicalize().unwrap();
        let physical = host.join("physical");
        let selected = host.join("selected");
        let share = physical.join("share");
        fs::create_dir_all(share.join("PIONEER/Artwork")).unwrap();
        fs::create_dir_all(share.join("PIONEER/USBANLZ")).unwrap();
        fs::write(share.join("PIONEER/Artwork/cover.jpg"), b"authored artwork").unwrap();
        fs::write(share.join("PIONEER/USBANLZ/old.DAT"), b"authored analysis").unwrap();
        fs::write(physical.join("outside.DAT"), b"outside selected share").unwrap();
        symlink(&physical, &selected).unwrap();
        let target = RekordboxSyncTarget {
            id: "authored-alias".into(),
            installation: TargetInstallation {
                root: physical.clone(),
                executable: physical.join("rekordbox.exe"),
                version: "7.1".into(),
                major: 7,
                version_source: "explicit_version_hint".into(),
            },
            database: physical.join("master.db"),
            options_file: physical.join("options.json"),
            asset_root: share.canonicalize().unwrap(),
            environment: TargetEnvironment::WindowsCompatibility {
                prefix: host,
                installation: None,
                mappings: vec![DriveMapping {
                    windows_root: "C:\\".into(),
                    host_root: selected,
                }],
            },
        };
        let analysis = asset_relative(&target, r"C:\share\PIONEER\USBANLZ\old.DAT").unwrap();
        assert_eq!(analysis, Path::new("PIONEER/USBANLZ/old.DAT"));
        assert_eq!(
            source(&target.asset_root, &analysis).unwrap(),
            share.join("PIONEER/USBANLZ/old.DAT")
        );
        let artwork = artwork_relative(&target, r"C:\share\PIONEER\Artwork\cover.jpg").unwrap();
        assert_eq!(artwork, Path::new("PIONEER/Artwork/cover.jpg"));
        assert_eq!(
            artwork_observation(
                &target,
                r"C:\share\PIONEER\Artwork\cover.jpg",
                &CancellationToken::default()
            )
            .unwrap(),
            json!(
                file_sha256_cancellable(
                    &share.join("PIONEER/Artwork/cover.jpg"),
                    &CancellationToken::default()
                )
                .unwrap()
            )
        );
        assert!(asset_relative(&target, r"C:\outside.DAT").is_err());
        symlink(physical.join("outside.DAT"), share.join("escape.DAT")).unwrap();
        assert!(asset_relative(&target, r"C:\share\escape.DAT").is_err());
        symlink(
            share.join("PIONEER/USBANLZ/old.DAT"),
            share.join("alias.DAT"),
        )
        .unwrap();
        assert!(
            asset_relative(&target, r"C:\share\alias.DAT")
                .and_then(|alias| source(&target.asset_root, &alias))
                .is_err(),
            "Final symbolic native asset files remain refused"
        );
        assert_eq!(
            asset_relative(&target, "/PIONEER/USBANLZ/future/ANLZ0000.DAT").unwrap(),
            Path::new("PIONEER/USBANLZ/future/ANLZ0000.DAT")
        );
    }

    #[cfg(windows)]
    #[test]
    fn ordinary_windows_asset_references_match_verbatim_canonical_share_roots() {
        let dir = tempfile::tempdir().unwrap();
        let canonical = dir.path().canonicalize().unwrap();
        let share = canonical.join("share");
        fs::create_dir(&share).unwrap();
        fs::write(share.join("old.DAT"), b"authored analysis").unwrap();
        let ordinary = cuerithm_dj_sync::destination_reference(&share.join("old.DAT"));
        let target = RekordboxSyncTarget {
            id: "authored-windows".into(),
            installation: TargetInstallation {
                root: canonical.clone(),
                executable: canonical.join("rekordbox.exe"),
                version: "7.1".into(),
                major: 7,
                version_source: "explicit_version_hint".into(),
            },
            database: canonical.join("master.db"),
            options_file: canonical.join("options.json"),
            asset_root: share.canonicalize().unwrap(),
            environment: TargetEnvironment::Native,
        };
        assert_eq!(
            asset_relative(&target, ordinary.to_str().unwrap()).unwrap(),
            Path::new("old.DAT")
        );
    }
    #[test]
    fn native_artwork_observations_share_one_bounded_resolver_phase() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().canonicalize().unwrap();
        let share = home.join("share");
        let artwork = share.join("PIONEER/Artwork");
        fs::create_dir_all(&artwork).unwrap();
        for index in 0..128 {
            fs::write(
                artwork.join(format!("Cover{index:03}.jpg")),
                b"authored artwork",
            )
            .unwrap();
        }
        let target = RekordboxSyncTarget {
            id: "authored-bounded-assets".into(),
            installation: TargetInstallation {
                root: home.clone(),
                executable: home.join("rekordbox.exe"),
                version: "7.1".into(),
                major: 7,
                version_source: "explicit_version_hint".into(),
            },
            database: home.join("master.db"),
            options_file: home.join("options.json"),
            asset_root: share,
            environment: TargetEnvironment::WindowsCompatibility {
                prefix: home.clone(),
                installation: None,
                mappings: vec![DriveMapping {
                    windows_root: "C:\\".into(),
                    host_root: home,
                }],
            },
        };
        let paths = target.environment.resolver().unwrap();
        let cancel = CancellationToken::default();
        let expected =
            json!(file_sha256_cancellable(&artwork.join("Cover000.jpg"), &cancel).unwrap());
        for index in 0..128 {
            assert_eq!(
                artwork_observation_resolved(
                    &target,
                    &paths,
                    &format!(r"C:\share\PIONEER\Artwork\Cover{index:03}.jpg"),
                    &cancel
                )
                .unwrap(),
                expected
            );
        }
        paths.verify().unwrap();
        assert!(
            paths.directory_enumerations() > 0,
            "Asset helpers must consume the supplied operation resolver"
        );
        assert!(
            paths.directory_enumerations() <= 12,
            "128 asset observations must reuse directory indices; observed {} enumerations",
            paths.directory_enumerations()
        );
    }
}
