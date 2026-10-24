//! Lossless source provenance and optional machine-local bindings.
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PathFlavor {
    Posix,
    WindowsDrive,
    WindowsUnc,
    OpaquePosix,
    OpaqueWindows,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PathEncoding {
    Utf8,
    Base64Bytes,
    Base64Utf16le,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EncodedPath {
    pub encoding: PathEncoding,
    pub data: String,
}
impl EncodedPath {
    /// Windows filename rules are destination constraints even when the source
    /// was a POSIX path. Preserve unrepresentable atoms unresolved, never split
    /// a source backslash into a new destination directory.
    pub fn windows_filename_compatible(&self) -> Result<bool> {
        let units = self.units()?;
        if units.is_empty()
            || units
                .iter()
                .any(|&n| n < 32 || [47, 92, 58, 60, 62, 34, 124, 63, 42].contains(&n))
            || matches!(units.last(), Some(32 | 46))
        {
            return Ok(false);
        }
        let stem: Vec<_> = units.iter().copied().take_while(|&n| n != 46).collect();
        let ascii = stem
            .iter()
            .map(|&n| char::from_u32(n).unwrap_or('\u{fffd}'))
            .collect::<String>()
            .to_ascii_uppercase();
        if ["CON", "PRN", "AUX", "NUL"].contains(&ascii.as_str())
            || ((ascii.starts_with("COM") || ascii.starts_with("LPT"))
                && ascii.len() == 4
                && ascii.as_bytes()[3].is_ascii_digit()
                && ascii.as_bytes()[3] != b'0')
        {
            return Ok(false);
        }
        Ok(true)
    }
    pub fn utf8(data: impl Into<String>) -> Self {
        Self {
            encoding: PathEncoding::Utf8,
            data: data.into(),
        }
    }
    pub fn bytes(&self) -> Result<Vec<u8>> {
        match self.encoding {
            PathEncoding::Utf8 => Ok(self.data.as_bytes().to_vec()),
            PathEncoding::Base64Bytes | PathEncoding::Base64Utf16le => {
                let b = STANDARD.decode(&self.data)?;
                ensure!(
                    self.encoding != PathEncoding::Base64Utf16le || b.len().is_multiple_of(2),
                    "Odd UTF-16LE path length"
                );
                Ok(b)
            }
        }
    }
    fn units(&self) -> Result<Vec<u32>> {
        let bytes = self.bytes()?;
        Ok(if self.encoding == PathEncoding::Base64Utf16le {
            bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|b| u16::from_le_bytes([b[0], b[1]]) as u32)
                .collect()
        } else {
            bytes.into_iter().map(u32::from).collect()
        })
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PortableLocator {
    pub id: String,
    pub flavor: PathFlavor,
    pub original: EncodedPath,
    pub source_root: Option<EncodedPath>,
    pub components: Option<Vec<EncodedPath>>,
}
/// Convert only ordinary absolute Windows paths and known canonical wrappers to
/// logical slash-separated units. Original native code units remain untouched.
pub(crate) fn windows_logical_units(units: &[u16]) -> Option<Vec<u16>> {
    let sep = |u: &u16| matches!(*u, 47 | 92);
    let drive = |u: &[u16]| {
        u.len() >= 3
            && u[0] <= 127
            && (u[0] as u8).is_ascii_alphabetic()
            && u[1] == 58
            && sep(&u[2])
    };
    let mut logical = if units.starts_with(&[92, 92, 63, 92]) {
        let tail = &units[4..];
        if drive(tail) {
            tail.to_vec()
        } else if tail.len() >= 4
            && tail[..3]
                .iter()
                .zip([85u16, 78, 67])
                .all(|(a, b)| *a == b || *a == b + 32)
            && tail[3] == 92
        {
            let mut out = vec![47, 47];
            out.extend_from_slice(&tail[4..]);
            out
        } else {
            return None;
        }
    } else {
        units.to_vec()
    };
    for u in &mut logical {
        if *u == 92 {
            *u = 47;
        }
    }
    if logical.starts_with(&[47, 47]) {
        let parts = logical[2..].split(|u| *u == 47).collect::<Vec<_>>();
        if parts.len() < 2
            || parts[0].is_empty()
            || parts[1].is_empty()
            || parts[0] == [63]
            || parts[0] == [46]
        {
            return None;
        }
    } else if !drive(&logical) {
        return None;
    }
    Some(logical)
}
impl PortableLocator {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.id.is_empty() && self.id.len() <= 256,
            "Invalid locator ID"
        );
        self.original.bytes()?;
        if let Some(root) = &self.source_root {
            let units = root.units()?;
            ensure!(!units.contains(&0), "NUL in source root");
            let separator = |n: &u32| {
                *n == 47
                    || (!matches!(self.flavor, PathFlavor::Posix | PathFlavor::OpaquePosix)
                        && *n == 92)
            };
            match self.flavor {
                PathFlavor::Posix | PathFlavor::OpaquePosix => {
                    ensure!(units.first() == Some(&47), "POSIX root must be absolute")
                }
                PathFlavor::WindowsDrive => ensure!(
                    units.len() >= 3
                        && units[0] <= 127
                        && (units[0] as u8).is_ascii_alphabetic()
                        && units[1] == 58
                        && separator(&units[2]),
                    "Windows root must be drive absolute"
                ),
                PathFlavor::OpaqueWindows => ensure!(
                    (units.len() >= 3
                        && units[0] <= 127
                        && (units[0] as u8).is_ascii_alphabetic()
                        && units[1] == 58
                        && separator(&units[2]))
                        || (units.len() >= 5
                            && separator(&units[0])
                            && separator(&units[1])
                            && units[2] != 63
                            && units[2] != 46),
                    "Opaque Windows root must be absolute"
                ),
                PathFlavor::WindowsUnc => {
                    ensure!(
                        units.len() >= 5
                            && separator(&units[0])
                            && separator(&units[1])
                            && units[2] != 63
                            && units[2] != 46,
                        "Invalid UNC/device root"
                    );
                    ensure!(
                        units[2..]
                            .split(separator)
                            .filter(|p| !p.is_empty())
                            .count()
                            >= 2,
                        "UNC root needs server and share"
                    );
                }
            }
            ensure!(
                !units.split(separator).any(|p| p == [46] || p == [46, 46]),
                "Dot component in source root"
            );
        }
        ensure!(
            self.source_root.is_some() == self.components.is_some(),
            "Root and components must both be present or absent"
        );
        if let Some(parts) = &self.components {
            ensure!(parts.len() <= 1024, "Too many locator components");
            for p in parts {
                let u = p.units()?;
                ensure!(
                    !u.is_empty()
                        && !u.contains(&0)
                        && !u.contains(&47)
                        && (matches!(self.flavor, PathFlavor::Posix | PathFlavor::OpaquePosix)
                            || !u.contains(&92)),
                    "Unsafe locator component"
                );
                ensure!(u != [46] && u != [46, 46], "Dot locator component");
                if !matches!(self.flavor, PathFlavor::Posix | PathFlavor::OpaquePosix) {
                    ensure!(!u.contains(&58), "Windows drive/device component");
                }
            }
        }
        Ok(())
    }
    pub fn from_native_path_bytes(id: impl Into<String>, native: &[u8]) -> Result<Self> {
        let id = id.into();
        let (flavor, encoding, bytes, split) = if let Some(bytes) = native.strip_prefix(b"unix:") {
            (
                if std::str::from_utf8(bytes).is_ok() {
                    PathFlavor::Posix
                } else {
                    PathFlavor::OpaquePosix
                },
                PathEncoding::Base64Bytes,
                bytes,
                bytes
                    .iter()
                    .rposition(|b| *b == b'/')
                    .filter(|_| bytes.starts_with(b"/"))
                    .map(|n| (if n == 0 { 1 } else { n }, n + 1)),
            )
        } else if let Some(bytes) = native.strip_prefix(b"windows:") {
            ensure!(
                bytes.len().is_multiple_of(2),
                "Odd existing UTF-16 path length"
            );
            let original_units: Vec<u16> = bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .collect();
            let logical = windows_logical_units(&original_units);
            let flavor = if String::from_utf16(&original_units).is_err() {
                PathFlavor::OpaqueWindows
            } else if logical.as_ref().is_some_and(|u| u.starts_with(&[47, 47])) {
                PathFlavor::WindowsUnc
            } else {
                PathFlavor::WindowsDrive
            };
            let mut locator = Self {
                id,
                flavor,
                original: EncodedPath {
                    encoding: PathEncoding::Base64Utf16le,
                    data: STANDARD.encode(bytes),
                },
                source_root: None,
                components: None,
            };
            if let Some(units) = logical
                && let Some(n) = units
                    .iter()
                    .rposition(|u| *u == 47)
                    .filter(|n| n + 1 < units.len())
            {
                let root_end = if n == 2 && units.get(1) == Some(&58) {
                    n + 1
                } else {
                    n
                };
                let encode = |u: &[u16]| EncodedPath {
                    encoding: PathEncoding::Base64Utf16le,
                    data: STANDARD
                        .encode(u.iter().flat_map(|n| n.to_le_bytes()).collect::<Vec<_>>()),
                };
                locator.source_root = Some(encode(&units[..root_end]));
                locator.components = Some(vec![encode(&units[n + 1..])]);
            }
            if locator.validate().is_err() {
                locator.source_root = None;
                locator.components = None;
            }
            locator.validate()?;
            return Ok(locator);
        } else {
            anyhow::bail!("Unknown existing native path encoding");
        };
        let original = EncodedPath {
            encoding: encoding.clone(),
            data: STANDARD.encode(bytes),
        };
        let (source_root, components) =
            if let Some((root_end, name_start)) = split.filter(|(_, name)| *name < bytes.len()) {
                (
                    Some(EncodedPath {
                        encoding: encoding.clone(),
                        data: STANDARD.encode(&bytes[..root_end]),
                    }),
                    Some(vec![EncodedPath {
                        encoding,
                        data: STANDARD.encode(&bytes[name_start..]),
                    }]),
                )
            } else {
                (None, None)
            };
        let mut locator = Self {
            id,
            flavor,
            original,
            source_root,
            components,
        };
        if locator.validate().is_err() {
            locator.source_root = None;
            locator.components = None;
        }
        locator.validate()?;
        Ok(locator)
    }
    pub fn from_local_path(id: impl Into<String>, path: &Path) -> Self {
        // Native paths are emitted by the same encoder, hence always parseable.
        Self::from_native_path_bytes(id, &crate::path_bytes(path))
            .expect("Native path encoder must produce a valid locator")
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisStatus {
    Complete,
    Imported,
    NotAnalyzed,
    Stale,
}
impl AnalysisStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Imported => "imported",
            Self::NotAnalyzed => "not_analyzed",
            Self::Stale => "stale",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortableTrack {
    pub id: String,
    pub locator: PortableLocator,
    pub source_sha256: Option<String>,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub metadata: BTreeMap<String, serde_json::Value>,
    pub key: Option<String>,
    pub bpm: Option<f64>,
    pub energy: Option<u8>,
    pub duration: Option<f64>,
    pub archived: bool,
    pub origin: String,
    pub analysis_status: AnalysisStatus,
    pub musical: Option<crate::AcceptedMusicalData>,
}
impl PortableTrack {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.id.is_empty() && self.id.len() <= 256,
            "Invalid track ID"
        );
        self.locator.validate()?;
        if let Some(hash) = &self.source_sha256 {
            ensure!(
                hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()),
                "Invalid SHA-256"
            );
        }
        ensure!(
            self.bpm.is_none_or(|n| n.is_finite() && n > 0.)
                && self.duration.is_none_or(|n| n.is_finite() && n >= 0.)
                && self.energy.is_none_or(|n| n <= 10),
            "Invalid track musical scalar"
        );
        if let Some(m) = &self.musical {
            m.validate()?;
        }
        Ok(())
    }
}
/// Rebuild only tracks to remove mandatory local identity. Preserve all indexes,
/// triggers, columns and foreign-key target names from the current schema.
pub(crate) fn migrate_identity(conn: &Connection) -> Result<()> {
    let mut rebuilds = Vec::new();
    for table in ["tracks", "custom_tag_categories", "custom_tags"] {
        let create: String = conn.query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name=?1",
            [table],
            |r| r.get(0),
        )?;
        let mut objects=conn.prepare("SELECT sql FROM sqlite_master WHERE tbl_name=?1 AND type IN ('index','trigger') AND sql IS NOT NULL ORDER BY type,name")?;
        let objects = objects
            .query_map([table], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut columns = conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let columns = columns
            .query_map([], |r| r.get::<_, String>(1))?
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .map(|s| format!("\"{s}\""))
            .collect::<Vec<_>>()
            .join(",");
        let body = create.find('(').context("Missing table definition")?;
        let replacement = format!("CREATE TABLE portable_{table}_new {}", &create[body..])
            .replace("path BLOB NOT NULL", "path BLOB")
            .replace("source_hash TEXT NOT NULL", "source_hash TEXT")
            .replace("name TEXT NOT NULL UNIQUE", "name TEXT NOT NULL")
            .replace(",UNIQUE(category_id,name)", "");
        rebuilds.push((table, replacement, columns, objects));
    }
    conn.pragma_update(None, "foreign_keys", false)?;
    conn.pragma_update(None, "legacy_alter_table", true)?;
    let result = (|| -> Result<()> {
        conn.execute_batch("BEGIN IMMEDIATE")?;
        for (table, replacement, columns, objects) in rebuilds {
            conn.execute_batch(&replacement)?;
            conn.execute_batch(&format!("INSERT INTO portable_{table}_new ({columns}) SELECT {columns} FROM {table}; DROP TABLE {table}; ALTER TABLE portable_{table}_new RENAME TO {table};"))?;
            for sql in objects {
                conn.execute_batch(&sql)?;
            }
        }
        conn.execute_batch(include_str!("../migrations/029_portable_identity.sql"))?;
        let mut statement = conn.prepare(
            "SELECT id,path FROM tracks WHERE source_locator_json IS NULL AND path IS NOT NULL",
        )?;
        let bindings = statement
            .query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(statement);
        for (id, path) in bindings {
            let locator = PortableLocator::from_native_path_bytes(format!("locator:{id}"), &path)?;
            locator.validate()?;
            conn.execute(
                "UPDATE tracks SET source_locator_json=?1 WHERE id=?2",
                params![serde_json::to_string(&locator)?, id],
            )?;
        }
        let violations: i64 =
            conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| {
                r.get(0)
            })?;
        ensure!(violations == 0, "Portable migration foreign key violation");
        conn.execute_batch("COMMIT")?;
        Ok(())
    })();
    if result.is_err() && !conn.is_autocommit() {
        conn.execute_batch("ROLLBACK")?;
    }
    conn.pragma_update(None, "legacy_alter_table", false)?;
    conn.pragma_update(None, "foreign_keys", true)?;
    result
}
impl crate::Library {
    pub fn source_locator(&self, id: &str) -> Result<Option<PortableLocator>> {
        let json: Option<String> = self.conn()?.query_row(
            "SELECT source_locator_json FROM tracks WHERE id=?1 UNION ALL SELECT NULL FROM import_items WHERE id=?1 AND local_track_id IS NULL AND state='imported' LIMIT 1",
            [id],
            |r| r.get(0),
        )?;
        json.map(|s| serde_json::from_str(&s).map_err(Into::into))
            .transpose()
    }
    pub fn source_hash_optional(&self, id: &str) -> Result<Option<String>> {
        Ok(self
            .conn()?
            .query_row(
                "SELECT source_hash FROM tracks WHERE id=?1 UNION ALL SELECT source_hash FROM import_items WHERE id=?1 AND local_track_id IS NULL AND state='imported' LIMIT 1",
                [id],
                |r| r.get(0),
            )?)
    }

