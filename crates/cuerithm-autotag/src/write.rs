use crate::{
    AnalysisTagSource, AutotagConfig, CatalogTrack, MatchStatus, NoMatchPolicy, TagInput,
    TagMatchOutcome, asf::AsfTag,
};
use anyhow::{Context, Result, ensure};
use chrono::Datelike;
use cuerithm_media::{MediaInfo, MediaRuntime, PreparedAudio, file_sha256};
use onetagger_tag::{AudioFileFormat, CoverType, Field, Tag, TagDate, TagImpl};
use onetagger_tagger::{StylesOptions, SupportedTag};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnalysisTags {
    pub camelot_key: Option<String>,
    pub standard_key: Option<String>,
    pub bpm: Option<f64>,
    pub energy: Option<u8>,
}
#[derive(Clone)]
pub struct TagWritePlan {
    input: TagInput,
    matches: Vec<CatalogTrack>,
    config: AutotagConfig,
    analysis: AnalysisTags,
    artwork: Option<crate::image::Artwork>,
    provenance: serde_json::Value,
    cleanup: Option<cuerithm_cleanup::CleanupConfig>,
    metadata_edits: Vec<cuerithm_cleanup::FieldChange>,
    custom_tags: Vec<cuerithm_cleanup::CustomTag>,
    transfer: Option<crate::TransferPatch>,
    editor_patch: Option<crate::MetadataPatch>,
}
pub struct StagedTagWrite {
    pub path: PathBuf,
    pub backup_path: PathBuf,
    pub source_path: PathBuf,
    pub source_before_sha256: String,
    pub source_after_sha256: String,
    pub pcm_sha256: String,
    pub frames: u64,
    pub decoder_identity: String,
    pub metadata: MediaInfo,
    pub sidecars: Vec<crate::StagedSidecar>,
    pub provenance: serde_json::Value,
    pub cleanup: Option<cuerithm_cleanup::CleanupReport>,
    pub native_metadata: cuerithm_cleanup::Metadata,
    pub editor_receipt_json: Option<String>,
    workspace: PathBuf,
    private: Option<tempfile::TempDir>,
}
impl StagedTagWrite {
    pub fn workspace(&self) -> &Path {
        &self.workspace
    }
    pub fn retain_for_journal(&mut self) {
        if let Some(dir) = self.private.take() {
            let _ = dir.keep();
        }
    }
}
impl TagWritePlan {
    pub fn build(
        input: TagInput,
        outcome: &TagMatchOutcome,
        config: &AutotagConfig,
        analysis: &AnalysisTags,
    ) -> Result<Self> {
        config.validate()?;
        ensure!(
            !matches!(
                outcome.status,
                MatchStatus::Ambiguous | MatchStatus::Unavailable
            ),
            "Catalog identity or availability requires review"
        );
        ensure!(
            outcome.matches.is_empty() == (outcome.status != MatchStatus::Matched),
            "Invalid match outcome"
        );
        ensure!(
            outcome.status != MatchStatus::NoMatch
                || config.no_match == NoMatchPolicy::KeepExisting,
            "No catalog match; review required"
        );
        ensure!(
            analysis
                .bpm
                .is_none_or(|v| v.is_finite() && v > 0. && v < 1000.),
            "Invalid analyzed BPM"
        );
        ensure!(
            analysis.energy.is_none_or(|v| (1..=10).contains(&v)),
            "Invalid track energy"
        );
        if config.options.tag_enabled(SupportedTag::OtherTags) {
            let format = input.audio_info(config)?.format;
            for track in &outcome.matches {
                for (name, values) in &track.track.other {
                    let key = name.by_format(&format);
                    ensure!(
                        !crate::fields::protected_custom_target(&key),
                        "Provider custom tag targets protected native metadata: {key}"
                    );
                    ensure!(
                        !crate::fields::custom_bypasses_field_policy(&key, &format, false),
                        "Provider custom tag bypasses selected-field policy: {key}"
                    );
                    ensure!(
                        key.len() <= 256
                            && !key.chars().any(char::is_control)
                            && values.len() <= 128
                            && values.iter().all(|v| v.len() <= 65_536),
                        "Provider custom tag exceeds native field limits"
                    );
                }
            }
        }
        if config.options.tag_enabled(SupportedTag::Style)
            && config.options.styles_options == StylesOptions::CustomTag
        {
            let name = config
                .options
                .styles_custom_tag
                .as_ref()
                .context("Missing custom style tag")?;
            let format = input.audio_info(config)?.format;
            let key = name.by_format(&format);
            ensure!(
                !crate::fields::custom_bypasses_field_policy(&key, &format, true)
                    && !crate::fields::protected_custom_target(&key)
                    && key.len() <= 256
                    && !key.chars().any(char::is_control),
                "Custom style tag targets protected native metadata"
            );
        }
        let mut provenance = serde_json::to_value(outcome)?;
        provenance["evidence"] = serde_json::to_value(outcome.provenance(&input, config)?)?;
        Ok(Self {
            input,
            matches: outcome.matches.iter().map(|m| m.track.clone()).collect(),
            config: config.clone(),
            analysis: analysis.clone(),
            artwork: None,
            provenance,
            cleanup: None,
            metadata_edits: vec![],
            custom_tags: vec![],
            transfer: None,
            editor_patch: None,
        })
    }
    pub fn with_custom_tags(mut self, tags: Vec<cuerithm_cleanup::CustomTag>) -> Result<Self> {
        cuerithm_cleanup::Metadata {
            custom_tags: tags.clone(),
            ..Default::default()
        }
        .validate()?;
        self.custom_tags = tags;
        Ok(self)
    }
    pub fn with_editor_patch(mut self, patch: crate::MetadataPatch) -> Result<Self> {
        patch.validate()?;
        self.editor_patch = Some(patch);
        Ok(self)
    }
    pub fn with_transfer(mut self, patch: crate::TransferPatch) -> Result<Self> {
        patch.validate()?;
        self.provenance["transfer"] = serde_json::to_value(&patch)?;
        self.transfer = Some(patch);
        Ok(self)
    }
    pub fn with_cleanup(mut self, config: cuerithm_cleanup::CleanupConfig) -> Result<Self> {
        config.validate()?;
        self.cleanup = Some(config);
        Ok(self)
    }
    pub fn with_metadata_edits(
        mut self,
        edits: Vec<cuerithm_cleanup::FieldChange>,
    ) -> Result<Self> {
        let mut metadata = cuerithm_cleanup::Metadata::default();
        let mut fields = std::collections::BTreeSet::new();
        for edit in &edits {
            ensure!(fields.insert(edit.field), "Duplicate metadata edit");
            metadata.set(edit.field, edit.before.clone());
        }
        metadata.validate()?;
        for edit in &edits {
            metadata.set(edit.field, edit.after.clone());
        }
        metadata.validate()?;
        self.metadata_edits = edits;
        Ok(self)
    }
    fn apply_cleanup(
        &self,
        tag: &mut dyn TagImpl,
        format: &AudioFileFormat,
    ) -> Result<(
        Option<cuerithm_cleanup::CleanupReport>,
        cuerithm_cleanup::Metadata,
    )> {
        let mut expected = crate::metadata::read(tag, format)?;
        expected.custom_tags = self.custom_tags.clone();
        crate::metadata::edit(tag, format, &self.metadata_edits)?;
        for edit in &self.metadata_edits {
            expected.set(edit.field, edit.after.clone());
        }
        if let Some(config) = &self.cleanup {
            let report = cuerithm_cleanup::apply(config, &expected)?;
            crate::metadata::edit(tag, format, &report.changes)?;
            let expected = report.metadata.clone();
            Ok((Some(report), expected))
        } else {
            Ok((None, expected))
        }
    }
    pub fn with_artwork(mut self, bytes: Vec<u8>) -> Result<Self> {
        ensure!(
            self.config.options.tag_enabled(SupportedTag::AlbumArt),
            "Artwork is not selected"
        );
        self.artwork = Some(crate::image::Artwork::validate(bytes)?);
        Ok(self)
    }
    pub fn is_noop(&self) -> bool {
        self.tags_are_noop()
            && self.cleanup.is_none()
            && self.metadata_edits.is_empty()
            && self.editor_patch.as_ref().is_none_or(|p| p.is_noop())
            && self.transfer.as_ref().is_none_or(|p| p.before == p.after)
    }
    fn tags_are_noop(&self) -> bool {
        let measured = self.config.analysis_tag_source == AnalysisTagSource::Measured
            && ((self.config.options.tag_enabled(SupportedTag::Key)
                && (self.analysis.camelot_key.is_some() || self.analysis.standard_key.is_some()))
                || (self.config.options.tag_enabled(SupportedTag::BPM)
                    && self.analysis.bpm.is_some()));
        (self.matches.is_empty() || self.config.options.tags.is_empty())
            && !measured
            && (!self.config.write_energy || self.analysis.energy.is_none())
            && self.artwork.is_none()
    }
    pub fn stage(&self, runtime: &MediaRuntime, cache: &Path) -> Result<StagedTagWrite> {
        let audio = runtime.decode(&self.input.path, cache)?;
        self.stage_prepared(runtime, &audio, cache)
    }
    pub fn stage_prepared(
        &self,
        runtime: &MediaRuntime,
        audio: &PreparedAudio,
        cache: &Path,
    ) -> Result<StagedTagWrite> {
        self.stage_prepared_cancellable(
            runtime,
            audio,
            cache,
            &cuerithm_media::CancellationToken::default(),
        )
    }
    pub fn stage_prepared_cancellable(
        &self,
        runtime: &MediaRuntime,
        audio: &PreparedAudio,
        cache: &Path,
        cancel: &cuerithm_media::CancellationToken,
    ) -> Result<StagedTagWrite> {
        cancel.check()?;
        ensure!(
            audio.source_sha256 == self.input.source_sha256
                && audio.source_is_unchanged_cancellable(cancel)?,
            "Source changed before tagging"
        );
        ensure!(
            audio.decoder_identity == runtime.identity,
            "Different decoder for tag verification"
        );
        if let Some(patch) = &self.transfer {
            ensure!(
                self.tags_are_noop() && self.cleanup.is_none() && self.metadata_edits.is_empty(),
                "Native transfer cannot be mixed with catalog or cleanup edits"
            );
            ensure!(
                crate::read_transfer_metadata(&audio.snapshot_path, &audio.info)? == patch.before,
                "Native transfer preview is stale"
            );
        }
        std::fs::create_dir_all(cache)?;
        let mut private = tempfile::Builder::new();
        private.prefix("cuerithm-tags-");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            private.permissions(std::fs::Permissions::from_mode(0o700));
        }
        let private = private.tempdir_in(cache)?;
        let format = self.input.audio_info(&self.config)?.format;
        let extension = match format {
            AudioFileFormat::FLAC => "flac",
            AudioFileFormat::AIFF => "aiff",
            AudioFileFormat::MP3 => "mp3",
            AudioFileFormat::MP4 => "m4a",
            AudioFileFormat::WAV => "wav",
            AudioFileFormat::OGG => {
                if self.input.media.codec == "opus" {
                    "opus"
                } else {
                    "ogg"
                }
            }
            AudioFileFormat::AAC => "aac",
            AudioFileFormat::WMA => "wma",
        };
        let backup = private.path().join(format!("original.{extension}"));
        let path = private.path().join(format!("staged.{extension}"));
        std::fs::copy(&audio.snapshot_path, &backup)?;
        cancel.check()?;
        std::fs::copy(&backup, &path)?;
        cancel.check()?;
        ensure!(
            file_sha256(&backup)? == self.input.source_sha256,
            "Snapshot changed before tagging"
        );
        let permissions = std::fs::metadata(&self.input.path)?.permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                &path,
                std::fs::Permissions::from_mode(permissions.mode() | 0o200),
            )?;
        }
        #[cfg(windows)]
        {
            let mut p = std::fs::metadata(&path)?.permissions();
            // Windows clears its read-only attribute; Unix uses owner-write
            // permissions above and never executes this branch.
            #[allow(clippy::permissions_set_readonly_false)]
            p.set_readonly(false);
            std::fs::set_permissions(&path, p)?;
        }
        let other_bindings: Vec<String> = self
            .matches
            .iter()
            .flat_map(|track| track.other.iter().map(|(name, _)| name.by_format(&format)))
            .collect();
        let custom_style = if self.config.options.styles_options == StylesOptions::CustomTag {
            self.config
                .options
                .styles_custom_tag
                .as_ref()
                .map(|name| name.by_format(&format))
        } else {
            None
        };
        let fields_before = crate::fields::values(
            &backup,
            &audio.info,
            &self.config.options.tags,
            &other_bindings,
            custom_style.as_deref(),
        )?;
        let mut sidecars = Vec::new();
        let mut cleanup = None;
        let mut expected_metadata = None;
        let mut expected_editor = None;
        let mut editor_before = self
            .editor_patch
            .as_ref()
            .map(|_| crate::read_metadata_snapshot(&backup, &audio.info))
            .transpose()?;
        if !self.is_noop() {
            match format {
                AudioFileFormat::WMA => {
                    let mut tag = AsfTag::open(&path)?;
                    tag.set_separator(
                        self.config
                            .options
                            .separators
                            .vorbis
                            .as_deref()
                            .unwrap_or("; "),
                    );
                    let request = self.apply(&mut tag, &format)?;
                    if let Some(patch) = &self.editor_patch {
                        editor_before = Some(crate::editing::native::snapshot_asf(&tag)?);
                        patch.apply_asf(&mut tag)?;
                    }
                    let (report, expected) = self.apply_cleanup(&mut tag, &format)?;
                    if let Some(patch) = &self.transfer {
                        patch.apply(&mut tag, &format)?;
                    }
                    cleanup = report;
                    expected_metadata = Some(expected);
                    if !self.tags_are_noop()
                        || self.metadata_edits.iter().any(|e| e.before != e.after)
                        || cleanup.as_ref().is_some_and(|r| !r.changes.is_empty())
                        || self.transfer.as_ref().is_some_and(|p| p.before != p.after)
                        || self.editor_patch.as_ref().is_some_and(|p| !p.is_noop())
                    {
                        tag.save_file(&path)?;
                    }
                    if self.editor_patch.is_some() {
                        expected_editor = Some(crate::editing::native::snapshot_asf(&tag)?);
                    }
                    if let Some(request) = request {
                        sidecars.push(crate::sidecar::stage(
                            request,
                            private.path(),
                            sidecars.len(),
                        )?);
                    }
                }
                _ => {
                    crate::id3_integrity::validate_container(&path, &format)
                        .context("Read existing metadata; refusing malformed ID3 reconstruction")?;
                    let mut wrapper = Tag::load_file(&path, true)
                        .context("Read existing metadata; refusing to discard an unreadable tag")?;
                    wrapper.set_separators(&self.config.options.separators);
                    if !self.tags_are_noop()
                        && let Tag::ID3(tag) = &mut wrapper
                    {
                        tag.set_id3v24(self.config.options.id3v24);
                        if let Some(lang) = &self.config.options.id3_comm_lang {
                            tag.set_comm_lang(lang.clone());
                        }
                    }
                    {
                        let tag = wrapper.tag_mut();
                        let request = self.apply(&mut **tag, &format)?;
                        drop(tag);
                        if let Some(patch) = &self.editor_patch {
                            editor_before =
                                Some(crate::editing::native::snapshot_tag(&wrapper, &format)?);
                            patch.apply_tag(&mut wrapper, &format)?;
                        }
                        let tag = wrapper.tag_mut();
                        let (report, expected) = self.apply_cleanup(&mut **tag, &format)?;
                        if let Some(patch) = &self.transfer {
                            patch.apply(&mut **tag, &format)?;
                        }
                        cleanup = report;
                        expected_metadata = Some(expected);
                        if !self.tags_are_noop()
                            || self.metadata_edits.iter().any(|e| e.before != e.after)
                            || cleanup.as_ref().is_some_and(|r| !r.changes.is_empty())
                            || self.transfer.as_ref().is_some_and(|p| p.before != p.after)
                            || self.editor_patch.as_ref().is_some_and(|p| !p.is_noop())
                            || self.editor_patch.as_ref().is_some_and(|p| !p.is_noop())
                        {
                            tag.save_file(&path)?;
                        }
                        if let Some(request) = request {
                            sidecars.push(crate::sidecar::stage(
                                request,
                                private.path(),
                                sidecars.len(),
                            )?);
                        }
                    }
                    if self.editor_patch.is_some() {
                        expected_editor =
                            Some(crate::editing::native::snapshot_tag(&wrapper, &format)?);
                    }
                }
            }
        }
        let mut after = runtime
            .decode_cancellable(&path, cache, cancel)
            .context("Verify tagged audio")?;
        if let Some(patch) = &self.editor_patch {
            patch.project_release_date(&mut after.info);
        }
        let native_metadata = crate::metadata::read_metadata(&path, &after.info)?;
        if let Some(expected) = expected_editor {
            let actual = crate::read_metadata_snapshot(&path, &after.info)?;
            ensure!(
                actual == expected,
                "Native container cannot preserve this exact typed metadata edit; source remains unchanged"
            );
        }
        let fields_after = crate::fields::values(
            &path,
            &after.info,
            &self.config.options.tags,
            &other_bindings,
            custom_style.as_deref(),
        )?;
        let external_art_kept = sidecars.is_empty()
            && self.config.options.tag_enabled(SupportedTag::AlbumArt)
            && crate::fields::external_art_exists(&path, &after.info, &self.input, &self.config)?;
        let fields: Vec<crate::FieldWriteOutcome> = self
            .config
            .options
            .tags
            .iter()
            .enumerate()
            .map(|(i, field)| {
                let external_art = *field == SupportedTag::AlbumArt && !sidecars.is_empty();
                let represented_clean = *field == SupportedTag::Explicit
                    && matches!(format, AudioFileFormat::FLAC | AudioFileFormat::OGG)
                    && fields_after[i].is_none()
                    && self.matches.iter().any(|m| m.explicit == Some(false));
                let status = if external_art
                    || represented_clean
                    || (fields_after[i].is_some() && fields_after[i] != fields_before[i])
                {
                    crate::FieldWriteStatus::Written
                } else if (*field == SupportedTag::AlbumArt && external_art_kept)
                    || fields_after[i].is_some()
                {
                    crate::FieldWriteStatus::KeptExisting
                } else {
                    crate::FieldWriteStatus::Unavailable
                };
                crate::FieldWriteOutcome {
                    field: *field,
                    status,
                    required: self.config.required_tags.contains(field),
                }
            })
            .collect();
        for field in &fields {
            ensure!(
                !field.required || field.status != crate::FieldWriteStatus::Unavailable,
                "Required selected field unavailable after tagging: {:?}",
                field.field
            );
        }
        let mut provenance = self.provenance.clone();
        provenance["fields"] = serde_json::to_value(&fields)?;
        if let Some(patch) = &self.transfer {
            patch.verify(&path, &after.info)?;
        }
        if (self.cleanup.is_some() || !self.metadata_edits.is_empty())
            && let Some(expected) = expected_metadata
        {
            ensure!(
                native_metadata.fields == expected.fields,
                "Metadata cannot be represented exactly in this tag format"
            );
        }
        let pcm = audio.pcm_sha256_cancellable(cancel)?;
        ensure!(
            after.frames == audio.frames && after.pcm_sha256_cancellable(cancel)? == pcm,
            "Tagging changed playable audio or timeline"
        );
        ensure!(
            audio.source_is_unchanged_cancellable(cancel)?,
            "Source changed during tagging"
        );
        std::fs::File::options()
            .read(true)
            .write(true)
            .open(&path)?
            .sync_all()?;
        std::fs::set_permissions(&path, permissions)?;
        let editor_receipt_json = editor_before
            .map(|before| {
                crate::read_metadata_snapshot(&path, &after.info).and_then(|after| {
                    Ok(serde_json::to_string(&crate::MetadataEditorReceipt {
                        before,
                        after,
                    })?)
                })
            })
            .transpose()?;
        Ok(StagedTagWrite {
            source_path: audio.source_path.clone(),
            source_before_sha256: audio.source_sha256.clone(),
            source_after_sha256: cuerithm_media::file_sha256_cancellable(&path, cancel)?,
            metadata: after.info,
            sidecars,
            path,
            backup_path: backup,
            pcm_sha256: pcm,
            frames: audio.frames,
            decoder_identity: runtime.identity.clone(),
            provenance,
            cleanup,
            native_metadata,
            editor_receipt_json,
            workspace: private.path().to_owned(),
            private: Some(private),
        })
    }
    fn apply(
        &self,
        tag: &mut dyn TagImpl,
        format: &AudioFileFormat,
    ) -> Result<Option<crate::sidecar::SidecarRequest>> {
        let config = &self.config.options;
        let mut explicit_present = crate::fields::explicit_at(&self.input.path, format, tag)?;
        for original in &self.matches {
            let mut track = original.clone();
            let genres = track.genres.clone();
            let styles = track.styles.clone();
            match config.styles_options {
                StylesOptions::OnlyGenres => track.styles.clear(),
                StylesOptions::OnlyStyles => track.genres.clear(),
                StylesOptions::MergeToGenres => {
                    track.genres.extend(styles);
                    track.styles.clear();
                }
                StylesOptions::MergeToStyles => {
                    track.styles.extend(genres);
                    track.genres.clear();
                }
                StylesOptions::StylesToGenre => {
                    track.genres = styles;
                    track.styles.clear();
                }
                StylesOptions::GenresToStyle => {
                    track.styles = genres;
                    track.genres.clear();
                }
                _ => {}
            }
            let enabled = |t| config.tag_enabled(t);
            let overwrite = |t| config.overwrite_tag(t);
            if enabled(SupportedTag::Title) {
                tag.set_field(
                    Field::Title,
                    vec![if config.short_title {
                        track.title.clone()
                    } else {
                        track.full_title()
                    }],
                    overwrite(SupportedTag::Title),
                );
            }
            for (selected, field, values) in [
                (SupportedTag::Artist, Field::Artist, track.artists.clone()),
                (
                    SupportedTag::AlbumArtist,
                    Field::AlbumArtist,
                    track.album_artists.clone(),
                ),
                (SupportedTag::Genre, Field::Genre, track.genres.clone()),
                (SupportedTag::Style, Field::Style, track.styles.clone()),
                (
                    SupportedTag::Remixer,
                    Field::Remixer,
                    track.remixers.clone(),
                ),
            ] {
                if enabled(selected) && !values.is_empty() {
                    let mut values = values;
                    if config.merge_genres && (field == Field::Genre || field == Field::Style) {
                        let mut old = tag.get_field(field.clone()).unwrap_or_default();
                        for v in values {
                            if !old.iter().any(|x| x.to_lowercase() == v.to_lowercase()) {
                                old.push(v);
                            }
                        }
                        values = old;
                    }
                    if field == Field::Genre && config.capitalize_genres {
                        values = values
                            .into_iter()
                            .map(|v| {
                                v.split_whitespace()
                                    .map(|word| {
                                        let mut c = word.chars();
                                        c.next()
                                            .map(|head| {
                                                head.to_uppercase().collect::<String>() + c.as_str()
                                            })
                                            .unwrap_or_default()
                                    })
                                    .collect::<Vec<_>>()
                                    .join(" ")
                            })
                            .collect();
                    }
                    if field == Field::Style && config.styles_options == StylesOptions::CustomTag {
                        let custom = config
                            .styles_custom_tag
                            .as_ref()
                            .context("Missing custom style tag")?;
                        tag.set_raw(&custom.by_format(format), values, overwrite(selected));
                    } else {
                        tag.set_field(field, values, overwrite(selected));
                    }
                }
            }
            for (selected, field, value) in [
                (SupportedTag::Album, Field::Album, track.album.clone()),
                (SupportedTag::Label, Field::Label, track.label.clone()),
                (SupportedTag::Version, Field::Version, track.version.clone()),
                (SupportedTag::ISRC, Field::ISRC, track.isrc.clone()),
                (
                    SupportedTag::CatalogNumber,
                    Field::CatalogNumber,
                    track.catalog_number.clone(),
                ),
            ] {
                if enabled(selected)
                    && let Some(value) = value
                {
                    if field == Field::Label
                        && (*format == AudioFileFormat::FLAC || *format == AudioFileFormat::OGG)
                    {
                        let has_label = tag.get_raw("LABEL").is_some();
                        if !overwrite(selected)
                            && (has_label || tag.get_field(Field::Label).is_some())
                        {
                            continue;
                        }
                        if has_label {
                            tag.set_raw("LABEL", vec![value.clone()], true);
                        }
                    }
                    if field == Field::Version
                        && matches!(format, AudioFileFormat::FLAC | AudioFileFormat::OGG)
                    {
                        let has_legacy = tag.get_native("VERSION").is_some();
                        if !overwrite(selected)
                            && (has_legacy || tag.get_field(Field::Version).is_some())
                        {
                            continue;
                        }
                        if has_legacy {
                            tag.set_native("VERSION", vec![value.clone()]);
                        }
                    }
                    tag.set_field(field, vec![value], overwrite(selected));
                }
            }
            if self.config.analysis_tag_source == AnalysisTagSource::Catalog {
                self.apply_key_bpm(
                    tag,
                    format,
                    track.key.as_deref(),
                    track.bpm.map(|n| n as f64),
                )?;
            }
            for (selected, date, year, publish) in [
                (
                    SupportedTag::ReleaseDate,
                    track.release_date,
                    track.release_year,
                    false,
                ),
                (
                    SupportedTag::PublishDate,
                    track.publish_date,
                    track.publish_year,
                    true,
                ),
            ] {
                if enabled(selected) {
                    let d = date
                        .map(|d| TagDate {
                            year: d.year(),
                            month: if config.only_year {
                                None
                            } else {
                                Some(d.month() as u8)
                            },
                            day: if config.only_year {
                                None
                            } else {
                                Some(d.day() as u8)
                            },
                        })
                        .or_else(|| {
                            year.map(|y| TagDate {
                                year: y.into(),
                                month: None,
                                day: None,
                            })
                        });
                    if let Some(d) = d {
                        if publish {
                            tag.set_publish_date(&d, overwrite(selected));
                        } else {
                            tag.set_date(&d, overwrite(selected));
                        }
                    }
                }
            }
            if enabled(SupportedTag::OtherTags) {
                for (key, values) in &track.other {
                    tag.set_raw(
                        &key.by_format(format),
                        values.clone(),
                        overwrite(SupportedTag::OtherTags),
                    );
                }
            }
            for (selected, name, value) in [
                (
                    SupportedTag::TrackId,
                    format!("{}_TRACK_ID", track.platform.to_uppercase()),
                    track.track_id.clone(),
                ),
                (
                    SupportedTag::ReleaseId,
                    format!("{}_RELEASE_ID", track.platform.to_uppercase()),
                    track.release_id.clone(),
                ),
                (
                    SupportedTag::URL,
                    "WWWAUDIOFILE".into(),
                    Some(track.url.clone()),
                ),
            ] {
                if enabled(selected)
                    && let Some(value) = value
                {
                    tag.set_raw(&name, vec![value], overwrite(selected));
                }
            }
            apply_track_numbers(tag, format, config, &track);
            if enabled(SupportedTag::Duration) {
                let seconds = self.input.duration_seconds;
                let value = if matches!(
                    format,
                    AudioFileFormat::MP3
                        | AudioFileFormat::AAC
                        | AudioFileFormat::AIFF
                        | AudioFileFormat::WAV
                ) {
                    (seconds * 1000.).round().to_string()
                } else {
                    format!("{seconds:.3}")
                };
                tag.set_field(
                    Field::Duration,
                    vec![value],
                    overwrite(SupportedTag::Duration),
                );
            }
            if enabled(SupportedTag::Mood)
                && let Some(mood) = &track.mood
            {
                tag.set_field(
                    Field::Mood,
                    vec![mood.clone()],
                    overwrite(SupportedTag::Mood),
                );
            }
            if enabled(SupportedTag::Explicit)
                && (overwrite(SupportedTag::Explicit) || !explicit_present)
                && let Some(explicit) = track.explicit
            {
                tag.set_explicit(explicit);
                explicit_present = true;
            }
            if enabled(SupportedTag::DiscNumber)
                && let Some(n) = track.disc_number
            {
                tag.set_field(
                    Field::DiscNumber,
                    vec![n.to_string()],
                    overwrite(SupportedTag::DiscNumber),
                );
            }
        }
        if self.config.analysis_tag_source == AnalysisTagSource::Measured {
            self.apply_key_bpm(
                tag,
                format,
                if config.camelot {
                    self.analysis.camelot_key.as_deref()
                } else {
                    self.analysis.standard_key.as_deref()
                },
                self.analysis.bpm,
            )?;
        }
        let mut sidecar = None;
        if config.tag_enabled(SupportedTag::AlbumArt)
            && (config.album_art_file
                || config.overwrite_tag(SupportedTag::AlbumArt)
                || !tag.has_art())
        {
            let target = if config.album_art_file {
                Some(crate::sidecar::target_name(&self.input, &self.config, tag)?)
            } else {
                None
            };
            let preserve = target
                .as_ref()
                .is_some_and(|p| p.exists() && !config.overwrite_tag(SupportedTag::AlbumArt));
            let mut artwork = self.matches.iter().filter_map(|track| {
                track
                    .art
                    .as_deref()
                    .map(|url| (url, track.platform.as_str()))
            });
            let remote = if config.overwrite_tag(SupportedTag::AlbumArt) {
                artwork.next_back()
            } else {
                artwork.next()
            };
            let art = if preserve {
                None
            } else if let Some(image) = &self.artwork {
                Some(image.clone())
            } else {
                remote
                    .map(|(url, platform)| crate::image::Artwork::fetch(url, platform))
                    .transpose()
                    .context("Selected artwork could not be fetched or decoded")?
            };
            if let Some(art) = art {
                if let Some(target) = target {
                    sidecar = Some(crate::sidecar::SidecarRequest { target, image: art });
                } else {
                    if config.remove_all_covers {
                        for kind in CoverType::types() {
                            tag.remove_art(kind);
                        }
                    } else {
                        tag.remove_art(CoverType::CoverFront);
                    }
                    tag.set_art(CoverType::CoverFront, art.mime, Some("Cover"), art.bytes);
                }
            }
        }
        if self.config.write_energy
            && let Some(energy) = self.analysis.energy
        {
            tag.set_raw("ENERGY", vec![energy.to_string()], true);
        }
        if config.tag_enabled(SupportedTag::MetaTags) {
            tag.set_raw(
                "CUERITHM_TAGGEDDATE",
                vec![format!("{}_AT", chrono::Utc::now().to_rfc3339())],
                true,
            );
        }
        Ok(sidecar)
    }
    fn apply_key_bpm(
        &self,
        tag: &mut dyn TagImpl,
        format: &AudioFileFormat,
        key: Option<&str>,
        bpm: Option<f64>,
    ) -> Result<()> {
        let config = &self.config.options;
        if config.tag_enabled(SupportedTag::Key)
            && let Some(key) = key
        {
            let value = if self.config.analysis_tag_source == AnalysisTagSource::Catalog
                && config.camelot
            {
                onetagger_tagger::to_camelot(key)
            } else {
                key
            };
            tag.set_field(
                Field::Key,
                vec![value.to_owned()],
                config.overwrite_tag(SupportedTag::Key),
            );
        }
        if config.tag_enabled(SupportedTag::BPM)
            && let Some(value) = bpm
        {
            let overwrite = config.overwrite_tag(SupportedTag::BPM);
            if *format == AudioFileFormat::MP4 {
                if overwrite
                    || (tag.get_field(Field::BPM).is_none()
                        && tag.get_raw("----:com.apple.iTunes:BPM").is_none())
                {
                    tag.set_field(Field::BPM, vec![value.round().to_string()], true);
                    tag.set_raw(
                        "----:com.apple.iTunes:BPM",
                        vec![format!("{value:.2}")],
                        true,
                    );
                }
            } else {
                tag.set_field(Field::BPM, vec![format!("{value:.2}")], overwrite);
            }
        }
        Ok(())
    }
}

