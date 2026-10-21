use super::*;
use crate::{Library, PreparationSettings};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Document {
    pub format: String,
    pub version: u32,
    pub name: String,
    pub rules: PreparationPresetRules,
}
pub(super) fn valid_name(name: &str) -> Result<String> {
    let name = name.trim();
    ensure!(
        !name.is_empty() && name.chars().count() <= 80 && !name.chars().any(char::is_control),
        "Preset name must contain 1–80 characters without control characters"
    );
    Ok(name.to_owned())
}
pub(super) fn validate(rules: &PreparationPresetRules) -> Result<()> {
    ensure!(
        serde_json::to_vec(rules)?.len() <= MAX_PREPARATION_PRESET_DOCUMENT_BYTES,
        "Preset exceeds the 1 MiB limit"
    );
    let mut settings = PreparationSettings::default();
    settings.engine.lexicon_config = Some(rules.cue_generation.clone());
    settings.engine.validate()?;
    organization(
        rules,
        &PreparationPresetConnections {
            destination_root_id: Some("unconnected-portable-slot".into()),
        },
    )
    .validate()?;
    if let Some(cleanup) = &rules.cleanup {
        cleanup.validate()?;
    }
    ensure!(
        !rules.cleanup_enabled || rules.cleanup.is_some(),
        "Configure cleanup rules before enabling them"
    );
    ensure!(
        !rules.autotag_enabled || rules.tagging.is_some(),
        "Configure tagging rules before enabling them"
    );
    if let Some(tagging) = &rules.tagging {
        let options = &tagging.options;
        ensure!(
            matches!(tagging.no_match.as_str(), "keep_existing" | "attention"),
            "Unsupported no-match policy"
        );
        ensure!(
            matches!(tagging.analysis_tag_source.as_str(), "measured" | "catalog"),
            "Unsupported analysis-tag source"
        );
        ensure!(
            !options.platforms.is_empty() && options.platforms.len() <= 10,
            "Choose at least one supported metadata provider"
        );
        let mut seen = std::collections::BTreeSet::new();
        for provider in &options.platforms {
            ensure!(
                matches!(
                    provider.as_str(),
                    "beatport"
                        | "traxsource"
                        | "musicbrainz"
                        | "sonovault"
                        | "discogs"
                        | "itunes"
                        | "bpmsupreme"
                        | "bandcamp"
                        | "deezer"
                        | "volumo"
                ) && seen.insert(provider),
                "Unsupported or duplicate preset provider"
            );
        }
        let tags = [
            "title",
            "artist",
            "album",
            "key",
            "genre",
            "style",
            "releaseDate",
            "publishDate",
            "albumArt",
            "otherTags",
            "catalogNumber",
            "trackId",
            "releaseId",
            "version",
            "duration",
            "albumArtist",
            "remixer",
            "trackNumber",
            "trackTotal",
            "discNumber",
            "mood",
            "syncedLyrics",
            "unsyncedLyrics",
            "label",
            "explicit",
            "metaTags",
            "bpm",
            "url",
            "isrc",
        ];
        for selected in [
            &options.tags,
            &options.overwrite_tags,
            &tagging.required_tags,
        ] {
            ensure!(
                selected.len() <= tags.len()
                    && selected.iter().all(|tag| tags.contains(&tag.as_str())),
                "Unsupported preset tag field"
            );
            ensure!(
                selected
                    .iter()
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    == selected.len(),
                "Duplicate preset tag field"
            );
        }
        ensure!(
            tagging
                .required_tags
                .iter()
                .all(|tag| options.tags.contains(tag)),
            "Required fields must be selected"
        );
        ensure!(
            options.strictness.is_finite() && (0.0..=1.0).contains(&options.strictness),
            "Matching strictness must be between 0 and 1"
        );
        ensure!(
            options.album_tagging_ratio.is_finite()
                && (0.0..=1.0).contains(&options.album_tagging_ratio),
            "Invalid album matching ratio"
        );
        ensure!(
            matches!(
                options.multiple_matches.as_str(),
                "Default" | "Oldest" | "Newest"
            ),
            "Unsupported multiple-match policy"
        );
        ensure!(
            matches!(
                options.styles_options.as_str(),
                "default"
                    | "onlyGenres"
                    | "onlyStyles"
                    | "mergeToGenres"
                    | "mergeToStyles"
                    | "stylesToGenre"
                    | "genresToStyle"
                    | "customTag"
            ),
            "Unsupported style policy"
        );
        ensure!(
            options.track_number_leading_zeroes <= 8,
            "Track padding exceeds 8 digits"
        );
        ensure!(
            options.id3_comm_lang.as_ref().is_none_or(
                |lang| lang.len() == 3 && lang.bytes().all(|byte| byte.is_ascii_alphabetic())
            ),
            "Invalid ID3 comment language"
        );
        if options
            .platforms
            .iter()
            .any(|provider| provider == "beatport")
        {
            let provider = options
                .custom
                .beatport
                .as_ref()
                .context("Missing Beatport preset options")?;
            ensure!(
                (1..=10).contains(&provider.max_pages)
                    && (200..=1600).contains(&provider.art_resolution)
                    && provider.art_resolution.is_multiple_of(100),
                "Invalid Beatport preset options"
            );
        }
        #[cfg(feature = "autotag")]
        {
            let config: cuerithm_autotag::AutotagConfig =
                serde_json::from_value(tagging_value(None, tagging)?)?;
            config.validate()?;
        }
    }
    Ok(())
}
pub(super) fn organization(
    rules: &PreparationPresetRules,
    connections: &PreparationPresetConnections,
) -> crate::OrganizationSettings {
    crate::OrganizationSettings {
        mode: rules.organization.mode,
        pattern: rules.organization.pattern.clone(),
        missing: rules.organization.missing,
        collision: rules.organization.collision,
        organization_optional: rules.organization.organization_optional,
        destination_root_id: connections.destination_root_id.clone(),
    }
}
fn tagging_value(base: Option<Value>, rules: &PreparationPresetTagging) -> Result<Value> {
    // Only these excluded defaults are reconstructed when no global config
    // exists. Existing global transport/ingestion/credentials remain intact.
    let mut base=base.filter(Value::is_object).unwrap_or_else(||json!({"solver":{"endpoint":null,"timeout_seconds":120},"options":{"path":null,"threads":16,"postCommand":null,"enableShazam":false,"forceShazam":false,"includeSubfolders":true,"moveSuccess":false,"moveSuccessPath":null,"moveFailed":false,"moveFailedPath":null,"writeLrc":false,"enhancedLrc":false,"spotify":null}}));
    let portable = serde_json::to_value(rules)?;
    let target = base
        .as_object_mut()
        .context("Invalid global tagging config")?;
    for key in [
        "no_match",
        "analysis_tag_source",
        "write_energy",
        "required_tags",
    ] {
        target.insert(key.into(), portable[key].clone());
    }
    let target_options = target
        .entry("options")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .context("Invalid tagging options")?;
    let options = portable["options"]
        .as_object()
        .context("Invalid portable tagging options")?;
    for (key, value) in options {
        if key == "custom" {
            let custom = target_options
                .entry(key)
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .context("Invalid provider settings")?;
            for (provider, values) in value
                .as_object()
                .context("Invalid portable provider options")?
            {
                if values.is_null() {
                    continue;
                }
                let target = custom
                    .entry(provider)
                    .or_insert_with(|| json!({}))
                    .as_object_mut()
                    .context("Invalid global provider options")?;
                for (key, value) in values
                    .as_object()
                    .context("Invalid portable provider options")?
                {
                    target.insert(key.clone(), value.clone());
                }
            }
        } else {
            target_options.insert(key.clone(), value.clone());
        }
    }
    Ok(base)
}
pub(super) fn overlay(
    base: &PreparationSettings,
    rules: &PreparationPresetRules,
    connections: &PreparationPresetConnections,
) -> Result<PreparationSettings> {
    validate(rules)?;
    let mut result = base.clone();
    result.engine.lexicon_config = Some(rules.cue_generation.clone());
    result.autotag_enabled = rules.autotag_enabled;
    result.cleanup_enabled = rules.cleanup_enabled;
    result.cleanup = rules.cleanup.clone();
    result.organization = organization(rules, connections);
    result.organization.validate()?;
    if let Some(tagging) = &rules.tagging {
        result.tagging = Some(serde_json::from_value(tagging_value(
            base.tagging
                .as_ref()
                .map(serde_json::to_value)
                .transpose()?,
            tagging,
        )?)?);
    }
    Ok(result)
}
pub(super) fn project(settings: &PreparationSettings) -> Result<PreparationPresetProjection> {
    let tagging=settings.tagging.as_ref().map(|config|->Result<_>{
        let raw=serde_json::to_value(config)?;let mut options=raw["options"].as_object().context("Invalid tagging options")?.clone();
        for key in ["path","threads","postCommand","enableShazam","forceShazam","includeSubfolders","moveSuccess","moveSuccessPath","moveFailed","moveFailedPath","writeLrc","enhancedLrc","spotify"] {options.remove(key);}
        let custom=options.remove("custom").unwrap_or_else(||json!({}));
        let beatport=custom.get("beatport").filter(|value|value.is_object()).map(|value|json!({"max_pages":value["max_pages"],"art_resolution":value["art_resolution"],"ignore_version":value["ignore_version"]}));
        let mut safe_custom=json!({"beatport":beatport,"traxsource":custom.get("traxsource").map(|_|json!({}))});
        for provider in ["musicbrainz","sonovault","discogs","itunes","bpmsupreme","bandcamp","deezer","volumo"] {
            safe_custom[provider]=custom.get(provider).map(|_|json!({})).unwrap_or(Value::Null);
        }
        options.insert("custom".into(),safe_custom);
        Ok(PreparationPresetTagging{options:serde_json::from_value(Value::Object(options))?,no_match:serde_json::from_value(raw["no_match"].clone())?,analysis_tag_source:serde_json::from_value(raw["analysis_tag_source"].clone())?,write_energy:serde_json::from_value(raw["write_energy"].clone())?,required_tags:serde_json::from_value(raw["required_tags"].clone())?})
    }).transpose()?;
    let result = PreparationPresetProjection {
        rules: PreparationPresetRules {
            cue_generation: settings
                .engine
                .lexicon_config
                .clone()
                .context("Preparation requires replacement cues")?,
            autotag_enabled: settings.autotag_enabled,
            tagging,
            cleanup_enabled: settings.cleanup_enabled,
            cleanup: settings.cleanup.clone(),
            organization: PreparationPresetOrganization {
                mode: settings.organization.mode,
                pattern: settings.organization.pattern.clone(),
                missing: settings.organization.missing,
                collision: settings.organization.collision,
                organization_optional: settings.organization.organization_optional,
            },
        },
        connections: PreparationPresetConnections {
            destination_root_id: settings.organization.destination_root_id.clone(),
        },
    };
    validate(&result.rules)?;
    Ok(result)
}
fn reject_ignored(input: &Value, output: &Value, path: &str) -> Result<()> {
    match (input, output) {
        (Value::Object(input), Value::Object(output)) => {
            for (key, value) in input {
                let next = format!("{path}.{key}");
                reject_ignored(
                    value,
                    output
                        .get(key)
                        .with_context(|| format!("Unsupported preset field {next}"))?,
                    &next,
                )?;
            }
        }
        (Value::Array(input), Value::Array(output)) => {
            ensure!(
                input.len() == output.len(),
                "Duplicate or unsupported values at {path}"
            );
            for (index, (input, output)) in input.iter().zip(output).enumerate() {
                reject_ignored(input, output, &format!("{path}[{index}]"))?;
            }
        }
        _ => {}
    }
    Ok(())
}
pub(super) fn parse(document: &str) -> Result<Document> {
    ensure!(
        document.len() <= MAX_PREPARATION_PRESET_DOCUMENT_BYTES,
        "Preset exceeds the 1 MiB limit"
    );
    let input: Value = serde_json::from_str(document).context("Invalid preset JSON")?;
    let result: Document =
        serde_json::from_value(input.clone()).context("Invalid preset document")?;
    ensure!(
        result.format == "cuerithm-preparation-preset" && result.version == 1,
        "Unsupported preparation preset format/version"
    );
    reject_ignored(&input, &serde_json::to_value(&result)?, "preset")?;
    valid_name(&result.name)?;
    validate(&result.rules)?;
    Ok(result)
}
impl Library {
    pub fn project_preparation_preset(
        &self,
        settings: &PreparationSettings,
    ) -> Result<PreparationPresetProjection> {
        project(settings)
    }
    pub fn overlay_preparation_preset(
        &self,
        settings: &PreparationSettings,
        id: &str,
        revision: &str,
    ) -> Result<PreparationSettings> {
        let preset = self.preparation_preset(id)?;
        ensure!(
            preset.revision == revision,
            "Preset changed; reload before applying it"
        );
        ensure!(
            preset.status == "ready",
            "Reconnect this preset before loading it"
        );
        overlay(settings, &preset.rules, &preset.connections)
    }
    pub fn preview_preparation_preset_import(
        &self,
        document: &str,
    ) -> Result<PreparationPresetImportPreview> {
        let parsed = parse(document)?;
        let (status, issues) =
            self.preparation_preset_status(&parsed.rules, &Default::default())?;
        Ok(PreparationPresetImportPreview {
            name: parsed.name,
            rules: parsed.rules,
            status,
            issues,
        })
    }
    pub fn import_preparation_preset(
        &self,
        document: &str,
        name: Option<&str>,
    ) -> Result<PreparationPreset> {
        let parsed = parse(document)?;
        self.save_preparation_preset(PreparationPresetSave {
            id: None,
            expected_revision: None,
            name: name.unwrap_or(&parsed.name).into(),
            rules: parsed.rules,
            connections: Default::default(),
        })
    }
    pub fn export_preparation_preset(
        &self,
        id: &str,
        revision: &str,
    ) -> Result<PreparationPresetExport> {
        let preset = self.preparation_preset(id)?;
        ensure!(
            preset.revision == revision,
            "Preset changed; reload before exporting"
        );
        let document = serde_json::to_string_pretty(&Document {
            format: "cuerithm-preparation-preset".into(),
            version: 1,
            name: preset.name,
            rules: preset.rules,
        })?;
        ensure!(
            document.len() <= MAX_PREPARATION_PRESET_DOCUMENT_BYTES,
            "Preset exceeds the 1 MiB limit"
        );
        Ok(PreparationPresetExport {
            document,
            warnings: vec![],
        })
    }
}
