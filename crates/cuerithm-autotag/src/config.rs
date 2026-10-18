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
    pub options: TaggerConfig,
    pub no_match: NoMatchPolicy,
    pub analysis_tag_source: AnalysisTagSource,
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
            options,
            no_match: NoMatchPolicy::default(),
            analysis_tag_source: AnalysisTagSource::default(),
        }
    }
}
impl AutotagConfig {
    pub fn validate(&self) -> Result<()> {
        let c = &self.options;
        ensure!(
            !c.platforms.is_empty() && c.platforms.len() <= 2,
            "Choose Beatport and/or Traxsource"
        );
        let mut seen = std::collections::HashSet::new();
        for id in &c.platforms {
            ensure!(
                (id == "beatport" || id == "traxsource") && seen.insert(id),
                "Unsupported or duplicate provider"
            );
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
