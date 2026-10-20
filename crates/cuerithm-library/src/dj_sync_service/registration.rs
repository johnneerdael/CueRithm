//! Remount-stable volume identity plus replacement-sensitive native objects.
use super::*;

pub(super) fn capture(target: &SyncTarget) -> Result<String> {
    capture_with(target, &object_token)
}

fn capture_with(target: &SyncTarget, token: &impl Fn(&Path) -> Result<String>) -> Result<String> {
    let mut catalogs = Vec::new();
    if target.software == DjSoftware::PioneerUsb {
        // A USB root survives catalog replacement. Include both catalog objects
        // so replacing a database on the same volume cannot inherit ownership.
        for directory in ["PIONEER", ".PIONEER"] {
            for name in ["export.pdb", "exportLibrary.db"] {
                let path = target.path.join(directory).join("rekordbox").join(name);
                let identity = match fs::symlink_metadata(&path) {
                    Ok(_) => Some(token(&path)?),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                    Err(error) => return Err(error.into()),
                };
                catalogs.push((path, identity));
            }
        }
    }
    digest(&(
        "native-registration-v2",
        &target.path,
        token(&target.path)?,
        // A removable root's host mount directory (/Volumes, /media/user) is
        // outside the registered volume and may be recreated between mounts.
        if target.software == DjSoftware::PioneerUsb {
            None
        } else {
            Some(token(
                target.path.parent().context("DJ target parent missing")?,
            )?)
        },
        target
            .source_root
            .as_ref()
            .map(|path| token(path))
            .transpose()?,
        catalogs,
    ))
}

fn object_token(path: &Path) -> Result<String> {
    object_token_on_volume(path, volume_identity(path)?)
}

struct VolumeIdentity {
    id: String,
    persistent_inodes: bool,
}

#[cfg(unix)]
fn durable_object_token(
    volume: &VolumeIdentity,
    inode: u64,
    created: Option<std::time::SystemTime>,
) -> Result<String> {
    digest(&(
        &volume.id,
        volume.persistent_inodes.then_some(inode),
        created,
    ))
}

fn object_token_on_volume(path: &Path, volume: VolumeIdentity) -> Result<String> {
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        !metadata.file_type().is_symlink(),
        "DJ registration cannot be a symbolic link"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // st_dev identifies this mount session, not the persisted filesystem.
        // Keep the per-volume file ID and birth time to detect atomic replacement.
        if volume.persistent_inodes {
            durable_object_token(&volume, metadata.ino(), metadata.created().ok())
        } else {
            // FAT/exFAT Linux inode numbers are allocated per mount. Their
            // on-disk birth time is persistent; the volume root has no entry.
            let root = path.parent().is_none_or(|parent| {
                fs::metadata(parent).is_ok_and(|parent| parent.dev() != metadata.dev())
            });
            let created = if root {
                None
            } else {
                Some(
                    metadata
                        .created()
                        .context("Filesystem cannot identify this DJ catalog across remounts")?,
                )
            };
            durable_object_token(&volume, metadata.ino(), created)
        }
    }
    #[cfg(not(unix))]
    {
        digest(&(
            volume.id,
            volume.persistent_inodes,
            metadata
                .created()
                .context("Filesystem cannot identify the selected DJ target")?,
        ))
    }
}

#[cfg(target_os = "macos")]
fn volume_identity(path: &Path) -> Result<VolumeIdentity> {
    use std::{
        ffi::{CStr, CString},
        os::unix::ffi::OsStrExt,
    };
    let path = CString::new(path.as_os_str().as_bytes())?;
    let mut filesystem = std::mem::MaybeUninit::<libc::statfs>::uninit();
    // statfs supplies the volume root; volume attributes require that root.
    if unsafe { libc::statfs(path.as_ptr(), filesystem.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error()).context("Identify DJ filesystem");
    }
    let filesystem = unsafe { filesystem.assume_init() };
    let root = unsafe { CStr::from_ptr(filesystem.f_mntonname.as_ptr()) };
    let mut attributes = libc::attrlist {
        bitmapcount: libc::ATTR_BIT_MAP_COUNT,
        reserved: 0,
        commonattr: 0,
        volattr: libc::ATTR_VOL_INFO | libc::ATTR_VOL_UUID,
        dirattr: 0,
        fileattr: 0,
        forkattr: 0,
    };
    #[repr(C)]
    struct VolumeUuid {
        length: u32,
        uuid: [u8; 16],
    }
    let mut result = VolumeUuid {
        length: 0,
        uuid: [0; 16],
    };
    if unsafe {
        libc::getattrlist(
            root.as_ptr(),
            (&mut attributes as *mut libc::attrlist).cast(),
            (&mut result as *mut VolumeUuid).cast(),
            std::mem::size_of::<VolumeUuid>(),
            0,
        )
    } != 0
    {
        return Err(std::io::Error::last_os_error()).context("Read persistent DJ volume UUID");
    }
    ensure!(
        result.length as usize == std::mem::size_of::<VolumeUuid>() && result.uuid != [0; 16],
        "Filesystem has no persistent DJ volume UUID"
    );
    let kind = unsafe { CStr::from_ptr(filesystem.f_fstypename.as_ptr()) }.to_bytes();
    Ok(VolumeIdentity {
        id: digest(&("volume-uuid", result.uuid))?,
        persistent_inodes: !matches!(kind, b"msdos" | b"exfat"),
    })
}

