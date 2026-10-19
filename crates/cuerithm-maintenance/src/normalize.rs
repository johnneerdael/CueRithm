use crate::Tolerance;
use regex::{Regex, RegexBuilder};
use std::{
    collections::{HashMap, HashSet},
    sync::LazyLock,
};
use unicode_normalization::UnicodeNormalization;
static COLLATOR: LazyLock<icu_collator::CollatorBorrowed<'static>> = LazyLock::new(|| {
    icu_collator::Collator::try_new(Default::default(), Default::default())
        .expect("Pinned root collator")
});

static MAP: LazyLock<HashMap<char, String>> = LazyLock::new(|| {
    let values: HashMap<String, String> =
        serde_json::from_str(include_str!("../../../vendor/slugify/char-map.json"))
            .expect("Pinned MIT slugify character map");
    values
        .into_iter()
        .map(|(k, v)| (k.chars().next().unwrap(), v))
        .collect()
});
static PATTERNS: LazyLock<HashMap<&'static str, Regex>> = LazyLock::new(|| {
    [
    ("original",r"(?i)\(?original( mix)?\)?$"),
    ("artist_prefix",r"(?i)(^| )(dj |mc )"),
    ("the",r"(?i)^the "),
    ("artists",r"(?i) and | & | feat\.? | ft\.? | vs\.? | ?, | / | X "),
    ("featured",r"( feat)|( ft)\.?"),
    ("ampersand",r" ?&"),
    ("plus",r" ?\+ ?"),
    ("ft",r"(?i) ft\.? "),
    ("quality",r"(?i)\s*- (hd|qhd|1080p|720p|4k|mix|original)\s*$"),
    ("remastered",r"(?i)\s*- (\d{4}\s+)?remaster(ed)?(\s+\d{4})?\s*$"),
    ("edition",r"(?i)\s*- ((\d{4}\s+)?remaster(ed)?(\s+\d{4})?\s+)?((deluxe|anniversary|special|collector'?s?|limited|expanded|platinum|gold|super|ultimate)\s+)?edition\s*$"),
    ("featuring",r"(?i)( \(?(feat|ft|with).*\)?)$"),
    ("clean",r"(?i) -clean$"),
    ("versions",r"(?i)(extended version|vocal version|original version|extended mix|original mix|bootleg|club mix|club edit|intro remix|short edit|clean short edit|dirty short edit|dirty edit|dirty intro|intro dirty|quick hit clean|dirty extended|intro mix|radio edit|radio mix|radio version|remix| rmx|super clean|dj intro|dj outro|ck cut|ck intro|ck quantized edit|ck fix|loop outro|hype intro|acapella|instrumental|single version|mixed cut|remastered|long version|short version|lyric video|video remix|smashvids|xtendz|xtendamix|ultimix|x-?mix|redrum|redub|dance mix|official video|official (hd|music)? ?video|official audio|music video|lyrics video)"),
    ("open",r"[\[<{]"),("close",r"[\]>}]"),
    ("bracket_versions",r"(?i)\( *(extended|original|clean|extra clean|clean edit|edit|clean intro|intro clean|clean extended|dirty|intro|single|extend|quickhit|xxx|master|remaster|live|unofficial|main|deluxe|bonus track|album version) *\)"),
    ("and",r"(?i) and "),
    ("with",r"(?i)with "),
    ("dash_original",r"(?i) - original$"),
    ("parentheses",r"(\(.*\)|\{.*\}|\[.*\])"),
].into_iter().map(|(k,v)| {
    // These shipped expressions use legacy JavaScript case folding. Rust's
    // default Unicode folding would also turn long S into ASCII s.
    let expression=v.replace(r"\d", "[0-9]")
        .replace(r"\s", r"(?u:[\u{9}-\u{d} \u{a0}\u{1680}\u{2000}-\u{200a}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}\u{feff}])")
        .replace(".*", r"(?u:[^\n\r\u{2028}\u{2029}])*");
    (k,RegexBuilder::new(&expression).unicode(false).build().expect("Fixed maintenance pattern"))
}).collect()
});

