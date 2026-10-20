use cuerithm_smartlists::FieldValue;
use std::collections::BTreeMap;

pub(super) fn split_artists(value: &str) -> Vec<String> {
    let mut normalized = value.to_owned();
    for marker in [
        " featuring ",
        " feat. ",
        " feat ",
        " ft. ",
        " ft ",
        " vs. ",
        " vs ",
        " x ",
        " & ",
    ] {
        loop {
            let lower = normalized.to_lowercase();
            let Some(at) = lower.find(marker) else { break };
            // Find byte offsets in the original string, including Unicode case changes.
            let start = normalized
                .char_indices()
                .find(|(i, _)| normalized[..*i].to_lowercase().len() == at)
                .map(|(i, _)| i);
            let Some(start) = start else { break };
            let end = start + marker.len();
            normalized.replace_range(start..end, ";");
        }
    }
    unique(normalized.split([',', ';', '\n']).map(str::to_owned))
}
pub(super) fn unique(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut names = BTreeMap::new();
    for value in values {
        let name = value.trim();
        if !name.is_empty() {
            names
                .entry(name.to_lowercase())
                .or_insert_with(|| name.to_owned());
        }
    }
    names.into_values().collect()
}
pub(super) fn artists(artist: &str, remixer: &str, title: &str) -> Vec<String> {
    let mut values = split_artists(artist);
    if !remixer.trim().is_empty() {
        values.extend(split_artists(remixer));
    } else {
        for credit in title.split(['(', '[']).skip(1) {
            let credit = credit.split([')', ']']).next().unwrap_or("").trim();
            let lower = credit.to_lowercase();
            if lower.ends_with(" remix") {
                let mut name = &credit[..credit.len() - " remix".len()];
                for suffix in [" extended", " radio", " club"] {
                    if name.to_lowercase().ends_with(suffix) {
                        name = &name[..name.len() - suffix.len()];
                    }
                }
                if !matches!(
                    name.to_lowercase().as_str(),
                    "original" | "extended" | "radio" | "club"
                ) {
                    values.extend(split_artists(name));
                }
            }
        }
    }
    unique(values)
}
pub(super) fn release_month(tags: &BTreeMap<String, String>) -> Option<u32> {
    [
        "releasedate",
        "release_date",
        "release date",
        "release_time",
        "releasetime",
        "tdrl",
        "date",
    ]
    .iter()
    .filter_map(|key| tags.get(*key))
    .find_map(|value| {
        let date = value.trim();
        if date.len() != 7 && date.len() != 10 {
            return None;
        }
        if date.as_bytes().get(4) != Some(&b'-') {
            return None;
        }
        let year = date.get(..4)?.parse::<u32>().ok()?;
        let month = date.get(5..7)?.parse::<u32>().ok()?;
        if year == 0 || !(1..=12).contains(&month) {
            return None;
        }
        let valid = if date.len() == 7 {
            format!("{date}-01")
        } else {
            date.to_owned()
        };
        cuerithm_smartlists::calendar_day_epoch(&valid, "UTC").ok()?;
        Some(year * 100 + month)
    })
}
pub(super) fn text(fields: &BTreeMap<String, FieldValue>, field: &str) -> String {
    match fields.get(field) {
        Some(FieldValue::Text(s)) => s.clone(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn named_remixers_support_unicode_and_explicit_tags_take_priority() {
        assert_eq!(
            artists("Alpha feat. Beta; ALPHA", "", "Track (İris Extended Remix)"),
            ["Alpha", "Beta", "İris"]
        );
        assert_eq!(
            artists("Alpha", "Gamma, Delta", "Track (Other Remix)"),
            ["Alpha", "Delta", "Gamma"]
        );
        assert_eq!(artists("Alpha", "", "Track (Original Mix)"), ["Alpha"]);
    }
    #[test]
    fn release_month_retains_precision_and_explicit_source_priority() {
        for unknown in [
            "2026",
            "2026-00",
            "2026-13",
            "2026-02-30",
            "2026-10-05T00:00:00Z",
            "２０２６-01",
        ] {
            assert_eq!(
                release_month(&BTreeMap::from([("date".into(), unknown.into())])),
                None,
                "{unknown}"
            );
        }
        for known in ["2026-02", "2026-02-28"] {
            assert_eq!(
                release_month(&BTreeMap::from([("date".into(), known.into())])),
                Some(202602)
            );
        }
        assert_eq!(
            release_month(&BTreeMap::from([
                ("release_time".into(), "2026-09-12".into()),
                ("date".into(), "2025-02-01".into())
            ])),
            Some(202609)
        );
    }
}