// ID3 stores both independently configured fields in one TRCK frame. Resolve
// each component first so selecting/overwriting one cannot erase the other.
fn apply_track_numbers(
    tag: &mut dyn TagImpl,
    format: &AudioFileFormat,
    config: &onetagger_tagger::TaggerConfig,
    track: &CatalogTrack,
) {
    let number = config
        .tag_enabled(SupportedTag::TrackNumber)
        .then_some(track.track_number.as_ref())
        .flatten()
        .map(|n| n.to_string_with_zeroes(config.track_number_leading_zeroes));
    let total = config
        .tag_enabled(SupportedTag::TrackTotal)
        .then_some(track.track_total)
        .flatten();
    if matches!(
        format,
        AudioFileFormat::MP3 | AudioFileFormat::AAC | AudioFileFormat::AIFF | AudioFileFormat::WAV
    ) {
        let current = tag
            .get_raw("TRCK")
            .and_then(|v| v.first().cloned())
            .unwrap_or_default();
        let mut parts = current.splitn(2, '/');
        let old_number = parts.next().unwrap_or_default();
        let old_total = parts.next().filter(|s| !s.is_empty());
        let write_number = number.is_some()
            && (config.overwrite_tag(SupportedTag::TrackNumber) || old_number.is_empty());
        let write_total = total.is_some()
            && (config.overwrite_tag(SupportedTag::TrackTotal) || old_total.is_none());
        if write_number || write_total {
            let resolved_number = if write_number {
                number.as_deref().unwrap()
            } else if old_number.is_empty() {
                "0"
            } else {
                old_number
            };
            let resolved_total = if write_total {
                total.map(|n| n.to_string())
            } else {
                old_total.map(str::to_owned)
            };
            let value = match resolved_total {
                Some(total) => format!("{resolved_number}/{total}"),
                None => resolved_number.to_owned(),
            };
            tag.set_raw("TRCK", vec![value], true);
        }
    } else {
        if let Some(number) = number {
            tag.set_track_number(
                &number,
                None,
                config.overwrite_tag(SupportedTag::TrackNumber),
            );
        }
        if let Some(total) = total {
            tag.set_field(
                Field::TrackTotal,
                vec![total.to_string()],
                config.overwrite_tag(SupportedTag::TrackTotal),
            );
        }
    }
}
