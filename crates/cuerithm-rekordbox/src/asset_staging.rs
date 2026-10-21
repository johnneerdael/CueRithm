//! Owned, proof-bearing asset staging. No function publishes to a native target.
use crate::anlz::{AnlzFile, AnlzTag, Beat, ExtendedCue, LegacyCue, ParseLimits};
use anyhow::{Context, Result, ensure};
use cuerithm_media::CancellationToken;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeTime(u32);
impl NativeTime {
    /// Convert already-exported playable time, after the shared DJ timing API.
    /// This does not apply codec offsets or infer source timing.
    pub fn from_seconds(seconds: f64) -> Result<Self> {
        let ms = (seconds * 1000.).floor();
        ensure!(
            seconds.is_finite() && seconds >= 0. && ms <= f64::from(u32::MAX),
            "Native clock is outside unsigned millisecond range"
        );
        Ok(Self(ms as u32))
    }
    pub fn milliseconds(self) -> u32 {
        self.0
    }
    pub fn millis(self) -> u32 {
        self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NativeCueSlot {
    Memory,
    Hot(u8),
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeCue {
    pub slot: NativeCueSlot,
    pub start: NativeTime,
    pub end: Option<NativeTime>,
    pub active_loop: bool,
    pub order_first: u16,
    pub order_last: u16,
    pub comment: String,
    pub memory_color: u8,
    pub color: [u8; 4],
    pub loop_fraction: (u16, u16),
}
#[derive(Debug, Clone)]
pub enum NativeAnlzEdit {
    /// Regenerate only the known waveform/VBR group for this sidecar. Build the
    /// waveform-only container with `native_waveform_template` first.
    Waveforms {
        template: Vec<u8>,
    },
    AudioPath {
        tag_index: usize,
        path: String,
    },
    /// Ordered ownership proofs pair with replacement entries. Existing unknown
    /// extensions survive updates; surplus owned entries are removed and surplus
    /// replacements are appended. Unowned record positions stay unchanged.
    Cues {
        tag_index: usize,
        owned_record_sha256: Vec<String>,
        replacement: Vec<NativeCue>,
    },
    Grid {
        tag_index: usize,
        beats: Vec<Beat>,
    },
    /// Only the explicitly identified Lexicon microsecond PQT2 dialect is writable.
    MicrosecondGrid {
        tag_index: usize,
        beats: Vec<Beat>,
        fractions: Vec<u16>,
        variable: bool,
        downbeat: usize,
    },
}

const DAT_WAVEFORMS: [[u8; 4]; 3] = [*b"PVBR", *b"PWAV", *b"PWV2"];
const EXT_WAVEFORMS: [[u8; 4]; 3] = [*b"PWV3", *b"PWV4", *b"PWV5"];
const TWO_EX_WAVEFORMS: [[u8; 4]; 3] = [*b"PWV6", *b"PWV7", *b"PWVC"];
fn waveform_group(extension: &str) -> Result<&'static [[u8; 4]; 3]> {
    match extension {
        "DAT" => Ok(&DAT_WAVEFORMS),
        "EXT" => Ok(&EXT_WAVEFORMS),
        "2EX" => Ok(&TWO_EX_WAVEFORMS),
        _ => anyhow::bail!("Native waveform regeneration is unavailable for this resource type"),
    }
}
/// Extract the complete known waveform group from a freshly measured generated
/// container. Path, grid, cues and unknown sections never enter this template.
pub fn native_waveform_template(generated_analysis: &[u8]) -> Result<Vec<u8>> {
    let generated = AnlzFile::parse(generated_analysis, ParseLimits::default())?;
    let mut selected = BTreeMap::new();
    let mut group = None;
    for tag in generated.tags() {
        let matching = [&DAT_WAVEFORMS, &EXT_WAVEFORMS, &TWO_EX_WAVEFORMS]
            .into_iter()
            .find(|group| group.contains(&tag.fourcc()));
        if let Some(matching) = matching {
            ensure!(
                group.is_none_or(|group| group == matching),
                "Generated container contains mixed native waveform groups"
            );
            group = Some(matching);
            tag.validate_waveform_template()?;
            ensure!(
                selected.insert(tag.fourcc(), tag.clone()).is_none(),
                "Generated container has duplicate native waveform sections"
            );
        }
    }
    let group = group.context("Generated container has no native waveform group")?;
    ensure!(
        group.iter().all(|id| selected.contains_key(id)),
        "Generated native waveform group is incomplete"
    );
    AnlzFile::new(
        group
            .iter()
            .map(|id| selected.remove(id).unwrap())
            .collect(),
    )?
    .to_bytes()
}
fn merge_waveforms(file: &mut AnlzFile, template: &[u8], extension: &str) -> Result<()> {
    let group = waveform_group(extension)?;
    let generated = AnlzFile::parse(template, ParseLimits::default())?;
    ensure!(
        generated.tags().len() == group.len(),
        "Native waveform template must contain a complete known group"
    );
    let mut replacements = BTreeMap::new();
    for tag in generated.tags() {
        ensure!(
            group.contains(&tag.fourcc()),
            "Native waveform template includes a protected or unsupported section"
        );
        tag.validate_waveform_template()?;
        ensure!(
            replacements.insert(tag.fourcc(), tag.clone()).is_none(),
            "Native waveform template contains duplicate sections"
        );
    }
    let mut replaced = BTreeSet::new();
    for index in 0..file.tags().len() {
        let current = &file.tags()[index];
        if let Some(generated) = replacements.get(&current.fourcc()) {
            ensure!(
                replaced.insert(current.fourcc()),
                "Observed native waveform section is ambiguous"
            );
            let changed = current.with_waveform(generated)?;
            file.replace(index, changed)?;
        }
    }
    for id in group {
        if !replaced.contains(id) {
            file.append(
                replacements
                    .remove(id)
                    .context("Native waveform replacement is missing")?,
            )?;
        }
    }
    Ok(())
}
#[derive(Debug, Clone)]
pub enum NativeAssetContent {
    /// Existing opaque embedding sidecar: observation and retention only.
    OpaqueExisting,
    Analysis {
        template: Option<Vec<u8>>,
        edits: Vec<NativeAnlzEdit>,
    },
    Artwork {
        jpeg: Vec<u8>,
    },
}
#[derive(Debug, Clone)]
pub struct NativeAssetEdit {
    pub owner_content_id: String,
    pub relative_path: PathBuf,
    pub expected_sha256: Option<String>,
    pub content: NativeAssetContent,
}
#[derive(Debug, Clone)]
pub struct NativeAssetPlan {
    pub source_root: PathBuf,
    pub edits: Vec<NativeAssetEdit>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeAssetProof {
    pub owner_content_id: String,
    pub relative_path: PathBuf,
    pub staged_path: Option<PathBuf>,
    pub before_sha256: Option<String>,
    pub after_sha256: String,
    pub changed: bool,
}
pub struct StagedNativeAssets {
    pub manifest: Vec<NativeAssetProof>,
    _owned: tempfile::TempDir,
}
#[derive(Debug, Clone, Copy)]
pub enum NativeAnalysisKind {
    Dat,
    Ext,
    TwoEx,
}

/// Author path, beat and cue sections only. This is a minimal known-format
/// container, not proof of acceptance by a particular rekordbox version.
/// EXT/2EX carry both legacy activity flags and extended cue details; PCP2's
/// producer byte 29 is deliberately not interpreted as an active-loop flag.
pub fn new_native_analysis(
    audio_path: &str,
    beats: &[Beat],
    cues: &[NativeCue],
    kind: NativeAnalysisKind,
) -> Result<Vec<u8>> {
    ensure!(
        !audio_path.is_empty() && !audio_path.contains('\0'),
        "Invalid native audio reference"
    );
    validate_cues(cues)?;
    let mut tags = vec![AnlzTag::path(audio_path)?, AnlzTag::grid(beats)?];
    for list_kind in [0, 1] {
        let selected: Vec<_> = cues
            .iter()
            .filter(|cue| cue.list_kind() == list_kind)
            .collect();
        tags.push(AnlzTag::legacy_cues(
            list_kind,
            &selected.iter().map(|cue| cue.legacy()).collect::<Vec<_>>(),
        )?);
        if !matches!(kind, NativeAnalysisKind::Dat) {
            tags.push(AnlzTag::extended_cues(
                list_kind,
                &selected
                    .iter()
                    .map(|cue| cue.extended())
                    .collect::<Vec<_>>(),
            )?);
        }
    }
    AnlzFile::new(tags)?.to_bytes()
}

impl NativeCue {
    fn list_kind(&self) -> u32 {
        u32::from(matches!(self.slot, NativeCueSlot::Hot(_)))
    }
    fn slot_number(&self) -> u32 {
        match self.slot {
            NativeCueSlot::Memory => 0,
            NativeCueSlot::Hot(slot) => u32::from(slot) + 1,
        }
    }
    fn legacy(&self) -> LegacyCue {
        LegacyCue {
            hot_cue: self.slot_number(),
            cue_type: if self.end.is_some() { 2 } else { 1 },
            time_ms: self.start.0,
            loop_time_ms: self.end.map_or(0, |time| time.0),
            status: if self.active_loop { 4 } else { 0 },
            order_first: self.order_first,
            order_last: self.order_last,
        }
    }
    fn extended(&self) -> ExtendedCue {
        ExtendedCue {
            hot_cue: self.slot_number(),
            cue_type: if self.end.is_some() { 2 } else { 1 },
            time_ms: self.start.0,
            loop_time_ms: self.end.map_or(0, |time| time.0),
            memory_color: self.memory_color,
            loop_fraction: self.loop_fraction,
            comment: self.comment.clone(),
            color: self.color,
        }
    }
}
fn validate_cues(cues: &[NativeCue]) -> Result<()> {
    let mut hot = BTreeSet::new();
    let mut active_memory = 0;
    for cue in cues {
        if let NativeCueSlot::Hot(slot) = cue.slot {
            ensure!(
                slot < 8 && hot.insert(slot),
                "Invalid or duplicate native hot-cue slot"
            );
        } else if cue.active_loop {
            active_memory += 1;
        }
        ensure!(
            cue.end.is_none_or(|end| end.0 > cue.start.0),
            "Native loop end must follow its start"
        );
        ensure!(
            !cue.active_loop || cue.end.is_some(),
            "Only saved loops can be active"
        );
        ensure!(
            !cue.comment.contains('\0'),
            "Native cue comment contains NUL"
        );
        let (n, d) = cue.loop_fraction;
        ensure!((n == 0) == (d == 0), "Invalid native loop fraction");
    }
    ensure!(active_memory <= 1, "Multiple active memory loops");
    Ok(())
}
fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn valid_hash(hash: &str) -> bool {
    hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit())
}

fn relative_key(path: &Path) -> Result<String> {
    let text = path
        .to_str()
        .context("Native resource path is not Unicode")?;
    ensure!(
        !text.is_empty()
            && text.len() <= 4096
            && !text.contains(['\\', ':'])
            && !text.chars().any(char::is_control),
        "Invalid native relative resource path"
    );
    ensure!(
        path.components()
            .all(|component| matches!(component, Component::Normal(_))),
        "Native resource path escapes its root"
    );
    // Path components normalize repeated separators and dot components on some
    // platforms. Reject their original spellings too, before journal creation.
    ensure!(
        text.split('/')
            .all(|part| !part.is_empty() && part != "." && part != ".."),
        "Native resource path is not normalized"
    );
    Ok(text.to_lowercase())
}
fn source_path(root: &Path, relative: &Path) -> Result<PathBuf> {
    relative_key(relative)?;
    let mut path = root.to_path_buf();
    let components: Vec<_> = relative.components().collect();
    for (index, component) in components.iter().enumerate() {
        path.push(component.as_os_str());
        match std::fs::symlink_metadata(&path) {
            Ok(meta) => {
                ensure!(
                    !meta.file_type().is_symlink(),
                    "Native resource path contains a symlink"
                );
                ensure!(
                    if index + 1 == components.len() {
                        meta.is_file()
                    } else {
                        meta.is_dir()
                    },
                    "Native resource path has an invalid component"
                );
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("Inspect native resource path"),
        }
    }
    Ok(path)
}
fn read_source(path: &Path, cancel: &CancellationToken) -> Result<Vec<u8>> {
    let file = std::fs::File::open(path)?;
    ensure!(
        file.metadata()?.len() <= ParseLimits::default().max_bytes as u64,
        "Native resource exceeds staging budget"
    );
    let mut bytes = Vec::new();
    let mut buffer = [0; 64 * 1024];
    let mut reader = file.take(ParseLimits::default().max_bytes as u64 + 1);
    loop {
        cancel.check()?;
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..count]);
        ensure!(
            bytes.len() <= ParseLimits::default().max_bytes,
            "Native resource exceeds staging budget"
        );
    }
    Ok(bytes)
}
fn edit_cues(tag: &AnlzTag, owned: &[String], replacement: &[NativeCue]) -> Result<AnlzTag> {
    ensure!(
        matches!(&tag.fourcc(), b"PCOB" | b"PCO2"),
        "Native cue edit selected a different section"
    );
    validate_cues(replacement)?;
    let kind = u32::from_be_bytes(tag.bytes()[12..16].try_into()?);
    ensure!(
        kind <= 1 && replacement.iter().all(|cue| cue.list_kind() == kind),
        "Native cue-list scope mismatch"
    );
    let mut ownership = BTreeMap::new();
    for (index, hash) in owned.iter().enumerate() {
        ensure!(
            valid_hash(hash) && ownership.insert(hash.to_lowercase(), index).is_none(),
            "Invalid or duplicate native cue ownership proof"
        );
    }
    let records = tag.cue_records()?;
    let mut found = BTreeSet::new();
    let mut merged = Vec::new();
    let legacy = tag.fourcc() == *b"PCOB";
    let mut occupied = BTreeSet::new();
    for record in records {
        let hash = sha256(record.bytes());
        if let Some(index) = ownership.get(&hash) {
            ensure!(found.insert(hash), "Ambiguous native cue ownership proof");
            if let Some(cue) = replacement.get(*index) {
                merged.push(if legacy {
                    record.with_legacy_cue(kind, &cue.legacy())?
                } else {
                    record.with_extended_cue(kind, &cue.extended())?
                });
            }
        } else {
            if kind == 1 && matches!(&record.fourcc(), b"PCPT" | b"PCP2") {
                ensure!(record.bytes().len() >= 16, "Truncated unowned cue record");
                occupied.insert(u32::from_be_bytes(record.bytes()[12..16].try_into()?));
            }
            merged.push(record);
        }
    }
    ensure!(
        found.len() == ownership.len(),
        "Native cue ownership changed since preview"
    );
    for cue in replacement {
        ensure!(
            kind == 0 || !occupied.contains(&cue.slot_number()),
            "Native hot-cue slot belongs to an unowned cue"
        );
    }
    for cue in replacement.iter().skip(owned.len()) {
        let built = if legacy {
            AnlzTag::legacy_cues(kind, &[cue.legacy()])?
        } else {
            AnlzTag::extended_cues(kind, &[cue.extended()])?
        };
        merged.extend(built.cue_records()?);
    }
    // PCP2 carries no verified active-loop field. Requests to change activity
    // must also update the legacy companion list in this same sidecar bundle.
    tag.with_cue_records(&merged)
}
fn edit_analysis(before: &[u8], edits: &[NativeAnlzEdit], extension: &str) -> Result<Vec<u8>> {
    let mut file = AnlzFile::parse(before, ParseLimits::default())?;
    let mut indices = BTreeSet::new();
    let mut waveforms = None;
    for edit in edits {
        if let NativeAnlzEdit::Waveforms { template } = edit {
            ensure!(
                waveforms.replace(template.as_slice()).is_none(),
                "Native waveform group has multiple edit owners"
            );
            continue;
        }
        let index = match edit {
            NativeAnlzEdit::Cues { tag_index, .. }
            | NativeAnlzEdit::Grid { tag_index, .. }
            | NativeAnlzEdit::MicrosecondGrid { tag_index, .. }
            | NativeAnlzEdit::AudioPath { tag_index, .. } => *tag_index,
            NativeAnlzEdit::Waveforms { .. } => unreachable!(),
        };
        ensure!(
            indices.insert(index),
            "Native section has multiple edit owners"
        );
        let tag = file
            .tags()
            .get(index)
            .context("Native section is unavailable")?;
        let replacement = match edit {
            NativeAnlzEdit::Waveforms { .. } => unreachable!(),
            NativeAnlzEdit::AudioPath { path, .. } => tag.with_audio_path(path)?,
            NativeAnlzEdit::Cues {
                owned_record_sha256,
                replacement,
                ..
            } => edit_cues(tag, owned_record_sha256, replacement)?,
            NativeAnlzEdit::Grid { beats, .. } => tag.with_beats(beats)?,
            NativeAnlzEdit::MicrosecondGrid {
                beats,
                fractions,
                variable,
                downbeat,
                ..
            } => tag.with_microsecond_grid(beats, fractions, *variable, *downbeat)?,
        };
        file.replace(index, replacement)?;
    }
    if let Some(template) = waveforms {
        merge_waveforms(&mut file, template, extension)?;
    }
    file.to_bytes()
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct CueActivityScope {
    owner: String,
    kind: u32,
    owned_count: usize,
    cues: Vec<(u32, u32, Option<u32>, bool)>,
}
fn validate_activity_companions(
    plan: &NativeAssetPlan,
    root: &Path,
    cancel: &CancellationToken,
) -> Result<()> {
    let mut legacy = BTreeSet::new();
    let mut extended = Vec::new();
    for asset in &plan.edits {
        let NativeAssetContent::Analysis { template, edits } = &asset.content else {
            continue;
        };
        if !edits
            .iter()
            .any(|edit| matches!(edit, NativeAnlzEdit::Cues { .. }))
        {
            continue;
        }
        cancel.check()?;
        let source = source_path(root, &asset.relative_path)?;
        let bytes = match template {
            Some(bytes) => bytes.clone(),
            None => read_source(&source, cancel)?,
        };
        let file = AnlzFile::parse(&bytes, ParseLimits::default())?;
        for edit in edits {
            let NativeAnlzEdit::Cues {
                tag_index,
                owned_record_sha256,
                replacement,
            } = edit
            else {
                continue;
            };
            let tag = file
                .tags()
                .get(*tag_index)
                .context("Native cue section is unavailable")?;
            ensure!(
                matches!(&tag.fourcc(), b"PCOB" | b"PCO2"),
                "Native cue edit selected a different section"
            );
            // Parse the list envelope before reading its native kind field.
            tag.cue_records()?;
            let mut cues: Vec<_> = replacement
                .iter()
                .map(|cue| {
                    (
                        cue.slot_number(),
                        cue.start.0,
                        cue.end.map(|time| time.0),
                        cue.active_loop,
                    )
                })
                .collect();
            cues.sort();
            let scope = CueActivityScope {
                owner: asset.owner_content_id.clone(),
                kind: u32::from_be_bytes(tag.bytes()[12..16].try_into()?),
                owned_count: owned_record_sha256.len(),
                cues,
            };
            if tag.fourcc() == *b"PCOB" {
                legacy.insert(scope);
            } else {
                extended.push(scope);
            }
        }
    }
    ensure!(
        extended.iter().all(|scope| legacy.contains(scope)),
        "Extended cue changes require equivalent legacy activity edits in the same asset plan"
    );
    Ok(())
}

/// Stage only inside a newly owned temporary directory. The complete manifest
/// contains no-op proofs too, so the coordinated publisher can validate all
/// before versions without rewriting unchanged files.
pub fn stage_native_assets(
    plan: &NativeAssetPlan,
    workspace: &Path,
    cancel: &CancellationToken,
) -> Result<StagedNativeAssets> {
    cancel.check()?;
    let metadata = std::fs::symlink_metadata(&plan.source_root)?;
    ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "Native asset root must be a regular directory"
    );
    let root = plan.source_root.canonicalize()?;
    ensure!(
        plan.edits.len() <= 100_000,
        "Native asset plan exceeds entry budget"
    );
    let mut planned = BTreeMap::new();
    for edit in &plan.edits {
        ensure!(
            !edit.owner_content_id.is_empty()
                && edit.owner_content_id.len() <= 128
                && !edit.owner_content_id.chars().any(char::is_control),
            "Invalid native asset owner"
        );
        let key = relative_key(&edit.relative_path)?;
        ensure!(
            planned.insert(key, edit).is_none(),
            "Native resource has multiple owners or case aliases"
        );
        ensure!(
            edit.expected_sha256
                .as_ref()
                .is_none_or(|hash| valid_hash(hash)),
            "Invalid native resource before proof"
        );
    }
    // All present DAT/EXT/2EX siblings must be captured, even when unchanged.
    for edit in &plan.edits {
        if matches!(
            &edit.content,
            NativeAssetContent::Analysis { .. } | NativeAssetContent::OpaqueExisting
        ) {
            let extension = edit
                .relative_path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_uppercase();
            ensure!(
                matches!(extension.as_str(), "DAT" | "EXT" | "2EX" | "3EX"),
                "Unsupported native analysis resource type"
            );
            for extension in ["DAT", "EXT", "2EX", "3EX"] {
                let sibling = edit.relative_path.with_extension(extension);
                let path = source_path(&root, &sibling)?;
                let key = relative_key(&sibling)?;
                let sibling_edit = planned.get(&key);
                ensure!(
                    !path.exists() || sibling_edit.is_some(),
                    "Native analysis sidecar is missing from the staging plan"
                );
                if let Some(sibling_edit) = sibling_edit {
                    let supported_content = if extension == "3EX" {
                        matches!(&sibling_edit.content, NativeAssetContent::OpaqueExisting)
                    } else {
                        matches!(&sibling_edit.content, NativeAssetContent::Analysis { .. })
                    };
                    ensure!(
                        sibling_edit.owner_content_id == edit.owner_content_id && supported_content,
                        "Native analysis sidecar ownership mismatch"
                    );
                }
            }
        }
    }
    validate_activity_companions(plan, &root, cancel)?;
    std::fs::create_dir_all(workspace)?;
    let owned = tempfile::Builder::new()
        .prefix("cuerithm-native-assets-")
        .tempdir_in(workspace)?;
    let mut manifest = Vec::new();
    for edit in &plan.edits {
        cancel.check()?;
        let path = source_path(&root, &edit.relative_path)?;
        let before = if path.exists() {
            Some(read_source(&path, cancel)?)
        } else {
            None
        };
        let before_hash = before.as_deref().map(sha256);
        ensure!(
            before_hash
                == edit
                    .expected_sha256
                    .as_ref()
                    .map(|hash| hash.to_lowercase()),
            "Native resource changed since preview"
        );
        let after = match &edit.content {
            NativeAssetContent::OpaqueExisting => {
                ensure!(
                    edit.relative_path
                        .extension()
                        .is_some_and(|value| value.eq_ignore_ascii_case("3EX")),
                    "Only observed opaque embedding sidecars can be retained"
                );
                before
                    .clone()
                    .context("Opaque embedding sidecars cannot be synthesized")?
            }
            NativeAssetContent::Analysis { template, edits } => {
                ensure!(
                    before.is_some() != template.is_some(),
                    "New analysis requires a template; existing analysis cannot replace its source template"
                );
                edit_analysis(
                    before
                        .as_deref()
                        .or(template.as_deref())
                        .context("Missing native analysis bytes")?,
                    edits,
                    &edit
                        .relative_path
                        .extension()
                        .and_then(|value| value.to_str())
                        .unwrap_or_default()
                        .to_ascii_uppercase(),
                )?
            }
            NativeAssetContent::Artwork { jpeg } => {
                let extension = edit
                    .relative_path
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                ensure!(
                    matches!(extension.as_str(), "jpg" | "jpeg")
                        && jpeg.len() >= 4
                        && jpeg.len() <= 16 * 1024 * 1024
                        && jpeg.starts_with(&[255, 216])
                        && jpeg.ends_with(&[255, 217]),
                    "Invalid native JPEG artwork resource"
                );
                jpeg.clone()
            }
        };
        let after_sha256 = sha256(&after);
        let changed = before_hash.as_ref() != Some(&after_sha256);
        let staged_path = if changed {
            let staged = owned.path().join(&edit.relative_path);
            std::fs::create_dir_all(staged.parent().context("Native resource has no parent")?)?;
            let mut output = std::fs::File::options()
                .write(true)
                .create_new(true)
                .open(&staged)?;
            for chunk in after.chunks(64 * 1024) {
                cancel.check()?;
                output.write_all(chunk)?;
            }
            output.sync_all()?;
            Some(staged)
        } else {
            None
        };
        manifest.push(NativeAssetProof {
            owner_content_id: edit.owner_content_id.clone(),
            relative_path: edit.relative_path.clone(),
            staged_path,
            before_sha256: before_hash,
            after_sha256,
            changed,
        });
    }
    cancel.check()?;
    Ok(StagedNativeAssets {
        manifest,
        _owned: owned,
    })
}
