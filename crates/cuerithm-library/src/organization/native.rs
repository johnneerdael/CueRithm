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
    fingerprint_bounded(file, u64::MAX)
}
/// Reject oversized dependencies before hashing and stop if a file grows while read.
pub(crate) fn fingerprint_bounded(file: &mut File, max_bytes: u64) -> Result<Fingerprint> {
    let before = file.metadata()?;
    ensure!(
        before.is_file(),
        "Only regular organization files are supported"
    );
    ensure!(
        before.len() <= max_bytes,
        "Organization dependency exceeds backup size budget"
    );
    let id = identity(file)?;
    file.seek(SeekFrom::Start(0))?;
    let mut sha = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut bytes = 0;
    loop {
        let limit = max_bytes
            .saturating_sub(bytes)
            .saturating_add(1)
            .min(buffer.len() as u64) as usize;
        let count = file.read(&mut buffer[..limit])?;
        if count == 0 {
            break;
        }
        ensure!(
            count as u64 <= max_bytes.saturating_sub(bytes),
            "Organization dependency exceeds backup size budget"
        );
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