    pub fn import_portable_tracks(&self, tracks: &[PortableTrack]) -> Result<()> {
        for t in tracks {
            t.validate()?;
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        for t in tracks {
            tx.execute("INSERT INTO tracks(id,path,source_hash,title,artist,album,genre,search_text,sort_title,sort_artist,state,archived,metadata_json,musical_key,bpm,energy,duration,created,updated,source_locator_json,origin,analysis_status,selected_musical_revision) VALUES(?1,NULL,?2,?3,?4,?5,?6,?7,?8,?9,'ready',?10,?11,?12,?13,?14,?15,?16,?16,?17,?18,?19,?20)",params![t.id,t.source_sha256,t.title,t.artist,t.album,t.genre,format!("{} {} {} {}",t.title,t.artist,t.album,t.genre).to_lowercase(),t.title.to_lowercase(),t.artist.to_lowercase(),t.archived,serde_json::to_string(&t.metadata)?,t.key,t.bpm,t.energy,t.duration,crate::now(),serde_json::to_string(&t.locator)?,t.origin,t.analysis_status.as_str(),t.musical.as_ref().map(|m|&m.id)])?;
            if let Some(m) = &t.musical {
                tx.execute("INSERT INTO accepted_musical_revisions(id,track_id,json,created) VALUES(?1,?2,?3,?4)",params![m.id,t.id,serde_json::to_string(m)?,crate::now()])?;
            }
        }
        tx.execute(
            "UPDATE meta SET value=value+1 WHERE key IN ('revision','smart_input_revision')",
            [],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn portable_tracks(&self) -> Result<Vec<PortableTrack>> {
        let conn = self.conn()?;
        portable_tracks_from_connection(&conn)
    }
    pub fn local_track_binding(&self, id: &str) -> Result<Option<PathBuf>> {
        let b: Option<Vec<u8>> =
            self.conn()?
                .query_row("SELECT path FROM tracks WHERE id=?1", [id], |r| r.get(0))?;
        b.as_deref().map(crate::bytes_path).transpose()
    }
    /// Bind only after caller's reviewed file proof. The canonical source locator
    /// remains immutable across machine migration.
    pub fn bind_portable_track(&self, id: &str, path: &Path, hash: Option<&str>) -> Result<()> {
        ensure!(
            path.is_absolute() && path.is_file(),
            "Binding requires an existing absolute regular file"
        );
        if let Some(h) = hash {
            ensure!(
                h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit()),
                "Invalid SHA-256"
            );
        }
        let path = path.canonicalize()?;
        let changed=self.conn()?.execute("UPDATE tracks SET path=?1,source_hash=COALESCE(?2,source_hash),path_revision=path_revision+1,updated=?3 WHERE id=?4",params![crate::path_bytes(&path),hash,crate::now(),id])?;
        ensure!(changed == 1, "Unknown track");
        Ok(())
    }
    /// Restore binds the reviewed canonical destination itself. Never replace it
    /// with a later canonicalization which could follow a swapped parent outside
    /// the approved root.
    pub fn bind_portable_track_in_root(
        &self,
        id: &str,
        path: &Path,
        root: &Path,
        hash: &str,
        cancel: &cuerithm_media::CancellationToken,
    ) -> Result<()> {
        let check = || -> Result<()> {
            ensure!(
                path.is_absolute() && root.is_absolute() && path.starts_with(root),
                "Binding escapes approved root"
            );
            ensure!(
                root.canonicalize()? == root && path.canonicalize()? == path,
                "Binding parent changed or redirects outside approved root"
            );
            for ancestor in path.ancestors() {
                let metadata = std::fs::symlink_metadata(ancestor)?;
                ensure!(
                    !metadata.file_type().is_symlink(),
                    "Binding contains a symbolic-link parent"
                );
                if ancestor == root {
                    break;
                }
            }
            ensure!(
                std::fs::symlink_metadata(path)?.is_file(),
                "Binding target must be a regular file"
            );
            Ok(())
        };
        check()?;
        let original = same_file::Handle::from_path(path)?;
        ensure!(
            cuerithm_media::file_sha256_cancellable(path, cancel)? == hash,
            "Binding file changed during verification"
        );
        cancel.check()?;
        check()?;
        ensure!(
            same_file::Handle::from_path(path)? == original,
            "Binding file identity changed"
        );
        let mut connection = self.conn()?;
        let transaction = connection.transaction()?;
        check()?;
        ensure!(
            same_file::Handle::from_path(path)? == original,
            "Binding file identity changed"
        );
        let changed=transaction.execute("UPDATE tracks SET path=?1,source_hash=?2,path_revision=path_revision+1,updated=?3 WHERE id=?4",params![crate::path_bytes(path),hash,crate::now(),id])?;
        ensure!(changed == 1, "Unknown track");
        cancel.check()?;
        transaction.commit()?;
        Ok(())
    }
}

pub(crate) fn portable_tracks_from_connection(conn: &Connection) -> Result<Vec<PortableTrack>> {
    let mut stmt=conn.prepare("SELECT id,path,source_hash,title,artist,album,genre,metadata_json,musical_key,bpm,energy,duration,archived,source_locator_json,origin,analysis_status,(SELECT json FROM accepted_musical_revisions WHERE id=tracks.selected_musical_revision AND track_id=tracks.id) FROM tracks ORDER BY id")?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<Vec<u8>>>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, String>(7)?,
                r.get::<_, Option<String>>(8)?,
                r.get::<_, Option<f64>>(9)?,
                r.get::<_, Option<u8>>(10)?,
                r.get::<_, Option<f64>>(11)?,
                r.get::<_, bool>(12)?,
                r.get::<_, Option<String>>(13)?,
                r.get::<_, String>(14)?,
                r.get::<_, String>(15)?,
                r.get::<_, Option<String>>(16)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    rows.into_iter()
        .map(
            |(
                id,
                path,
                source_sha256,
                title,
                artist,
                album,
                genre,
                metadata,
                key,
                bpm,
                energy,
                duration,
                archived,
                locator,
                origin,
                status,
                musical,
            )| {
                Ok(PortableTrack {
                    locator: match locator {
                        Some(s) => serde_json::from_str(&s)?,
                        None => PortableLocator::from_local_path(
                            format!("locator:{id}"),
                            &crate::bytes_path(&path.context("Missing source locator")?)?,
                        ),
                    },
                    id,
                    source_sha256,
                    title,
                    artist,
                    album,
                    genre,
                    metadata: serde_json::from_str(&metadata).unwrap_or_default(),
                    key,
                    bpm,
                    energy,
                    duration,
                    archived,
                    origin,
                    analysis_status: serde_json::from_value(status.into())?,
                    musical: musical.map(|s| serde_json::from_str(&s)).transpose()?,
                })
            },
        )
        .collect()
}

/// Component units used only to choose export mapping roots. Source bytes and
/// persisted locators are never rewritten by root aggregation.
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct PortableSourceParts {
    pub windows: bool,
    pub wide: bool,
    pub prefix: usize,
    pub anchors: usize,
    pub parts: Vec<Vec<u16>>,
}
impl PortableSourceParts {
    pub fn from_locator(locator: &PortableLocator) -> Result<Option<Self>> {
        Self::parse(locator, false)
    }
    pub fn from_directory_locator(locator: &PortableLocator) -> Result<Option<Self>> {
        Self::parse(locator, true)
    }
    fn parse(locator: &PortableLocator, directory: bool) -> Result<Option<Self>> {
        let windows = !matches!(locator.flavor, PathFlavor::Posix | PathFlavor::OpaquePosix);
        let bytes = locator.original.bytes()?;
        let wide = windows;
        let units = if windows {
            let raw: Vec<u16> = if locator.original.encoding == PathEncoding::Base64Utf16le {
                bytes
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|b| u16::from_le_bytes(*b))
                    .collect()
            } else if locator.original.encoding == PathEncoding::Utf8 {
                locator.original.data.encode_utf16().collect()
            } else {
                return Ok(None);
            };
            let Some(units) = windows_logical_units(&raw) else {
                return Ok(None);
            };
            units
        } else {
            if !bytes.starts_with(b"/") {
                return Ok(None);
            }
            bytes.iter().map(|b| u16::from(*b)).collect()
        };
        let prefix = if windows {
            if units.starts_with(&[47, 47]) { 2 } else { 0 }
        } else {
            1
        };
        let parts = units
            .split(|u| *u == 47)
            .filter(|p| !p.is_empty())
            .map(<[u16]>::to_vec)
            .collect::<Vec<_>>();
        if ((!directory || windows) && parts.is_empty())
            || parts
                .iter()
                .any(|p| p == &[46] || p == &[46, 46] || p.contains(&0))
        {
            return Ok(None);
        }
        let mut anchors = if windows {
            if prefix == 2 { 2 } else { 1 }
        } else if parts
            .first()
            .is_some_and(|p| p == &[86, 111, 108, 117, 109, 101, 115])
        {
            2
        } else {
            0
        };
        if directory && !windows {
            anchors = anchors.min(parts.len());
        }
        if parts.len() < anchors || (!directory && parts.len() == anchors) {
            return Ok(None);
        }
        Ok(Some(Self {
            windows,
            wide,
            prefix,
            anchors,
            parts,
        }))
    }
    pub fn key(&self, count: usize) -> Result<String> {
        if !self.windows {
            return Ok(serde_json::to_string(&(
                self.windows,
                self.prefix,
                &self.parts[..count],
            ))?);
        }
        let parts = self.parts[..count]
            .iter()
            .map(|part| {
                if self.windows {
                    part.iter()
                        .map(|unit| {
                            if (65..=90).contains(unit) {
                                unit + 32
                            } else {
                                *unit
                            }
                        })
                        .collect()
                } else {
                    part.clone()
                }
            })
            .collect::<Vec<Vec<u16>>>();
        Ok(serde_json::to_string(&(self.windows, self.prefix, parts))?)
    }
    pub fn common_prefix_len(&self, other: &Self) -> usize {
        self.parts
            .iter()
            .zip(&other.parts)
            .take_while(|(left, right)| {
                left.len() == right.len()
                    && left.iter().zip(right.iter()).all(|(a, b)| {
                        let fold = |unit: u16| {
                            if self.windows && (65..=90).contains(&unit) {
                                unit + 32
                            } else {
                                unit
                            }
                        };
                        fold(*a) == fold(*b)
                    })
            })
            .count()
    }
    pub fn encoded(&self, parts: &[Vec<u16>]) -> EncodedPath {
        let mut units = vec![47; self.prefix];
        for (index, p) in parts.iter().enumerate() {
            if index > 0 {
                units.push(47);
            }
            units.extend_from_slice(p);
        }
        if self.windows && units.last() == Some(&58) {
            units.push(47);
        }
        if self.wide {
            EncodedPath {
                encoding: PathEncoding::Base64Utf16le,
                data: STANDARD.encode(
                    units
                        .iter()
                        .flat_map(|u| u.to_le_bytes())
                        .collect::<Vec<_>>(),
                ),
            }
        } else {
            EncodedPath {
                encoding: PathEncoding::Base64Bytes,
                data: STANDARD.encode(units.iter().map(|u| *u as u8).collect::<Vec<_>>()),
            }
        }
    }
    pub fn rebase(&self, root: &Self, locator: &PortableLocator) -> Result<PortableLocator> {
        ensure!(
            self.windows == root.windows
                && self.prefix == root.prefix
                && self.parts.len() >= root.parts.len()
                && self.key(root.parts.len())? == root.key(root.parts.len())?,
            "Export root is not a source ancestor"
        );
        let encode_component = |p: &Vec<u16>| {
            if self.wide {
                EncodedPath {
                    encoding: PathEncoding::Base64Utf16le,
                    data: STANDARD
                        .encode(p.iter().flat_map(|u| u.to_le_bytes()).collect::<Vec<_>>()),
                }
            } else {
                EncodedPath {
                    encoding: PathEncoding::Base64Bytes,
                    data: STANDARD.encode(p.iter().map(|u| *u as u8).collect::<Vec<_>>()),
                }
            }
        };
        let mut output = locator.clone();
        output.source_root = Some(root.encoded(&root.parts));
        output.components = Some(
            self.parts[root.parts.len()..]
                .iter()
                .map(encode_component)
                .collect(),
        );
        output.validate()?;
        Ok(output)
    }
}
