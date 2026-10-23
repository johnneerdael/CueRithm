use crate::DjSoftware;
#[cfg(not(target_os = "linux"))]
use anyhow::Context;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
#[cfg(not(target_os = "linux"))]
use std::process::Command;
#[cfg(any(target_os = "linux", test))]
mod linux_process;
#[cfg(any(windows, test))]
mod windows_registry;

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
    #[cfg(windows)]
    let registry_home = windows_registry::virtualdj_home();
    #[cfg(not(windows))]
    let registry_home = None;
    let local_app_data = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute());
    discover_with_virtualdj_home(
        home,
        music,
        documents,
        volumes,
        registry_home,
        local_app_data.as_deref(),
    )
}

fn discover_with_virtualdj_home(
    home: &Path,
    music: &Path,
    documents: &Path,
    volumes: &[PathBuf],
    registry_home: Option<PathBuf>,
    local_app_data: Option<&Path>,
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
    candidates.extend(
        crate::mixxx::database_candidates(home, local_app_data)
            .into_iter()
            .map(|path| (DjSoftware::Mixxx, path)),
    );
    if let Some(home) = registry_home
        && home.is_absolute()
        && home.join("database.xml").is_file()
    {
        candidates.insert(0, (DjSoftware::VirtualDj, home.join("database.xml")));
    }
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
        // Mixxx's library is one regular file; a same-named folder is not a library.
        let present = if software == DjSoftware::Mixxx {
            path.is_file()
        } else {
            path.exists()
        };
        if present
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
        DjSoftware::VirtualDj => {
            matches!(
                name,
                "virtualdj" | "virtualdj8" | "virtualdj64" | "virtualdj_home"
            )
        }
        DjSoftware::EngineDj => name == "engine dj",
        DjSoftware::DjayPro => matches!(name, "djay" | "djay pro" | "djayapp"),
        DjSoftware::Serato4 => matches!(name, "serato dj pro" | "serato dj"),
        DjSoftware::Mixxx => name == "mixxx",
        DjSoftware::PioneerUsb | DjSoftware::Rekordbox => {
            matches!(name, "rekordbox" | "rekordboxagent" | "rekordbox agent")
        }
    }
}
#[cfg(any(test, feature = "test-support"))]
thread_local! {
    static PROCESS_GUARD: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}
