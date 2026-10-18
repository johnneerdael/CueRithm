use super::text;
use anyhow::Result;
use regex::{Regex, RegexBuilder};
use unicode_normalization::UnicodeNormalization;
pub(super) fn artist(title: &str, separator: &str, part: u16) -> Result<Option<(String, String)>> {
    let pattern = if separator == " - " {
        format!("{}|{}", regex::escape(separator), regex::escape(" – "))
    } else {
        regex::escape(separator)
    };
    let regex = Regex::new(&pattern)?;
    if !regex.is_match(title) {
        return Ok(None);
    }
    let parts: Vec<_> = regex.split(title).collect();
    let part = usize::from(part);
    if part > parts.len() {
        return Ok(None);
    }
    let artist = parts[part - 1].trim().to_owned();
    let title = if parts.len() > part {
        parts[part..].join(separator)
    } else {
        parts[..part - 1].join(separator)
    };
    Ok(Some((artist, title.trim().to_owned())))
}
fn generic_mix(value: &str) -> bool {
    let value = value.to_lowercase();
    if value == "bootleg" {
        return true;
    }
    let prefixes = [
        "original", "extended", "vip", "club", "vocal", "intro", "main",
    ];
    if prefixes.iter().any(|prefix| value.starts_with(prefix)) && value.ends_with("mix") {
        return true;
    }
    [
        "dance",
        "dubstep",
        "moombahton",
        "bootleg",
        "progressive",
        "house",
        "tech",
        "12 inch",
        "12\"",
        "7 inch",
        "7\"",
        "extended vocal",
        "instrumental",
        "dub",
    ]
    .iter()
    .any(|prefix| value == format!("{prefix} mix") || value == format!("{prefix} remix"))
}
pub(super) fn remixer(
    title: &str,
    word: &str,
    update_title: bool,
) -> Result<Option<(String, String)>> {
    let word = word.trim();
    let pattern = format!(r"\(([^)]*){}\)", regex::escape(word));
    let regex = RegexBuilder::new(&pattern).case_insensitive(true).build()?;
    let Some(found) = regex.captures(title) else {
        return Ok(None);
    };
    let matched = found.get(0).unwrap();
    let prefix = found.get(1).unwrap().as_str();
    if !prefix.ends_with(' ') {
        return Ok(None);
    }
    let content = matched
        .as_str()
        .trim_start_matches('(')
        .trim_end_matches(')');
    if generic_mix(content) {
        return Ok(None);
    }
    let mut name = prefix.trim().to_owned();
    for qualifier in [
        "original",
        "extended",
        "vip",
        "club",
        "vocal",
        "intro",
        "main",
        "dance",
        "dubstep",
        "moombahton",
        "bootleg",
        "progressive",
        "house",
        "tech",
        "12 inch",
        "12\"",
        "7 inch",
        "7\"",
        "extended vocal",
        "instrumental",
        "dub",
    ] {
        let lower = name.to_lowercase();
        if lower.ends_with(qualifier) {
            let start = name.len() - qualifier.len();
            if start == 0 || name[..start].ends_with(' ') {
                name.truncate(start);
                name = name.trim().into();
                break;
            }
        }
    }
    if name.is_empty() {
        return Ok(None);
    }
    let name = name.nfc().collect();
    let clean = if update_title {
        text::garbage(&format!(
            "{}{}",
            &title[..matched.start()],
            &title[matched.end()..]
        ))
    } else {
        title.to_owned()
    };
    Ok(Some((name, clean)))
}
pub(super) fn mix_parentheses(title: &str) -> Result<String> {
    let suffix = if let Some(index) = title.find(" - ") {
        Some(index)
    } else {
        let lower = title.to_lowercase();
        ["original mix", "extended mix", "club mix", "radio edit"]
            .iter()
            .filter_map(|suffix| {
                lower
                    .ends_with(suffix)
                    .then_some(title.len() - suffix.len())
            })
            .min()
    };
    let Some(index) = suffix else {
        return Ok(title.to_owned());
    };
    let before = &title[..index];
    if before.matches('(').count() > before.matches(')').count() {
        return Ok(title.to_owned());
    }
    let descriptor = text::edges(&title[index..]);
    if descriptor.is_empty() {
        return Ok(title.to_owned());
    }
    let wrapped = format!("({descriptor})");
    let mut before = before.to_owned();
    let duplicate = RegexBuilder::new(&regex::escape(&wrapped))
        .case_insensitive(true)
        .build()?;
    before = duplicate.replacen(&before, 1, "").into_owned();
    let result = format!("{} {}", before.trim(), wrapped).trim().to_owned();
    let mut out = String::with_capacity(result.len());
    let mut previous = None;
    for c in result.chars() {
        if matches!(c, '(' | ')') && previous == Some(c) {
            continue;
        }
        out.push(c);
        previous = Some(c);
    }
    Ok(out)
}
