use super::{ImportIssue, LocationEvidence};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::path::{Component, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathMapping {
    pub source_prefix: String,
    #[serde(with = "crate::path_serde")]
    pub local_root: PathBuf,
    pub root_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationResolution {
    #[serde(with = "crate::path_serde::option")]
    pub resolved: Option<PathBuf>,
    pub mapping_root_id: Option<String>,
    pub issues: Vec<ImportIssue>,
}
fn percent_decode(input: &str) -> Result<String> {
    let mut bytes = Vec::with_capacity(input.len());
    let input = input.as_bytes();
    let mut i = 0;
    while i < input.len() {
        if input[i] == b'%' {
            ensure!(i + 2 < input.len(), "Malformed file URI escape");
            let hi = (input[i + 1] as char)
                .to_digit(16)
                .context("Malformed file URI escape")?;
            let lo = (input[i + 2] as char)
                .to_digit(16)
                .context("Malformed file URI escape")?;
            bytes.push((hi * 16 + lo) as u8);
            i += 3;
        } else {
            bytes.push(input[i]);
            i += 1;
        }
    }
    ensure!(!bytes.contains(&0), "NUL in source path");
    String::from_utf8(bytes)
        .context("File URI contains non-UTF8 path bytes; retain raw path evidence")
}
pub(super) fn normalized(input: &str, windows: bool) -> Result<Option<String>> {
    ensure!(!input.contains('\0'), "NUL in source path");
    let path = if let Some(uri) = input.strip_prefix("file://") {
        ensure!(
            !uri.contains(['?', '#']),
            "File URI query/fragment is not a media path"
        );
        let decoded = percent_decode(uri)?;
        if let Some(path) = decoded.strip_prefix("localhost/") {
            format!("/{path}")
        } else if decoded.starts_with('/') {
            decoded
        } else if windows {
            format!("//{decoded}")
        } else {
            bail!("Remote file URI requires an explicit volume mapping");
        }
    } else if input.contains("://") {
        return Ok(None);
    } else {
        input.to_owned()
    };
    let mut path = if windows {
        path.replace('\\', "/")
    } else {
        path
    };
    if windows && path.as_bytes().first() == Some(&b'/') && path.as_bytes().get(2) == Some(&b':') {
        path.remove(0);
    }
    ensure!(
        !path.split('/').any(|p| p == ".." || p == "."),
        "Source path traversal requires explicit resolution"
    );
    Ok(Some(path.trim_end_matches('/').to_owned()))
}
fn comparable(input: &str, case_sensitive: bool) -> String {
    // Preserve Unicode. Windows ASCII case folding is conservative: unfamiliar case
    // equivalences remain unresolved instead of merging two possible files.
    if case_sensitive {
        input.to_owned()
    } else {
        input.to_ascii_lowercase()
    }
}
pub fn resolve_location(
    evidence: &LocationEvidence,
    mappings: &[PathMapping],
) -> Result<LocationResolution> {
    let mut result = LocationResolution {
        resolved: None,
        mapping_root_id: None,
        issues: Vec::new(),
    };
    if evidence.platform.as_deref() == Some(std::env::consts::OS)
        && let Some(path) = &evidence.native_path
    {
        ensure!(
            path.is_absolute() && !path.components().any(|c| matches!(c, Component::ParentDir)),
            "Invalid native source path"
        );
        if mappings.is_empty() {
            result.resolved = Some(path.clone());
            return Ok(result);
        }
    }
    let Some(original) = &evidence.original else {
        return Ok(result);
    };
    let windows = evidence.platform.as_deref() == Some("windows");
    let case_sensitive = evidence.case_sensitive.unwrap_or(!windows);
    let Some(path) = normalized(original, windows)? else {
        result.issues.push(ImportIssue {
            code: "nonlocal_media_reference".into(),
            field: Some("location".into()),
            ..Default::default()
        });
        return Ok(result);
    };
    let comparison = comparable(&path, case_sensitive);
    let mut candidates = Vec::new();
    for mapping in mappings {
        ensure!(
            mapping.local_root.is_absolute() && !mapping.root_id.is_empty(),
            "Mapping requires a validated absolute local root"
        );
        let prefix =
            normalized(&mapping.source_prefix, windows)?.context("Nonlocal mapping prefix")?;
        let prefix_comparison = comparable(&prefix, case_sensitive);
        if comparison == prefix_comparison
            || comparison
                .strip_prefix(&prefix_comparison)
                .is_some_and(|s| s.starts_with('/'))
        {
            candidates.push((prefix.len(), mapping));
        }
    }
    candidates.sort_by_key(|(length, _)| std::cmp::Reverse(*length));
    if let Some((length, mapping)) = candidates.first() {
        ensure!(
            candidates.get(1).is_none_or(|other| other.0 != *length),
            "Ambiguous source path mappings"
        );
        let suffix = path
            .get(*length..)
            .context("Path prefix boundary is not valid UTF-8")?
            .trim_start_matches('/');
        let mut resolved = mapping.local_root.clone();
        for part in suffix.split('/').filter(|s| !s.is_empty()) {
            ensure!(
                !part.contains(['\\', ':']),
                "Mapped path component is not portable"
            );
            resolved.push(part);
        }
        ensure!(
            resolved
                .components()
                .all(|c| !matches!(c, Component::ParentDir)),
            "Mapped path escapes local root"
        );
        if resolved.exists() {
            let root = mapping.local_root.canonicalize()?;
            let actual = resolved.canonicalize()?;
            ensure!(
                actual.starts_with(root),
                "Mapped symlink escapes selected root"
            );
            result.resolved = Some(actual);
        } else {
            result.resolved = Some(resolved);
        }
        result.mapping_root_id = Some(mapping.root_id.clone());
    } else {
        let host_windows = cfg!(windows);
        if windows == host_windows
            && evidence
                .platform
                .as_deref()
                .is_none_or(|platform| platform == std::env::consts::OS)
        {
            let path = PathBuf::from(&path);
            if path.is_absolute() {
                result.resolved = Some(path);
            }
        }
        if result.resolved.is_none() {
            result.issues.push(ImportIssue {
                code: "path_mapping_required".into(),
                field: Some("location".into()),
                ..Default::default()
            });
        }
    }
    Ok(result)
}