/// Tests only: answer the process check without looking at the real process list,
/// which would otherwise depend on what the developer happens to have open.
#[cfg(any(test, feature = "test-support"))]
pub fn pretend_target_running(running: bool) {
    PROCESS_GUARD.set(Some(running));
}
/// Obtain a fresh OS process snapshot; inspection failure must not imply a closed app.
pub fn ensure_closed(software: DjSoftware) -> Result<()> {
    #[cfg(any(test, feature = "test-support"))]
    if let Some(running) = PROCESS_GUARD.get() {
        ensure!(!running, "Close the target DJ software before syncing");
        return Ok(());
    }
    #[cfg(target_os = "linux")]
    {
        ensure!(
            !linux_process::is_running(software)?,
            "Close the target DJ software before syncing"
        );
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    {
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
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_registry_home_adds_a_library_and_retains_standard_fallbacks() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        let documents = root.path().join("documents");
        let standard = documents.join("VirtualDJ/database.xml");
        let selected_home = root.path().join("Selected 音楽 Library");
        for path in [&standard, &selected_home.join("database.xml")] {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"fixture").unwrap();
        }
        let targets = discover_with_virtualdj_home(
            &home,
            &home.join("Music"),
            &documents,
            &[],
            Some(selected_home.clone()),
            None,
        );
        assert_eq!(
            targets[0].path,
            selected_home.join("database.xml"),
            "The application's selected home must precede a stale default library"
        );
        assert!(targets.iter().any(|t| t.path == standard));
        let duplicate = discover_with_virtualdj_home(
            &home,
            &home.join("Music"),
            &documents,
            &[],
            Some(documents.join("VirtualDJ")),
            None,
        );
        assert_eq!(duplicate.iter().filter(|t| t.path == standard).count(), 1);
    }

    #[test]
    fn windows_registry_home_missing_invalid_or_relative_keeps_fallbacks() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        let documents = root.path().join("documents");
        let standard = documents.join("VirtualDJ/database.xml");
        std::fs::create_dir_all(standard.parent().unwrap()).unwrap();
        std::fs::write(&standard, b"fixture").unwrap();
        let not_file = root.path().join("database-is-directory");
        std::fs::create_dir_all(not_file.join("database.xml")).unwrap();
        for registry_home in [
            None,
            Some(root.path().join("missing")),
            Some(not_file),
            Some("relative".into()),
        ] {
            let targets = discover_with_virtualdj_home(
                &home,
                &home.join("Music"),
                &documents,
                &[],
                registry_home,
                None,
            );
            assert_eq!(targets.len(), 1);
            assert_eq!(targets[0].path, standard);
        }
    }

    #[test]
    fn mixxx_discovery_lists_only_existing_files_and_dedups() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        let documents = root.path().join("documents");
        let mac = home.join(
            "Library/Containers/org.mixxx.mixxx/Data/Library/Application Support/Mixxx/mixxxdb.sqlite",
        );
        let unsandboxed = home.join("Library/Application Support/Mixxx/mixxxdb.sqlite");
        let flatpak = home.join(".var/app/org.mixxx.Mixxx/.mixxx/mixxxdb.sqlite");
        let redirected = root.path().join("Redirected/Mixxx/mixxxdb.sqlite");
        for path in [&mac, &unsandboxed, &flatpak, &redirected] {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"fixture").unwrap();
        }
        // A folder named like the database is not a library.
        std::fs::create_dir_all(home.join(".mixxx/mixxxdb.sqlite")).unwrap();
        let found = |local: Option<&Path>| {
            discover_with_virtualdj_home(&home, &home.join("Music"), &documents, &[], None, local)
                .into_iter()
                .filter(|t| t.software == DjSoftware::Mixxx)
                .map(|t| t.path)
                .collect::<Vec<_>>()
        };
        // `.mixxx/mixxxdb.sqlite` is a directory, so `exists()` alone would accept it.
        let default = found(None);
        assert!(
            default.contains(&mac) && default.contains(&unsandboxed) && default.contains(&flatpak)
        );
        assert!(
            default.iter().position(|p| p == &mac) < default.iter().position(|p| p == &unsandboxed),
            "the sandbox container is the official build's library and comes first"
        );
        assert!(!default.contains(&redirected));
        let with_local = found(Some(&root.path().join("Redirected")));
        assert!(with_local.contains(&redirected));
        let mut sorted = with_local.clone();
        sorted.dedup();
        assert_eq!(sorted, with_local);
        assert!(
            !with_local.contains(&home.join(".mixxx/mixxxdb.sqlite")),
            "a directory must not be offered as a library"
        );
    }

    #[test]
    fn process_matches_accepts_mixxx_spellings() {
        for command in [
            "Mixxx",
            "/Applications/Mixxx.app/Contents/MacOS/Mixxx",
            "mixxx",
            "/app/bin/mixxx",
            "mixxx.exe",
            "MIXXX.EXE",
            r"C:\Program Files\Mixxx\mixxx.exe",
        ] {
            assert!(process_matches(DjSoftware::Mixxx, command), "{command}");
        }
        for command in ["mixxx-helper", "not-mixxx.exe", "mixxxdb.sqlite", ""] {
            assert!(!process_matches(DjSoftware::Mixxx, command), "{command}");
        }
        assert!(!process_matches(DjSoftware::VirtualDj, "mixxx"));
    }

    #[test]
    fn virtualdj_windows_process_aliases_match_exact_names() {
        for command in [
            "virtualdj8.exe",
            "VIRTUALDJ8.EXE",
            r"C:\Program Files\VirtualDJ\virtualdj8.exe",
        ] {
            assert!(process_matches(DjSoftware::VirtualDj, command), "{command}");
        }
        for command in ["virtualdj8-helper.exe", "other-virtualdj8.exe"] {
            assert!(
                !process_matches(DjSoftware::VirtualDj, command),
                "{command}"
            );
        }
    }
    #[test]
    fn djay_windows_process_aliases_match_exact_names() {
        for command in [
            "djayApp.exe",
            "DJAYAPP.EXE",
            r"C:\Program Files\djay Pro\djayApp.exe",
        ] {
            assert!(process_matches(DjSoftware::DjayPro, command), "{command}");
        }
        for command in ["djayApp-helper.exe", "other-djayApp.exe"] {
            assert!(!process_matches(DjSoftware::DjayPro, command), "{command}");
        }
    }
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
