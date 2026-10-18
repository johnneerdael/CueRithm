pub(super) fn replace_characters(value: &str, characters: &str) -> String {
    let mut result = value.to_owned();
    for character in characters.chars() {
        result = result.replace(character, " ").trim().to_owned();
    }
    result
}
pub(super) fn entities(value: &str) -> String {
    let mut current = value.to_owned();
    for _ in 0..8 {
        let decoded = html_escape::decode_html_entities(&current).into_owned();
        if decoded == current {
            break;
        }
        current = decoded;
    }
    current
}
fn empty_brackets(value: &str) -> String {
    let mut out: Vec<char> = Vec::with_capacity(value.chars().count());
    for c in value.chars() {
        let opener = match c {
            ')' => Some('('),
            ']' => Some('['),
            '}' => Some('{'),
            '>' => Some('<'),
            '|' => Some('|'),
            _ => None,
        };
        if let Some(opener) = opener {
            let mut end = out.len();
            while end > 0 && out[end - 1].is_whitespace() {
                end -= 1;
            }
            if end > 0 && out[end - 1] == opener {
                out.truncate(end - 1);
                continue;
            }
        }
        out.push(c);
    }
    out.into_iter().collect()
}
pub(super) fn edges(value: &str) -> String {
    let mut text = value;
    loop {
        text = text.trim_start_matches(|c: char| c.is_whitespace() || "-_|])?.}\\/".contains(c));
        let leading: usize = text
            .chars()
            .take_while(|c| matches!(c, '&' | '+'))
            .map(char::len_utf8)
            .sum();
        let rest = &text[leading..];
        if leading == 0 || rest.chars().next().is_some_and(|c| c.is_alphanumeric()) {
            break;
        }
        text = rest;
    }
    loop {
        text = text.trim_end_matches(|c: char| c.is_whitespace() || "-_|[(\\/".contains(c));
        let trailing: usize = text
            .chars()
            .rev()
            .take_while(|c| matches!(c, '&' | '+'))
            .map(char::len_utf8)
            .sum();
        let rest = &text[..text.len() - trailing];
        if trailing == 0
            || rest
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric())
        {
            break;
        }
        text = rest;
    }
    text.to_owned()
}

pub(super) fn garbage(value: &str) -> String {
    let text = edges(&empty_brackets(value));
    let mut out = String::with_capacity(text.len());
    let mut whitespace = 0;
    for c in text.chars() {
        if c.is_whitespace() {
            whitespace += 1;
            if whitespace == 1 {
                out.push(c);
            } else if whitespace == 2 {
                out.pop();
                out.push(' ');
            }
        } else {
            whitespace = 0;
            out.push(c);
        }
    }
    out.trim().to_owned()
}
fn initial(value: &str) -> String {
    let mut result = String::new();
    let mut leading = true;
    for c in value.chars() {
        if leading && "([{\"'_".contains(c) {
            result.push(c);
        } else if leading {
            result.extend(c.to_uppercase());
            leading = false;
        } else {
            result.push(c);
        }
    }
    result
}
fn dotted(value: &str) -> bool {
    let mut chars = value.chars().peekable();
    if chars
        .peek()
        .is_some_and(|c| !c.is_ascii_alphanumeric() && *c != '_')
    {
        chars.next();
    }
    loop {
        match (chars.next(), chars.next()) {
            (None, None) => return true,
            (Some(a), Some('.')) if a.is_ascii_uppercase() => {}
            _ => return false,
        }
    }
}
fn word(value: &str) -> String {
    if value.is_empty() || dotted(value) {
        return value.to_owned();
    }
    if value.eq_ignore_ascii_case("vs") {
        return "vs".into();
    }
    let acronym: String = value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    if ["mc", "dj", "va"].contains(&acronym.as_str()) {
        return value.to_uppercase();
    }
    let value = if value.to_uppercase() == value {
        value.to_lowercase()
    } else {
        value.to_owned()
    };
    if value.contains('@') && value.contains('.')
        || value.contains("://")
        || value.starts_with('/')
        || value.rsplit_once('.').is_some_and(|(a, b)| {
            !a.is_empty()
                && (1..=3).contains(&b.len())
                && b.chars().all(|c| c.is_ascii_alphanumeric())
        })
    {
        return value;
    }
    let mut previous_lower = false;
    for c in value.chars() {
        if previous_lower && c.is_ascii_uppercase() {
            return value;
        }
        previous_lower = c.is_ascii_lowercase();
    }
    let dashes: Vec<_> = value.chars().filter(|c| "-‑–—".contains(*c)).collect();
    if let Some(dash) = dashes.first() {
        return value
            .split(*dash)
            .map(|part| {
                if dashes.len() > 1
                    && [
                        "a", "an", "and", "at", "but", "by", "for", "in", "nor", "of", "on", "or",
                        "so", "the", "to", "up", "yet", "v", "v.", "vs", "vs.",
                    ]
                    .contains(&part.to_lowercase().as_str())
                {
                    part.to_owned()
                } else {
                    initial(part)
                }
            })
            .collect::<Vec<_>>()
            .join(&dash.to_string());
    }
    if value.contains('/') {
        return value.split('/').map(initial).collect::<Vec<_>>().join("/");
    }
    initial(&value)
}
pub(super) fn casing(value: &str, uppercase: bool, lowercase: bool) -> String {
    let is_upper = uppercase
        && value.chars().count() != 1
        && value.chars().any(|c| c.is_ascii_uppercase())
        && value.to_uppercase() == value;
    let is_lower =
        lowercase && value.chars().any(|c| c.is_ascii_lowercase()) && value.to_lowercase() == value;
    if !is_upper && !is_lower {
        return value.to_owned();
    }
    value
        .trim()
        .split(' ')
        .map(word)
        .collect::<Vec<_>>()
        .join(" ")
}
