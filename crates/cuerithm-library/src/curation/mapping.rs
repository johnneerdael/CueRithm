use super::*;
use anyhow::{Context, Result};
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct FrozenMapping {
    pub mapping: CurationMapping,
    pub values: Vec<String>,
}
pub(super) fn freeze(
    catalog: &CurationCatalog,
    after: &CurationValues,
    before: &CurationValues,
    all: bool,
) -> Result<Vec<FrozenMapping>> {
    catalog
        .mappings
        .iter()
        .filter(|mapping| mapping.enabled)
        .filter(|mapping| {
            all || match &mapping.source {
                CurationMappingSource::Group { group_id } => {
                    after.categories.get(group_id) != before.categories.get(group_id)
                }
                CurationMappingSource::Rating => after.user_rating != before.user_rating,
                CurationMappingSource::Notes => after.notes != before.notes,
            }
        })
        .map(|mapping| {
            let values = match &mapping.source {
                CurationMappingSource::Group { group_id } => {
                    let selected = after.categories.get(group_id).cloned().unwrap_or_default();
                    if selected.is_empty() {
                        vec![]
                    } else {
                        let group = catalog
                            .groups
                            .iter()
                            .find(|group| group.id == *group_id)
                            .context("Mapped category was retired")?;
                        group
                            .values
                            .iter()
                            .filter(|value| selected.contains(&value.id))
                            .map(|value| value.name.clone())
                            .collect()
                    }
                }
                CurationMappingSource::Rating => after
                    .user_rating
                    .map(|value| vec![value.to_string()])
                    .unwrap_or_default(),
                CurationMappingSource::Notes => {
                    if after.notes.is_empty() {
                        vec![]
                    } else {
                        vec![after.notes.clone()]
                    }
                }
            };
            Ok(FrozenMapping {
                mapping: mapping.clone(),
                values,
            })
        })
        .collect()
}

/// Read the bounded native evidence chain only for an explicit re-export.
pub(super) fn owned(
    service: &CurationService,
    mut id: Option<String>,
) -> Result<Vec<FrozenMapping>> {
    use rusqlite::OptionalExtension;
    let mut seen = std::collections::BTreeSet::new();
    let mut operations = std::collections::BTreeSet::new();
    let mut result = Vec::new();
    let mut budget = 0usize;
    while let Some(current) = id {
        anyhow::ensure!(
            operations.insert(current.clone()),
            "Curate native evidence chain contains a cycle"
        );
        let row: Option<(String, Option<String>)> = service
            .library
            .conn()?
            .query_row(
                "SELECT mappings_json,predecessor_id FROM curation_publications WHERE id=?1",
                [current],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((text, prior)) = row else {
            anyhow::bail!("Missing Curate native predecessor evidence")
        };
        budget = budget.saturating_add(text.len());
        anyhow::ensure!(
            budget <= super::storage::MAX_JSON,
            "Curate native evidence exceeds64MiB; inspect its retained history"
        );
        for frozen in serde_json::from_str::<Vec<FrozenMapping>>(&text)? {
            for family in &frozen.mapping.containers {
                let key = (
                    super::taxonomy::target_key(&frozen.mapping)?,
                    serde_json::to_string(family)?,
                );
                if seen.insert(key) {
                    let mut record = frozen.clone();
                    record.mapping.containers = vec![*family];
                    result.push(record);
                }
            }
        }
        id = prior;
    }
    Ok(result)
}
