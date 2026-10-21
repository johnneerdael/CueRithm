//! Complete immutable previews. No file-system mutation or provider access.
use crate::{
    paths::{NativePath, PathRules},
    template::{MissingPolicy, NamingContext},
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSnapshot {
    pub path: NativePath,
    pub identity: String,
    pub sha256: String,
    pub byte_len: u64,
    pub path_revision: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootSnapshot {
    pub path: NativePath,
    pub identity: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceSnapshot {
    pub id: String,
    pub revision: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sidecar {
    pub source: FileSnapshot,
    pub destination_name: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackInput {
    pub id: String,
    pub source: FileSnapshot,
    pub metadata_revision: u64,
    pub metadata: NamingContext,
    pub sidecars: Vec<Sidecar>,
    pub references: Vec<ReferenceSnapshot>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrganizationMode {
    LeaveInPlace,
    RenameInPlace,
    Move,
    Copy,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanConfig {
    pub mode: OrganizationMode,
    pub pattern: String,
    pub missing: MissingPolicy,
    pub organization_optional: bool,
    pub rules: PathRules,
    pub managed_root: Option<RootSnapshot>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanInput {
    pub config: PlanConfig,
    pub tracks: Vec<TrackInput>,
    pub existing: Vec<FileSnapshot>,
    pub non_files: Vec<NonFileEntry>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NonFileKind {
    Directory,
    Symlink,
    Other,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NonFileEntry {
    pub path: NativePath,
    pub kind: NonFileKind,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RowOutcome {
    Ready,
    Unchanged,
    IdenticalCopy,
    Skipped { reason: String },
    Blocked { reason: String },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedFile {
    pub source: FileSnapshot,
    pub destination: NativePath,
    pub existing_destination: Option<FileSnapshot>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanRow {
    pub track: TrackInput,
    pub files: Vec<PlannedFile>,
    pub outcome: RowOutcome,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletedPlan {
    pub config: PlanConfig,
    pub digest: String,
    pub rows: Vec<PlanRow>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanError {
    Cancelled,
    Invalid(String),
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlanTotals {
    pub total: usize,
    pub ready: usize,
    pub blocked: usize,
    pub skipped: usize,
    pub unchanged: usize,
    pub identical_copies: usize,
}
impl CompletedPlan {
    pub fn totals(&self) -> PlanTotals {
        let mut totals = PlanTotals {
            total: self.rows.len(),
            ..Default::default()
        };
        for row in &self.rows {
            match row.outcome {
                RowOutcome::Ready => totals.ready += 1,
                RowOutcome::Blocked { .. } => totals.blocked += 1,
                RowOutcome::Skipped { .. } => totals.skipped += 1,
                RowOutcome::Unchanged => totals.unchanged += 1,
                RowOutcome::IdenticalCopy => totals.identical_copies += 1,
            }
        }
        totals
    }

    pub fn page(&self, offset: usize, limit: usize) -> &[PlanRow] {
        let start = offset.min(self.rows.len());
        &self.rows[start..start.saturating_add(limit.min(1000)).min(self.rows.len())]
    }
    pub fn can_apply(&self) -> bool {
        self.rows.iter().any(|r| r.outcome == RowOutcome::Ready)
    }
    pub fn matches_inputs(&self, input: &PlanInput) -> bool {
        digest(input).is_ok_and(|d| d == self.digest)
    }
}
pub fn build_plan(
    input: PlanInput,
    mut cancelled: impl FnMut() -> bool,
) -> Result<CompletedPlan, PlanError> {
    use std::collections::{BTreeMap, BTreeSet};
    if cancelled() {
        return Err(PlanError::Cancelled);
    }
    let template =
        crate::template::Template::parse(&input.config.pattern).map_err(PlanError::Invalid)?;
    if matches!(
        input.config.mode,
        OrganizationMode::Move | OrganizationMode::Copy
    ) && input.config.managed_root.is_none()
    {
        return Err(PlanError::Invalid("Managed root is required".into()));
    }
    if let Some(root) = &input.config.managed_root {
        validate_native(&root.path)?;
        if root.identity.is_empty() {
            return Err(PlanError::Invalid("Missing root identity".into()));
        }
    }
    let mut ids = BTreeSet::new();
    let mut existing: BTreeMap<String, Vec<FileSnapshot>> = BTreeMap::new();
    let mut occupied_directories = BTreeSet::new();
    for file in input
        .existing
        .iter()
        .chain(input.tracks.iter().flat_map(|track| {
            std::iter::once(&track.source).chain(track.sidecars.iter().map(|s| &s.source))
        }))
    {
        if cancelled() {
            return Err(PlanError::Cancelled);
        }
        validate_snapshot(file)?;
        for parent in file.path.0.ancestors().skip(1) {
            occupied_directories
                .insert(crate::paths::collision_key(parent).map_err(PlanError::Invalid)?);
        }
        let entries = existing
            .entry(crate::paths::collision_key(&file.path.0).map_err(PlanError::Invalid)?)
            .or_default();
        if !entries.contains(file) {
            entries.push(file.clone());
        }
    }
    let mut non_files = BTreeMap::new();
    for entry in &input.non_files {
        validate_native(&entry.path)?;
        let key = crate::paths::collision_key(&entry.path.0).map_err(PlanError::Invalid)?;
        if non_files.insert(key, entry.kind).is_some() {
            return Err(PlanError::Invalid(
                "Duplicate non-file inventory entry".into(),
            ));
        }
    }
    let fingerprint = digest(&input)?;
    let mut tracks = input.tracks;
    tracks.sort_by(|a, b| a.id.cmp(&b.id));
    let mut rows = Vec::with_capacity(tracks.len());
    let mut reservations: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for track in tracks {
        if cancelled() {
            return Err(PlanError::Cancelled);
        }
        if track.id.is_empty() || !ids.insert(track.id.clone()) {
            return Err(PlanError::Invalid("Empty or duplicate track id".into()));
        }
        validate_snapshot(&track.source)?;
        for sidecar in &track.sidecars {
            validate_snapshot(&sidecar.source)?;
        }
        let index = rows.len();
        let mut row = PlanRow {
            track,
            files: vec![],
            outcome: RowOutcome::Ready,
        };
        match files_for(&row.track, &input.config, &template) {
            Ok(Some(files)) => {
                if let Some(reason) = row_conflict(&files, input.config.mode) {
                    row.outcome = RowOutcome::Blocked { reason };
                }
                row.files = files;
            }
            Ok(None) => row.outcome = RowOutcome::Unchanged,
            Err(RowOutcome::Skipped { reason }) => row.outcome = RowOutcome::Skipped { reason },
            Err(outcome) => row.outcome = outcome,
        }
        if row.outcome == RowOutcome::Ready {
            let mut identical = 0;
            let mut unchanged = 0;
            for file in &mut row.files {
                let key =
                    crate::paths::collision_key(&file.destination.0).map_err(PlanError::Invalid)?;
                reservations.entry(key.clone()).or_default().push(index);
                if occupied_directories.contains(&key)
                    || non_files.contains_key(&key)
                    || file.destination.0.ancestors().skip(1).any(|ancestor| {
                        let key = crate::paths::collision_key(ancestor)
                            .expect("Native collision key serializes");
                        existing.contains_key(&key)
                            || non_files
                                .get(&key)
                                .is_some_and(|kind| *kind != NonFileKind::Directory)
                    })
                {
                    row.outcome = RowOutcome::Blocked {
                        reason: "Destination is occupied or has a non-directory/symlink ancestor"
                            .into(),
                    };
                    continue;
                }

                if file.destination == file.source.path {
                    unchanged += 1;
                    continue;
                }
                if let Some(matches) = existing.get(&key) {
                    if matches.len() == 1
                        && input.config.mode == OrganizationMode::Copy
                        && matches[0].path == file.destination
                        && matches[0].sha256 == file.source.sha256
                        && matches[0].byte_len == file.source.byte_len
                    {
                        file.existing_destination = Some(matches[0].clone());
                        identical += 1;
                    } else {
                        row.outcome = RowOutcome::Blocked {
                            reason: "Destination already exists or aliases an existing path".into(),
                        };
                    }
                }
            }
            if row.outcome == RowOutcome::Ready && unchanged == row.files.len() {
                row.outcome = RowOutcome::Unchanged;
            } else if row.outcome == RowOutcome::Ready && identical + unchanged == row.files.len() {
                row.outcome = RowOutcome::IdenticalCopy;
            } else if row.outcome == RowOutcome::Ready && identical > 0 {
                // The coordinator rejects an existing destination and requires the
                // audio first, so a request cannot express a partly present copy.
                row.outcome = RowOutcome::Blocked {
                    reason: "Part of this copy already exists at the destination; remove it or choose another destination".into(),
                };
            }
        }
        rows.push(row);
    }
    // A row already blocked for another reason cannot apply, so it neither claims a
    // destination nor loses its own reason. Snapshot first so the result does not
    // depend on the order the reservations are visited.
    let live: Vec<bool> = rows
        .iter()
        .map(|row| !matches!(row.outcome, RowOutcome::Blocked { .. }))
        .collect();
    for claimants in reservations.values() {
        if cancelled() {
            return Err(PlanError::Cancelled);
        }
        let claimants: Vec<usize> = claimants.iter().copied().filter(|&i| live[i]).collect();
        if claimants.len() > 1 {
            for index in claimants {
                rows[index].outcome = RowOutcome::Blocked {
                    reason: "Multiple selected files reserve the same destination".into(),
                };
            }
        }
    }
    if cancelled() {
        return Err(PlanError::Cancelled);
    }
    Ok(CompletedPlan {
        config: input.config,
        digest: fingerprint,
        rows,
    })
}
/// Why one row's own files could never become a coordinator request. The coordinator
/// accepts one audio file plus at most 255 sidecars, each source once, keeps every
/// Rename file in its folder, and refuses a destination that is another file's path.
fn row_conflict(files: &[PlannedFile], mode: OrganizationMode) -> Option<String> {
    if files.len() > 256 {
        return Some("An organization can include at most 255 owned sidecars".into());
    }
    let mut sources = std::collections::BTreeSet::new();
    if !files.iter().all(|file| sources.insert(&file.source.path)) {
        return Some("The same file is listed more than once".into());
    }
    if mode == OrganizationMode::RenameInPlace
        && files
            .iter()
            .any(|file| file.source.path.0.parent() != file.destination.0.parent())
    {
        return Some("Rename in place cannot move an owned sidecar to another folder".into());
    }
    if files.iter().enumerate().any(|(index, file)| {
        files
            .iter()
            .enumerate()
            .any(|(other, owned)| other != index && owned.source.path == file.destination)
    }) {
        return Some("A destination is another owned file's current path".into());
    }
    None
}
fn files_for(
    track: &TrackInput,
    config: &PlanConfig,
    template: &crate::template::Template,
) -> Result<Option<Vec<PlannedFile>>, RowOutcome> {
    use crate::template::Rendered;
    let blocked = |reason: String| RowOutcome::Blocked { reason };
    if config.mode == OrganizationMode::LeaveInPlace {
        return Ok(None);
    }
    if config.mode == OrganizationMode::RenameInPlace && config.pattern.contains('/') {
        return Err(blocked(
            "Rename in place cannot create subdirectories; use Move to folder".into(),
        ));
    }
    let name = match template
        .render(
            &track.metadata,
            config.missing,
            config.organization_optional,
            config.rules,
        )
        .map_err(blocked)?
    {
        Rendered::Path(path) => path,
        Rendered::Skipped { missing } => {
            return Err(RowOutcome::Skipped {
                reason: format!("Optional organization skipped: missing {missing}"),
            });
        }
    };
    let root = if config.mode == OrganizationMode::RenameInPlace {
        track
            .source
            .path
            .0
            .parent()
            .ok_or_else(|| blocked("Missing source parent".into()))?
    } else {
        &config.managed_root.as_ref().unwrap().path.0
    };
    // Pattern names a stem. Append, never replace, the actual source extension.
    let mut relative = name;
    if let Some(extension) = track.source.path.0.extension() {
        let extension = extension
            .to_str()
            .ok_or_else(|| blocked("Non-Unicode destination extension".into()))?;
        relative.push('.');
        relative.push_str(extension);
    }
    // Validate the logical '/' grammar before Windows renders native '\\'
    // separators. Native path spelling is not a naming-template input.
    crate::paths::validate_relative(&relative, config.rules).map_err(blocked)?;
    let destination = root.join(relative);
    let mut files = vec![PlannedFile {
        source: track.source.clone(),
        destination: NativePath(destination.clone()),
        existing_destination: None,
    }];
    for sidecar in &track.sidecars {
        crate::paths::validate_relative(&sidecar.destination_name, config.rules)
            .map_err(blocked)?;
        files.push(PlannedFile {
            source: sidecar.source.clone(),
            destination: NativePath(
                destination
                    .parent()
                    .unwrap()
                    .join(&sidecar.destination_name),
            ),
            existing_destination: None,
        });
    }
    Ok(Some(files))
}
fn validate_native(path: &NativePath) -> Result<(), PlanError> {
    if !path.0.is_absolute()
        || path.0.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
    {
        return Err(PlanError::Invalid(
            "Snapshot path must be absolute without traversal".into(),
        ));
    }
    Ok(())
}
fn validate_snapshot(file: &FileSnapshot) -> Result<(), PlanError> {
    validate_native(&file.path)?;
    if file.identity.is_empty()
        || file.sha256.len() != 64
        || !file.sha256.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(PlanError::Invalid(
            "Missing native identity or SHA-256 proof".into(),
        ));
    }
    Ok(())
}
fn digest(input: &PlanInput) -> Result<String, PlanError> {
    use sha2::{Digest, Sha256};
    struct HashWriter(Sha256);
    impl std::io::Write for HashWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.update(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut writer = HashWriter(Sha256::new());
    serde_json::to_writer(&mut writer, input).map_err(|e| PlanError::Invalid(e.to_string()))?;
    Ok(format!("{:x}", writer.0.finalize()))
}
