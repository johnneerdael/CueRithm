//! Inspect complete Linux argv records without relying on the truncated kernel comm.
//! Never report command-line values: arguments can contain unrelated private data.
use crate::DjSoftware;
use anyhow::{Context, Result, bail, ensure};
use std::{
    fs::{self, File},
    io::{self, Read},
    path::Path,
};

#[derive(Clone, Copy)]
struct Limits {
    processes: usize,
    process_bytes: usize,
    total_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        // Bound a fresh snapshot even when a process exposes an unusually large argv.
        Self {
            processes: 32_768,
            process_bytes: 64 * 1024,
            total_bytes: 16 * 1024 * 1024,
        }
    }
}

#[cfg(target_os = "linux")]
pub(super) fn is_running(software: DjSoftware) -> Result<bool> {
    running_in_proc_with_reader(
        software,
        Path::new("/proc"),
        Limits::default(),
        read_limited,
    )
}

#[cfg(target_os = "linux")]
pub(super) fn source_is_running(provider: crate::import::SourceProvider) -> Result<bool> {
    running_in_proc_matching(
        Path::new("/proc"),
        Limits::default(),
        read_limited,
        |bytes| source_command_line_matches(provider, bytes),
    )
}
fn source_command_line_matches(
    provider: crate::import::SourceProvider,
    bytes: &[u8],
) -> Result<bool> {
    command_line_matches_by(bytes, |argument| {
        basename(argument)
            .and_then(|name| std::str::from_utf8(name).ok())
            .is_some_and(|name| super::source_process_matches(provider, name))
    })
}

fn basename(argument: &[u8]) -> Option<&[u8]> {
    let argument = match (argument.first(), argument.last()) {
        (Some(b'"'), Some(b'"')) if argument.len() >= 2 => &argument[1..argument.len() - 1],
        (Some(b'"'), _) | (_, Some(b'"')) => return None,
        _ => argument,
    };
    argument.rsplit(|byte| matches!(byte, b'/' | b'\\')).next()
}

fn named(argument: &[u8], names: &[&[u8]]) -> bool {
    basename(argument).is_some_and(|base| names.iter().any(|name| base.eq_ignore_ascii_case(name)))
}

fn app_matches(software: DjSoftware, argument: &[u8]) -> bool {
    basename(argument)
        .and_then(|name| std::str::from_utf8(name).ok())
        .is_some_and(|name| super::process_matches(software, name))
}

fn operand<'a>(arguments: &[&'a [u8]], index: usize) -> Result<&'a [u8]> {
    arguments
        .get(index)
        .copied()
        .filter(|argument| !argument.is_empty())
        .context("Incomplete Linux process executable arguments")
}

fn command_line_matches(software: DjSoftware, bytes: &[u8]) -> Result<bool> {
    command_line_matches_by(bytes, |argument| app_matches(software, argument))
}

fn command_line_matches_by(bytes: &[u8], app_matches: impl Fn(&[u8]) -> bool) -> Result<bool> {
    if bytes.is_empty() {
        return Ok(false);
    }
    ensure!(bytes.last() == Some(&0), "Incomplete Linux process record");
    let arguments: Vec<_> = bytes[..bytes.len() - 1].split(|byte| *byte == 0).collect();
    let executable = operand(&arguments, 0)?;
    if app_matches(executable) {
        return Ok(true);
    }
    if named(
        executable,
        &[b"wine", b"wine64", b"wine-preloader", b"wine64-preloader"],
    ) {
        return Ok(app_matches(operand(&arguments, 1)?));
    }
    // Only unwrap known launcher operands; unrelated arguments are never executables.
    let mode_index = if named(executable, &[b"proton"]) {
        1
    } else if named(executable, &[b"python", b"python3"])
        && arguments
            .get(1)
            .is_some_and(|script| named(script, &[b"proton"]))
    {
        2
    } else {
        return Ok(false);
    };
    if matches!(
        operand(&arguments, mode_index)?,
        b"run" | b"waitforexitandrun"
    ) {
        return Ok(app_matches(operand(&arguments, mode_index + 1)?));
    }
    Ok(false)
}

fn read_limited(path: &Path, limit: usize) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(limit.saturating_add(1) as u64)
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn running_in_proc_with_reader(
    software: DjSoftware,
    root: &Path,
    limits: Limits,
    read: impl FnMut(&Path, usize) -> io::Result<Vec<u8>>,
) -> Result<bool> {
    running_in_proc_matching(root, limits, read, |bytes| {
        command_line_matches(software, bytes)
    })
}

