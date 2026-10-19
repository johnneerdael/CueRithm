use crate::{FieldValue, Operator, SmartTrack};
use anyhow::Result;
use cuerithm_regex::Pattern;
pub fn trim(value: &str) -> &str {
    value.trim_matches(|c|matches!(c,'\u{9}'..='\u{d}'|' '|'\u{a0}'|'\u{1680}'|'\u{2000}'..='\u{200a}'|'\u{2028}'|'\u{2029}'|'\u{202f}'|'\u{205f}'|'\u{3000}'|'\u{feff}'))
}
pub(crate) fn truthy(value: &FieldValue) -> bool {
    match value {
        FieldValue::Null => false,
        FieldValue::Text(s) | FieldValue::Color(s) => !s.is_empty(),
        FieldValue::Number(n) => *n != 0. && !n.is_nan(),
        FieldValue::Date(n) => *n != 0,
        FieldValue::Boolean(b) => *b,
    }
}
pub(crate) fn raw(value: &FieldValue) -> String {
    if !truthy(value) {
        return String::new();
    }
    match value {
        FieldValue::Text(s) | FieldValue::Color(s) => s.clone(),
        FieldValue::Number(n) => n.to_string(),
        FieldValue::Date(n) => n.to_string(),
        FieldValue::Boolean(b) => b.to_string(),
        FieldValue::Null => String::new(),
    }
}
pub(crate) fn matches(
    track: &SmartTrack,
    field: &str,
    op: Operator,
    query: &str,
    pattern: Option<&Pattern>,
) -> Result<bool> {
    let value = track.value(field);
    if matches!(
        op,
        Operator::StringRegExpMatch | Operator::StringNotRegExpMatch
    ) {
        let found = if truthy(&value) {
            pattern.expect("compiled regex").is_match(&raw(&value))?
        } else {
            false
        };
        return Ok(if op == Operator::StringNotRegExpMatch {
            !found
        } else {
            found
        });
    }
    let displayed = if field == "key" {
        track
            .key
            .as_deref()
            .and_then(crate::keys::camelot)
            .unwrap_or_default()
    } else if field == "fileType" {
        raw(&track.value("fileTypeLabel"))
    } else {
        raw(&value)
    };
    let text = trim(&displayed).to_lowercase();
    let q = query.to_lowercase();
    use Operator::*;
    Ok(match op {
        StringEquals => text == q,
        StringNotEquals => text != q,
        StringContains => !q.is_empty() && text.contains(&q),
        StringNotContains => q.is_empty() || !text.contains(&q),
        StringStartsWith => !q.is_empty() && text.starts_with(&q),
        StringNotStartsWith => q.is_empty() || !text.starts_with(&q),
        StringEndsWith => !q.is_empty() && text.ends_with(&q),
        StringNotEndsWith => q.is_empty() || !text.ends_with(&q),
        _ => unreachable!(),
    })
}
