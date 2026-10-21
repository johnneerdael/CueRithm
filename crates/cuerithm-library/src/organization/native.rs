//! Native authority: every ancestor is opened without following links. Unix
//! mutations use retained directory descriptors; Windows parents deny delete
//! sharing for the entire boundary, preventing rename/reparse replacement.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Component, Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    pub volume: u64,
    pub object: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativePath(pub Vec<u8>);
impl NativePath {
    pub fn new(path: &Path) -> Self {
        Self(crate::model::path_bytes(path))
    }
    pub fn path(&self) -> Result<PathBuf> {
        crate::model::bytes_path(&self.0)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Authority {
    pub path: NativePath,
    pub ancestors: Vec<Identity>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fingerprint {
    pub identity: Identity,
    pub sha256: String,
    pub bytes: u64,
}
pub struct Parent {
    handles: Vec<File>,
    path: PathBuf,
}
impl Parent {
    pub fn capture(path: &Path) -> Result<(Self, Authority)> {
        ensure!(path.is_absolute(), "Organization paths must be absolute");
        ensure!(
            !path
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::CurDir)),
            "Invalid directory traversal"
        );
        let mut handles = Vec::new();
        let mut current = PathBuf::new();
        for component in path.components() {
            current.push(component.as_os_str());
            if matches!(component, Component::Prefix(_)) {
                continue;
            }
            let file = open_directory(&current, handles.last(), component)?;
            ensure!(
                file.metadata()?.is_dir(),
                "Organization parent is not a directory"
            );
            handles.push(file);
        }
        ensure!(
            !handles.is_empty(),
            "Missing organization directory authority"
        );
        let ancestors = handles.iter().map(identity).collect::<Result<Vec<_>>>()?;
        Ok((
            Self {
                handles,
                path: path.to_owned(),
            },
            Authority {
                path: NativePath::new(path),
                ancestors,
            },
        ))
    }
    pub fn reopen(authority: &Authority) -> Result<Self> {
        let (parent, current) = Self::capture(&authority.path.path()?)?;
        ensure!(
            current.ancestors == authority.ancestors,
            "Organization parent identity changed; re-plan required"
        );
        Ok(parent)
    }
    pub fn read(&self, name: &Path) -> Result<File> {
        leaf(name)?;
        ensure!(
            !self.handles.is_empty(),
            "Missing retained directory authority"
        );
        let file = open_file(self, name, false)?;
        ensure!(
            file.metadata()?.is_file(),
            "Only regular organization files are supported"
        );
        Ok(file)
    }
    pub fn create(&self, name: &Path) -> Result<File> {
        leaf(name)?;
        open_file(self, name, true)
    }
    pub fn fingerprint(&self, name: &Path) -> Result<Fingerprint> {
        fingerprint(&mut self.read(name)?)
    }
    pub fn single_link(&self, name: &Path) -> Result<bool> {
        link_count(&self.read(name)?).map(|count| count == 1)
    }
    pub fn optional(&self, name: &Path) -> Result<Option<Fingerprint>> {
        match self.read(name) {
            Ok(mut f) => Ok(Some(fingerprint(&mut f)?)),
            Err(error)
                if error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
            {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }
    pub fn rename(&self, source: &Path, target_parent: &Self, target: &Path) -> Result<()> {
        leaf(source)?;
        leaf(target)?;
        rename_exclusive(self, source, target_parent, target)?;
        self.sync()?;
        target_parent.sync()?;
        // Windows rename_exclusive uses MOVEFILE_WRITE_THROUGH. Staged file
        // data and copied permissions were flushed through its original writable
        // handle before this call. Reopening a read-only publication for writing
        // here would turn a successful native rename into an avoidable failure.
        Ok(())
    }
    /// Copy a previously verified staged artifact into this retained parent.
    #[cfg(feature = "autotag")]
    pub(crate) fn copy_prepared(
        &self,
        from: &Path,
        name: &Path,
        expected: &str,
        cancel: &cuerithm_media::CancellationToken,
    ) -> Result<Fingerprint> {
        self.copy_prepared_with(from, name, expected, cancel, || Ok(()))
    }
    #[cfg(feature = "autotag")]
    pub(crate) fn copy_prepared_verified(
        &self,
        from: &Path,
        name: &Path,
        expected: &str,
        cancel: &cuerithm_media::CancellationToken,
        expected_parent: &Authority,
        expected_fingerprint: &Fingerprint,
    ) -> Result<Fingerprint> {
        self.copy_prepared_from(
            from,
            name,
            expected,
            cancel,
            Some((expected_parent, expected_fingerprint)),
            || Ok(()),
        )
    }
    #[cfg(feature = "autotag")]
    fn copy_prepared_with(
        &self,
        from: &Path,
        name: &Path,
        expected: &str,
        cancel: &cuerithm_media::CancellationToken,
        after_copy: impl FnOnce() -> Result<()>,
    ) -> Result<Fingerprint> {
        self.copy_prepared_from(from, name, expected, cancel, None, after_copy)
    }
    #[cfg(feature = "autotag")]
    fn copy_prepared_from(
        &self,
        from: &Path,
        name: &Path,
        expected: &str,
        cancel: &cuerithm_media::CancellationToken,
        source: Option<(&Authority, &Fingerprint)>,
        after_copy: impl FnOnce() -> Result<()>,
    ) -> Result<Fingerprint> {
        use std::io::Write;
        cancel.check()?;
        let directory = from
            .parent()
            .context("Missing native staged input parent")?;
        let input_parent = if let Some((authority, fingerprint)) = source {
            ensure!(
                authority.path.path()? == directory && fingerprint.sha256 == expected,
                "Native staged input receipt differs"
            );
            Self::reopen(authority)?
        } else {
            Self::capture(directory)?.0
        };
        let mut input = input_parent.read(Path::new(
            from.file_name()
                .context("Missing native staged input name")?,
        ))?;
        if let Some((_, expected)) = source {
            ensure!(
                matches(&fingerprint(&mut input)?, expected),
                "Native staged input identity or bytes changed; retained"
            );
            input.seek(SeekFrom::Start(0))?;
        }
        let mut output = self.create(name)?;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            cancel.check()?;
            let count = input.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            output.write_all(&buffer[..count])?;
        }
        output.set_permissions(input.metadata()?.permissions())?;
        output.sync_all()?;
        self.sync()?;
        let result = fingerprint(&mut output)?;
        ensure!(
            result.sha256 == expected,
            "Staged metadata bytes changed during native copy"
        );
        drop(output);
        after_copy()?;
        ensure!(
            matches(&self.fingerprint(name)?, &result),
            "Native copied leaf changed; unknown bytes retained"
        );
        Ok(result)
    }
    #[cfg(feature = "autotag")]
    pub(crate) fn create_private_directory(&self, name: &Path) -> Result<(Self, Authority)> {
        let authority = self.create_directory(name)?;
        let private = Self::reopen(&authority)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            let handle = private
                .handles
                .last()
                .context("Missing private directory")?;
            ensure!(
                handle.metadata()?.uid() == unsafe { libc::geteuid() },
                "Private cleanup directory owner changed"
            );
            handle.set_permissions(std::fs::Permissions::from_mode(0o700))?;
        }
        private.validate_private_directory()?;
        Ok((private, authority))
    }
    #[cfg(feature = "autotag")]
    pub(crate) fn reopen_private(authority: &Authority) -> Result<Self> {
        let parent = Self::reopen(authority)?;
        parent.validate_private_directory()?;
        Ok(parent)
    }
    #[cfg(feature = "autotag")]
    pub(crate) fn remove_private_directory(
        &self,
        name: &Path,
        authority: &Authority,
    ) -> Result<()> {
        leaf(name)?;
        ensure!(
            authority.path.path()? == self.path.join(name),
            "Private directory leaves its saved parent"
        );
        let private = match Self::reopen_private(authority) {
            Ok(parent) => parent,
            Err(error)
                if error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
            {
                return Ok(());
            }
            Err(error) => return Err(error),
        };
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let name = c_path(name)?;
            let fd = self
                .handles
                .last()
                .context("Missing retained parent")?
                .as_raw_fd();
            if unsafe { libc::unlinkat(fd, name.as_ptr(), libc::AT_REMOVEDIR) } != 0 {
                return Err(std::io::Error::last_os_error())
                    .context("Retire empty private cleanup directory");
            };
            drop(private);
        }
        #[cfg(windows)]
        {
            use std::os::{windows::fs::OpenOptionsExt, windows::io::AsRawHandle};
            use windows_sys::Win32::Storage::FileSystem::*;
            drop(private);
            let file = File::options()
                .access_mode(0x80000000 | 0x00010000)
                .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
                .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
                .open(self.path.join(name))?;
            ensure!(
                authority.ancestors.last() == Some(&identity(&file)?),
                "Private cleanup directory identity changed; retained"
            );
            let info = FILE_DISPOSITION_INFO { DeleteFile: true };
            ensure!(
                unsafe {
                    SetFileInformationByHandle(
                        file.as_raw_handle(),
                        FileDispositionInfo,
                        &info as *const _ as *const _,
                        std::mem::size_of_val(&info) as u32,
                    )
                } != 0,
                "Could not retire empty private cleanup directory: {}",
                std::io::Error::last_os_error()
            );
            drop(file);
        }
        self.sync()
    }
    #[cfg(feature = "autotag")]
    fn validate_private_directory(&self) -> Result<()> {
        let metadata = self
            .handles
            .last()
            .context("Missing private directory")?
            .metadata()?;
        ensure!(
            metadata.is_dir(),
            "Private cleanup parent is not a directory"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            ensure!(
                metadata.uid() == unsafe { libc::geteuid() }
                    && metadata.permissions().mode() & 0o777 == 0o700,
                "Private cleanup directory permissions or owner changed"
            );
        }
        Ok(())
    }
    /// Retire a proven temporary leaf beneath the retained no-follow parent.
    #[cfg(feature = "autotag")]
    pub(crate) fn remove_private_leaf(&self, name: &Path, expected: &Fingerprint) -> Result<()> {
        self.validate_private_directory()?;
        leaf(name)?;
        let Some(actual) = self.optional(name)? else {
            return Ok(());
        };
        ensure!(
            matches(&actual, expected),
            "Native temporary identity or bytes changed; retained"
        );
        #[cfg(unix)]
        {
            use std::os::{fd::AsRawFd, unix::ffi::OsStrExt};
            let name = std::ffi::CString::new(name.as_os_str().as_bytes())?;
            let fd = self
                .handles
                .last()
                .context("Missing retained directory")?
                .as_raw_fd();
            if unsafe { libc::unlinkat(fd, name.as_ptr(), 0) } != 0 {
                return Err(std::io::Error::last_os_error())
                    .context("Retire owned native temporary");
            }
        }
        #[cfg(windows)]
        {
            let path = self.path.join(name);
            let mut permissions = std::fs::metadata(&path)?.permissions();
            if permissions.readonly() {
                #[allow(clippy::permissions_set_readonly_false)]
                permissions.set_readonly(false);
                std::fs::set_permissions(&path, permissions)?;
            }
            std::fs::remove_file(path)?;
        }
        self.sync()
    }
    /// Exchange two leaves beneath this retained native directory authority.
    #[cfg(feature = "autotag")]
    pub(crate) fn exchange(&self, source: &Path, target: &Path, displaced: &Path) -> Result<()> {
        leaf(source)?;
        leaf(target)?;
        leaf(displaced)?;
        #[cfg(unix)]
        {
            use std::os::{fd::AsRawFd, unix::ffi::OsStrExt};
            ensure!(source == displaced, "Invalid native displaced leaf");
            let source = std::ffi::CString::new(source.as_os_str().as_bytes())?;
            let target = std::ffi::CString::new(target.as_os_str().as_bytes())?;
            let fd = self
                .handles
                .last()
                .context("Missing retained native parent")?
                .as_raw_fd();
            #[cfg(target_os = "macos")]
            let result = unsafe {
                libc::renameatx_np(fd, source.as_ptr(), fd, target.as_ptr(), libc::RENAME_SWAP)
            };
            #[cfg(target_os = "linux")]
            let result = unsafe {
                libc::renameat2(
                    fd,
                    source.as_ptr(),
                    fd,
                    target.as_ptr(),
                    libc::RENAME_EXCHANGE,
                )
            };
            if result != 0 {
                return Err(std::io::Error::last_os_error())
                    .context("Exchange native metadata publication");
            }
            self.sync()?;
        }
        #[cfg(windows)]
        {
            // Retained ancestors deny delete sharing, keeping these paths bound.
            crate::publish_files::replace(
                &self.path.join(source),
                &self.path.join(target),
                &self.path.join(displaced),
            )?;
        }
        Ok(())
    }
    pub fn create_directory(&self, name: &Path) -> Result<Authority> {
        leaf(name)?;
        create_directory(self, name)?;
        self.sync()?;
        let (_, authority) = Self::capture(&self.path.join(name))?;
        let expected = self
            .handles
            .iter()
            .map(identity)
            .collect::<Result<Vec<_>>>()?;
        ensure!(
            authority.ancestors.starts_with(&expected),
            "Directory authority changed during creation"
        );
        Ok(authority)
    }
    pub fn sync(&self) -> Result<()> {
        #[cfg(unix)]
        self.handles
            .last()
            .context("Missing directory authority")?
            .sync_all()?;
        Ok(())
    }
}
pub fn leaf(path: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        ensure!(
            !path
                .as_os_str()
                .encode_wide()
                .any(|unit| unit == 0 || unit == u16::from(b':')),
            "Alternate data streams are not organization files"
        );
    }
    ensure!(
        path.components().count() == 1
            && matches!(path.components().next(), Some(Component::Normal(_))),
        "Expected one native file name"
    );
    Ok(())
}
pub fn fingerprint(file: &mut File) -> Result<Fingerprint> {
    let before = file.metadata()?;
    ensure!(
        before.is_file(),
        "Only regular organization files are supported"
    );
    let id = identity(file)?;
    file.seek(SeekFrom::Start(0))?;
    let mut sha = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut bytes = 0;
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        sha.update(&buffer[..count]);
        bytes += count as u64;
    }
    let after = file.metadata()?;
    ensure!(
        before.len() == after.len()
            && before.modified()? == after.modified()?
            && bytes == after.len()
            && identity(file)? == id,
        "Organization file changed while reading"
    );
    Ok(Fingerprint {
        identity: id,
        sha256: format!("{:x}", sha.finalize()),
        bytes,
    })
}
pub fn matches(actual: &Fingerprint, expected: &Fingerprint) -> bool {
    actual.identity == expected.identity
        && actual.sha256 == expected.sha256
        && actual.bytes == expected.bytes
}
#[cfg(unix)]
fn identity(file: &File) -> Result<Identity> {
    use std::os::unix::fs::MetadataExt;
    let m = file.metadata()?;
    Ok(Identity {
        volume: m.dev(),
        object: m.ino(),
    })
}
#[cfg(windows)]
fn identity(file: &File) -> Result<Identity> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::*;
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    ensure!(
        unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } != 0,
        "Read native organization identity: {}",
        std::io::Error::last_os_error()
    );
    ensure!(
        info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT == 0,
        "Reparse points are not allowed for organization"
    );
    Ok(Identity {
        volume: info.dwVolumeSerialNumber as u64,
        object: ((info.nFileIndexHigh as u64) << 32) | info.nFileIndexLow as u64,
    })
}
#[cfg(unix)]
fn c_path(path: &Path) -> Result<std::ffi::CString> {
    use std::os::unix::ffi::OsStrExt;
    Ok(std::ffi::CString::new(path.as_os_str().as_bytes())?)
}
#[cfg(unix)]
fn open_directory(path: &Path, previous: Option<&File>, component: Component<'_>) -> Result<File> {
    use std::os::fd::{AsRawFd, FromRawFd};
    let name = if previous.is_some() {
        c_path(Path::new(component.as_os_str()))?
    } else {
        c_path(path)?
    };
    let fd = unsafe {
        libc::openat(
            previous.map_or(libc::AT_FDCWD, AsRawFd::as_raw_fd),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error())
            .context("Open organization directory without following links");
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}
#[cfg(windows)]
fn open_directory(path: &Path, _: Option<&File>, _: Component<'_>) -> Result<File> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::*;
    let file = File::options()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    identity(&file)?;
    Ok(file)
}
#[cfg(unix)]
fn open_file(parent: &Parent, path: &Path, create: bool) -> Result<File> {
    use std::os::fd::{AsRawFd, FromRawFd};
    let name = c_path(path)?;
    let flags = libc::O_CLOEXEC
        | libc::O_NOFOLLOW
        | libc::O_NONBLOCK
        | if create {
            libc::O_RDWR | libc::O_CREAT | libc::O_EXCL
        } else {
            libc::O_RDONLY
        };
    let fd = unsafe {
        libc::openat(
            parent
                .handles
                .last()
                .context("Missing organization authority")?
                .as_raw_fd(),
            name.as_ptr(),
            flags,
            0o600,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error())
            .context("Open organization file without following links");
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}
#[cfg(windows)]
fn ensure_regular_windows(file: &File) -> Result<()> {
    identity(file)?;
    ensure!(
        file.metadata()?.is_file(),
        "Only regular organization files are supported"
    );
    Ok(())
}
#[cfg(windows)]
fn open_file(parent: &Parent, path: &Path, create: bool) -> Result<File> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::*;
    let file = File::options()
        .read(true)
        .write(create)
        .create_new(create)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(parent.path.join(path))?;
    ensure_regular_windows(&file)?;
    Ok(file)
}
#[cfg(unix)]
fn rename_exclusive(from: &Parent, source: &Path, to: &Parent, target: &Path) -> Result<()> {
    use std::os::fd::AsRawFd;
    let source = c_path(source)?;
    let target = c_path(target)?;
    let a = from
        .handles
        .last()
        .context("Missing source authority")?
        .as_raw_fd();
    let b = to
        .handles
        .last()
        .context("Missing target authority")?
        .as_raw_fd();
    #[cfg(target_os = "macos")]
    let result =
        unsafe { libc::renameatx_np(a, source.as_ptr(), b, target.as_ptr(), libc::RENAME_EXCL) };
    #[cfg(target_os = "linux")]
    let result = unsafe {
        libc::renameat2(
            a,
            source.as_ptr(),
            b,
            target.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result != 0 {
        return Err(std::io::Error::last_os_error()).context("Exclusive organization publication");
    }
    Ok(())
}
#[cfg(windows)]
fn rename_exclusive(from: &Parent, source: &Path, to: &Parent, target: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::*;
    let a: Vec<u16> = from
        .path
        .join(source)
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let b: Vec<u16> = to
        .path
        .join(target)
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    if unsafe { MoveFileExW(a.as_ptr(), b.as_ptr(), MOVEFILE_WRITE_THROUGH) } == 0 {
        return Err(std::io::Error::last_os_error()).context("Exclusive organization publication");
    }
    Ok(())
}

#[cfg(unix)]
fn create_directory(parent: &Parent, name: &Path) -> Result<()> {
    use std::os::fd::AsRawFd;
    let name = c_path(name)?;
    let result = unsafe {
        libc::mkdirat(
            parent
                .handles
                .last()
                .context("Missing directory authority")?
                .as_raw_fd(),
            name.as_ptr(),
            0o755,
        )
    };
    if result != 0 {
        return Err(std::io::Error::last_os_error())
            .context("Create exclusive organization directory");
    }
    Ok(())
}
#[cfg(windows)]
fn create_directory(parent: &Parent, name: &Path) -> Result<()> {
    std::fs::create_dir(parent.path.join(name)).context("Create exclusive organization directory")
}

#[cfg(unix)]
fn link_count(file: &File) -> Result<u64> {
    use std::os::unix::fs::MetadataExt;
    Ok(file.metadata()?.nlink())
}
#[cfg(windows)]
fn link_count(file: &File) -> Result<u64> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::*;
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
        return Err(std::io::Error::last_os_error()).context("Read organization hard-link count");
    }
    Ok(info.nNumberOfLinks as u64)
}

#[cfg(all(test, feature = "autotag"))]
mod metadata_copy_tests {
    use super::*;
    #[test]
    fn verified_copy_rejects_an_equal_byte_replacement_of_saved_input() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let source = root.join("backup");
        std::fs::write(&source, b"original metadata bytes").unwrap();
        let (parent, authority) = Parent::capture(&root).unwrap();
        let original = parent.fingerprint(Path::new("backup")).unwrap();
        let replacement = root.join("replacement");
        std::fs::write(&replacement, b"original metadata bytes").unwrap();
        std::fs::rename(replacement, &source).unwrap();
        assert!(
            parent
                .copy_prepared_verified(
                    &source,
                    Path::new("restored"),
                    &original.sha256,
                    &cuerithm_media::CancellationToken::default(),
                    &authority,
                    &original
                )
                .is_err(),
            "same-byte backup replacement cannot acquire saved source authority"
        );
        assert!(!root.join("restored").exists());
    }
    #[cfg(unix)]
    #[test]
    fn prepared_copy_rejects_a_symlink_to_equal_frozen_input_bytes() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let source = root.join("stage");
        let alias = root.join("alias");
        std::fs::write(&source, b"frozen stage bytes").unwrap();
        std::os::unix::fs::symlink(&source, &alias).unwrap();
        let expected = cuerithm_media::file_sha256(&source).unwrap();
        let (parent, _) = Parent::capture(&root).unwrap();
        assert!(
            parent
                .copy_prepared(
                    &alias,
                    Path::new("published"),
                    &expected,
                    &cuerithm_media::CancellationToken::default()
                )
                .is_err(),
            "native staged input must not follow links even when bytes match"
        );
        assert!(!root.join("published").exists());
    }
    #[test]
    fn prepared_copy_cannot_adopt_an_equal_byte_replacement_of_its_created_leaf() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let source = root.join("asset");
        std::fs::write(&source, b"owned stage bytes").unwrap();
        let (parent, _) = Parent::capture(&root).unwrap();
        let expected = cuerithm_media::file_sha256(&source).unwrap();
        let target = root.join("published");
        let result = parent.copy_prepared_with(
            &source,
            Path::new("published"),
            &expected,
            &cuerithm_media::CancellationToken::default(),
            || {
                let other = root.join("unknown");
                std::fs::write(&other, b"owned stage bytes")?;
                std::fs::rename(other, &target)?;
                Ok(())
            },
        );
        assert!(
            result.is_err(),
            "a same-byte leaf swap must not become the owned copy receipt"
        );
        assert_eq!(std::fs::read(&target).unwrap(), b"owned stage bytes");
    }
}