#[cfg(target_os = "linux")]
fn volume_identity(path: &Path) -> Result<VolumeIdentity> {
    use std::{
        ffi::CString,
        os::unix::{ffi::OsStrExt, fs::MetadataExt},
    };
    let metadata = fs::metadata(path)?;
    let name = CString::new(path.as_os_str().as_bytes())?;
    let mut statfs = std::mem::MaybeUninit::<libc::statfs>::uninit();
    if unsafe { libc::statfs(name.as_ptr(), statfs.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error()).context("Identify DJ filesystem type");
    }
    let statfs = unsafe { statfs.assume_init() };
    let persistent_inodes = !matches!(statfs.f_type as u64, 0x4d44 | 0x2011_bab0);
    // ext4_statfs derives f_fsid from the on-disk superblock UUID. Do not use
    // this fallback for FAT/exFAT: those drivers encode the transient bd_dev.
    if statfs.f_type as u64 != 0xef53 {
        // udev's links expose the filesystem UUID/serial, not the reconnect-dependent
        // block-device number. Use st_dev only to find the matching block device.
        if let Ok(entries) = fs::read_dir("/dev/disk/by-uuid") {
            let mut identities = BTreeSet::new();
            for entry in entries {
                let entry = entry?;
                let device = match fs::metadata(entry.path()) {
                    Ok(device) => device,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(error) => return Err(error.into()),
                };
                use std::os::unix::fs::FileTypeExt;
                if device.file_type().is_block_device() && device.rdev() == metadata.dev() {
                    identities.insert(
                        entry
                            .file_name()
                            .into_string()
                            .map_err(|_| anyhow::anyhow!("Invalid filesystem UUID"))?,
                    );
                }
            }
            ensure!(identities.len() <= 1, "Ambiguous persistent DJ volume UUID");
            if let Some(uuid) = identities.into_iter().next() {
                return Ok(VolumeIdentity {
                    id: digest(&("filesystem-uuid", uuid))?,
                    persistent_inodes,
                });
            }
        }
        anyhow::bail!("Filesystem has no accessible persistent DJ volume UUID");
    }
    let mut filesystem = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    if unsafe { libc::statvfs(name.as_ptr(), filesystem.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error()).context("Read persistent DJ filesystem ID");
    }
    let filesystem = unsafe { filesystem.assume_init() };
    ensure!(
        filesystem.f_fsid != 0,
        "Filesystem has no persistent DJ filesystem ID"
    );
    Ok(VolumeIdentity {
        id: digest(&("ext-filesystem-uuid", filesystem.f_fsid))?,
        persistent_inodes: true,
    })
}

