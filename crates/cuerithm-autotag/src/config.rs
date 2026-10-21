use anyhow::{Result, ensure};
use onetagger_tagger::TaggerConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NoMatchPolicy {
    #[default]
    KeepExisting,
    Attention,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisTagSource {
    #[default]
    Measured,
    Catalog,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AutotagConfig {
    pub solver: crate::SolverConfig,
    pub recognition: crate::RecognitionConfig,
    pub options: TaggerConfig,
    pub no_match: NoMatchPolicy,
    pub analysis_tag_source: AnalysisTagSource,
    pub write_energy: bool,
    /// Selected fields that must be present after preparation. Optional fields report absence.
    pub required_tags: Vec<onetagger_tagger::SupportedTag>,
}
impl Default for AutotagConfig {
    fn default() -> Self {
        let mut options = TaggerConfig {
            platforms: vec!["beatport".into(), "traxsource".into()],
            ..Default::default()
        };
        for provider in crate::Tagger::provider_options() {
            options
                .custom
                .0
                .insert(provider.id, provider.custom_options.get_defaults());
        }
        Self {
            solver: crate::SolverConfig::default(),
            recognition: crate::RecognitionConfig::default(),
            options,
            no_match: NoMatchPolicy::default(),
            analysis_tag_source: AnalysisTagSource::default(),
            write_energy: true,
            required_tags: vec![],
        }
    }
}
impl AutotagConfig {
    pub fn validate(&self) -> Result<()> {
        self.solver.validate()?;
        self.recognition.validate()?;
        let c = &self.options;
        ensure!(
            self.required_tags.iter().all(|t| c.tags.contains(t)),
            "Required fields must be selected"
        );
        ensure!(
            self.required_tags
                .iter()
                .enumerate()
                .all(|(i, t)| !self.required_tags[..i].contains(t)),
            "Duplicate required field"
        );
        ensure!(
            c.track_number_leading_zeroes <= 8,
            "Track number padding must be between 0 and 8"
        );
        if let Some(language) = &c.id3_comm_lang {
            ensure!(
                language.len() == 3 && language.bytes().all(|b| b.is_ascii_alphabetic()),
                "ID3 comment language must contain three ASCII letters"
            );
        }
        ensure!(
            c.album_tagging_ratio.is_finite() && (0.0..=1.0).contains(&c.album_tagging_ratio),
            "Invalid album matching ratio"
        );
        ensure!(
            !c.platforms.is_empty() && c.platforms.len() <= crate::ProviderId::ALL.len(),
            "Choose at least one supported metadata provider"
        );
        let mut seen = std::collections::HashSet::new();
        for id in &c.platforms {
            ensure!(
                crate::ProviderId::parse(id).is_some() && seen.insert(id),
                "Unsupported or duplicate provider"
            );
        }
        for (id, value) in &c.custom.0 {
            let provider = crate::ProviderId::parse(id)
                .ok_or_else(|| anyhow::anyhow!("Unsupported provider options"))?;
            let values = value
                .as_object()
                .ok_or_else(|| anyhow::anyhow!("Provider options must be an object"))?;
            if !matches!(
                provider,
                crate::ProviderId::Beatport | crate::ProviderId::Traxsource
            ) {
                ensure!(
                    values.is_empty(),
                    "Provider credentials and unsupported options cannot be saved in preparation settings"
                );
            }
        }
        ensure!(
            c.strictness.is_finite() && (0.0..=1.0).contains(&c.strictness),
            "Matching strictness must be between 0 and 1"
        );
        ensure!(
            (1..=64).contains(&c.threads),
            "Tagging concurrency must be between 1 and 64"
        );
        ensure!(
            c.post_command.is_none()
                && c.spotify.is_none()
                && !c.enable_shazam
                && !c.force_shazam
                && !c.move_success
                && !c.move_failed
                && !c.write_lrc
                && !c.enhanced_lrc,
            "Excluded tools are not available in automatic preparation"
        );
        if c.parse_filename {
            filename_template(
                c.filename_template
                    .as_deref()
                    .unwrap_or("%artists% - %title%"),
            )?;
        }
        if let Some(pattern) = &c.title_regex {
            regex::Regex::new(pattern)?;
        }
        if c.platforms.iter().any(|p| p == "beatport") {
            let value = c
                .custom
                .get("beatport")
                .ok_or_else(|| anyhow::anyhow!("Missing Beatport options"))?;
            let pages = value["max_pages"].as_u64().unwrap_or(0);
            let resolution = value["art_resolution"].as_u64().unwrap_or(0);
            ensure!(
                (1..=10).contains(&pages),
                "Beatport pages must be between 1 and 10"
            );
            ensure!(
                (200..=1600).contains(&resolution) && resolution.is_multiple_of(100),
                "Invalid artwork resolution"
            );
            ensure!(
                value["ignore_version"].is_boolean(),
                "Invalid version-matching option"
            );
        }
        Ok(())
    }
}
// Adapted from OneTagger AudioFileInfoImpl::parse_template; GPLv3.
pub(crate) fn filename_template(template: &str) -> Result<regex::Regex> {
    let mut text = template.to_owned();
    for c in ".+*$^()[]/|".chars() {
        text = text.replace(c, &format!("\\{c}"));
    }
    text = text
        .replace("%title%", "(?P<title>.+?)")
        .replace("%artist%", "(?P<artists>.+?)")
        .replace("%artists%", "(?P<artists>.+?)");
    text = regex::Regex::new("%[a-zA-Z0-9 ]+%")?
        .replace_all(&text, "(.+)")
        .into_owned();
    Ok(regex::Regex::new(&format!(
        "{}\\.[a-zA-Z0-9]{{2,4}}$",
        text.trim()
    ))?)
}

#[cfg(test)]
mod credential_settings_tests {
    use super::*;
    #[test]
    fn personal_secrets_are_not_accepted_as_provider_settings() {
        let mut c = AutotagConfig::default();
        c.options.custom.0.insert(
            "sonovault".into(),
            serde_json::json!({"api_key":"synthetic-do-not-save"}),
        );
        assert!(c.validate().is_err());
        c.options
            .custom
            .0
            .insert("sonovault".into(), serde_json::json!({}));
        assert!(c.validate().is_ok());
        c.options.platforms = vec!["spotify".into()];
        assert!(c.validate().is_err());
    }
}
