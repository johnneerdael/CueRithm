//! Reserve analysis names across both catalogs, existing assets and this export.
use super::pdb;
use anyhow::{Context, Result};
use cuerithm_rekordbox::anlz::{AnlzFile, ParseLimits};
use rusqlite::Connection;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Component, Path},
};

pub(super) fn pioneer_directory(root: &Path) -> &'static str {
    let candidates = ["PIONEER", ".PIONEER"];
    candidates
        .into_iter()
        .find(|name| {
            let database = root.join(name).join("rekordbox");
            database.join("exportLibrary.db").is_file() || database.join("export.pdb").is_file()
        })
        .or_else(|| candidates.into_iter().find(|name| root.join(name).is_dir()))
        .unwrap_or("PIONEER")
}

pub(super) struct AnalysisPaths<'a> {
    root: &'a Path,
    pioneer: &'a str,
    reserved: HashSet<String>,
    allocated: HashSet<String>,
    owners: HashMap<String, HashSet<(u32, String)>>,
    previous: HashMap<u32, (String, String)>,
}
impl<'a> AnalysisPaths<'a> {
    pub(super) fn new(
        root: &'a Path,
        pioneer: &'a str,
        db: &Connection,
        pdb: Option<&pdb::Tables>,
    ) -> Result<Self> {
        let mut paths = Self {
            root,
            pioneer,
            reserved: HashSet::new(),
            allocated: HashSet::new(),
            owners: HashMap::new(),
            previous: HashMap::new(),
        };
        let rows = db
            .prepare("SELECT content_id,path,analysisDataFilePath FROM content")?
            .query_map([], |r| {
                Ok((
                    r.get::<_, u32>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (id, media, analysis) in rows {
            paths.reserve_owner(id, &media, &analysis);
            paths.previous.insert(id, (media, analysis));
        }
        if let Some(tables) = pdb {
            for row in tables.get(&0).into_iter().flatten() {
                let field = |index: usize| {
                    pdb::decode_string(row, pdb::read16(row, 94 + index * 2)? as usize)
                };
                paths.reserve_owner(pdb::read32(row, 72)?, &field(20)?, &field(14)?);
            }
        }
        let assets = root.join(pioneer).join("USBANLZ");
        if assets.is_dir() {
            paths.reserve_assets(&assets)?;
        }
        Ok(paths)
    }
    fn reserve_owner(&mut self, id: u32, media: &str, analysis: &str) {
        let key = name_key(analysis);
        self.reserved.insert(key.clone());
        self.owners
            .entry(key)
            .or_default()
            .insert((id, media.into()));
    }
    fn reserve_assets(&mut self, directory: &Path) -> Result<()> {
        for entry in fs::read_dir(directory).context("Read existing USB analysis directory")? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() {
                self.reserve_assets(&entry.path())?;
            } else {
                let relative = entry.path();
                let relative = relative.strip_prefix(self.root)?;
                let name = format!("/{}", relative.to_string_lossy().replace('\\', "/"));
                // Any member of an analysis triplet reserves the shared stem.
                self.reserved.insert(name_key(&name));
            }
        }
        Ok(())
    }
    pub(super) fn allocate(&mut self, id: u32, hash: &str) -> Result<String> {
        if let Some((media, analysis)) = self.previous.get(&id) {
            let key = name_key(analysis);
            let owners = &self.owners[&key];
            if !self.allocated.contains(&key)
                && owners.len() == 1
                && owners.contains(&(id, media.clone()))
                && self.safe_previous(analysis, media)?
            {
                let analysis = analysis.clone();
                self.allocated.insert(key);
                return Ok(analysis);
            }
        }
        let directory = format!(
            "/{}/USBANLZ/P{}/{}",
            self.pioneer,
            hash[..3].to_uppercase(),
            hash[..8].to_uppercase()
        );
        for number in 0..=u32::MAX {
            let path = format!("{directory}/ANLZ{number:04}.DAT");
            let key = name_key(&path);
            if self.reserved.insert(key.clone()) {
                self.allocated.insert(key);
                return Ok(path);
            }
        }
        anyhow::bail!("Pioneer USB analysis filename space exhausted")
    }
    fn safe_previous(&self, analysis: &str, media: &str) -> Result<bool> {
        let Some(base) = analysis.strip_suffix(".DAT") else {
            return Ok(false);
        };
        let relative = Path::new(analysis.trim_start_matches('/'));
        if !relative.starts_with(Path::new(self.pioneer).join("USBANLZ"))
            || !relative
                .components()
                .all(|c| matches!(c, Component::Normal(_)))
        {
            return Ok(false);
        }
        for suffix in ["DAT", "EXT", "2EX"] {
            let path = self
                .root
                .join(format!("{base}.{suffix}").trim_start_matches('/'));
            if !path.exists() {
                continue;
            }
            if !fs::symlink_metadata(&path)?.is_file() {
                return Ok(false);
            }
            let bytes = fs::read(path)?;
            let Ok(parsed) = AnlzFile::parse(&bytes, ParseLimits::default()) else {
                return Ok(false);
            };
            if !parsed
                .tags()
                .iter()
                .any(|tag| tag.fourcc() == *b"PPTH" && tag.audio_path().is_ok_and(|p| p == media))
            {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
fn name_key(path: &str) -> String {
    let normalized =
        format!("/{}", path.replace('\\', "/").trim_start_matches('/')).to_ascii_lowercase();
    match normalized.rsplit_once('.') {
        Some((base, "dat" | "ext" | "2ex")) => format!("{base}.dat"),
        _ => normalized,
    }
}
