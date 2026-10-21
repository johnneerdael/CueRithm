use crate::DjSoftware;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredTarget {
    pub software: DjSoftware,
    #[serde(with = "crate::path_serde")]
    pub path: PathBuf,
}

/// Return all found libraries, retaining multiple installations for explicit selection.
pub fn discover(
    home: &Path,
    music: &Path,
    documents: &Path,
    volumes: &[PathBuf],
) -> Vec<DiscoveredTarget> {
    let mut candidates = vec![
        (DjSoftware::VirtualDj,home.join("Library/Containers/com.atomixproductions.virtualdj/Data/Library/Application Support/VirtualDJ/database.xml")),
        (DjSoftware::VirtualDj,home.join("Library/Application Support/VirtualDJ/database.xml")),
        (DjSoftware::VirtualDj,home.join("AppData/Local/VirtualDJ/database.xml")),
        (DjSoftware::VirtualDj,documents.join("VirtualDJ/database.xml")),
        (DjSoftware::EngineDj,music.join("Engine Library/Database2/m.db")),
        (DjSoftware::DjayPro,music.join("djay/djay Media Library.djayMediaLibrary/MediaLibrary.db")),
        (DjSoftware::DjayPro,music.join("djay/djay Media Library/MediaLibrary.db")),
        (DjSoftware::Serato4,home.join("Library/Application Support/Serato/Library/root.sqlite")),
        (DjSoftware::Serato4,home.join("AppData/Local/Serato/Library/root.sqlite")),
    ];
    for volume in volumes {
        candidates.push((
            DjSoftware::EngineDj,
            volume.join("Engine Library/Database2/m.db"),
        ));
        for name in ["PIONEER", ".PIONEER"] {
            if volume.join(name).is_dir() {
                candidates.push((DjSoftware::PioneerUsb, volume.clone()));
                break;
            }
        }
    }
    let mut out = vec![];
    for (software, path) in candidates {
        if path.exists()
            && !out
                .iter()
                .any(|t: &DiscoveredTarget| t.software == software && t.path == path)
        {
            out.push(DiscoveredTarget { software, path });
        }
    }
    out
}
pub fn process_matches(software: DjSoftware, command: &str) -> bool {
    let name = command
        .trim()
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .to_lowercase();
    let name = name.strip_suffix(".exe").unwrap_or(&name);
    match software {
        DjSoftware::VirtualDj => matches!(name, "virtualdj" | "virtualdj64" | "virtualdj_home"),
        DjSoftware::EngineDj => name == "engine dj",
        DjSoftware::DjayPro => matches!(name, "djay" | "djay pro"),
        DjSoftware::Serato4 => matches!(name, "serato dj pro" | "serato dj"),
        DjSoftware::PioneerUsb | DjSoftware::Rekordbox => {
            matches!(name, "rekordbox" | "rekordboxagent" | "rekordbox agent")
        }
    }
}
/// Obtain a fresh OS process snapshot; inspection failure must not imply a closed app.
pub fn ensure_closed(software: DjSoftware) -> Result<()> {
    #[cfg(windows)]
    let output = Command::new("tasklist")
        .args(["/FO", "CSV", "/NH"])
        .output()
        .context("Cannot inspect running DJ software")?;
    #[cfg(not(windows))]
    let output = Command::new("ps")
        .args(["-A", "-o", "comm="])
        .output()
        .context("Cannot inspect running DJ software")?;
    ensure!(
        output.status.success(),
        "Cannot inspect running DJ software"
    );
    let text = String::from_utf8(output.stdout)?;
    let running = text.lines().any(|line| {
        #[cfg(windows)]
        let line = line
            .trim_start_matches('"')
            .split("\",\"")
            .next()
            .unwrap_or("");
        process_matches(software, line)
    });
    ensure!(!running, "Close the target DJ software before syncing");
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn process_names_are_exact_and_windows_paths_are_supported() {
        assert!(process_matches(
            DjSoftware::Serato4,
            r"C:\Program Files\Serato\Serato DJ Pro.exe"
        ));
        assert!(process_matches(
            DjSoftware::EngineDj,
            "/Applications/Engine DJ.app/Contents/MacOS/Engine DJ"
        ));
        assert!(!process_matches(
            DjSoftware::VirtualDj,
            "/tmp/virtualdj-converter"
        ));
        assert!(!process_matches(DjSoftware::PioneerUsb, "rekordbox-backup"));
    }
}
