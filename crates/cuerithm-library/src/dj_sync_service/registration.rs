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
    // ext4's native ID comes from its UUID. Other filesystems can expose an
    // anonymous st_dev, so resolve their backing source through mountinfo.
    if statfs.f_type as u64 != 0xef53 {
        return linux_mounts::capture(path, metadata.dev());
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

#[cfg(all(unix, any(target_os = "linux", test)))]
mod linux_mounts {
    use super::*;
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};

    #[derive(Debug, PartialEq, Eq)]
    struct Mount {
        device: (u32, u32),
        root: PathBuf,
        point: PathBuf,
        filesystem: Vec<u8>,
        source: PathBuf,
    }

    fn unescape(value: &[u8]) -> Result<PathBuf> {
        let mut output = Vec::new();
        let mut index = 0;
        while index < value.len() {
            if value[index] == b'\\' {
                let escape = value
                    .get(index + 1..index + 4)
                    .context("Truncated mountinfo escape")?;
                output.push(match escape {
                    b"040" => b' ',
                    b"011" => b'\t',
                    b"012" => b'\n',
                    b"134" => b'\\',
                    _ => anyhow::bail!("Invalid mountinfo escape"),
                });
                index += 4;
            } else {
                output.push(value[index]);
                index += 1;
            }
        }
        Ok(OsString::from_vec(output).into())
    }

    fn select(path: &Path, device: (u32, u32), information: &[u8]) -> Result<Mount> {
        let mut selected: Option<Mount> = None;
        for line in information
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
        {
            let fields: Vec<_> = line.split(|byte| *byte == b' ').collect();
            let separator = fields
                .iter()
                .position(|field| *field == b"-")
                .context("Invalid mountinfo separator")?;
            ensure!(
                separator >= 6 && fields.len() >= separator + 4,
                "Invalid mountinfo fields"
            );
            let pair = std::str::from_utf8(fields[2])?
                .split_once(':')
                .context("Invalid mountinfo device")?;
            let mount_device = (pair.0.parse::<u32>()?, pair.1.parse::<u32>()?);
            let point = unescape(fields[4])?;
            if mount_device != device || !path.starts_with(&point) {
                continue;
            }
            let mount = Mount {
                device: mount_device,
                root: unescape(fields[3])?,
                point,
                filesystem: fields[separator + 1].to_vec(),
                source: unescape(fields[separator + 2])?,
            };
            let depth = mount.point.components().count();
            match &selected {
                Some(prior) if prior.point.components().count() > depth => {}
                Some(prior) if prior.point.components().count() == depth => {
                    ensure!(*prior == mount, "Ambiguous DJ backing mount");
                }
                _ => selected = Some(mount),
            }
        }
        selected.context("Cannot resolve the DJ target's backing mount")
    }

    fn resolve(
        path: &Path,
        device: (u32, u32),
        information: &[u8],
        block_device: &impl Fn(&Path) -> Result<u64>,
        device_uuids: &impl Fn(u64) -> Result<BTreeSet<String>>,
    ) -> Result<VolumeIdentity> {
        let mount = select(path, device, information)?;
        ensure!(
            mount.source.is_absolute(),
            "DJ backing mount has no block-device source"
        );
        let uuids = device_uuids(block_device(&mount.source)?)?;
        ensure!(uuids.len() <= 1, "Ambiguous persistent DJ volume UUID");
        let uuid = uuids
            .into_iter()
            .next()
            .context("Filesystem has no accessible persistent DJ volume UUID")?;
        let id = if mount.filesystem == b"btrfs" {
            digest(&("filesystem-uuid", uuid, &mount.root))?
        } else {
            digest(&("filesystem-uuid", uuid))?
        };
        Ok(VolumeIdentity {
            id,
            // FAT/exFAT and block-backed FUSE can synthesize inode numbers.
            persistent_inodes: !matches!(
                mount.filesystem.as_slice(),
                b"vfat" | b"msdos" | b"exfat"
            ) && !mount.filesystem.starts_with(b"fuse"),
        })
    }

    #[cfg(target_os = "linux")]
    pub(super) fn capture(path: &Path, mounted_device: u64) -> Result<VolumeIdentity> {
        use std::os::unix::fs::{FileTypeExt, MetadataExt};
        let canonical = fs::canonicalize(path)?;
        let information = fs::read("/proc/self/mountinfo").context("Read DJ backing mounts")?;
        resolve(
            &canonical,
            (libc::major(mounted_device), libc::minor(mounted_device)),
            &information,
            &|source| {
                let metadata = fs::metadata(source).context("Inspect DJ backing device")?;
                ensure!(
                    metadata.file_type().is_block_device(),
                    "DJ backing source is not a block device"
                );
                Ok(metadata.rdev())
            },
            &device_uuids,
        )
    }

    #[cfg(target_os = "linux")]
    fn device_uuids(device: u64) -> Result<BTreeSet<String>> {
        use std::os::unix::fs::{FileTypeExt, MetadataExt};
        let mut identities = BTreeSet::new();
        // Every Btrfs member has the shared filesystem UUID in its udev record;
        // the single by-uuid symlink can point at a different member.
        let record = PathBuf::from(format!(
            "/run/udev/data/b{}:{}",
            libc::major(device),
            libc::minor(device)
        ));
        match fs::read(&record) {
            Ok(record) => {
                identities.extend(udev_uuids(&record)?);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("Read DJ backing device UUID"),
        }
        match fs::read_dir("/dev/disk/by-uuid") {
            Ok(entries) => {
                for entry in entries {
                    let entry = entry?;
                    let metadata = match fs::metadata(entry.path()) {
                        Ok(metadata) => metadata,
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                        Err(error) => return Err(error.into()),
                    };
                    if metadata.file_type().is_block_device() && metadata.rdev() == device {
                        identities.insert(normalize_uuid(
                            &entry
                                .file_name()
                                .into_string()
                                .map_err(|_| anyhow::anyhow!("Invalid filesystem UUID"))?,
                        )?);
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("Read persistent DJ volume UUID links"),
        }
        Ok(identities)
    }

    fn udev_uuids(record: &[u8]) -> Result<BTreeSet<String>> {
        record
            .split(|byte| *byte == b'\n')
            .filter_map(|line| {
                line.strip_prefix(b"E:ID_FS_UUID=")
                    .or_else(|| line.strip_prefix(b"E:ID_FS_UUID_ENC="))
            })
            .map(|uuid| normalize_uuid(std::str::from_utf8(uuid)?))
            .collect()
    }

    fn normalize_uuid(uuid: &str) -> Result<String> {
        ensure!(
            !uuid.is_empty()
                && uuid
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-'),
            "Invalid persistent DJ volume UUID"
        );
        Ok(uuid.to_ascii_lowercase())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn virtual_btrfs_and_fuse_devices_resolve_the_backing_mount_source() {
            for filesystem in ["btrfs", "fuseblk"] {
                let information =
                    format!("32 20 0:47 / /media/usb rw - {filesystem} /dev/sdb1 rw\n");
                let volume = resolve(
                    Path::new("/media/usb/catalog.db"),
                    (0, 47),
                    information.as_bytes(),
                    &|source| {
                        assert_eq!(source, Path::new("/dev/sdb1"));
                        Ok(0x0811)
                    },
                    &|backing| {
                        assert_eq!(backing, 0x0811);
                        Ok(BTreeSet::from(["filesystem-A".into()]))
                    },
                )
                .unwrap();
                assert_eq!(volume.persistent_inodes, filesystem == "btrfs");
            }
        }

        #[test]
        fn nested_mounts_and_escaped_mountpoint_source_and_root_are_decoded() {
            let information = br"20 1 0:47 / /media rw - btrfs /dev/root rw
21 20 0:47 /Music\040Library /media/DJ\040USB rw shared:3 - btrfs /dev/disk/DJ\040Drive\134one rw
22 20 0:99 / /media/DJ\040USB/foreign rw - fuseblk /dev/foreign rw
";
            let mount =
                select(Path::new("/media/DJ USB/catalog.db"), (0, 47), information).unwrap();
            assert_eq!(mount.point, Path::new("/media/DJ USB"));
            assert_eq!(mount.root, Path::new("/Music Library"));
            assert_eq!(mount.source, Path::new("/dev/disk/DJ Drive\\one"));
            let escaped = unescape(br"/tab\011line\012slash\134").unwrap();
            assert_eq!(escaped, Path::new("/tab\tline\nslash\\"));
            assert!(unescape(br"/invalid\777").is_err());
        }

        #[test]
        fn btrfs_member_changes_keep_identity_and_subvolumes_remain_distinct() {
            let resolve_member = |source: &str, root: &str, backing| {
                let information = format!("32 20 0:47 {root} /media/usb rw - btrfs {source} rw\n");
                resolve(
                    Path::new("/media/usb/m.db"),
                    (0, 47),
                    information.as_bytes(),
                    &|_| Ok(backing),
                    &|_| Ok(BTreeSet::from(["shared-multidevice-uuid".into()])),
                )
                .unwrap()
                .id
            };
            let first = resolve_member("/dev/sdb1", "/@music", 0x0811);
            assert_eq!(first, resolve_member("/dev/sdc1", "/@music", 0x0821));
            assert_ne!(first, resolve_member("/dev/sdb1", "/@another", 0x0811));
        }

        #[test]
        fn backing_member_udev_uuid_is_normalized_and_conflicts_are_retained() {
            let record =
                b"E:ID_FS_UUID=ABCD-0123\nE:ID_FS_UUID_ENC=ABCD-0123\nE:ID_SERIAL=unrelated\n";
            assert_eq!(
                udev_uuids(record).unwrap(),
                BTreeSet::from(["abcd-0123".into()])
            );
            assert_eq!(
                udev_uuids(b"E:ID_FS_UUID=aaaa\nE:ID_FS_UUID_ENC=bbbb\n")
                    .unwrap()
                    .len(),
                2
            );
            assert!(udev_uuids(b"E:ID_FS_UUID=invalid uuid\n").is_err());
        }

        #[test]
        fn missing_ambiguous_or_unrelated_backing_identity_fails_closed() {
            let information = b"32 20 0:47 / /media/usb rw - fuseblk /dev/sdb1 rw\n";
            let resolve_uuids = |ids: BTreeSet<String>| {
                resolve(
                    Path::new("/media/usb/m.db"),
                    (0, 47),
                    information,
                    &|_| Ok(0x0811),
                    &|_| Ok(ids.clone()),
                )
            };
            assert!(resolve_uuids(BTreeSet::<String>::new()).is_err());
            assert!(resolve_uuids(BTreeSet::from(["uuid-A".into(), "uuid-B".into()])).is_err());
            assert!(select(Path::new("/media/usb/m.db"), (0, 99), information).is_err());
            let ambiguous = b"32 20 0:47 / /media/usb rw - fuseblk /dev/sdb1 rw\n33 20 0:47 / /media/usb rw - fuseblk /dev/sdc1 rw\n";
            assert!(select(Path::new("/media/usb/m.db"), (0, 47), ambiguous).is_err());
            let non_block = b"32 20 0:47 / /media/usb rw - fuse.sshfs server:path rw\n";
            assert!(
                resolve(
                    Path::new("/media/usb/m.db"),
                    (0, 47),
                    non_block,
                    &|_| Ok(1),
                    &|_| Ok(BTreeSet::from(["uuid".into()]))
                )
                .is_err()
            );
        }
    }
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
