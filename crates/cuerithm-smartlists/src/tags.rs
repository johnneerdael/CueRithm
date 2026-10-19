use crate::{FieldCatalog, Operator, SmartTrack};
pub(crate) fn matches(
    track: &SmartTrack,
    catalog: &FieldCatalog,
    op: Operator,
    selected: &[String],
) -> bool {
    if selected.is_empty() {
        return false;
    }
    use Operator::*;
    let known: std::collections::BTreeSet<_> = track.tags.intersection(&catalog.tags).collect();
    match op {
        TagsIncludes => !known.is_empty() && selected.iter().all(|id| known.contains(id)),
        TagsNotIncludes => selected.iter().all(|id| !known.contains(id)),
        TagsAtleastOne => selected.iter().any(|id| known.contains(id)),
        TagsExact => {
            selected.len() == known.len()
                && !known.is_empty()
                && selected.iter().all(|id| known.contains(id))
        }
        TagCategoryNone | TagCategoryAtleastOneAnyCat | TagCategoryAtleastOneAllCAt => {
            let states: Vec<bool> = selected
                .iter()
                .map(|id| {
                    catalog.categories[id]
                        .iter()
                        .any(|tag| track.tags.contains(tag))
                })
                .collect();
            match op {
                TagCategoryNone => !states.iter().any(|v| *v),
                TagCategoryAtleastOneAnyCat => states.iter().any(|v| *v),
                _ => states.iter().all(|v| *v),
            }
        }
        _ => unreachable!(),
    }
}