#[cfg(windows)]
fn volume_identity(path: &Path) -> Result<VolumeIdentity> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{GetVolumeInformationW, GetVolumePathNameW};
    let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut root = vec![0; 32768];
    if unsafe { GetVolumePathNameW(path.as_ptr(), root.as_mut_ptr(), root.len() as u32) } == 0 {
        return Err(std::io::Error::last_os_error()).context("Locate DJ volume");
    }
    let mut serial = 0;
    if unsafe {
        GetVolumeInformationW(
            root.as_ptr(),
            std::ptr::null_mut(),
            0,
            &mut serial,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error()).context("Read persistent DJ volume serial");
    }
    Ok(VolumeIdentity {
        id: digest(&("volume-serial", serial))?,
        persistent_inodes: false,
    })
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn volume_identity(_path: &Path) -> Result<VolumeIdentity> {
    anyhow::bail!("Persistent DJ volume identity is unavailable on this platform")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(root: &Path, software: DjSoftware) -> SyncTarget {
        SyncTarget {
            software,
            path: root.into(),
            source_root: None,
        }
    }

    #[test]
    fn simulated_remount_changes_session_device_without_changing_registration() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let pioneer = target(&root, DjSoftware::PioneerUsb);
        let engine_database = root.join("m.db");
        let database = Connection::open(&engine_database).unwrap();
        database.execute_batch("CREATE TABLE Information(uuid TEXT);INSERT INTO Information VALUES('durable-engine-catalog');").unwrap();
        drop(database);
        let engine = target(&engine_database, DjSoftware::EngineDj);
        for target in [pioneer, engine] {
            // Simulate two mount sessions resolving to the same persistent
            // volume. Unlike the old token, no session device enters capture.
            let mounts = [(101_u64, "volume-A"), (909_u64, "volume-A")];
            assert_ne!(mounts[0].0, mounts[1].0);
            let registrations: Vec<_> = mounts
                .into_iter()
                .map(|(_, volume)| {
                    capture_with(&target, &|path| {
                        object_token_on_volume(
                            path,
                            VolumeIdentity {
                                id: volume.into(),
                                persistent_inodes: true,
                            },
                        )
                    })
                    .unwrap()
                })
                .collect();
            assert_eq!(registrations[0], registrations[1]);
            let another_volume = capture_with(&target, &|path| {
                object_token_on_volume(
                    path,
                    VolumeIdentity {
                        id: "volume-B".into(),
                        persistent_inodes: true,
                    },
                )
            })
            .unwrap();
            assert_ne!(registrations[0], another_volume);
        }
    }

    #[test]
    fn same_volume_same_byte_catalog_replacement_changes_registration() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let database_path = root.join("PIONEER/rekordbox/export.pdb");
        fs::create_dir_all(database_path.parent().unwrap()).unwrap();
        fs::write(&database_path, b"same native catalog bytes").unwrap();
        let pioneer = target(&root, DjSoftware::PioneerUsb);
        let before = capture(&pioneer).unwrap();
        let replacement = database_path.with_extension("replacement");
        fs::write(&replacement, fs::read(&database_path).unwrap()).unwrap();
        fs::rename(&replacement, &database_path).unwrap();
        assert_ne!(before, capture(&pioneer).unwrap());
        let engine_path = root.join("m.db");
        let database = Connection::open(&engine_path).unwrap();
        database
            .execute_batch(
                "CREATE TABLE Information(uuid TEXT);INSERT INTO Information VALUES('catalog-A');",
            )
            .unwrap();
        let engine = target(&engine_path, DjSoftware::EngineDj);
        drop(database);
        let before = capture(&engine).unwrap();
        let replacement = engine_path.with_extension("replacement");
        fs::write(&replacement, fs::read(&engine_path).unwrap()).unwrap();
        fs::rename(&replacement, &engine_path).unwrap();
        assert_ne!(before, capture(&engine).unwrap());
    }

    #[test]
    fn pioneer_registration_does_not_bind_recreated_host_mount_parent() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let target = target(&root, DjSoftware::PioneerUsb);
        let parent = root.parent().unwrap();
        let registration = |host_parent: &str| {
            capture_with(&target, &|path| {
                if path == parent {
                    Ok(host_parent.into())
                } else {
                    object_token_on_volume(
                        path,
                        VolumeIdentity {
                            id: "USB-volume".into(),
                            persistent_inodes: true,
                        },
                    )
                }
            })
            .unwrap()
        };
        assert_eq!(
            registration("old host mount directory"),
            registration("new host mount directory")
        );
    }

    #[cfg(unix)]
    #[test]
    fn fat_remount_ignores_allocated_inodes_but_replacement_birth_time_is_protected() {
        let volume = VolumeIdentity {
            id: "persistent-FAT-volume-serial".into(),
            persistent_inodes: false,
        };
        let birth = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(100);
        let before = durable_object_token(&volume, 11, Some(birth)).unwrap();
        assert_eq!(
            before,
            durable_object_token(&volume, 992, Some(birth)).unwrap()
        );
        assert_ne!(
            before,
            durable_object_token(
                &volume,
                992,
                Some(birth + std::time::Duration::from_millis(10))
            )
            .unwrap()
        );
        let another_volume = VolumeIdentity {
            id: "another-FAT-volume-serial".into(),
            persistent_inodes: false,
        };
        assert_ne!(
            before,
            durable_object_token(&another_volume, 11, Some(birth)).unwrap()
        );
    }
}
