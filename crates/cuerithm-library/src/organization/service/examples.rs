use super::*;
use anyhow::{Result, ensure};
use cuerithm_organize::{
    paths::PathRules,
    template::{MissingPolicy, NamingContext, Rendered, Template},
};
impl OrganizationSettings {
    pub fn enabled(&self) -> bool {
        self.mode != OrganizationSettingMode::LeaveInPlace
    }
    pub fn validate(&self) -> Result<()> {
        if !self.enabled() {
            return Ok(());
        }
        Template::parse(&self.pattern).map_err(anyhow::Error::msg)?;
        ensure!(
            self.mode != OrganizationSettingMode::RenameInPlace || !self.pattern.contains('/'),
            "Rename in place cannot create folders; choose Move instead"
        );
        ensure!(
            self.collision != OrganizationCollisionPolicy::Skip || self.organization_optional,
            "Only optional organization may skip collisions"
        );
        ensure!(
            self.missing != OrganizationMissingPolicy::SkipOptional || self.organization_optional,
            "Only optional organization may skip missing values"
        );
        if matches!(
            self.mode,
            OrganizationSettingMode::Move | OrganizationSettingMode::Copy
        ) {
            ensure!(
                self.destination_root_id
                    .as_ref()
                    .is_some_and(|id| !id.is_empty()),
                "Choose a destination folder"
            );
        }
        Ok(())
    }
    pub(crate) fn missing_policy(&self) -> MissingPolicy {
        match self.missing {
            OrganizationMissingPolicy::Block => MissingPolicy::Block,
            OrganizationMissingPolicy::OmitOptional => MissingPolicy::OmitOptional,
            OrganizationMissingPolicy::SkipOptional => MissingPolicy::SkipOptional,
        }
    }
}
impl Library {
    pub fn organization_example(
        &self,
        settings: OrganizationSettings,
    ) -> Result<OrganizationExample> {
        let tokens = [
            "artist",
            "artists",
            "title",
            "album",
            "album_artist",
            "album_artists",
            "genre",
            "genres",
            "label",
            "version",
            "release_date",
            "year",
            "key",
            "bpm",
            "energy",
            "track",
            "disc",
            "original_name",
            "parent",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        let template = match Template::parse(&settings.pattern) {
            Ok(template) => template,
            Err(error) => {
                return Ok(OrganizationExample {
                    valid: false,
                    error: Some(error),
                    tokens,
                    examples: vec![],
                });
            }
        };
        let contexts = [
            (
                "Complete title",
                NamingContext {
                    artists: vec!["Example Artist".into()],
                    title: Some("Example (Club Mix)".into()),
                    version: Some("Club Mix".into()),
                    album: Some("Example Album".into()),
                    genres: vec!["House".into()],
                    key: Some("8A".into()),
                    bpm_hundredths: Some(12600),
                    energy: Some(7),
                    original_name: Some("example".into()),
                    parent: Some("Incoming".into()),
                    ..Default::default()
                },
            ),
            (
                "Missing artist",
                NamingContext {
                    title: Some("Another Example".into()),
                    original_name: Some("another-example".into()),
                    ..Default::default()
                },
            ),
        ];
        let examples = contexts
            .into_iter()
            .map(|(label, context)| {
                let original_name = format!(
                    "{}.flac",
                    context.original_name.as_deref().unwrap_or("example")
                );
                let (result, outcome, reason) = match template.render(
                    &context,
                    settings.missing_policy(),
                    settings.organization_optional,
                    PathRules::Portable,
                ) {
                    Ok(Rendered::Path(value)) => (Some(format!("{value}.flac")), "ready", None),
                    Ok(Rendered::Skipped { missing }) => (
                        None,
                        "skipped",
                        Some(format!("Missing naming field: {missing}")),
                    ),
                    Err(error) => (None, "blocked", Some(error)),
                };
                OrganizationExampleRow {
                    label: label.into(),
                    original_name,
                    result,
                    outcome: outcome.into(),
                    reason,
                }
            })
            .collect();
        let error = settings.validate().err().map(|e| format!("{e:#}"));
        Ok(OrganizationExample {
            valid: error.is_none(),
            error,
            tokens,
            examples,
        })
    }
}