fn replace(value: &str, key: &str, with: &str, all: bool) -> String {
    if all {
        PATTERNS[key].replace_all(value, with).into_owned()
    } else {
        PATTERNS[key].replace(value, with).into_owned()
    }
}
fn js_space(c: char) -> bool {
    matches!(c,'\u{9}'..='\u{d}'|' '|'\u{a0}'|'\u{1680}'|'\u{2000}'..='\u{200a}'|'\u{2028}'|'\u{2029}'|'\u{202f}'|'\u{205f}'|'\u{3000}'|'\u{feff}')
}
pub fn simple(value: &str) -> String {
    let mut output = String::new();
    let mut space = false;
    for c in value.replace(['$', '&', '%'], "").nfc() {
        let mapped = MAP.get(&c).map(String::as_str);
        let fallback = c.to_string();
        for ch in mapped.unwrap_or(&fallback).chars() {
            if js_space(ch) {
                space = !output.is_empty();
            } else if ch.is_ascii_alphanumeric() {
                if space {
                    output.push(' ');
                    space = false;
                }
                output.push(ch);
            }
        }
    }
    output
}
fn clean_artist(value: &str) -> String {
    let lower = value.to_lowercase();
    let first = PATTERNS["featured"].split(&lower).next().unwrap_or("");
    let first = first.split(" with").next().unwrap_or("");
    let first = replace(first, "ampersand", ",", true);
    let first = replace(&first, "ft", ",", false);
    replace(&first, "plus", ", ", false)
}
pub fn artist_parts(value: &str, broad: bool) -> Vec<String> {
    let parts: Vec<_> = if broad && value.contains(" and ") {
        value.split(" and ").collect()
    } else {
        vec![value]
    };
    let mut keys: Vec<_> = parts
        .into_iter()
        .map(|s| simple(&clean_artist(s)).to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    if broad {
        keys.sort();
    }
    keys
}
pub fn artists(value: &str, broad: bool) -> Vec<String> {
    let stripped = replace(value, "artist_prefix", " ", true);
    let stripped = replace(stripped.trim(), "the", "", false)
        .trim()
        .replace('\'', "");
    let mut parts: Vec<_> = PATTERNS["artists"].split(&stripped).collect();
    if !broad {
        parts.sort_by(|a, b| COLLATOR.compare(&a.to_lowercase(), &b.to_lowercase()));
    }
    artist_parts(&parts.join(" and "), broad)
}
pub fn original(value: &str) -> String {
    replace(value, "original", "", false).trim().to_owned()
}
pub fn versionless(value: &str) -> String {
    let mut result = value.to_owned();
    for key in ["quality", "remastered", "edition"] {
        result = replace(&result, key, "", false);
    }
    result = result.replacen(" - ", " ", 1);
    result = replace(&result, "featuring", "", false);
    result = replace(&result, "clean", "", false);
    result = replace(&result, "versions", "", true);
    result = replace(&result, "open", "(", true);
    result = replace(&result, "close", ")", true);
    result = replace(&result, "bracket_versions", "", true);
    result = replace(&result, "and", " & ", true);
    simple(&result).to_lowercase().nfc().collect()
}
pub fn loose_title(value: &str) -> String {
    let mut result = versionless(value);
    if let Some(m) = PATTERNS["with"].find(&result)
        && m.start() > 0
    {
        result.replace_range(m.range(), "");
    }
    result = replace(&result, "dash_original", "", false).replace(" - VIP", " VIP");
    result = result.replace(['\'', '"', '&'], "");
    result.split_whitespace().collect::<Vec<_>>().join(" ")
}
pub fn trigrams(value: &str, tolerance: Tolerance) -> Vec<String> {
    let mut title = original(value);
    if tolerance == Tolerance::HighIgnoreParentheses {
        title = replace(&title, "parentheses", "", true).trim().into();
    }
    if tolerance == Tolerance::High {
        title = loose_title(&title);
    }
    let title = simple(&title).to_lowercase().replace(' ', "");
    if title.is_empty() {
        return vec![];
    }
    let title = format!(" {title} ");
    if title.len() <= 3 {
        return vec![];
    }
    let mut values = Vec::new();
    let mut seen = HashSet::new();
    for bytes in title.as_bytes().windows(3) {
        let text = String::from_utf8(bytes.into()).unwrap();
        if seen.insert(text.clone()) {
            values.push(text);
        }
    }
    values
}