fn running_in_proc_matching(
    root: &Path,
    limits: Limits,
    mut read: impl FnMut(&Path, usize) -> io::Result<Vec<u8>>,
    matches: impl Fn(&[u8]) -> Result<bool>,
) -> Result<bool> {
    let mut processes = 0;
    let mut observed_records = 0;
    let mut remaining_bytes = limits.total_bytes;
    for entry in fs::read_dir(root).context("Cannot enumerate Linux processes")? {
        let entry = entry.context("Cannot enumerate a Linux process")?;
        let name = entry.file_name();
        let Some(pid) = name.to_str() else {
            continue;
        };
        if pid.is_empty() || !pid.bytes().all(|byte| byte.is_ascii_digit()) {
            continue;
        }
        ensure!(
            processes < limits.processes,
            "Linux process count limit exceeded"
        );
        processes += 1;
        let path = entry.path();
        let limit = limits.process_bytes.min(remaining_bytes);
        let bytes = match read(&path.join("cmdline"), limit) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => match fs::metadata(&path) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                _ => bail!("Cannot read an extant Linux process record"),
            },
            Err(_) => bail!("Cannot read a Linux process record"),
        };
        ensure!(bytes.len() <= limit, "Linux process byte limit exceeded");
        observed_records += 1;
        remaining_bytes -= bytes.len();
        if matches(&bytes)? {
            return Ok(true);
        }
    }
    ensure!(
        observed_records > 0,
        "Linux process snapshot has no readable processes"
    );
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[test]
    fn source_checks_preserve_explicit_launcher_and_quoted_path_rules() {
        use crate::import::SourceProvider::*;
        for (provider, argv) in [
            (Traktor, b"wine64\0C:\\Traktor.exe\0".as_slice()),
            (MixedInKey11, b"Mixed In Key 11\0".as_slice()),
            (DjStudio, b"proton\0run\0DJ.Studio.exe\0".as_slice()),
            (
                AppleMusicXml,
                b"/Applications/Music.app/Contents/MacOS/Music\0".as_slice(),
            ),
            (Mixxx, b"\"C:\\Program Files\\Mixxx.exe\"\0".as_slice()),
            (Mixxx, b"/authored/\xff/Mixxx\0".as_slice()),
        ] {
            assert!(source_command_line_matches(provider, argv).unwrap());
            assert!(!source_command_line_matches(provider, b"editor\0Traktor.exe\0").unwrap());
        }
    }

    fn generous() -> Limits {
        Limits {
            processes: 10,
            process_bytes: 1024,
            total_bytes: 4096,
        }
    }

    fn process(root: &Path, pid: &str, bytes: &[u8]) {
        let path = root.join(pid);
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("cmdline"), bytes).unwrap();
    }

    fn scan(root: &Path, limits: Limits) -> Result<bool> {
        running_in_proc_with_reader(DjSoftware::Serato4, root, limits, read_limited)
    }

    #[test]
    fn linux_cmdline_matches_exact_apps_and_explicit_launcher_operands() {
        for (software, bytes) in [
            (DjSoftware::Serato4, b"Serato DJ Pro.exe\0".as_slice()),
            (DjSoftware::Rekordbox, b"REKORDBOXAGENT.EXE\0".as_slice()),
            (
                DjSoftware::Serato4,
                b"C:\\Program Files\\Serato\\Serato DJ Pro.exe\0--argument\0".as_slice(),
            ),
            (
                DjSoftware::Serato4,
                b"\"C:\\Program Files\\Serato\\Serato DJ Pro.exe\"\0".as_slice(),
            ),
            (
                DjSoftware::Serato4,
                b"wine64\0C:\\Program Files\\Serato\\Serato DJ Pro.exe\0".as_slice(),
            ),
            (
                DjSoftware::Serato4,
                b"/usr/bin/wine-preloader\0C:\\Serato DJ Pro.exe\0".as_slice(),
            ),
            (
                DjSoftware::Rekordbox,
                b"proton\0run\0C:\\rekordboxAgent.exe\0".as_slice(),
            ),
            (
                DjSoftware::Rekordbox,
                b"python3\0/prefix/proton\0waitforexitandrun\0C:\\rekordboxAgent.exe\0".as_slice(),
            ),
        ] {
            assert!(command_line_matches(software, bytes).unwrap());
        }
    }

    #[test]
    fn linux_cmdline_ignores_unrelated_arguments_and_partial_names() {
        for bytes in [
            b"editor\0Serato DJ Pro.exe\0".as_slice(),
            b"wine64\0unrelated.exe\0Serato DJ Pro.exe\0".as_slice(),
            b"python3\0-c\0Serato DJ Pro.exe\0".as_slice(),
            b"Serato DJ Pro.exe-helper\0".as_slice(),
            b"Serato DJ Pro.e\0".as_slice(),
            b"bash\0-c\0wine \"Serato DJ Pro.exe\"\0".as_slice(),
            b"wine64\0--help\0Serato DJ Pro.exe\0".as_slice(),
            b"proton\0getcompatpath\0Serato DJ Pro.exe\0".as_slice(),
            b"/non-utf8/\xff-unrelated\0Serato DJ Pro.exe\0".as_slice(),
            b"\"C:\\Serato DJ Pro.exe\0".as_slice(),
        ] {
            assert!(!command_line_matches(DjSoftware::Serato4, bytes).unwrap());
        }
        assert!(!command_line_matches(DjSoftware::VirtualDj, b"virtualdj8-helper.exe\0").unwrap());
        assert!(!command_line_matches(DjSoftware::Rekordbox, b"rekordboxAgent.\0").unwrap());
    }

    #[test]
    fn linux_cmdline_rejects_incomplete_records_without_exposing_arguments() {
        for bytes in [
            b"Serato DJ Pro.exe".as_slice(),
            b"wine64\0".as_slice(),
            b"proton\0run\0".as_slice(),
            b"python3\0/prefix/proton\0waitforexitandrun\0".as_slice(),
        ] {
            let error = command_line_matches(DjSoftware::Serato4, bytes)
                .unwrap_err()
                .to_string();
            assert!(!error.contains("Serato"));
            assert!(!error.contains("prefix"));
        }
        assert!(!command_line_matches(DjSoftware::Serato4, b"").unwrap());
    }

    #[test]
    fn linux_proc_reads_complete_argv_instead_of_truncated_comm() {
        let root = tempfile::tempdir().unwrap();
        process(root.path(), "123", b"C:\\Serato DJ Pro.exe\0");
        fs::write(root.path().join("123/comm"), b"Serato DJ Pro.e\n").unwrap();
        assert!(scan(root.path(), generous()).unwrap());
        process(root.path(), "123", b"C:\\rekordboxAgent.exe\0");
        fs::write(root.path().join("123/comm"), b"rekordboxAgent.\n").unwrap();
        assert!(
            running_in_proc_with_reader(
                DjSoftware::Rekordbox,
                root.path(),
                generous(),
                |path, _| fs::read(path),
            )
            .unwrap()
        );
    }

    #[test]
    fn linux_proc_skips_empty_and_vanished_processes() {
        let root = tempfile::tempdir().unwrap();
        process(root.path(), "123", b"");
        process(root.path(), "456", b"unrelated\0");
        fs::create_dir(root.path().join("not-a-pid")).unwrap();
        assert!(!scan(root.path(), generous()).unwrap());
        let raced = tempfile::tempdir().unwrap();
        process(raced.path(), "123", b"Serato DJ Pro.exe\0");
        process(raced.path(), "456", b"unrelated\0");
        assert!(
            !running_in_proc_with_reader(
                DjSoftware::Serato4,
                raced.path(),
                generous(),
                |path, _| {
                    if path.parent().unwrap().ends_with("123") {
                        fs::remove_dir_all(path.parent().unwrap())?;
                        Err(io::ErrorKind::NotFound.into())
                    } else {
                        fs::read(path)
                    }
                },
            )
            .unwrap()
        );
    }

    #[test]
    fn linux_proc_rejects_snapshots_without_any_observed_process() {
        let root = tempfile::tempdir().unwrap();
        assert!(scan(root.path(), generous()).is_err());
        fs::create_dir(root.path().join("not-a-pid")).unwrap();
        assert!(scan(root.path(), generous()).is_err());
        process(root.path(), "123", b"unrelated\0");
        assert!(
            running_in_proc_with_reader(DjSoftware::Serato4, root.path(), generous(), |path, _| {
                fs::remove_dir_all(path.parent().unwrap())?;
                Err(io::ErrorKind::NotFound.into())
            },)
            .is_err()
        );
    }

    #[test]
    fn linux_proc_errors_on_unreadable_extant_records_and_missing_snapshots() {
        let root = tempfile::tempdir().unwrap();
        process(root.path(), "123", b"Serato DJ Pro.exe\0");
        for kind in [io::ErrorKind::PermissionDenied, io::ErrorKind::NotFound] {
            assert!(
                running_in_proc_with_reader(
                    DjSoftware::Serato4,
                    root.path(),
                    generous(),
                    |_, _| Err(kind.into()),
                )
                .is_err()
            );
        }
        assert!(scan(&root.path().join("missing"), generous()).is_err());
    }

    #[test]
    fn linux_proc_enforces_process_and_byte_budgets() {
        let root = tempfile::tempdir().unwrap();
        process(root.path(), "123", b"unrelated\0");
        process(root.path(), "456", b"unrelated\0");
        for limits in [
            Limits {
                processes: 1,
                ..generous()
            },
            Limits {
                process_bytes: 3,
                ..generous()
            },
            Limits {
                total_bytes: 15,
                ..generous()
            },
        ] {
            assert!(scan(root.path(), limits).is_err());
        }
        let mut observed_limit = None;
        assert!(
            running_in_proc_with_reader(
                DjSoftware::Serato4,
                root.path(),
                Limits {
                    total_bytes: 3,
                    ..generous()
                },
                |_, limit| {
                    observed_limit = Some(limit);
                    Ok(vec![b'x'; limit + 1])
                },
            )
            .is_err()
        );
        assert_eq!(observed_limit, Some(3));
    }
}
