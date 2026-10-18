mod extract;
mod remove;
mod text;
use crate::*;
use anyhow::Result;
pub(crate) fn apply_fix(
    fix: &ConfiguredFix,
    config: &CleanupConfig,
    metadata: &mut Metadata,
) -> Result<()> {
    let field = fix.rule.target();
    if matches!(
        fix.rule,
        SmartFix::ExtractArtist { .. } | SmartFix::ExtractRemixer { .. }
    ) {
        let Some(title) = metadata.first(MetadataField::Title).map(str::to_owned) else {
            return Ok(());
        };
        if fix.ignored_values.contains(&title) {
            return Ok(());
        }
        match &fix.rule {
            SmartFix::ExtractArtist { separator, part }
                if !metadata
                    .fields
                    .get(&MetadataField::Artist)
                    .is_some_and(|v| v.iter().any(|s| !s.is_empty())) =>
            {
                if let Some((artist, title)) = extract::artist(&title, separator, *part)? {
                    metadata.set(
                        MetadataField::Artist,
                        if artist.is_empty() {
                            vec![]
                        } else {
                            vec![artist]
                        },
                    );
                    set_first(metadata, MetadataField::Title, title);
                }
            }
            SmartFix::ExtractRemixer { word, remove_title }
                if !metadata
                    .fields
                    .get(&MetadataField::Remixer)
                    .is_some_and(|v| v.iter().any(|s| !s.is_empty())) =>
            {
                if let Some((remixer, title)) = extract::remixer(&title, word, *remove_title)? {
                    metadata.set(MetadataField::Remixer, vec![remixer]);
                    set_first(metadata, MetadataField::Title, title);
                }
            }
            _ => {}
        }
        return Ok(());
    }
    let before = metadata.fields.get(&field).cloned().unwrap_or_default();
    let mut after = Vec::new();
    for value in before {
        if fix.ignored_values.contains(&value) || config.protects_text(field, &value) {
            after.push(value);
            continue;
        }
        let result = match &fix.rule {
            SmartFix::TitleCase {
                uppercase,
                lowercase,
                ..
            } => Some(text::casing(&value, *uppercase, *lowercase)),
            SmartFix::ReplaceCharacters { characters } => {
                Some(text::replace_characters(&value, characters))
            }
            SmartFix::DecodeEntities => Some(text::entities(&value)),
            SmartFix::RemoveGarbage { .. } => Some(text::garbage(&value)),
            SmartFix::RemoveNumberPrefix => Some(remove::number_prefix(&value)),
            SmartFix::RemoveUrls { clear_field, .. } => match remove::urls(&value)? {
                Some(_) if *clear_field => None,
                Some(clean) => Some(clean),
                None => Some(value.clone()),
            },
            SmartFix::MixParentheses => Some(extract::mix_parentheses(&value)?),
            SmartFix::RemoveCommonText { pattern } => Some(remove::common(&value, *pattern)?),
            SmartFix::RemovePromotionalText { .. } => Some(remove::promotional(&value)?),
            SmartFix::ExtractArtist { .. } | SmartFix::ExtractRemixer { .. } => unreachable!(),
        };
        if let Some(result) = result
            && (!result.is_empty() || value.is_empty())
        {
            after.push(result);
        }
    }
    metadata.set(field, after);
    Ok(())
}
fn set_first(metadata: &mut Metadata, field: MetadataField, value: String) {
    let mut values = metadata.fields.get(&field).cloned().unwrap_or_default();
    if values.is_empty() {
        if !value.is_empty() {
            values.push(value)
        }
    } else if value.is_empty() {
        values.remove(0);
    } else {
        values[0] = value;
    }
    metadata.set(field, values);
}
