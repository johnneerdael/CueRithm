use crate::*;
use anyhow::{Result, ensure};
use std::collections::{BTreeMap, BTreeSet};
fn table(mappings: &[CanonicalMapping]) -> BTreeMap<&str, Option<&str>> {
    mappings
        .iter()
        .map(|m| (m.from.as_str(), m.to.as_deref().map(str::trim)))
        .collect()
}
pub(crate) fn validate(mappings: &[CanonicalMapping]) -> Result<()> {
    ensure!(mappings.len() <= 4096, "Too many canonical mappings");
    let mut names = BTreeSet::new();
    for mapping in mappings {
        ensure!(
            !mapping.from.is_empty() && mapping.from.len() <= 4096 && names.insert(&mapping.from),
            "Invalid or duplicate canonical source"
        );
        if let Some(target) = &mapping.to {
            ensure!(
                !target.trim().is_empty() && target.len() <= 4096,
                "Invalid canonical name"
            );
        }
    }
    let table = table(mappings);
    for start in table.keys() {
        let mut seen = BTreeSet::new();
        let mut value = *start;
        while let Some(Some(next)) = table.get(value) {
            if *next == value {
                break;
            }
            ensure!(seen.insert(value), "Cyclic canonical mappings");
            value = next;
        }
    }
    Ok(())
}
fn resolve(
    value: &str,
    mappings: &BTreeMap<&str, Option<&str>>,
    protected: &BTreeSet<String>,
) -> Option<String> {
    let mut current = value;
    for _ in 0..=mappings.len() {
        if protected.contains(current) {
            break;
        }
        match mappings.get(current) {
            Some(Some(next)) if *next != current => current = next,
            Some(None) => return None,
            _ => break,
        }
    }
    Some(current.to_owned())
}
fn unique(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter(|value| seen.insert(value.clone()))
        .collect()
}
pub(crate) fn apply(config: &CleanupConfig, metadata: &mut Metadata) -> Result<()> {
    if config.genres.enabled {
        let mappings = table(&config.genres.mappings);
        let before = metadata
            .fields
            .get(&MetadataField::Genre)
            .cloned()
            .unwrap_or_default();
        let mut after = Vec::new();
        let mut touched = false;
        for value in &before {
            if config.genres.protected_values.contains(value) {
                after.push(value.clone());
                continue;
            }
            if let Some(conversion) = config
                .genres
                .conversion
                .as_ref()
                .filter(|c| c.genres.contains(value))
            {
                if !value.trim().is_empty() {
                    let tag = CustomTag {
                        category: conversion.category.trim().into(),
                        value: value.trim().into(),
                    };
                    if !metadata.custom_tags.contains(&tag) {
                        metadata.custom_tags.push(tag);
                    }
                }
                touched = true;
                if conversion.remove_genre {
                    continue;
                }
            }
            let mapped = resolve(value, &mappings, &config.genres.protected_values);
            touched |= mapped.as_ref() != Some(value);
            if let Some(value) = mapped {
                after.push(value);
            }
        }
        if touched {
            metadata.set(MetadataField::Genre, unique(after));
        }
    }
    if config.artists.enabled {
        let mappings = table(&config.artists.mappings);
        for role in &config.artists.roles {
            let field = role.field();
            let before = metadata.fields.get(&field).cloned().unwrap_or_default();
            let after: Vec<_> = before
                .iter()
                .filter_map(|value| resolve(value, &mappings, &config.artists.protected_values))
                .collect();
            if before != after {
                metadata.set(field, unique(after));
            }
        }
    }
    Ok(())
}
