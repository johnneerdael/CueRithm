//! Reviewed, database-only reconnection. No audio is renamed or tagged.
use crate::{
    Library,
    model::{bytes_path, path_bytes},
};
use anyhow::{Context, Result, ensure};
use cuerithm_media::{CancellationToken, MediaRuntime, file_sha256_cancellable};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    time::{Instant, UNIX_EPOCH},
};
use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RelocationRequest {
    pub mode: String,
    pub track_ids: Vec<String>,
    pub search_root: Option<PathBuf>,
    pub replacement_file: Option<PathBuf>,
    pub source_root: Option<String>,
    pub destination_root: Option<PathBuf>,
    pub extension: Option<String>,
    pub all_tracks: bool,
    pub include_unavailable: bool,
    #[serde(default)]
    pub resume_plan: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelocationCandidate {
    pub id: String,
    pub path: PathBuf,
    pub evidence: String,
    pub preselected: bool,
    pub replacement: bool,
    pub occupied_by: Option<String>,
    pub size_bytes: u64,
    #[serde(default)]
    pub group_track_ids: Vec<String>,
    proof: Proof,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelocationRow {
    pub track_id: String,
    pub old_path: String,
    pub availability: String,
    pub state: String,
    pub candidates: Vec<RelocationCandidate>,
    pub decision: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelocationPlan {
    pub id: String,
    pub created: i64,
    pub revision: u64,
    pub rows: Vec<RelocationRow>,
    pub skipped: usize,
    pub incomplete: bool,
    pub unsearched: Vec<String>,
    #[serde(default)]
    pub unsearched_count: usize,
    pub paused: bool,
    generation: String,
    stamp: String,
    request: RelocationRequest,
    index_path: Vec<u8>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelocationChoice {
    pub track_id: String,
    pub candidate_id: String,
    pub survivor: String,
    #[serde(default)]
    pub survivor_track_id: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelocationResult {
    pub operation_id: String,
    pub applied: usize,
    pub no_op: usize,
    pub skipped: usize,
    pub backup_path: Option<PathBuf>,
    pub warnings: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelocationProgress {
    pub phase: String,
    pub completed: usize,
    pub total: Option<usize>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Proof {
    path: Vec<u8>,
    root: Vec<u8>,
    size: u64,
    mtime: String,
    hash: String,
    #[serde(default)]
    identity: Option<(u64, u64)>,
}
#[derive(Serialize, Deserialize)]
struct TrackBefore {
    id: String,
    path: Option<Vec<u8>>,
    hash: Option<String>,
    locator: Option<String>,
    archived: bool,
    analysis_status: String,
    selected_musical_revision: Option<String>,
    path_revision: i64,
    updated: i64,
    metadata: String,
}
#[derive(Serialize, Deserialize)]
struct Undo {
    before: Vec<TrackBefore>,
    members: Vec<(String, String, i64)>,
    links: Vec<(String, String, Option<String>, String)>,
    stamp: String,
    undone: bool,
}
fn init(conn: &Connection) -> Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS relocation_plans(id TEXT PRIMARY KEY,json TEXT NOT NULL);CREATE TABLE IF NOT EXISTS relocation_operations(id TEXT PRIMARY KEY,json TEXT NOT NULL,created INTEGER NOT NULL);")?;
    Ok(())
}
fn stamp(conn: &Connection) -> Result<String> {
    let mut s = conn.prepare("SELECT key,value FROM meta ORDER BY key")?;
    let v = s
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(&v)?)))
}
fn generation(lib: &Library) -> String {
    format!("{:x}", Sha256::digest(path_bytes(lib.data_dir())))
}
fn file_identity(path: &Path) -> Result<u64> {
    use std::hash::{Hash, Hasher};
    let mut key = std::collections::hash_map::DefaultHasher::new();
    same_file::Handle::from_path(path)?.hash(&mut key);
    Ok(key.finish())
}
struct OccupancyIndex {
    paths: BTreeMap<Vec<u8>, String>,
    identities: BTreeMap<u64, Vec<(PathBuf, String)>>,
}
impl OccupancyIndex {
    fn build(
        library: &Library,
        cancel: &CancellationToken,
        progress: &mut impl FnMut(RelocationProgress),
    ) -> Result<Self> {
        let rows = {
            let conn = library.conn()?;
            let mut query =
                conn.prepare("SELECT id,path FROM tracks WHERE path IS NOT NULL ORDER BY id")?;
            query
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        let mut index = Self {
            paths: BTreeMap::new(),
            identities: BTreeMap::new(),
        };
        let total = rows.len();
        let mut last = Instant::now();
        progress(RelocationProgress {
            phase: "Checking existing file bindings".into(),
            completed: 0,
            total: Some(total),
        });
        for (completed, (id, raw)) in rows.into_iter().enumerate() {
            cancel.check()?;
            if last.elapsed() >= std::time::Duration::from_millis(250) {
                progress(RelocationProgress {
                    phase: "Checking existing file bindings".into(),
                    completed,
                    total: Some(total),
                });
                last = Instant::now();
            }
            let path = bytes_path(&raw)?;
            if let Ok(key) = file_identity(&path) {
                index
                    .identities
                    .entry(key)
                    .or_default()
                    .push((path, id.clone()));
            }
            index.paths.insert(raw, id);
        }
        progress(RelocationProgress {
            phase: "Checking existing file bindings".into(),
            completed: total,
            total: Some(total),
        });
        Ok(index)
    }
    fn occupied(&self, path: &Path, source: &str) -> Result<Option<String>> {
        if let Some(id) = self.paths.get(&path_bytes(path))
            && id != source
        {
            return Ok(Some(id.clone()));
        }
        let key = file_identity(path)?;
        Ok(self.identities.get(&key).and_then(|rows| {
            rows.iter()
                .find(|(other, id)| {
                    id != source && same_file::is_same_file(other, path).unwrap_or(false)
                })
                .map(|(_, id)| id.clone())
        }))
    }
}
// Keep both the frontier and visited identities on disk. In particular, neither
// sorting a directory nor WalkDir's max_open fallback is bounded for flat folders.
#[derive(Clone, Copy)]
struct SearchLimits {
    candidates: usize,
    duration: std::time::Duration,
}
impl Default for SearchLimits {
    fn default() -> Self {
        Self {
            candidates: 1_000_000,
            duration: std::time::Duration::from_secs(900),
        }
    }
}
fn is_redirect(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}
fn directory_identity(path: &Path, metadata: &std::fs::Metadata) -> Result<String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let _ = path;
        #[cfg(target_os = "linux")]
        {
            Ok(format!(
                "{}:{}:{}",
                metadata.dev(),
                metadata.ino(),
                mount_id(path)?
            ))
        }
        #[cfg(not(target_os = "linux"))]
        {
            Ok(format!("{}:{}", metadata.dev(), metadata.ino()))
        }
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        Ok(file_identity(path)?.to_string())
    }
}
fn same_volume(root: &std::fs::Metadata, entry: &std::fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        root.dev() == entry.dev()
    }
    #[cfg(not(unix))]
    {
        let _ = (root, entry);
        true
    } // Windows volume redirects carry REPARSE_POINT.
}
fn skipped_search(index: &Connection, plan: &mut RelocationPlan, path: &Path) -> Result<()> {
    record_unsearched(index, plan, path, "Inaccessible or redirected path")
}
fn record_unsearched(
    index: &Connection,
    plan: &mut RelocationPlan,
    path: &Path,
    reason: &str,
) -> Result<()> {
    let inserted = index.execute(
        "INSERT OR IGNORE INTO unsearched_paths(path) VALUES(?1)",
        [path_bytes(path)],
    )?;
    if inserted == 0 {
        return Ok(());
    }
    plan.unsearched_count += 1;
    if plan.unsearched.len() < 2000 {
        plan.unsearched.push(format!(
            "{}: {}",
            path.to_string_lossy(),
            reason.chars().take(512).collect::<String>()
        ));
    } else if plan.unsearched.len() == 2000 {
        plan.unsearched
            .push("Additional inaccessible or redirected paths omitted".into());
    }
    Ok(())
}
// Mount IDs distinguish same-device Linux bind mounts, which dev()/canonicalize
// cannot detect. Read mountinfo only when the kernel cannot supply STATX_MNT_ID.
#[cfg(target_os = "linux")]
fn mount_id(path: &Path) -> Result<u64> {
    use std::os::unix::ffi::OsStrExt;
    let native = std::ffi::CString::new(path.as_os_str().as_bytes())?;
    let mut stat = std::mem::MaybeUninit::<libc::statx>::zeroed();
    // SAFETY: CString and the writable statx buffer remain live for this syscall.
    let result = unsafe {
        libc::statx(
            libc::AT_FDCWD,
            native.as_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
            libc::STATX_MNT_ID,
            stat.as_mut_ptr(),
        )
    };
    if result == 0 {
        // SAFETY: a successful statx initialized the supplied statx structure.
        let stat = unsafe { stat.assume_init() };
        if stat.stx_mask & libc::STATX_MNT_ID != 0 {
            return Ok(stat.stx_mnt_id);
        }
    }
    mountinfo_id(
        std::io::BufReader::new(std::fs::File::open("/proc/self/mountinfo")?),
        path,
    )
    .context("Cannot verify mount identity; directory remains unsearched")
}
#[cfg(any(target_os = "linux", all(test, unix)))]
fn mountinfo_id(reader: impl std::io::BufRead, path: &Path) -> Result<u64> {
    use std::os::unix::ffi::OsStrExt;
    let mut best: Option<(usize, u64)> = None;
    for line in reader.split(b'\n') {
        let line = line?;
        let fields: Vec<_> = line.split(|b| *b == b' ').take(6).collect();
        ensure!(fields.len() >= 6, "Malformed mountinfo record");
        let id = std::str::from_utf8(fields[0])?.parse::<u64>()?;
        let mut decoded = Vec::with_capacity(fields[4].len());
        let mut i = 0;
        while i < fields[4].len() {
            if fields[4][i] == b'\\' {
                ensure!(
                    i + 3 < fields[4].len()
                        && fields[4][i + 1..i + 4]
                            .iter()
                            .all(|b| matches!(b, b'0'..=b'7')),
                    "Malformed mountinfo escape"
                );
                let value = (fields[4][i + 1] - b'0') as u16 * 64
                    + (fields[4][i + 2] - b'0') as u16 * 8
                    + (fields[4][i + 3] - b'0') as u16;
                ensure!(value <= 255, "Invalid mountinfo escape");
                decoded.push(value as u8);
                i += 4;
            } else {
                decoded.push(fields[4][i]);
                i += 1;
            }
        }
        let mount = Path::new(std::ffi::OsStr::from_bytes(&decoded));
        if path.starts_with(mount) && best.is_none_or(|(length, _)| decoded.len() > length) {
            best = Some((decoded.len(), id));
        }
    }
    best.map(|(_, id)| id)
        .context("Path has no verifiable mount identity")
}
fn discover_candidates(
    index: &Connection,
    selected_root: &Path,
    index_directory: &Path,
    plan: &mut RelocationPlan,
    cancel: &CancellationToken,
    progress: &mut impl FnMut(RelocationProgress),
    limits: SearchLimits,
) -> Result<()> {
    no_links(selected_root)?;
    let root = selected_root.canonicalize()?;
    let root_metadata = std::fs::symlink_metadata(&root)?;
    ensure!(
        root_metadata.is_dir() && !is_redirect(&root_metadata),
        "Search root is unavailable or redirected"
    );
    let root_identity = directory_identity(&root, &root_metadata)?;
    #[cfg(target_os = "linux")]
    let root_mount = mount_id(&root)?;
    index.execute_batch("CREATE TABLE IF NOT EXISTS directories(path BLOB PRIMARY KEY,identity TEXT UNIQUE NOT NULL,done INTEGER NOT NULL DEFAULT 0); CREATE INDEX IF NOT EXISTS pending_directories ON directories(done,path); CREATE TABLE IF NOT EXISTS search_root(path BLOB PRIMARY KEY,identity TEXT NOT NULL);")?;
    let old_root: Option<(Vec<u8>, String)> = index
        .query_row("SELECT path,identity FROM search_root LIMIT 1", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .optional()?;
    if let Some((path, identity)) = old_root {
        ensure!(
            path == path_bytes(&root) && identity == root_identity,
            "Search root changed; start a new search"
        );
    } else {
        index.execute(
            "INSERT INTO search_root VALUES(?1,?2)",
            params![path_bytes(&root), root_identity],
        )?;
        index.execute(
            "INSERT INTO directories(path,identity) VALUES(?1,?2)",
            params![path_bytes(&root), root_identity],
        )?;
    }
    ensure!(
        fs2::available_space(index_directory)? >= 256 * 1024 * 1024,
        "Insufficient disk space for relocation index (256 MiB reserve)"
    );
    let started = Instant::now();
    let mut tick = Instant::now();
    let mut count = 0;
    let mut entries = 0usize;
    progress(RelocationProgress {
        phase: "searching".into(),
        completed: 0,
        total: None,
    });
    'search: loop {
        cancel.check()?;
        let next: Option<(Vec<u8>, String)> = index
            .query_row(
                "SELECT path,identity FROM directories WHERE done=0 ORDER BY path LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((raw, identity)) = next else {
            break;
        };
        let path = bytes_path(&raw)?;
        let checked = (|| -> Result<std::fs::ReadDir> {
            ensure!(
                directory_identity(&root, &std::fs::symlink_metadata(&root)?)? == root_identity,
                "Search root identity changed"
            );
            no_links(&path)?;
            #[cfg(target_os = "linux")]
            ensure!(
                mount_id(&path)? == root_mount,
                "Directory crosses selected mount (including bind mount)"
            );
            let metadata = std::fs::symlink_metadata(&path)?;
            ensure!(
                metadata.is_dir()
                    && !is_redirect(&metadata)
                    && same_volume(&root_metadata, &metadata),
                "Redirected directory"
            );
            ensure!(
                directory_identity(&path, &metadata)? == identity
                    && path.canonicalize()?.starts_with(&root),
                "Directory identity changed or escaped root"
            );
            Ok(std::fs::read_dir(&path)?)
        })();
        let directory = match checked {
            Ok(directory) => directory,
            Err(error) => {
                record_unsearched(index, plan, &path, &error.to_string())?;
                index.execute("UPDATE directories SET done=1 WHERE path=?1", [&raw])?;
                continue;
            }
        };
        for entry in directory {
            cancel.check()?;
            if tick.elapsed() >= std::time::Duration::from_millis(250) {
                progress(RelocationProgress {
                    phase: "searching".into(),
                    completed: count,
                    total: None,
                });
                tick = Instant::now();
            }
            if count >= limits.candidates || started.elapsed() >= limits.duration {
                plan.paused = true;
                break 'search;
            }
            entries += 1;
            if entries.is_multiple_of(1024) {
                no_links(&path)?;
                ensure!(
                    directory_identity(&path, &std::fs::symlink_metadata(&path)?)? == identity,
                    "Search directory changed during enumeration"
                );
                ensure!(
                    directory_identity(&root, &std::fs::symlink_metadata(&root)?)? == root_identity,
                    "Search root changed during enumeration"
                );
                ensure!(
                    fs2::available_space(index_directory)? >= 256 * 1024 * 1024,
                    "Insufficient disk space for relocation index (256 MiB reserve)"
                );
            }
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => {
                    skipped_search(index, plan, &path)?;
                    continue;
                }
            };
            let candidate = entry.path();
            let metadata = match std::fs::symlink_metadata(&candidate) {
                Ok(metadata) => metadata,
                Err(_) => {
                    skipped_search(index, plan, &candidate)?;
                    continue;
                }
            };
            #[cfg(target_os = "linux")]
            match mount_id(&candidate) {
                Ok(id) if id == root_mount => {}
                Ok(_) => {
                    record_unsearched(
                        index,
                        plan,
                        &candidate,
                        "Entry crosses selected mount (including bind mount)",
                    )?;
                    continue;
                }
                Err(error) => {
                    record_unsearched(index, plan, &candidate, &error.to_string())?;
                    continue;
                }
            }
            if is_redirect(&metadata) || !same_volume(&root_metadata, &metadata) {
                skipped_search(index, plan, &candidate)?;
                continue;
            }
            if !candidate
                .canonicalize()
                .is_ok_and(|canonical| canonical.starts_with(&root) && canonical == candidate)
            {
                skipped_search(index, plan, &candidate)?;
                continue;
            }
            if metadata.is_dir() {
                let identity = directory_identity(&candidate, &metadata)?;
                index.execute(
                    "INSERT OR IGNORE INTO directories(path,identity) VALUES(?1,?2)",
                    params![path_bytes(&candidate), identity],
                )?;
            } else if metadata.is_file() && crate::watch::supported_path(&candidate) {
                let name = signature(&entry.file_name().to_string_lossy());
                let parent = path
                    .file_name()
                    .map(|s| s.to_string_lossy().nfc().collect::<String>())
                    .unwrap_or_default();
                count += index.execute(
                    "INSERT OR IGNORE INTO candidates VALUES(?1,?2,?3,?4,?5)",
                    params![
                        path_bytes(&candidate),
                        parent,
                        name,
                        name.to_lowercase(),
                        metadata.len().min(i64::MAX as u64) as i64
                    ],
                )?;
            }
        }
        no_links(&path)?;
        ensure!(
            directory_identity(&path, &std::fs::symlink_metadata(&path)?)? == identity,
            "Search directory changed during enumeration"
        );
        // A pause leaves the current directory pending. Resume streams it again,
        // deduplicating indexed files, without retaining an OS directory cursor.
        index.execute("UPDATE directories SET done=1 WHERE path=?1", [&raw])?;
    }
    no_links(&root)?;
    ensure!(
        directory_identity(&root, &std::fs::symlink_metadata(&root)?)? == root_identity,
        "Search root changed during enumeration"
    );
    progress(RelocationProgress {
        phase: "searching".into(),
        completed: count,
        total: None,
    });
    Ok(())
}
fn no_links(path: &Path) -> Result<()> {
    ensure!(path.is_absolute(), "Choose an absolute local path");
    for p in path.ancestors() {
        let m = std::fs::symlink_metadata(p)?;
        ensure!(!is_redirect(&m), "Symbolic-link parents are not allowed");
    }
    Ok(())
}
fn proof(path: &Path, root: &Path, cancel: &CancellationToken) -> Result<Proof> {
    ensure!(
        !is_redirect(&std::fs::symlink_metadata(root)?),
        "Search root cannot be a symbolic link"
    );
    for parent in path.ancestors() {
        ensure!(
            !is_redirect(&std::fs::symlink_metadata(parent)?),
            "Symbolic-link candidate or parent"
        );
        if parent == root {
            break;
        }
    }
    let root = root.canonicalize()?;
    let canonical = path.canonicalize()?;
    no_links(&canonical)?;
    ensure!(
        canonical.starts_with(&root),
        "Candidate escaped selected root"
    );
    #[cfg(target_os = "linux")]
    ensure!(
        mount_id(&root)? == mount_id(&canonical)?,
        "Candidate crosses selected mount (including bind mount)"
    );
    let m = std::fs::symlink_metadata(&canonical)?;
    ensure!(
        same_volume(&std::fs::symlink_metadata(&root)?, &m),
        "Candidate crossed a mounted volume"
    );
    ensure!(
        m.is_file() && !is_redirect(&m),
        "Candidate must be a regular file"
    );
    #[cfg(unix)]
    let identity = {
        use std::os::unix::fs::MetadataExt;
        Some((m.dev(), m.ino()))
    };
    #[cfg(not(unix))]
    let identity = None;
    let p = Proof {
        path: path_bytes(&canonical),
        root: path_bytes(&root),
        size: m.len(),
        mtime: m
            .modified()?
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .to_string(),
        hash: file_sha256_cancellable(&canonical, cancel)?,
        identity,
    };
    let after = std::fs::metadata(&canonical)?;
    no_links(&canonical)?;
    ensure!(
        after.len() == m.len() && after.modified()? == m.modified()?,
        "Candidate changed during verification"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        ensure!(
            after.dev() == m.dev() && after.ino() == m.ino(),
            "Candidate identity changed during verification"
        );
    }
    Ok(p)
}
fn parts(path: &str) -> Result<(bool, Vec<String>)> {
    let windows =
        path.starts_with("\\\\") || path.starts_with("//") || path.as_bytes().get(1) == Some(&b':');
    parts_flavored(path, windows)
}
fn parts_flavored(path: &str, windows: bool) -> Result<(bool, Vec<String>)> {
    let text = if windows {
        path.replace('\\', "/")
    } else {
        path.to_owned()
    };
    ensure!(
        !text.starts_with("//?/") && !text.starts_with("//./"),
        "Device paths are not binding inputs"
    );
    ensure!(
        if windows {
            text.starts_with("//")
                || (text.as_bytes().get(1) == Some(&b':') && text.as_bytes().get(2) == Some(&b'/'))
        } else {
            text.starts_with('/')
        },
        "Source root must be absolute"
    );
    let out: Vec<String> = text
        .split('/')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    ensure!(
        !out.iter()
            .any(|s| s == "." || s == ".." || s.contains('\0')),
        "Unsafe source components"
    );
    Ok((windows, out))
}
pub fn relocation_root_suffix(path: &str, root: &str) -> Result<Option<Vec<String>>> {
    let (w, p) = parts(path)?;
    let (rw, r) = parts(root)?;
    if w != rw || p.len() < r.len() {
        return Ok(None);
    }
    if !p
        .iter()
        .zip(&r)
        .all(|(a, b)| if w { a.eq_ignore_ascii_case(b) } else { a == b })
    {
        return Ok(None);
    }
    Ok(Some(p[r.len()..].to_vec()))
}
fn root_suffix_flavored(path: &str, root: &str, windows: bool) -> Result<Option<Vec<String>>> {
    let (_, p) = parts_flavored(path, windows)?;
    let (_, r) = parts_flavored(root, windows)?;
    if p.len() < r.len()
        || !p.iter().zip(&r).all(|(a, b)| {
            if windows {
                a.eq_ignore_ascii_case(b)
            } else {
                a == b
            }
        })
    {
        return Ok(None);
    }
    Ok(Some(p[r.len()..].to_vec()))
}
fn signature(name: &str) -> String {
    let n: String = name.nfc().collect();
    let (stem, ext) = n.rsplit_once('.').unwrap_or((&n, ""));
    let stem = if stem.to_ascii_lowercase().ends_with("_pn") {
        &stem[..stem.len() - 3]
    } else {
        stem
    };
    if ext.is_empty() {
        stem.to_owned()
    } else {
        format!("{stem}.{ext}")
    }
}
fn source_display(lib: &Library, id: &str, path: Option<&[u8]>) -> Result<String> {
    if let Some(b) = path {
        if let Some(bytes) = b.strip_prefix(b"windows:") {
            ensure!(bytes.len().is_multiple_of(2), "Odd native UTF16 path");
            let units = bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .collect::<Vec<_>>();
            let logical = crate::portable::windows_logical_units(&units)
                .context("Device namespace requires explicit file replacement")?;
            return String::from_utf16(&logical)
                .context("Opaque source path requires explicit file replacement");
        }
        return bytes_path(b)?
            .to_str()
            .map(str::to_owned)
            .context("Opaque source path requires explicit file replacement");
    }
    let raw: Option<String> = lib.conn()?.query_row(
        "SELECT source_locator_json FROM tracks WHERE id=?1",
        [id],
        |r| r.get(0),
    )?;
    if let Some(s) = raw {
        let l: crate::PortableLocator = serde_json::from_str(&s)?;
        let decode = |p: &crate::EncodedPath| -> Result<String> {
            let bytes = p.bytes()?;
            if p.encoding == crate::PathEncoding::Base64Utf16le {
                let units = bytes
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|b| u16::from_le_bytes([b[0], b[1]]))
                    .collect::<Vec<_>>();
                Ok(String::from_utf16(&units)?)
            } else {
                Ok(String::from_utf8(bytes)?)
            }
        };
        let original =
            decode(&l.original).context("Opaque source path requires explicit file replacement")?;
        let slashed = original.replace('\\', "/");
        if slashed.starts_with("//?/") || slashed.starts_with("//./") {
            l.validate()?;
            let root = l
                .source_root
                .as_ref()
                .context("Unbound device path requires explicit file replacement")?;
            let components = l
                .components
                .as_ref()
                .context("Unbound device path requires explicit file replacement")?;
            let mut text = decode(root)?;
            for component in components {
                if !text.ends_with(['/', '\\']) {
                    text.push('/');
                }
                text.push_str(&decode(component)?);
            }
            return Ok(text);
        }
        return Ok(original);
    }
    Ok("Unresolved reference".into())
}
fn availability(path: Option<&[u8]>) -> String {
    let Some(p) = path.and_then(|b| bytes_path(b).ok()) else {
        return "unsupported".into();
    };
    match std::fs::symlink_metadata(&p) {
        Ok(m) if m.is_file() && !is_redirect(&m) => "present",
        Ok(_) => "unavailable",
        Err(e)
            if e.kind() == std::io::ErrorKind::NotFound && p.parent().is_some_and(Path::is_dir) =>
        {
            "missing"
        }
        Err(_) => "unavailable",
    }
    .into()
}
fn snapshot_track(conn: &Connection, id: &str) -> Result<TrackBefore> {
    Ok(conn.query_row("SELECT id,path,source_hash,source_locator_json,archived,analysis_status,path_revision,updated,metadata_json,selected_musical_revision FROM tracks WHERE id=?1",[id],|r|Ok(TrackBefore{id:r.get(0)?,path:r.get(1)?,hash:r.get(2)?,locator:r.get(3)?,archived:r.get(4)?,analysis_status:r.get(5)?,path_revision:r.get(6)?,updated:r.get(7)?,metadata:r.get(8)?,selected_musical_revision:r.get(9)?}))?)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelocationHistory {
    pub operation_id: String,
    pub created: i64,
    pub applied: usize,
    pub undone: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelocationSourceRoot {
    pub source_root: String,
    pub track_count: usize,
}
impl Library {
    pub fn relocation_roots(&self) -> Result<Vec<RelocationSourceRoot>> {
        let rows = {
            let c = self.conn()?;
            let mut q = c.prepare("SELECT id,path FROM tracks ORDER BY id")?;
            q.query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, Option<Vec<u8>>>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
        };
        let mut roots = BTreeMap::<String, usize>::new();
        let mut groups = BTreeMap::<String, Vec<Vec<String>>>::new();
        for (id, path) in rows {
            let Ok(display) = source_display(self, &id, path.as_deref()) else {
                continue;
            };
            let windows = if path.is_some() {
                cfg!(windows)
            } else {
                self.source_locator(&id)?.is_some_and(|l| {
                    !matches!(
                        l.flavor,
                        crate::PathFlavor::Posix | crate::PathFlavor::OpaquePosix
                    )
                })
            };
            let Ok((_, mut components)) = parts_flavored(&display, windows) else {
                continue;
            };
            components.pop();
            if components.is_empty() {
                continue;
            }
            let prefix = if windows && display.starts_with(['/', '\\']) {
                "//"
            } else if windows {
                ""
            } else {
                "/"
            };
            let parent = format!("{prefix}{}", components.join("/"));
            *roots.entry(parent).or_default() += 1;
            let group = if windows {
                format!("{prefix}{}", components[0].to_lowercase())
            } else {
                "/".into()
            };
            groups.entry(group).or_default().push(components);
        }
        for (group, paths) in groups {
            let Some(first) = paths.first() else {
                continue;
            };
            let mut common = first.clone();
            for p in &paths[1..] {
                let n = common
                    .iter()
                    .zip(p)
                    .take_while(|(a, b)| {
                        if group == "/" {
                            a == b
                        } else {
                            a.eq_ignore_ascii_case(b)
                        }
                    })
                    .count();
                common.truncate(n);
            }
            if common.len() > 1 {
                let prefix = if group == "/" {
                    "/"
                } else if group.starts_with("//") {
                    "//"
                } else {
                    ""
                };
                roots.insert(format!("{prefix}{}", common.join("/")), paths.len());
            }
        }
        let mut roots: Vec<_> = roots
            .into_iter()
            .map(|(source_root, track_count)| RelocationSourceRoot {
                source_root,
                track_count,
            })
            .collect();
        roots.sort_by(|a, b| {
            b.track_count
                .cmp(&a.track_count)
                .then(a.source_root.cmp(&b.source_root))
        });
        roots.truncate(2000);
        Ok(roots)
    }
    /// Call under the installation operation lease, before any replacement is
    /// scheduled. Existing physical writers own these same gates and journals.
    pub fn ensure_portable_operation_safe(&self) -> Result<()> {
        let _sync = crate::library_backup::sync_lease(self.data_dir())?;
        let _organization = self.inner.organization_gate.try_lock().map_err(|_| {
            anyhow::anyhow!("Organization is busy; finish it before replacing the library")
        })?;
        #[cfg(feature = "autotag")]
        let _tag = self.inner.tag_gate.try_lock().map_err(|_| {
            anyhow::anyhow!("Metadata writer is busy; finish it before replacing the library")
        })?;
        let conn = self.conn()?;
        crate::library_backup::quiet_database(&conn)
    }
    pub fn plan_relocation(
        &self,
        request: RelocationRequest,
        runtime: &MediaRuntime,
        cancel: &CancellationToken,
        mut progress: impl FnMut(RelocationProgress),
    ) -> Result<RelocationPlan> {
        ensure!(
            request.track_ids.len() <= 100_000,
            "Too many selected tracks"
        );
        if let Some(ext) = &request.extension {
            ensure!(
                !ext.is_empty()
                    && ext.bytes().all(|b| b.is_ascii_alphanumeric())
                    && crate::watch::supported_path(Path::new(&format!("x.{ext}"))),
                "Unsupported replacement extension"
            );
        }
        let (input, revision, saved_stamp) = {
            let c = self.conn()?;
            init(&c)?;
            let mut q = c.prepare(
                "SELECT id,path,source_hash,state,metadata_json FROM tracks ORDER BY id",
            )?;
            let selected: BTreeSet<_> = request.track_ids.iter().collect();
            let rows = q
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, Option<Vec<u8>>>(1)?,
                        r.get::<_, Option<String>>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, String>(4)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?
                .into_iter()
                .filter(|r| request.mode == "root" || selected.contains(&r.0))
                .collect::<Vec<_>>();
            (rows, self.revision_unlocked(&c)?, stamp(&c)?)
        };
        let occupied_index = OccupancyIndex::build(self, cancel, &mut progress)?;
        let previous = request
            .resume_plan
            .as_ref()
            .map(|id| self.relocation_plan(id))
            .transpose()?;
        if let Some(previous) = &previous {
            ensure!(
                previous.stamp == saved_stamp && previous.generation == generation(self),
                "Library changed; start a new search"
            );
            ensure!(previous.paused, "Search is already complete");
        }
        let id = previous
            .as_ref()
            .map(|p| p.id.clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let directory = self.data_dir().join("relocation-indexes");
        std::fs::create_dir_all(&directory)?;
        ensure!(
            !std::fs::symlink_metadata(&directory)?
                .file_type()
                .is_symlink(),
            "Unsafe relocation index directory"
        );
        let index_path = directory.join(format!("{id}.sqlite"));
        let mut plan = RelocationPlan {
            id,
            created: crate::now(),
            revision,
            rows: vec![],
            skipped: 0,
            incomplete: false,
            unsearched: vec![],
            unsearched_count: 0,
            paused: false,
            generation: generation(self),
            stamp: saved_stamp,
            request: request.clone(),
            index_path: path_bytes(&index_path),
        };
        let mut index_conn = Connection::open(&index_path)?;
        index_conn.execute_batch("CREATE TABLE IF NOT EXISTS unsearched_paths(path BLOB PRIMARY KEY);CREATE TABLE IF NOT EXISTS candidates(path BLOB PRIMARY KEY,parent TEXT,name TEXT,fold TEXT,size INTEGER);CREATE INDEX IF NOT EXISTS signature ON candidates(parent,name);CREATE INDEX IF NOT EXISTS filename ON candidates(name);CREATE INDEX IF NOT EXISTS folded ON candidates(fold);")?;
        let index = index_conn.transaction()?;
        if let Some(previous) = &previous {
            plan.unsearched = previous.unsearched.clone();
            plan.unsearched_count = previous.unsearched_count.max(previous.unsearched.len());
        }
        if let Some(root) = &request.search_root {
            discover_candidates(
                &index,
                root,
                &directory,
                &mut plan,
                cancel,
                &mut progress,
                SearchLimits::default(),
            )?;
        }
        plan.incomplete = plan.paused || !plan.unsearched.is_empty();
        let total = input.len();
        for (id, path, hash, state, metadata) in input {
            cancel.check()?;
            let av = availability(path.as_deref());
            if av == "present" && !request.all_tracks {
                plan.skipped += 1;
                continue;
            }
            if av == "unavailable" && !request.include_unavailable {
                plan.skipped += 1;
                continue;
            }
            let old = match source_display(self, &id, path.as_deref()) {
                Ok(s) => s,
                Err(_) if request.replacement_file.is_some() => "Opaque source reference".into(),
                Err(_) => {
                    plan.rows.push(RelocationRow {
                        track_id: id,
                        old_path: "Opaque source reference".into(),
                        availability: av,
                        state,
                        candidates: vec![],
                        decision: "manual_selection_required".into(),
                    });
                    continue;
                }
            };
            let windows_source = if path.is_some() {
                cfg!(windows)
            } else {
                self.source_locator(&id)?.is_some_and(|l| {
                    !matches!(
                        l.flavor,
                        crate::PathFlavor::Posix | crate::PathFlavor::OpaquePosix
                    )
                })
            };
            let name = old
                .rsplit(if windows_source {
                    &['/', '\\'][..]
                } else {
                    &['/'][..]
                })
                .next()
                .unwrap_or(&old);
            let sig = signature(name);
            let parent = old
                .rsplit(if windows_source {
                    &['/', '\\'][..]
                } else {
                    &['/'][..]
                })
                .nth(1)
                .unwrap_or("")
                .nfc()
                .collect::<String>();
            let mut options: Vec<(PathBuf, String)> = vec![];
            if let Some(file) = &request.replacement_file {
                ensure!(
                    request.track_ids.len() == 1,
                    "A replacement file applies to one selected track"
                );
                options.push((file.clone(), "Explicit replacement".into()));
            } else if request.mode == "root" {
                let root = request
                    .source_root
                    .as_deref()
                    .context("Choose source root")?;
                let destination = request
                    .destination_root
                    .as_ref()
                    .context("Choose destination root")?;
                if let Some(suffix) = root_suffix_flavored(&old, root, windows_source)? {
                    let mut target = destination.clone();
                    for atom in suffix {
                        ensure!(
                            !atom.contains('/') && (!cfg!(windows) || !atom.contains('\\')),
                            "Unrepresentable destination component"
                        );
                        target.push(atom);
                    }
                    if let Some(ext) = &request.extension {
                        target.set_extension(ext);
                    }
                    if target.is_file() {
                        options.push((target, "Root mapping".into()));
                    }
                } else {
                    plan.skipped += 1;
                    continue;
                }
            } else {
                let mut q = index.prepare(
                    "SELECT path FROM candidates WHERE parent=?1 AND name=?2 ORDER BY path",
                )?;
                options = q
                    .query_map(params![parent, sig], |r| r.get::<_, Vec<u8>>(0))?
                    .map(|b| Ok((bytes_path(&b?)?, "Parent and filename".into())))
                    .collect::<Result<Vec<_>>>()?;
                if options.is_empty() {
                    let mut q =
                        index.prepare("SELECT path FROM candidates WHERE name=?1 ORDER BY path")?;
                    options = q
                        .query_map([&sig], |r| r.get::<_, Vec<u8>>(0))?
                        .map(|b| Ok((bytes_path(&b?)?, "Filename".into())))
                        .collect::<Result<Vec<_>>>()?;
                }
                if options.is_empty() {
                    let mut q =
                        index.prepare("SELECT path FROM candidates WHERE fold=?1 ORDER BY path")?;
                    options = q
                        .query_map([sig.to_lowercase()], |r| r.get::<_, Vec<u8>>(0))?
                        .map(|b| Ok((bytes_path(&b?)?, "Case-insensitive suggestion".into())))
                        .collect::<Result<Vec<_>>>()?;
                }
            }
            let saved_size = serde_json::from_str::<serde_json::Value>(&metadata)
                .ok()
                .and_then(|v| {
                    v.get("cuerithm.portable_content")
                        .and_then(|c| c.get("size_bytes"))
                        .or_else(|| v.get("format.size"))
                        .or_else(|| v.pointer("/properties/format.size"))
                        .and_then(|v| {
                            v.as_u64()
                                .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
                        })
                });
            let mut candidates = vec![];
            for (target, evidence) in options {
                cancel.check()?;
                let root = request
                    .search_root
                    .as_deref()
                    .or(request.destination_root.as_deref())
                    .or_else(|| target.parent())
                    .context("Target has no parent")?;
                let p = match proof(&target, root, cancel) {
                    Ok(p) => p,
                    Err(e) => {
                        record_unsearched(&index, &mut plan, &target, &e.to_string())?;
                        continue;
                    }
                };
                if runtime.probe_cancellable(&target, cancel).is_err() {
                    cancel.check()?;
                    continue;
                }
                let occupied = occupied_index.occupied(&target, &id)?;
                let ext_changed = Path::new(name).extension().map(|x| x.to_ascii_lowercase())
                    != target.extension().map(|x| x.to_ascii_lowercase());
                let replacement = ext_changed
                    || hash.as_ref().is_some_and(|h| h != &p.hash)
                    || saved_size.is_some_and(|s| s != p.size)
                    || evidence.contains("suggestion")
                    || evidence == "Explicit replacement";
                candidates.push(RelocationCandidate {
                    id: uuid::Uuid::new_v4().to_string(),
                    path: target,
                    evidence,
                    preselected: false,
                    replacement,
                    occupied_by: occupied,
                    size_bytes: p.size,
                    group_track_ids: vec![],
                    proof: p,
                });
            }
            let verified: Vec<_> = candidates
                .iter()
                .enumerate()
                .filter(|(_, c)| !c.replacement && hash.as_ref() == Some(&c.proof.hash))
                .map(|(i, _)| i)
                .collect();
            let sizes: Vec<_> = candidates
                .iter()
                .enumerate()
                .filter(|(_, c)| !c.replacement && saved_size == Some(c.size_bytes))
                .map(|(i, _)| i)
                .collect();
            let picked = if verified.len() == 1 {
                Some(verified[0])
            } else if !plan.incomplete && candidates.len() == 1 && !candidates[0].replacement {
                Some(0)
            } else if !plan.incomplete && sizes.len() == 1 {
                Some(sizes[0])
            } else {
                None
            };
            if let Some(i) = picked {
                candidates[i].preselected = candidates[i].occupied_by.is_none();
                if verified.len() == 1 {
                    candidates[i].evidence = "Verified encoded content".into();
                }
            }
            let decision = if candidates.is_empty() {
                if plan.incomplete {
                    "search_incomplete"
                } else {
                    "unmatched"
                }
            } else if picked.is_some() {
                "matched"
            } else if candidates.len() > 1 {
                "ambiguous"
            } else {
                "replacement"
            }
            .into();
            plan.rows.push(RelocationRow {
                track_id: id,
                old_path: old,
                availability: av,
                state,
                candidates,
                decision,
            });
            progress(RelocationProgress {
                phase: "matching".into(),
                completed: plan.rows.len() + plan.skipped,
                total: Some(total),
            });
        }
        // Encoded equality does not merge entries; only the same resolved file forms a group.
        let mut shared = BTreeMap::<u64, Vec<Vec<(usize, usize)>>>::new();
        for (row_index, row) in plan.rows.iter().enumerate() {
            for (candidate_index, candidate) in row.candidates.iter().enumerate() {
                cancel.check()?;
                let bucket = shared.entry(file_identity(&candidate.path)?).or_default();
                if let Some(group) = bucket.iter_mut().find(|positions| {
                    let (r, c) = positions[0];
                    same_file::is_same_file(&plan.rows[r].candidates[c].path, &candidate.path)
                        .unwrap_or(false)
                }) {
                    group.push((row_index, candidate_index));
                } else {
                    bucket.push(vec![(row_index, candidate_index)]);
                }
            }
        }
        let mut group_references = 0usize;
        for positions in shared.into_values().flatten() {
            let ids: BTreeSet<_> = positions
                .iter()
                .map(|(r, _)| plan.rows[*r].track_id.clone())
                .collect();
            if ids.len() < 2 {
                continue;
            }
            group_references = group_references
                .checked_add(
                    (ids.len() - 1)
                        .checked_mul(positions.len())
                        .context("Collision review exceeds resource limit")?,
                )
                .context("Collision review exceeds resource limit")?;
            ensure!(
                group_references <= 1_000_000,
                "Collision review exceeds resource limit; review a smaller track selection"
            );
            for (r, c) in positions {
                let own = plan.rows[r].track_id.clone();
                let candidate = &mut plan.rows[r].candidates[c];
                candidate.group_track_ids = ids.iter().filter(|id| **id != own).cloned().collect();
                candidate.preselected = false;
                plan.rows[r].decision = "group_collision".into();
            }
        }
        index.commit()?;
        plan.incomplete |= plan.unsearched_count > 0 || !plan.unsearched.is_empty();
        if plan.incomplete {
            for row in &mut plan.rows {
                for candidate in &mut row.candidates {
                    if candidate.evidence != "Verified encoded content" {
                        candidate.preselected = false;
                    }
                }
            }
        }
        cancel.check()?;
        let c = self.conn()?;
        ensure!(
            stamp(&c)? == plan.stamp,
            "Library changed during search; preview again"
        );
        c.execute(
            "INSERT OR REPLACE INTO relocation_plans VALUES(?1,?2)",
            params![plan.id, serde_json::to_string(&plan)?],
        )?;
        Ok(plan)
    }
    pub fn continue_relocation(
        &self,
        id: &str,
        runtime: &MediaRuntime,
        cancel: &CancellationToken,
        progress: impl FnMut(RelocationProgress),
    ) -> Result<RelocationPlan> {
        let plan = self.relocation_plan(id)?;
        let mut request = plan.request;
        request.resume_plan = Some(id.into());
        self.plan_relocation(request, runtime, cancel, progress)
    }
    pub fn relocation_history(&self) -> Result<Vec<RelocationHistory>> {
        let c = self.conn()?;
        init(&c)?;
        let mut q = c.prepare(
            "SELECT id,json,created FROM relocation_operations ORDER BY created DESC LIMIT 100",
        )?;
        let rows = q
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.into_iter()
            .map(|(operation_id, json, created)| {
                let u: Undo = serde_json::from_str(&json)?;
                Ok(RelocationHistory {
                    operation_id,
                    created,
                    applied: u.before.len(),
                    undone: u.undone,
                })
            })
            .collect()
    }
    fn revision_unlocked(&self, c: &Connection) -> Result<u64> {
        Ok(
            c.query_row("SELECT value FROM meta WHERE key='revision'", [], |r| {
                r.get(0)
            })?,
        )
    }
    pub fn relocation_plan(&self, id: &str) -> Result<RelocationPlan> {
        let c = self.conn()?;
        init(&c)?;
        let json: String =
            c.query_row("SELECT json FROM relocation_plans WHERE id=?1", [id], |r| {
                r.get(0)
            })?;
        Ok(serde_json::from_str(&json)?)
    }
    fn reviewed_relocation_occupant(&self, path: &Path, occupant: Option<&str>) -> Result<bool> {
        let Some(id) = occupant else {
            return Ok(true);
        };
        let conn = self.conn()?;
        let raw: Option<Vec<u8>> =
            conn.query_row("SELECT path FROM tracks WHERE id=?1", [id], |r| r.get(0))?;
        Ok(raw
            .and_then(|raw| bytes_path(&raw).ok())
            .is_some_and(|other| same_file::is_same_file(&other, path).unwrap_or(false)))
    }
    pub fn apply_relocation(
        &self,
        plan_id: &str,
        choices: &[RelocationChoice],
        runtime: &MediaRuntime,
        cancel: &CancellationToken,
        backup_path: Option<PathBuf>,
    ) -> Result<RelocationResult> {
        let plan = self.relocation_plan(plan_id)?;
        ensure!(
            plan.generation == generation(self) && crate::now() - plan.created < 86_400_000,
            "Relocation preview expired"
        );
        let mut seen = BTreeSet::new();
        let mut files = BTreeSet::new();
        let mut selected = vec![];
        let mut grouped = BTreeMap::<String, Vec<String>>::new();
        let mut affected = BTreeSet::new();
        let mut group_no_op = 0;
        for choice in choices {
            ensure!(
                seen.insert(&choice.track_id),
                "Duplicate relocation selection"
            );
            if choice.survivor == "skip" {
                continue;
            }
            ensure!(
                matches!(choice.survivor.as_str(), "source" | "existing"),
                "Invalid survivor choice"
            );
            let row = plan
                .rows
                .iter()
                .find(|r| r.track_id == choice.track_id)
                .context("Track was not reviewed")?;
            let candidate = row
                .candidates
                .iter()
                .find(|c| c.id == choice.candidate_id)
                .context("Candidate was not reviewed")?;
            let mut action = choice.survivor.as_str();
            let (row, candidate) = if !candidate.group_track_ids.is_empty() {
                let mut ids = candidate.group_track_ids.clone();
                ids.push(row.track_id.clone());
                ids.sort();
                ids.dedup();
                let keeper = choice
                    .survivor_track_id
                    .as_ref()
                    .context("Choose a survivor for the complete collision group")?;
                ensure!(ids.contains(keeper), "Group survivor was not reviewed");
                ensure!(
                    ids.iter().all(|id| affected.insert(id.clone())),
                    "Collision group was selected more than once"
                );
                let survivor_row = plan
                    .rows
                    .iter()
                    .find(|r| &r.track_id == keeper)
                    .context("Group survivor disappeared")?;
                let survivor_candidate = survivor_row
                    .candidates
                    .iter()
                    .find(|c| same_file::is_same_file(&c.path, &candidate.path).unwrap_or(false))
                    .context("Group survivor candidate disappeared")?;
                let final_keeper = if choice.survivor == "existing" {
                    candidate
                        .occupied_by
                        .as_ref()
                        .context("No existing group destination entry")?
                } else {
                    keeper
                };
                if final_keeper == keeper && action == "existing" {
                    action = "source";
                }
                if final_keeper != keeper && ids.contains(final_keeper) {
                    group_no_op += 1;
                }
                grouped.insert(
                    final_keeper.clone(),
                    ids.into_iter()
                        .filter(|id| id != keeper && id != final_keeper)
                        .collect(),
                );
                (survivor_row, survivor_candidate)
            } else {
                ensure!(
                    choice.survivor_track_id.is_none(),
                    "Unexpected group survivor"
                );
                ensure!(
                    affected.insert(row.track_id.clone()),
                    "Entry was selected more than once"
                );
                (row, candidate)
            };
            ensure!(
                files.insert(candidate.proof.path.clone()),
                "Several source entries claim one destination; review a single survivor"
            );
            let observed = proof(&candidate.path, &bytes_path(&candidate.proof.root)?, cancel)?;
            ensure!(
                observed == candidate.proof,
                "Candidate changed; preview again"
            );
            let technical = runtime
                .probe_cancellable(&candidate.path, cancel)?
                .columns();
            ensure!(
                self.reviewed_relocation_occupant(
                    &candidate.path,
                    candidate.occupied_by.as_deref()
                )?,
                "Destination occupancy changed; preview again"
            );
            let same_source = self.track_path(&row.track_id).ok().is_some_and(|path| {
                same_file::is_same_file(path, &candidate.path).unwrap_or(false)
            });
            selected.push((action, row, candidate, technical, same_source));
        }
        let _sync = crate::library_backup::sync_lease(self.data_dir())?;
        let _organization =
            self.inner.organization_gate.try_lock().map_err(|_| {
                anyhow::anyhow!("Organization is busy; finish it before relocating")
            })?;
        #[cfg(feature = "autotag")]
        let _tag = self
            .inner
            .tag_gate
            .try_lock()
            .map_err(|_| anyhow::anyhow!("Metadata writer is busy; retry after it finishes"))?;
        let mut c = self.conn()?;
        let tx = c.transaction()?;
        crate::library_backup::quiet_database(&tx)?;
        ensure!(stamp(&tx)? == plan.stamp, "Library changed; preview again");
        let running: i64 = tx.query_row(
            "SELECT COUNT(*) FROM jobs WHERE state IN ('queued','running')",
            [],
            |r| r.get(0),
        )?;
        ensure!(
            running == 0,
            "Preparation is busy; finish queued work before relocating"
        );
        let mut before = BTreeMap::new();
        let mut members_q=tx.prepare("SELECT playlist_id,track_id,position FROM playlist_members ORDER BY playlist_id,position")?;
        let members = members_q
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<Vec<(String, String, i64)>>>()?;
        drop(members_q);
        let mut links_q = tx
            .prepare("SELECT target_id,content_id,local_track_id,status FROM native_track_links")?;
        let links = links_q
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(links_q);
        let mut applied = 0;
        let mut no_op = group_no_op;
        let mut warnings = vec![];
        let mut new_members = members.clone();
        for (survivor, losers) in &grouped {
            for loser in losers {
                before
                    .entry(loser.clone())
                    .or_insert(snapshot_track(&tx, loser)?);
                tx.execute("UPDATE tracks SET path=NULL,archived=1,path_revision=path_revision+1,updated=?1 WHERE id=?2",params![crate::now(),loser])?;
                let surviving: BTreeSet<_> = new_members
                    .iter()
                    .filter(|(_, id, _)| id == survivor)
                    .map(|(p, _, _)| p.clone())
                    .collect();
                new_members.retain(|(p, id, _)| !(id == loser && surviving.contains(p)));
                for (_, id, _) in &mut new_members {
                    if id == loser {
                        *id = survivor.clone();
                    }
                }
                tx.execute("UPDATE native_track_links SET local_track_id=?1,status='conflict',updated=?2 WHERE local_track_id=?3",params![survivor,crate::now(),loser])?;
                applied += 1;
            }
            warnings.push("Shared destination group resolved with the reviewed complete survivor; losing entries retained for undo.".into());
        }
        for (action, row, target, technical, same_source) in selected {
            cancel.check()?;
            let current = snapshot_track(&tx, &row.track_id)?;
            if current.path.as_ref() == Some(&target.proof.path) || same_source {
                no_op += 1;
                continue;
            }
            before.entry(current.id.clone()).or_insert(current);
            let exact_occupant: Option<String> = tx
                .query_row(
                    "SELECT id FROM tracks WHERE path=?1 AND id!=?2",
                    params![target.proof.path, row.track_id],
                    |r| r.get(0),
                )
                .optional()?;
            ensure!(
                exact_occupant.is_none() || exact_occupant == target.occupied_by,
                "Destination occupancy changed; preview again"
            );
            let occupant = target.occupied_by.clone();
            if let Some(other) = &occupant {
                before
                    .entry(other.clone())
                    .or_insert(snapshot_track(&tx, other)?);
                let (survivor, loser) = if action == "existing" {
                    (other.as_str(), row.track_id.as_str())
                } else {
                    (row.track_id.as_str(), other.as_str())
                };
                tx.execute("UPDATE tracks SET path=NULL,archived=1,path_revision=path_revision+1,updated=?1 WHERE id=?2",params![crate::now(),loser])?;
                let mut surviving = BTreeSet::new();
                for (playlist, track, _) in &new_members {
                    if track == survivor {
                        surviving.insert(playlist.clone());
                    }
                }
                new_members.retain(|(p, t, _)| !(t == loser && surviving.contains(p)));
                for (_, track, _) in &mut new_members {
                    if track == loser {
                        *track = survivor.into();
                    }
                }
                tx.execute("UPDATE native_track_links SET local_track_id=?1,status='conflict',updated=?2 WHERE local_track_id=?3",params![survivor,crate::now(),loser])?;
                warnings.push("Collision survivor retained as a complete record; native links require review.".into());
                if action == "existing" {
                    applied += 1;
                    continue;
                }
            } else {
                ensure!(
                    action != "existing",
                    "There is no existing destination entry"
                );
            }
            let locator = crate::PortableLocator::from_local_path(
                format!("locator:{}", row.track_id),
                &target.path,
            );
            let metadata: String = tx.query_row(
                "SELECT metadata_json FROM tracks WHERE id=?1",
                [&row.track_id],
                |r| r.get(0),
            )?;
            let mut metadata: serde_json::Value = serde_json::from_str(&metadata)?;
            if target.replacement
                && let Some(saved) =
                    crate::musical::accepted_snapshot_from_analysis(&tx, &row.track_id)?
            {
                let selected: Option<String> = tx.query_row(
                    "SELECT selected_musical_revision FROM tracks WHERE id=?1",
                    [&row.track_id],
                    |r| r.get(0),
                )?;
                if selected.is_none() {
                    tx.execute("INSERT OR IGNORE INTO accepted_musical_revisions(id,track_id,json,created) VALUES(?1,?2,?3,?4)",params![saved.id,row.track_id,serde_json::to_string(&saved)?,crate::now()])?;
                    tx.execute(
                        "UPDATE tracks SET selected_musical_revision=?1 WHERE id=?2",
                        params![saved.id, row.track_id],
                    )?;
                }
            }
            if let Some(o) = metadata.as_object_mut() {
                if target.replacement {
                    o.insert("library.timing_needs_inspection".into(), true.into());
                }
                for (k, v) in technical {
                    if k.starts_with("audio.") || k.starts_with("format.") {
                        o.insert(k, v);
                    }
                }
            }
            tx.execute("UPDATE tracks SET path=?1,source_hash=?2,source_locator_json=?3,analysis_status=CASE WHEN ?4 THEN 'stale' ELSE analysis_status END,path_revision=path_revision+1,metadata_json=?5,updated=?6 WHERE id=?7",params![target.proof.path,target.proof.hash,serde_json::to_string(&locator)?,target.replacement,serde_json::to_string(&metadata)?,crate::now(),row.track_id])?;
            if target.replacement {
                warnings.push(
                    "Replacement connected. Saved cues are retained; check timing before playing."
                        .into(),
                );
            }
            applied += 1;
        }
        tx.execute("DELETE FROM playlist_members", [])?;
        let mut positions = BTreeMap::new();
        for (p, t, _) in new_members {
            let pos = positions.entry(p.clone()).or_insert(0i64);
            tx.execute(
                "INSERT INTO playlist_members VALUES(?1,?2,?3)",
                params![p, t, *pos],
            )?;
            *pos += 1;
        }
        tx.execute("DELETE FROM maintenance_scans WHERE kind IN ('broken','unused','duplicates','unused_removal_plan')", [])?;
        tx.execute("DELETE FROM playlist_previews", [])?;
        tx.execute("UPDATE meta SET value=value+1 WHERE key IN ('revision','smart_input_revision','playlist_revision','maintenance_revision')",[])?;
        let operation_id = uuid::Uuid::new_v4().to_string();
        let undo = Undo {
            before: before.into_values().collect(),
            members,
            links,
            stamp: stamp(&tx)?,
            undone: false,
        };
        tx.execute(
            "INSERT INTO relocation_operations VALUES(?1,?2,?3)",
            params![operation_id, serde_json::to_string(&undo)?, crate::now()],
        )?;
        cancel.check()?;
        tx.commit()?;
        Ok(RelocationResult {
            operation_id,
            applied,
            no_op,
            skipped: plan.rows.len().saturating_sub(applied + no_op),
            backup_path,
            warnings,
        })
    }
    pub fn undo_relocation(
        &self,
        id: &str,
        cancel: &CancellationToken,
    ) -> Result<RelocationResult> {
        let _sync = crate::library_backup::sync_lease(self.data_dir())?;
        let _organization =
            self.inner.organization_gate.try_lock().map_err(|_| {
                anyhow::anyhow!("Organization is busy; finish it before relocating")
            })?;
        #[cfg(feature = "autotag")]
        let _tag = self
            .inner
            .tag_gate
            .try_lock()
            .map_err(|_| anyhow::anyhow!("Metadata writer is busy"))?;
        let mut c = self.conn()?;
        init(&c)?;
        let tx = c.transaction()?;
        let json: String = tx.query_row(
            "SELECT json FROM relocation_operations WHERE id=?1",
            [id],
            |r| r.get(0),
        )?;
        let mut undo: Undo = serde_json::from_str(&json)?;
        ensure!(
            !undo.undone && stamp(&tx)? == undo.stamp,
            "Library changed after relocation; undo conflicts"
        );
        for row in &undo.before {
            tx.execute("UPDATE tracks SET path=NULL WHERE id=?1", [&row.id])?;
        }
        for row in &undo.before {
            if let Some(p) = &row.path {
                let occupied: i64 = tx.query_row(
                    "SELECT COUNT(*) FROM tracks WHERE path=?1 AND id!=?2",
                    params![p, row.id],
                    |r| r.get(0),
                )?;
                ensure!(occupied == 0, "Previous location is now occupied");
            }
            tx.execute("UPDATE tracks SET path=?1,source_hash=?2,source_locator_json=?3,archived=?4,analysis_status=?5,path_revision=?6,updated=?7,metadata_json=?8,selected_musical_revision=?10 WHERE id=?9",params![row.path,row.hash,row.locator,row.archived,row.analysis_status,row.path_revision,row.updated,row.metadata,row.id,row.selected_musical_revision])?;
        }
        tx.execute("DELETE FROM playlist_members", [])?;
        for (p, t, n) in &undo.members {
            tx.execute(
                "INSERT INTO playlist_members VALUES(?1,?2,?3)",
                params![p, t, n],
            )?;
        }
        for (target, content, local, status) in &undo.links {
            tx.execute("UPDATE native_track_links SET local_track_id=?1,status=?2 WHERE target_id=?3 AND content_id=?4",params![local,status,target,content])?;
        }
        tx.execute("UPDATE meta SET value=value+1 WHERE key IN ('revision','smart_input_revision','playlist_revision','maintenance_revision')",[])?;
        undo.undone = true;
        tx.execute(
            "UPDATE relocation_operations SET json=?1 WHERE id=?2",
            params![serde_json::to_string(&undo)?, id],
        )?;
        cancel.check()?;
        tx.commit()?;
        Ok(RelocationResult {
            operation_id: id.into(),
            applied: undo.before.len(),
            no_op: 0,
            skipped: 0,
            backup_path: None,
            warnings: vec![],
        })
    }
}

#[cfg(test)]
mod traversal_tests {
    use super::*;
    fn plan() -> RelocationPlan {
        RelocationPlan {
            id: String::new(),
            created: 0,
            revision: 0,
            rows: vec![],
            skipped: 0,
            incomplete: false,
            unsearched: vec![],
            unsearched_count: 0,
            paused: false,
            generation: String::new(),
            stamp: String::new(),
            request: RelocationRequest::default(),
            index_path: vec![],
        }
    }
    fn index() -> Connection {
        let index = Connection::open_in_memory().unwrap();
        index.execute_batch("CREATE TABLE candidates(path BLOB PRIMARY KEY,parent TEXT,name TEXT,fold TEXT,size INTEGER);CREATE TABLE unsearched_paths(path BLOB PRIMARY KEY)").unwrap();
        index
    }
    #[test]
    fn unsearched_count_is_exact_bounded_and_deduplicated() {
        let index = index();
        let mut plan = plan();
        for i in 0..2023 {
            let path = PathBuf::from(format!("/unavailable/{i}"));
            skipped_search(&index, &mut plan, &path).unwrap();
            skipped_search(&index, &mut plan, &path).unwrap();
        }
        assert_eq!(plan.unsearched_count, 2023);
        assert_eq!(plan.unsearched.len(), 2001);
        let mut saved = serde_json::to_value(&plan).unwrap();
        let restored: RelocationPlan = serde_json::from_value(saved.clone()).unwrap();
        assert_eq!(restored.unsearched_count, 2023);
        saved.as_object_mut().unwrap().remove("unsearched_count");
        assert_eq!(
            serde_json::from_value::<RelocationPlan>(saved)
                .unwrap()
                .unsearched_count,
            0
        );
    }
    #[cfg(unix)]
    #[test]
    fn mountinfo_detects_same_device_bind_mounts_and_component_boundaries() {
        let table = b"10 1 8:1 / / rw - ext4 /dev/root rw\n11 10 8:1 /outside /music/redirect rw - ext4 /dev/root rw\n12 10 8:1 / /music/with\\040space rw - ext4 /dev/root rw\n";
        let id = |path| mountinfo_id(std::io::Cursor::new(table), Path::new(path)).unwrap();
        assert_eq!(id("/music"), id("/music/normal/child.flac"));
        assert_ne!(id("/music"), id("/music/redirect/child.flac"));
        assert_eq!(id("/music/redirected/child.flac"), 10);
        assert_eq!(id("/music/with space/child.flac"), 12);
        assert!(mountinfo_id(std::io::Cursor::new(b"broken"), Path::new("/music")).is_err());
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn linux_normal_root_and_child_have_same_mount_identity() {
        let temp = tempfile::tempdir().unwrap();
        let child = temp.path().join("child");
        std::fs::create_dir(&child).unwrap();
        assert_eq!(mount_id(temp.path()).unwrap(), mount_id(&child).unwrap());
    }
    #[test]
    fn bounded_flat_directory_resumes_without_losing_entries() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        for i in 0..23 {
            std::fs::write(root.join(format!("{i}.flac")), []).unwrap();
        }
        let index = index();
        let mut plan = plan();
        let limits = SearchLimits {
            candidates: 5,
            ..Default::default()
        };
        let cancel = CancellationToken::default();
        let mut rounds = 0;
        loop {
            plan.paused = false;
            discover_candidates(
                &index,
                &root,
                &root,
                &mut plan,
                &cancel,
                &mut |_| {},
                limits,
            )
            .unwrap();
            rounds += 1;
            if !plan.paused {
                break;
            }
            assert!(rounds < 10);
        }
        assert_eq!(
            index
                .query_row("SELECT count(*) FROM candidates", [], |r| r
                    .get::<_, usize>(0))
                .unwrap(),
            23
        );
        assert!(rounds >= 5);
        assert!(plan.unsearched.is_empty());
    }
    #[test]
    fn resumed_search_rejects_replaced_root_identity() {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().canonicalize().unwrap();
        let root = base.join("search");
        std::fs::create_dir(&root).unwrap();
        for i in 0..3 {
            std::fs::write(root.join(format!("{i}.flac")), []).unwrap();
        }
        let index = index();
        let mut plan = plan();
        let limits = SearchLimits {
            candidates: 1,
            ..Default::default()
        };
        discover_candidates(
            &index,
            &root,
            &base,
            &mut plan,
            &CancellationToken::default(),
            &mut |_| {},
            limits,
        )
        .unwrap();
        assert!(plan.paused);
        std::fs::rename(&root, base.join("old-search")).unwrap();
        std::fs::create_dir(&root).unwrap();
        let result = discover_candidates(
            &index,
            &root,
            &base,
            &mut plan,
            &CancellationToken::default(),
            &mut |_| {},
            limits,
        );
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Search root changed")
        );
    }
    #[test]
    fn traversal_cancellation_is_polled_during_flat_enumeration() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let index = index();
        let mut plan = plan();
        let cancel = CancellationToken::default();
        for i in 0..5000 {
            std::fs::write(root.join(format!("{i}.flac")), []).unwrap();
        }
        let cancellation = cancel.clone();
        let started = Instant::now();
        let worker = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(5));
            cancellation.cancel();
        });
        let result = discover_candidates(
            &index,
            &root,
            &root,
            &mut plan,
            &cancel,
            &mut |_| {},
            SearchLimits::default(),
        );
        worker.join().unwrap();
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
        assert!(result.is_err());
    }
    #[cfg(unix)]
    #[test]
    fn traversal_rejects_symlink_files_and_directories() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("outside.flac"), []).unwrap();
        std::os::unix::fs::symlink(outside.path(), root.join("redirect")).unwrap();
        std::os::unix::fs::symlink(outside.path().join("outside.flac"), root.join("alias.flac"))
            .unwrap();
        let index = index();
        let mut plan = plan();
        discover_candidates(
            &index,
            &root,
            &root,
            &mut plan,
            &CancellationToken::default(),
            &mut |_| {},
            SearchLimits::default(),
        )
        .unwrap();
        assert_eq!(plan.unsearched.len(), 2);
        assert_eq!(
            index
                .query_row("SELECT count(*) FROM candidates", [], |r| r
                    .get::<_, usize>(0))
                .unwrap(),
            0
        );
    }
}
