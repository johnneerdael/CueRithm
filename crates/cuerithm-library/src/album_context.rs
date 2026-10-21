use crate::{Library, model::bytes_path};
use anyhow::Result;
use cuerithm_autotag::{AlbumHint, AutotagConfig, CatalogAlbum, ProviderId, Tagger};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, atomic::AtomicBool},
};
#[derive(Default)]
pub(crate) struct AlbumGroups {
    groups: Mutex<HashMap<(PathBuf, String), Group>>,
}
#[derive(Default)]
struct Group {
    members: HashSet<String>,
    done: HashSet<String>,
    counts: HashMap<(ProviderId, String), usize>,
    choice: Option<Arc<Choice>>,
}
pub(crate) struct Choice {
    pub provider: ProviderId,
    pub id: String,
    album: Mutex<Option<Arc<CatalogAlbum>>>,
}
impl Choice {
    pub fn hint(
        &self,
        tagger: &Tagger,
        config: &AutotagConfig,
        cancel: &Arc<AtomicBool>,
    ) -> Result<Option<AlbumHint>> {
        let mut cached = self
            .album
            .lock()
            .map_err(|_| anyhow::anyhow!("Album lookup interrupted"))?;
        if cached.is_none() {
            *cached = tagger.get_album(self.provider, &self.id, config, cancel)?;
        }
        Ok(cached.clone().map(|album| AlbumHint {
            provider: self.provider,
            album,
        }))
    }
}
impl AlbumGroups {
    pub fn prepare(&self, folder: &Path, revision: &str, members: &[String]) {
        let mut groups = self.groups.lock().unwrap();
        let key = (folder.to_owned(), revision.to_owned());
        if groups.len() > 256 {
            let old = groups
                .iter()
                .find(|(k, g)| **k != key && g.done.is_superset(&g.members))
                .map(|(k, _)| k.clone());
            if let Some(old) = old {
                groups.remove(&old);
            }
        }
        let group = groups.entry(key).or_default();
        if !group.members.is_empty() && group.done.is_superset(&group.members) {
            *group = Group::default();
        }
        group.members.extend(members.iter().cloned());
    }
    pub fn observe(
        &self,
        folder: &Path,
        revision: &str,
        track: &str,
        release: Option<(ProviderId, &str)>,
        ratio: f32,
    ) {
        let mut groups = self.groups.lock().unwrap();
        let Some(group) = groups.get_mut(&(folder.to_owned(), revision.to_owned())) else {
            return;
        };
        if !group.done.insert(track.to_owned()) || group.choice.is_some() {
            return;
        }
        if let Some((provider, release)) = release {
            let count = group
                .counts
                .entry((provider, release.to_owned()))
                .or_default();
            *count += 1;
            if *count as f32 / group.members.len().max(1) as f32 >= ratio {
                group.choice = Some(Arc::new(Choice {
                    provider,
                    id: release.to_owned(),
                    album: Mutex::new(None),
                }));
            }
        }
    }
    pub fn selected(&self, folder: &Path, revision: &str) -> Option<Arc<Choice>> {
        self.groups
            .lock()
            .unwrap()
            .get(&(folder.to_owned(), revision.to_owned()))
            .and_then(|g| g.choice.clone())
    }
}
impl Library {
    pub(crate) fn incoming_album_members(
        &self,
        folder: &Path,
        revision: &str,
    ) -> Result<Vec<String>> {
        let rows = {
            let conn = self.conn()?;
            let mut stmt=conn.prepare("SELECT t.id,t.path FROM tracks t JOIN jobs j ON j.id=t.current_job WHERE t.state='incoming' AND t.archived=0 AND j.settings_revision=?1")?;
            stmt.query_map([revision], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
        };
        let mut members = Vec::new();
        for (id, path) in rows {
            if bytes_path(&path)?.parent() == Some(folder) {
                members.push(id);
            }
        }
        Ok(members)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ratio_counts_distinct_incoming_files_and_retries_do_not_double_count() {
        let groups = AlbumGroups::default();
        let folder = std::path::Path::new("/fixture");
        let members = vec!["a".into(), "b".into(), "c".into()];
        groups.prepare(folder, "revision", &members);
        groups.observe(
            folder,
            "revision",
            "a",
            Some((ProviderId::Beatport, "release")),
            0.5,
        );
        assert!(groups.selected(folder, "revision").is_none());
        groups.observe(
            folder,
            "revision",
            "a",
            Some((ProviderId::Beatport, "release")),
            0.5,
        );
        assert!(groups.selected(folder, "revision").is_none());
        groups.observe(
            folder,
            "revision",
            "b",
            Some((ProviderId::Beatport, "release")),
            0.5,
        );
        assert_eq!(groups.selected(folder, "revision").unwrap().id, "release");
        groups.observe(folder, "revision", "c", None, 0.5);
        groups.prepare(folder, "revision", &["d".into()]);
        assert!(groups.selected(folder, "revision").is_none());
    }
    #[test]
    fn release_identity_includes_provider() {
        let groups = AlbumGroups::default();
        let folder = Path::new("/fixture");
        groups.prepare(folder, "revision", &["a".into(), "b".into(), "c".into()]);
        groups.observe(
            folder,
            "revision",
            "a",
            Some((ProviderId::Beatport, "456")),
            0.6,
        );
        groups.observe(
            folder,
            "revision",
            "b",
            Some((ProviderId::Traxsource, "456")),
            0.6,
        );
        assert!(groups.selected(folder, "revision").is_none());
        groups.observe(
            folder,
            "revision",
            "c",
            Some((ProviderId::Traxsource, "456")),
            0.6,
        );
        let selected = groups.selected(folder, "revision").unwrap();
        assert_eq!(selected.provider, ProviderId::Traxsource);
        assert_eq!(selected.id, "456");
    }
}
