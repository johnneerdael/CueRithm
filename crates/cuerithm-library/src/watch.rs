use anyhow::{Result, ensure};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant, SystemTime},
};

pub fn supported_path(path: &Path) -> bool {
    if path
        .file_name()
        .is_some_and(|n| n.to_string_lossy().starts_with('.'))
    {
        return false;
    }
    path.extension().is_some_and(|s| {
        matches!(
            s.to_string_lossy().to_ascii_lowercase().as_str(),
            "mp3"
                | "aac"
                | "m4a"
                | "alac"
                | "flac"
                | "wav"
                | "wave"
                | "aif"
                | "aiff"
                | "aifc"
                | "ogg"
                | "oga"
                | "opus"
                | "wma"
        )
    })
}
fn stamp(path: &Path) -> Result<(u64, SystemTime)> {
    let meta = path.metadata()?;
    ensure!(meta.is_file(), "Incoming path is not a file");
    Ok((meta.len(), meta.modified()?))
}
pub fn wait_for_stable(path: &Path, interval: Duration, stop: &AtomicBool) -> Result<()> {
    let mut last = stamp(path)?;
    let start = Instant::now();
    // Existing settled files do not each incur two full watch intervals.
    if SystemTime::now().duration_since(last.1).unwrap_or_default() >= interval * 2 {
        return Ok(());
    }
    let mut stable = 0;
    while stable < 2 {
        ensure!(!stop.load(Ordering::Relaxed), "Preparation is stopping");
        ensure!(
            start.elapsed() < Duration::from_secs(300),
            "File copy did not settle"
        );
        let end = Instant::now() + interval;
        while Instant::now() < end {
            ensure!(!stop.load(Ordering::Relaxed), "Preparation is stopping");
            std::thread::sleep(Duration::from_millis(10).min(interval));
        }
        let next = stamp(path)?;
        if next == last {
            stable += 1;
        } else {
            stable = 0;
            last = next;
        }
    }
    Ok(())
}

pub(crate) fn in_scope(path: &Path, folders: &[PathBuf], recursive: bool) -> bool {
    folders.iter().any(|folder| {
        if recursive {
            path.starts_with(folder)
        } else {
            path.parent() == Some(folder.as_path())
        }
    })
}

#[derive(Default)]
pub(crate) struct Discovery {
    seen: HashMap<PathBuf, (u64, SystemTime)>,
}
impl Discovery {
    pub fn scan(&mut self, folders: &[PathBuf], exclude: &Path, recursive: bool) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        for folder in folders {
            for entry in walkdir::WalkDir::new(folder)
                .max_depth(if recursive { usize::MAX } else { 1 })
                .follow_links(false)
                .into_iter()
                .filter_entry(|e| !e.path().starts_with(exclude))
            {
                let Ok(entry) = entry else {
                    continue;
                };
                let path = entry.path();
                if !entry.file_type().is_file() || !supported_path(path) {
                    continue;
                }
                if let Ok(value) = stamp(path)
                    && self.seen.get(path) != Some(&value)
                {
                    self.seen.insert(path.to_path_buf(), value);
                    paths.push(path.to_path_buf());
                }
            }
        }
        paths
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovery_and_live_events_obey_the_subfolder_setting() {
        let d = tempfile::tempdir().unwrap();
        let folder = d.path().join("incoming");
        std::fs::create_dir_all(folder.join("nested")).unwrap();
        let direct = folder.join("a.flac");
        let nested = folder.join("nested/b.flac");
        for path in [&direct, &nested] {
            std::fs::write(path, b"discovery fixture").unwrap();
        }
        let folders = vec![folder];
        let mut discovery = Discovery::default();
        assert_eq!(
            discovery.scan(&folders, &d.path().join("app"), false),
            vec![direct.clone()]
        );
        assert_eq!(
            discovery.scan(&folders, &d.path().join("app"), true),
            vec![nested.clone()]
        );
        assert!(in_scope(&direct, &folders, false));
        assert!(!in_scope(&nested, &folders, false));
        assert!(in_scope(&nested, &folders, true));
        assert!(!in_scope(&nested, &[], true));
    }
}
