//! Bounded ECMAScript predicates for local smart playlists.
use std::{error::Error, fmt};

const WORK_LIMIT: usize = 1_000_000;
const INPUT_LIMIT: usize = 1_048_576;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatternError {
    PatternTooLong,
    InvalidFlags,
    InvalidPattern(String),
    InputTooLong,
    BudgetExceeded,
}
impl fmt::Display for PatternError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PatternTooLong => {
                f.write_str("Regex patterns must contain at most 100 UTF-16 units")
            }
            Self::InvalidFlags => f.write_str("Invalid, duplicate or incompatible regex flags"),
            Self::InvalidPattern(message) => write!(f, "Invalid regex: {message}"),
            Self::InputTooLong => f.write_str("Regex metadata field exceeds 1 MiB"),
            Self::BudgetExceeded => f.write_str("Regex evaluation exceeded its work limit"),
        }
    }
}
impl Error for PatternError {}

#[derive(Debug)]
pub struct Pattern {
    regex: regress::Regex,
    sticky: bool,
}
fn js_whitespace(c: char) -> bool {
    matches!(c, '\u{9}'..='\u{d}' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
}
impl Pattern {
    pub fn compile(source: &str) -> Result<Self, PatternError> {
        let source = source.trim_matches(js_whitespace);
        if source.encode_utf16().take(101).count() > 100 {
            return Err(PatternError::PatternTooLong);
        }
        let (body, flags) = match source.rfind('/') {
            Some(index) if index == 0 || source.as_bytes()[index - 1] != b'\\' => {
                let body = &source[..index];
                (body.strip_prefix('/').unwrap_or(body), &source[index + 1..])
            }
            _ => (source, ""),
        };
        let mut seen = std::collections::BTreeSet::new();
        if flags
            .chars()
            .any(|c| !"dgimsuvy".contains(c) || !seen.insert(c))
            || (seen.contains(&'u') && seen.contains(&'v'))
        {
            return Err(PatternError::InvalidFlags);
        }
        let mut options = regress::Flags::from(flags);
        // UnicodeSets also uses Unicode character semantics at execution.
        options.unicode |= options.unicode_sets;
        options.legacy_code_units = !options.unicode;
        let regex = if options.unicode {
            regress::Regex::from_unicode(body.chars().map(u32::from), options)
        } else {
            regress::Regex::from_unicode(body.encode_utf16().map(u32::from), options)
        }
        .map_err(|e| PatternError::InvalidPattern(e.to_string()))?;
        Ok(Self {
            regex,
            sticky: seen.contains(&'y'),
        })
    }
    pub fn is_match(&self, text: &str) -> Result<bool, PatternError> {
        if text.len() > INPUT_LIMIT {
            return Err(PatternError::InputTooLong);
        }
        let units: Vec<u16> = text.encode_utf16().collect();
        self.regex
            .is_match_utf16_bounded(&units, WORK_LIMIT, self.sticky)
            .map_err(|_| PatternError::BudgetExceeded)
    }
}
