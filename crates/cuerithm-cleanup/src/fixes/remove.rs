use super::text;
use crate::CommonTextPattern;
use anyhow::Result;
use regex::{Regex, RegexBuilder};
pub(super) fn number_prefix(value: &str) -> String {
    if value.chars().all(|c| c.is_ascii_digit()) {
        return value.to_owned();
    }
    let end = if let Some((index, _)) = value.char_indices().find(|(_, c)| c.is_alphabetic()) {
        index
    } else {
        value
            .char_indices()
            .rev()
            .find(|(_, c)| c.is_whitespace() || matches!(c, '.' | '-'))
            .map_or(0, |(i, c)| i + c.len_utf8())
    };
    let prefix = &value[..end];
    if prefix.chars().count() < 2
        || !prefix
            .chars()
            .last()
            .is_some_and(|c| c.is_whitespace() || matches!(c, '.' | '-'))
    {
        return value.to_owned();
    }
    value[end..].trim().into()
}
fn key_prefix(value: &str) -> Option<usize> {
    let digits = value.chars().take_while(|c| c.is_ascii_digit()).count();
    if !(1..=2).contains(&digits) {
        return None;
    }
    let number = value[..digits].parse::<u8>().ok()?;
    if !(1..=12).contains(&number) || number.to_string() != value[..digits] {
        return None;
    }
    matches!(value.as_bytes().get(digits), Some(b'A' | b'B')).then_some(digits + 1)
}
pub(super) fn common(value: &str, pattern: CommonTextPattern) -> Result<String> {
    match pattern {
        CommonTextPattern::CamelotKeyFromTitle => {
            if let Some(end) = key_prefix(value) {
                let rest = &value[end..];
                if let Some(title) = rest.strip_prefix(" - ") {
                    return Ok(title.trim().into());
                }
                if let Some(title) = rest.strip_prefix(' ').filter(|s| !s.starts_with('-')) {
                    return Ok(title.trim().into());
                }
            }
            if let Some(index) = value.rfind(" - ") {
                let suffix = &value[index + 3..];
                if let Some(end) = key_prefix(suffix)
                    && suffix[end..]
                        .chars()
                        .all(|c| c == ' ' || c.is_ascii_digit())
                    {
                        return Ok(value[..index].trim().into());
                    }
            }
        }
        CommonTextPattern::OriginalMixFromTitle => {
            let regex = Regex::new(r"(?i)\s+-?\s*\(?(?:original(?: +mix)?)\)?$")?;
            if let Some(found) = regex.find(value) {
                return Ok(value[..found.start()].trim().into());
            }
        }
        CommonTextPattern::CamelotKeyEnergyFromTitle => {
            if let Some(end) = key_prefix(value)
                && let Some(rest) = value[end..].strip_prefix(" - ") {
                    let digits = rest.chars().take_while(|c| c.is_ascii_digit()).count();
                    if (1..=3).contains(&digits)
                        && let Some(title) = rest[digits..].strip_prefix(" - ") {
                            return Ok(title.trim().into());
                        }
                }
        }
        CommonTextPattern::BpmFromTitle => {
            if let Some((title, number)) = value.rsplit_once(' ')
                && number.len() >= 2
                    && !number.starts_with('0')
                    && number.chars().all(|c| c.is_ascii_digit())
                {
                    return Ok(title.trim().into());
                }
        }
    }
    Ok(value.to_owned())
}
pub(super) fn urls(value: &str) -> Result<Option<String>> {
    let regex = Regex::new(
        r"(?i)(?P<url>(?:https?://|www\.)[^\s]+)|(?P<mail>[a-z0-9.!#$%&'*+/=?^_\x60{|}~-]+@[a-z0-9.-]+\.[a-z]{2,63})|(?P<domain>(?:[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\.)+[a-z]{2,3}\b)",
    )?;
    let mut result = String::with_capacity(value.len());
    let mut end = 0;
    let mut changed = false;
    for found in regex.captures_iter(value) {
        let matched = found.get(0).unwrap();
        if let Some(domain) = found.name("domain") {
            let extension = domain
                .as_str()
                .rsplit('.')
                .next()
                .unwrap()
                .to_ascii_lowercase();
            if [
                "mp3", "aac", "m4a", "alac", "flac", "wav", "wave", "aif", "aiff", "aifc", "ogg",
                "oga", "opus", "wma",
            ]
            .contains(&extension.as_str())
            {
                continue;
            }
        }
        result.push_str(&value[end..matched.start()]);
        end = matched.end();
        changed = true;
    }
    if !changed {
        return Ok(None);
    }
    result.push_str(&value[end..]);
    Ok(Some(text::garbage(&result)))
}
fn phrases(words: &[&str]) -> String {
    words
        .iter()
        .map(|phrase| {
            phrase
                .split_whitespace()
                .map(regex::escape)
                .collect::<Vec<_>>()
                .join(r"\s*")
        })
        .collect::<Vec<_>>()
        .join("|")
}
pub(super) fn promotional(value: &str) -> Result<String> {
    let tails = phrases(&[
        "purchased",
        "bought",
        "acquired",
        "order from",
        "ordered from",
        "downloaded",
        "download from",
        "ripped",
        "recorded from",
        "taken from",
        "extracted from",
        "visit us",
        "visit our",
        "follow us",
        "follow me",
        "check out",
        "check us",
        "subscribe to",
        "find us on",
        "find me on",
        "listen on",
        "stream on",
        "available on",
        "now available",
        "out on",
        "released on",
        "released by",
        "more from",
        "more at",
        "for more",
        "all rights reserved",
        "get it on",
        "get it at",
        "get it from",
        "buy it on",
        "buy it at",
        "buy it from",
        "grab it on",
        "grab it at",
        "grab your copy",
        "contact us",
        "booking",
        "bookings",
        "demo",
        "demos",
    ]);
    let tail = RegexBuilder::new(&format!(
        r"(?:^|[^a-zA-Z0-9_])(?:{tails}|copyright\s*\d{{4}}|©\s*\d{{4}}|https?://|www\.).*$"
    ))
    .case_insensitive(true)
    .dot_matches_new_line(true)
    .build()?;
    let mut text = value.to_owned();
    if let Some(found) = tail.find(&text) {
        text.truncate(found.start());
    }
    let bracketed = phrases(&[
        "free download",
        "free dl",
        "download free",
        "dl free",
        "download",
        "downloads",
        "dl",
        "grab it",
        "get it",
        "out now",
        "available now",
        "available",
        "released",
        "buy",
        "buy here",
        "buy now",
        "buy it",
        "purchase",
        "get it here",
        "get it now",
        "exclusive",
        "promo",
        "promotional",
        "premiere",
        "premire",
        "supported by",
        "support the artist",
        "support",
        "beatport",
        "itunes",
        "spotify",
        "soundcloud",
        "bandcamp",
        "juno download",
        "traxsource",
        "click buy",
        "free",
        "gratis",
        "no copyright",
        "copyright free",
        "royalty free",
    ]);
    let bracket = RegexBuilder::new(&format!(
        r"[\s\-–—]*[\(\[\{{<]\s*(?:{bracketed})(?:[^\)\]\}}>]*)?[\)\]\}}>][\s\-–—]*"
    ))
    .case_insensitive(true)
    .build()?;
    text = bracket.replace_all(&text, " ").into_owned();
    let prefix = RegexBuilder::new(r"^\s*(?:premiere|premire)\b[\s\-–—_:]*")
        .case_insensitive(true)
        .build()?;
    text = prefix.replace(&text, "").into_owned();
    Ok(text::garbage(&text))
}
