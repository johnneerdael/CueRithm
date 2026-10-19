use super::*;
use cuerithm_smartlists::{
    CompiledSmartList, EvaluationContext, FieldKind, FieldValue, Operator, RuleValue,
    SmartDefinition, SmartRule,
};
use std::{collections::BTreeMap, sync::Arc};
mod templates;
#[cfg(test)]
mod tests;
use templates::Template;
const LIMIT: usize = 5000;
#[derive(Clone)]
struct Candidate {
    template: Template,
    members: BTreeSet<usize>,
}
fn field_of(dimension: &GenerationDimension) -> Option<&str> {
    match dimension {
        GenerationDimension::Field { field, .. } => Some(field),
        GenerationDimension::Decades => Some("year"),
        GenerationDimension::BpmRanges => Some("bpm"),
        GenerationDimension::BitrateBuckets => Some("bitrate"),
        GenerationDimension::SizeBuckets => Some("sizeBytes"),
        GenerationDimension::DurationBuckets => Some("duration"),
        _ => None,
    }
}
fn label(dimension: &GenerationDimension, snapshot: &snapshot::Snapshot) -> Result<String> {
    Ok(match dimension {
        GenerationDimension::Tags { .. } => "Tags".into(),
        GenerationDimension::Category { category_id } => snapshot
            .field_catalog
            .categories
            .iter()
            .find(|c| c.id == *category_id)
            .context("Unknown tag category")?
            .name
            .clone(),
        GenerationDimension::Decades => "Decades".into(),
        GenerationDimension::BpmRanges => "BPM ranges".into(),
        GenerationDimension::BitrateBuckets => "Bitrate".into(),
        GenerationDimension::SizeBuckets => "Size".into(),
        GenerationDimension::DurationBuckets => "Duration".into(),
        GenerationDimension::Field { field, .. } => match field.as_str() {
            "genre" => "Genre",
            "artist" => "Artist",
            "album" => "Album",
            "albumArtist" => "Album artist",
            "key" => "Key",
            "energy" => "Energy",
            "rating" => "Rating",
            "year" => "Year",
            _ => field,
        }
        .into(),
    })
}
fn observed(snapshot: &snapshot::Snapshot, field: &str) -> Result<Vec<FieldValue>> {
    let descriptor = snapshot
        .field_catalog
        .catalog
        .fields
        .get(field)
        .context("Unknown generation field")?;
    ensure!(
        descriptor.unavailable_reason.is_none(),
        "Generation field unavailable: {}",
        descriptor
            .unavailable_reason
            .as_deref()
            .unwrap_or("unknown")
    );
    ensure!(
        matches!(
            descriptor.kind,
            FieldKind::Text | FieldKind::Number | FieldKind::Color
        ),
        "Choose a text, numeric or color generation field"
    );
    let probe = templates::definition(vec![SmartRule {
        id: String::new(),
        field: field.into(),
        operator: Operator::Exists,
        value: RuleValue::None,
        or: false,
    }]);
    let mut values = BTreeMap::new();
    for row in snapshot.tracks.iter().filter(|r| r.prepared()) {
        evaluate::blocked(&probe, snapshot, row)?;
        let value = if field == "key" {
            row.track
                .key
                .as_deref()
                .map(|k| {
                    cuerithm_smartlists::key_to_camelot(k)
                        .map(FieldValue::Text)
                        .context("Invalid measured key")
                })
                .transpose()?
                .unwrap_or(FieldValue::Null)
        } else if field == "fileType" {
            row.track.value("fileTypeLabel")
        } else {
            row.track.value(field)
        };
        let (key, value) = match value {
            FieldValue::Text(s) => {
                let s = cuerithm_smartlists::javascript_trim(&s).to_string();
                if s.is_empty() {
                    continue;
                }
                (format!("t:{}", s.to_lowercase()), FieldValue::Text(s))
            }
            FieldValue::Color(s) => {
                if s.is_empty() {
                    continue;
                }
                (format!("c:{s}"), FieldValue::Color(s))
            }
            FieldValue::Number(n) => {
                ensure!(n.is_finite(), "Nonfinite observed value");
                let n = if field == "bpm" {
                    cuerithm_smartlists::comparison_bpm(n)?
                } else {
                    n
                };
                if field == "energy" {
                    ensure!(
                        (0. ..=10.).contains(&n) && n.fract() == 0.,
                        "Invalid measured energy"
                    );
                }
                (
                    format!(
                        "n:{:016x}",
                        if n == 0. { 0f64.to_bits() } else { n.to_bits() }
                    ),
                    FieldValue::Number(n),
                )
            }
            FieldValue::Null => continue,
            _ => anyhow::bail!("Non-scalar generation value"),
        };
        values.entry(key).or_insert(value);
        ensure!(
            values.len() <= LIMIT,
            "Too many observed values; choose a narrower field"
        );
    }
    let mut result = values.into_values().collect::<Vec<_>>();
    if descriptor.kind == FieldKind::Number {
        result.sort_by(|a, b| match (a, b) {
            (FieldValue::Number(a), FieldValue::Number(b)) => a.total_cmp(b),
            _ => std::cmp::Ordering::Equal,
        });
    }
    Ok(result)
}
fn build(snapshot: &snapshot::Snapshot, dimension: &GenerationDimension) -> Result<Vec<Template>> {
    if let Some(field) = field_of(dimension) {
        ensure!(
            !matches!(field, "plays" | "playCount" | "lastPlayed" | "danceability"),
            "This metric is excluded"
        );
    }
    match dimension {
        GenerationDimension::Field {
            field,
            include_unobserved,
        } => {
            let values = observed(snapshot, field)?;
            let values = if *include_unobserved {
                match field.as_str() {
                    "energy" => (0..=10).map(|n| FieldValue::Number(n as f64)).collect(),
                    "rating" => (0..=5).map(|n| FieldValue::Number(n as f64)).collect(),
                    _ => anyhow::bail!("This field has no bounded full domain"),
                }
            } else {
                values
            };
            templates::scalar(field, values)
        }
        GenerationDimension::Tags { tag_ids } => {
            let labels = snapshot
                .field_catalog
                .tags
                .iter()
                .map(|t| (t.id.clone(), t.name.clone()))
                .collect::<BTreeMap<_, _>>();
            let observed = snapshot
                .tracks
                .iter()
                .filter(|r| r.prepared())
                .flat_map(|r| r.track.tags.iter().cloned())
                .collect::<BTreeSet<_>>();
            let ids = tag_ids
                .clone()
                .unwrap_or_else(|| observed.into_iter().collect());
            ensure!(
                ids.iter().collect::<BTreeSet<_>>().len() == ids.len()
                    && ids.iter().all(|id| labels.contains_key(id)),
                "Unknown or duplicate selected tag"
            );
            Ok(templates::tags(&ids, &labels))
        }
        GenerationDimension::Category { category_id } => {
            let ids = snapshot
                .field_catalog
                .catalog
                .categories
                .get(category_id)
                .context("Unknown tag category")?
                .iter()
                .cloned()
                .collect::<Vec<_>>();
            let labels = snapshot
                .field_catalog
                .tags
                .iter()
                .map(|t| (t.id.clone(), t.name.clone()))
                .collect();
            Ok(templates::tags(&ids, &labels))
        }
        _ => {
            let field = field_of(dimension).unwrap();
            let values = observed(snapshot, field)?
                .into_iter()
                .map(|v| match v {
                    FieldValue::Number(n) => Ok(n),
                    _ => anyhow::bail!("Non-numeric bucket field"),
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(match dimension {
                GenerationDimension::Decades => templates::ranges(field, &values, true, 1.),
                // Numeric predicates compare BPM at two decimals, including bucket discovery.
                GenerationDimension::BpmRanges => templates::ranges(field, &values, false, 0.01),
                GenerationDimension::BitrateBuckets => {
                    templates::buckets(field, &values, &[128., 192., 256., 320.], 1., "kbps")
                }
                GenerationDimension::SizeBuckets => templates::buckets(
                    field,
                    &values,
                    &[
                        5. * 1048576.,
                        10. * 1048576.,
                        25. * 1048576.,
                        50. * 1048576.,
                    ],
                    1048576.,
                    "MB",
                ),
                GenerationDimension::DurationBuckets => {
                    templates::buckets(field, &values, &[120., 240., 360., 480., 600.], 60., "min")
                }
                _ => unreachable!(),
            })
        }
    }
}
fn candidates(snapshot: &snapshot::Snapshot, templates: Vec<Template>) -> Result<Vec<Candidate>> {
    ensure!(
        templates.len() <= LIMIT,
        "Generation would exceed 5000 nodes"
    );
    let catalog = Arc::new(snapshot.field_catalog.catalog.clone());
    let mut context = EvaluationContext::new(snapshot.now, &snapshot.zone)?;
    context.key_mixing_mode = snapshot.preferences.key_mixing_mode;
    context.protected_genres = snapshot.protected.clone();
    templates
        .into_iter()
        .map(|template| {
            let compiled = CompiledSmartList::new(template.definition.clone(), catalog.clone())?;
            let mut members = BTreeSet::new();
            for (i, row) in snapshot
                .tracks
                .iter()
                .enumerate()
                .filter(|(_, r)| r.prepared())
            {
                evaluate::blocked(&template.definition, snapshot, row)?;
                if compiled.matches(&row.track, &context)? {
                    members.insert(i);
                }
            }
            Ok(Candidate { template, members })
        })
        .collect()
}
fn folder(
    tree: &mut Tree,
    parent: Option<String>,
    name: String,
    created: &mut Vec<String>,
    reuse: bool,
) -> Result<String> {
    ensure!(
        !name.trim().is_empty() && name.chars().count() <= 255,
        "Folder names need 1–255 characters"
    );
    let matching = tree::children(tree, &parent, true)
        .into_iter()
        .filter(|id| tree[id].node.name == name)
        .collect::<Vec<_>>();
    if reuse && matching.len() == 1 && tree[&matching[0]].node.kind == PlaylistKind::Folder {
        return Ok(matching[0].clone());
    }
    ensure!(
        matching.is_empty(),
        "A conflicting destination name already exists: {name}"
    );
    tools::insert(
        tree,
        parent,
        PlaylistKind::Folder,
        name,
        vec![],
        created,
        None,
    )
}
fn same_definition(a: &SmartDefinition, b: &SmartDefinition) -> bool {
    a.match_all == b.match_all
        && a.rules.len() == b.rules.len()
        && a.rules.iter().zip(&b.rules).all(|(a, b)| {
            let same_value = match (&a.value, &b.value, a.operator) {
                (RuleValue::Text(a), RuleValue::Text(b), Operator::StringEquals) => {
                    a.to_lowercase() == b.to_lowercase()
                }
                _ => a.value == b.value,
            };
            a.field == b.field && a.operator == b.operator && same_value && a.or == b.or
        })
}
impl Library {
    pub fn preview_smart_generation(
        &self,
        request: SmartGenerationRequest,
    ) -> Result<GenerationPreview> {
        ensure!(
            !request.dimensions.is_empty() && request.dimensions.len() <= 16,
            "Choose 1–16 generation dimensions"
        );
        let unique = request
            .dimensions
            .iter()
            .map(|d| match d {
                GenerationDimension::Field { field, .. } => Ok(format!("field:{field}")),
                _ => serde_json::to_string(d),
            })
            .collect::<serde_json::Result<BTreeSet<_>>>()?;
        ensure!(
            unique.len() == request.dimensions.len(),
            "Duplicate generation dimension"
        );
        let probe = templates::definition(
            request
                .dimensions
                .iter()
                .filter_map(field_of)
                .map(|field| SmartRule {
                    id: String::new(),
                    field: field.into(),
                    operator: Operator::Exists,
                    value: RuleValue::None,
                    or: false,
                })
                .collect(),
        );
        let now = crate::now();
        let snapshot = self.smart_generation_snapshot(now, &probe)?;
        let mut groups = vec![];
        for dimension in &request.dimensions {
            groups.push((
                label(dimension, &snapshot)?,
                candidates(&snapshot, build(&snapshot, dimension)?)?,
            ));
        }
        groups.retain(|(_, items)| !items.is_empty());
        if request.combine && !groups.is_empty() {
            let mut combined = groups.remove(0).1;
            for (_, group) in groups.drain(..) {
                ensure!(
                    combined
                        .len()
                        .checked_mul(group.len())
                        .is_some_and(|n| n <= LIMIT),
                    "Cartesian generation would exceed 5000 nodes"
                );
                let mut next = vec![];
                for left in &combined {
                    for right in &group {
                        next.push(Candidate {
                            template: templates::and(&left.template, &right.template),
                            members: left.members.intersection(&right.members).copied().collect(),
                        });
                    }
                }
                combined = next;
            }
            groups = vec![(String::new(), combined)];
        }
        ensure!(
            groups.iter().map(|(_, g)| g.len()).sum::<usize>() <= LIMIT,
            "Generation would exceed 5000 nodes"
        );
        let mut after = snapshot.tree.clone();
        let mut created = vec![];
        let has_items = !groups.is_empty();
        let folder_id = match request.target {
            GenerationTarget::Folder { id } => {
                ensure!(
                    tree::active(&after, &id)?.node.kind == PlaylistKind::Folder,
                    "Destination is not a folder"
                );
                Some(id)
            }
            GenerationTarget::NewFolder { parent_id, name } => {
                ensure!(
                    !name.trim().is_empty() && name.chars().count() <= 255,
                    "Folder names need 1–255 characters"
                );
                if let Some(p) = &parent_id {
                    ensure!(
                        tree::active(&after, p)?.node.kind == PlaylistKind::Folder,
                        "Parent is not a folder"
                    );
                }
                if has_items {
                    Some(folder(&mut after, parent_id, name, &mut created, false)?)
                } else {
                    None
                }
            }
        };
        let separate = groups.len() > 1 && !request.combine;
        let mut items = vec![];
        for (name, group) in groups {
            let parent = if separate {
                folder(&mut after, folder_id.clone(), name, &mut created, true)?
            } else {
                folder_id.clone().unwrap()
            };
            let mut names = BTreeSet::new();
            for candidate in group {
                ensure!(
                    names.insert(candidate.template.name.clone()),
                    "Generated names conflict: {}",
                    candidate.template.name
                );
                let mut name = candidate.template.name;
                let mut definition = candidate.template.definition;
                definition.validate_shape()?;
                let matching = tree::children(&after, &Some(parent.clone()), true)
                    .into_iter()
                    .filter(|id| after[id].node.name.to_lowercase() == name.to_lowercase())
                    .collect::<Vec<_>>();
                let (id, is_new) = if matching.is_empty() {
                    (
                        tools::insert(
                            &mut after,
                            Some(parent.clone()),
                            PlaylistKind::Smartlist,
                            name.clone(),
                            vec![],
                            &mut created,
                            Some(definition.clone()),
                        )?,
                        true,
                    )
                } else {
                    ensure!(matching.len() == 1, "Ambiguous generated name: {name}");
                    let id = matching[0].clone();
                    let node = &after[&id].node;
                    ensure!(
                        node.kind == PlaylistKind::Smartlist
                            && node
                                .definition
                                .as_ref()
                                .is_some_and(|d| same_definition(d, &definition)),
                        "Existing playlist has a different definition: {name}"
                    );
                    name = node.name.clone();
                    definition = node.definition.as_ref().unwrap().clone();
                    (id, false)
                };
                items.push(GeneratedSmartList {
                    id,
                    parent_id: parent.clone(),
                    name,
                    definition,
                    track_count: candidate.members.len(),
                    is_new,
                });
            }
        }
        tree::normalize(&mut after);
        tree::validate(&after, &snapshot.all_ids)?;
        let new_nodes = created.len();
        let expires = snapshot::dynamic(&probe).then(|| now.saturating_add(60_000));
        let guard = previews::Guard {
            stamp: snapshot.field_catalog.stamp.clone(),
            data_bound: true,
            zone: Some(snapshot.zone.clone()),
            observations: snapshot.observations,
            created: now,
            expires,
            engine: env!("CUERITHM_SMART_ENGINE_ID").into(),
        };
        let saved = previews::save(self, "generation", guard, &snapshot.tree, &after, created)?;
        Ok(GenerationPreview {
            id: saved.id,
            stamp: saved.stamp,
            created: saved.created,
            expires: saved.expires,
            folder_id,
            new_nodes,
            items,
        })
    }
    pub fn apply_smart_generation(
        &self,
        id: &str,
        expected_revision: u64,
    ) -> Result<PlaylistChange> {
        previews::apply(self, "generation", id, expected_revision)
    }
}
