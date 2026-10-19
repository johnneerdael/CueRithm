use super::{CleanupFailure, custom_tags};
use crate::{Library, model::bytes_path};
use anyhow::{Result, ensure};
use cuerithm_cleanup::{CanonicalMapping, CleanupConfig, ContributorRole, Metadata, MetadataField};
use cuerithm_media::{MediaInfo, file_sha256};
use rusqlite::params;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
#[derive(Debug, Clone, Serialize)]
pub struct CleanupCatalogValue {
    pub value: String,
    pub track_count: u64,
    pub protected: bool,
    pub mapping: Option<CanonicalMapping>,
}
#[derive(Debug, Clone, Serialize)]
pub struct CleanupArtistValue {
    pub value: String,
    pub track_count: u64,
    pub roles: BTreeMap<ContributorRole, u64>,
    pub protected: bool,
    pub mapping: Option<CanonicalMapping>,
}
#[derive(Debug, Clone, Serialize)]
pub struct CleanupCatalog {
    pub genres: Vec<CleanupCatalogValue>,
    pub artists: Vec<CleanupArtistValue>,
    pub categories: Vec<String>,
    pub unavailable_tracks: Vec<CleanupFailure>,
}
impl Library {
    pub fn track_custom_tags(&self, id: &str) -> Result<Vec<cuerithm_cleanup::CustomTag>> {
        custom_tags(&*self.conn()?, id)
    }
    pub fn cleanup_catalog(&self, config: &CleanupConfig) -> Result<CleanupCatalog> {
        config.validate()?;
        let rows = {
            let conn = self.conn()?;
            let mut q=conn.prepare("SELECT id,path,source_hash,metadata_json,native_metadata_json FROM tracks WHERE state='ready' AND archived=0 ORDER BY id")?;
            q.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Vec<u8>>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, Option<String>>(4)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
        };
        let mut genres: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut artists: BTreeMap<String, BTreeMap<ContributorRole, BTreeSet<String>>> =
            BTreeMap::new();
        let mut unavailable = vec![];
        for (id, path, source, info, native) in rows {
            let read = (|| -> Result<Metadata> {
                if let Some(native) = native {
                    let metadata: Metadata = serde_json::from_str(&native)?;
                    metadata.validate()?;
                    return Ok(metadata);
                }
                let path = bytes_path(&path)?;
                ensure!(
                    file_sha256(&path)? == source,
                    "Source changed; refresh the track"
                );
                let media: MediaInfo = serde_json::from_str(&info)?;
                let metadata = cuerithm_autotag::read_metadata(&path, &media)?;
                ensure!(
                    file_sha256(&path)? == source,
                    "Source changed while reading metadata"
                );
                let conn = self.conn()?;
                ensure!(conn.query_row("SELECT state='ready' AND archived=0 AND source_hash=?2 AND metadata_json=?3 FROM tracks WHERE id=?1",params![id,source,info],|r|r.get::<_,bool>(0))?,"Library changed while reading metadata");
                conn.execute("UPDATE tracks SET native_metadata_json=?1 WHERE id=?2 AND native_metadata_json IS NULL",params![serde_json::to_string(&metadata)?,id])?;
                Ok(metadata)
            })();
            let metadata = match read {
                Ok(metadata) => metadata,
                Err(error) => {
                    unavailable.push(CleanupFailure {
                        track_id: id,
                        message: format!("{error:#}"),
                    });
                    continue;
                }
            };
            for value in metadata
                .fields
                .get(&MetadataField::Genre)
                .into_iter()
                .flatten()
                .filter(|s| !s.trim().is_empty())
            {
                genres.entry(value.clone()).or_default().insert(id.clone());
            }
            for role in [
                ContributorRole::Artist,
                ContributorRole::Remixer,
                ContributorRole::Producer,
                ContributorRole::Composer,
                ContributorRole::Lyricist,
            ] {
                for value in metadata
                    .fields
                    .get(&role.field())
                    .into_iter()
                    .flatten()
                    .filter(|s| !s.trim().is_empty())
                {
                    artists
                        .entry(value.clone())
                        .or_default()
                        .entry(role)
                        .or_default()
                        .insert(id.clone());
                }
            }
        }
        let genres = genres
            .into_iter()
            .map(|(value, tracks)| CleanupCatalogValue {
                track_count: tracks.len() as u64,
                protected: config.genres.protected_values.contains(&value),
                mapping: config
                    .genres
                    .mappings
                    .iter()
                    .find(|m| m.from == value)
                    .cloned(),
                value,
            })
            .collect();
        let artists = artists
            .into_iter()
            .map(|(value, roles)| {
                let tracks: BTreeSet<_> = roles.values().flat_map(|ids| ids.iter()).collect();
                CleanupArtistValue {
                    track_count: tracks.len() as u64,
                    roles: roles
                        .into_iter()
                        .map(|(r, ids)| (r, ids.len() as u64))
                        .collect(),
                    protected: config.artists.protected_values.contains(&value),
                    mapping: config
                        .artists
                        .mappings
                        .iter()
                        .find(|m| m.from == value)
                        .cloned(),
                    value,
                }
            })
            .collect();
        let categories = {
            let conn = self.conn()?;
            let mut q = conn.prepare("SELECT name FROM custom_tag_categories ORDER BY name")?;
            q.query_map([], |r| r.get(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        Ok(CleanupCatalog {
            genres,
            artists,
            categories,
            unavailable_tracks: unavailable,
        })
    }
}
