//! Independent, deterministic metadata cleanup; no I/O or musical analysis.
mod fixes;
mod mappings;
mod model;
use anyhow::Result;
pub use model::*;

pub fn apply(config: &CleanupConfig, input: &Metadata) -> Result<CleanupReport> {
    config.validate()?;
    input.validate()?;
    let mut metadata = input.clone();
    let mut applied_rules = Vec::new();
    for fix in &config.fixes {
        if fix.enabled {
            let before = metadata.clone();
            fixes::apply_fix(fix, config, &mut metadata)?;
            if metadata != before {
                applied_rules.push(format!("fix:{}", fix.id));
            }
        }
    }
    let before_mappings = metadata.clone();
    mappings::apply(config, &mut metadata)?;
    if before_mappings.fields.get(&MetadataField::Genre)
        != metadata.fields.get(&MetadataField::Genre)
        || before_mappings.custom_tags != metadata.custom_tags
    {
        applied_rules.push("genres".into());
    }
    if config
        .artists
        .roles
        .iter()
        .any(|role| before_mappings.fields.get(&role.field()) != metadata.fields.get(&role.field()))
    {
        applied_rules.push("artists".into());
    }
    let changes = MetadataField::ALL
        .iter()
        .filter_map(|field| {
            let before = input.fields.get(field).cloned().unwrap_or_default();
            let after = metadata.fields.get(field).cloned().unwrap_or_default();
            (before != after).then_some(FieldChange {
                field: *field,
                before,
                after,
            })
        })
        .collect();
    Ok(CleanupReport {
        original: input.clone(),
        metadata,
        changes,
        applied_rules,
    })
}
