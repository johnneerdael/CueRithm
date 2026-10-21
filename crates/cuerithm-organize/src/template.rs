use crate::paths::PathRules;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamingContext {
    pub artists: Vec<String>,
    pub title: Option<String>,
    pub album: Option<String>,
    pub album_artists: Vec<String>,
    pub genres: Vec<String>,
    pub label: Option<String>,
    pub version: Option<String>,
    pub release_date: Option<String>,
    pub key: Option<String>,
    /// Final accepted/measured tempo, rounded for filenames only; never used by DSP.
    pub bpm_hundredths: Option<u32>,
    pub energy: Option<u8>,
    pub track: Option<u32>,
    pub disc: Option<u32>,
    pub original_name: Option<String>,
    pub parent: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissingPolicy {
    Block,
    OmitOptional,
    SkipOptional,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rendered {
    Path(String),
    Skipped { missing: String },
}
#[derive(Clone, Debug)]
enum Node {
    Literal(String),
    Token {
        name: String,
        filter: Option<String>,
    },
    Optional(Vec<Node>),
}
#[derive(Clone, Debug)]
pub struct Template {
    nodes: Vec<Node>,
}
const TOKENS: &[&str] = &[
    "artist",
    "artists",
    "title",
    "album",
    "album_artist",
    "album_artists",
    "genre",
    "genres",
    "label",
    "version",
    "release_date",
    "year",
    "key",
    "bpm",
    "energy",
    "track",
    "disc",
    "original_name",
    "parent",
];
impl Template {
    pub fn parse(text: &str) -> Result<Self, String> {
        if text.is_empty() || text.len() > 2048 {
            return Err("Template must contain 1–2048 bytes".into());
        }
        let mut cursor = 0;
        let nodes = parse_nodes(text, &mut cursor, false)?;
        let shape = shape(&nodes);
        crate::paths::validate_relative(&shape, PathRules::Portable)?;
        Ok(Self { nodes })
    }
    pub fn render(
        &self,
        c: &NamingContext,
        policy: MissingPolicy,
        optional: bool,
        rules: PathRules,
    ) -> Result<Rendered, String> {
        if c.bpm_hundredths == Some(0) || c.energy.is_some_and(|value| !(1..=10).contains(&value)) {
            return Err("Invalid measured naming value".into());
        }
        let value = match render_nodes(&self.nodes, c, policy) {
            Ok(value) => value,
            Err(missing) if policy == MissingPolicy::SkipOptional && optional => {
                return Ok(Rendered::Skipped { missing });
            }
            Err(missing) => return Err(format!("Missing naming field: {missing}")),
        };
        crate::paths::validate_relative(&value, rules)?;
        Ok(Rendered::Path(value))
    }
}
fn parse_nodes(text: &str, cursor: &mut usize, optional: bool) -> Result<Vec<Node>, String> {
    let mut nodes = Vec::new();
    let bytes = text.as_bytes();
    while *cursor < bytes.len() {
        match bytes[*cursor] {
            b'[' => {
                if optional {
                    return Err("Nested optional segments are unsupported".into());
                }
                *cursor += 1;
                let start = *cursor;
                let inner = parse_nodes(text, cursor, true)?;
                if text[start..*cursor].contains('/') {
                    return Err("Optional segments cannot contain directories".into());
                }
                nodes.push(Node::Optional(inner));
            }
            b']' => {
                if !optional {
                    return Err("Unmatched optional delimiter".into());
                }
                *cursor += 1;
                return Ok(nodes);
            }
            b'{' => {
                *cursor += 1;
                let end = text[*cursor..].find('}').ok_or("Unclosed field")? + *cursor;
                let token = &text[*cursor..end];
                let (name, filter) = token
                    .split_once('|')
                    .map_or((token, None), |(n, f)| (n, Some(f)));
                if !TOKENS.contains(&name) {
                    return Err(format!("Unknown naming field: {name}"));
                }
                if let Some(f) = filter {
                    let text_filter = matches!(f, "upper" | "lower");
                    let numeric_filter = matches!(name, "track" | "disc")
                        && f.strip_prefix("pad:")
                            .and_then(|p| p.parse::<u8>().ok())
                            .is_some_and(|p| (1..=8).contains(&p));
                    if !(text_filter || numeric_filter) {
                        return Err(format!("Unsupported typed filter: {f}"));
                    }
                }
                nodes.push(Node::Token {
                    name: name.into(),
                    filter: filter.map(Into::into),
                });
                *cursor = end + 1;
            }
            b'}' => return Err("Unmatched field delimiter".into()),
            _ => {
                let start = *cursor;
                while *cursor < bytes.len() && !b"[]{}".contains(&bytes[*cursor]) {
                    *cursor += 1;
                }
                nodes.push(Node::Literal(text[start..*cursor].into()));
            }
        }
    }
    if optional {
        return Err("Unclosed optional segment".into());
    }
    Ok(nodes)
}
fn shape(nodes: &[Node]) -> String {
    nodes
        .iter()
        .map(|n| match n {
            Node::Literal(s) => s.clone(),
            Node::Token { .. } => "x".into(),
            Node::Optional(n) => shape(n),
        })
        .collect()
}
fn field(c: &NamingContext, name: &str) -> Option<String> {
    let one = |v: &Vec<String>| {
        v.iter()
            .find(|s| !s.trim().is_empty())
            .map(|s| s.trim().to_string())
    };
    let many = |v: &Vec<String>| {
        let values: Vec<_> = v
            .iter()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if values.is_empty() {
            None
        } else {
            Some(values.join(", "))
        }
    };
    match name {
        "artist" => one(&c.artists),
        "artists" => many(&c.artists),
        "album_artist" => one(&c.album_artists),
        "album_artists" => many(&c.album_artists),
        "genre" => one(&c.genres),
        "genres" => many(&c.genres),
        "title" => c.title.clone(),
        "album" => c.album.clone(),
        "label" => c.label.clone(),
        "version" => c.version.clone(),
        "release_date" => c.release_date.clone(),
        "year" => c.release_date.as_deref().and_then(|date| {
            let year = date.split('-').next()?;
            (year.len() == 4 && year.bytes().all(|v| v.is_ascii_digit()) && year != "0000")
                .then(|| year.to_owned())
        }),
        "key" => c.key.clone(),
        "bpm" => c
            .bpm_hundredths
            .map(|value| format!("{}.{:02}", value / 100, value % 100)),
        "energy" => c.energy.map(|value| value.to_string()),
        "track" => c.track.map(|v| v.to_string()),
        "disc" => c.disc.map(|v| v.to_string()),
        "original_name" => c.original_name.clone(),
        "parent" => c.parent.clone(),
        _ => None,
    }
    .filter(|v| !v.trim().is_empty())
}
fn render_nodes(
    nodes: &[Node],
    c: &NamingContext,
    policy: MissingPolicy,
) -> Result<String, String> {
    let mut output = String::new();
    for node in nodes {
        match node {
            Node::Literal(value) => output.push_str(value),
            Node::Optional(nodes) => match render_nodes(nodes, c, policy) {
                Ok(value) => output.push_str(&value),
                Err(_) if policy == MissingPolicy::OmitOptional => {}
                Err(e) => return Err(e),
            },
            Node::Token { name, filter } => {
                let value = field(c, name).ok_or_else(|| name.clone())?;
                let value = match filter.as_deref() {
                    Some("upper") => value.to_uppercase(),
                    Some("lower") => value.to_lowercase(),
                    Some(p) if p.starts_with("pad:") => format!(
                        "{:0>width$}",
                        value,
                        width = p[4..].parse::<usize>().unwrap()
                    ),
                    _ => value,
                };
                let safe = crate::paths::sanitize(&value);
                if safe.is_empty() {
                    return Err(name.clone());
                }
                output.push_str(&safe);
            }
        }
    }
    Ok(output)
}
