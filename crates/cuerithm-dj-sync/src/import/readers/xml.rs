use super::*;
use quick_xml::{Reader, events::Event};
#[derive(Default)]
pub(super) struct Node {
    pub name: String,
    pub attributes: BTreeMap<String, String>,
    pub text: String,
    pub children: Vec<Node>,
}
impl Node {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attributes.get(name).map(String::as_str)
    }
    pub fn child(&self, name: &str) -> Option<&Self> {
        self.children.iter().find(|n| n.name == name)
    }
    pub fn number(&self, name: &str) -> Option<f64> {
        self.attr(name)?
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
    }
    pub fn raw(&self) -> Value {
        self.raw_without(&[])
    }
    pub fn raw_without(&self, excluded: &[&str]) -> Value {
        let safe = |name: &str| {
            ![
                "password",
                "credential",
                "secret",
                "token",
                "account",
                "authorization",
                "cookie",
                "settings",
            ]
            .iter()
            .any(|p| name.to_ascii_lowercase().contains(p))
        };
        json!({"name":self.name,"attributes":self.attributes.iter().filter(|(k,_)|safe(k)).collect::<BTreeMap<_,_>>(),
            "text":self.text,"children":self.children.iter().filter(|n|safe(&n.name) && !excluded.contains(&n.name.as_str())).map(|n|n.raw_without(excluded)).collect::<Vec<_>>()})
    }
}
pub(super) fn parse(
    bytes: &[u8],
    limits: &ImportLimits,
    cancel: &CancellationToken,
) -> Result<Node> {
    ensure!(
        bytes.len() as u64 <= limits.text_bytes,
        "XML text byte limit"
    );
    let input = std::str::from_utf8(bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes))
        .context("XML export must use UTF-8")?;
    let mut reader = Reader::from_str(input);
    let mut stack: Vec<Node> = Vec::new();
    let mut root = None;
    let mut count = 0usize;
    loop {
        cancel.check()?;
        let event = reader.read_event().context("Malformed library XML")?;
        match event {
            Event::Start(ref element) | Event::Empty(ref element) => {
                count += 1;
                ensure!(
                    count <= 1_000_000 && stack.len() < limits.hierarchy_depth,
                    "XML node or depth limit"
                );
                let mut node = Node {
                    name: element.name().as_ref().into(),
                    ..Default::default()
                };
                for attribute in element.attributes() {
                    let attribute = attribute?;
                    let name = attribute.key.as_ref().to_owned();
                    ensure!(
                        node.attributes
                            .insert(
                                name,
                                attribute
                                    .normalized_value(quick_xml::XmlVersion::Implicit1_0)?
                                    .into_owned()
                            )
                            .is_none(),
                        "Duplicate XML attribute"
                    );
                }
                if matches!(event, Event::Start(_)) {
                    stack.push(node);
                } else if let Some(parent) = stack.last_mut() {
                    parent.children.push(node);
                } else {
                    ensure!(root.replace(node).is_none(), "Multiple XML roots");
                }
            }
            Event::End(_) => {
                let node = stack.pop().context("Unbalanced XML")?;
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(node);
                } else {
                    ensure!(root.replace(node).is_none(), "Multiple XML roots");
                }
            }
            Event::Text(text) => {
                let text = text.xml10_content();
                if let Some(node) = stack.last_mut() {
                    node.text.push_str(&text);
                } else {
                    ensure!(text.trim().is_empty(), "Text outside XML root");
                }
            }
            Event::CData(text) => {
                stack
                    .last_mut()
                    .context("CDATA outside XML root")?
                    .text
                    .push_str(&text.xml10_content());
            }
            Event::GeneralRef(reference) => {
                let value = format!("&{};", reference.as_ref());
                stack
                    .last_mut()
                    .context("Entity outside XML root")?
                    .text
                    .push_str(&quick_xml::escape::unescape(&value)?);
            }
            Event::DocType(doctype) => {
                let value: &str = doctype.as_ref();
                ensure!(
                    !value.contains('[')
                        && !value.contains("ENTITY")
                        && value.starts_with("plist ")
                        && value.contains("http://www.apple.com/DTDs/PropertyList-1.0.dtd"),
                    "Unsupported XML doctype or entity declaration"
                );
                // quick-xml never fetches the external DTD; only built-in references are resolved.
            }
            Event::Decl(declaration) => {
                if let Some(encoding) = declaration.encoding() {
                    ensure!(
                        encoding?.as_ref().eq_ignore_ascii_case("utf-8"),
                        "Unsupported XML encoding declaration"
                    );
                }
            }
            Event::Comment(_) | Event::PI(_) => {}
            Event::Eof => break,
        }
    }
    ensure!(stack.is_empty(), "Incomplete XML");
    root.context("Empty XML export")
}
pub(super) fn plist(node: Node) -> Result<Value> {
    Ok(match node.name.as_str() {
        "dict" => {
            ensure!(
                node.text.trim().is_empty() && node.children.len().is_multiple_of(2),
                "Malformed plist dictionary"
            );
            let mut result = serde_json::Map::new();
            let mut children = node.children.into_iter();
            while let Some(key) = children.next() {
                ensure!(
                    key.name == "key" && key.children.is_empty(),
                    "Invalid plist dictionary key"
                );
                ensure!(
                    result
                        .insert(
                            key.text,
                            plist(children.next().context("Missing plist value")?)?
                        )
                        .is_none(),
                    "Duplicate plist key"
                );
            }
            Value::Object(result)
        }
        "array" => {
            ensure!(node.text.trim().is_empty(), "Text inside plist array");
            Value::Array(
                node.children
                    .into_iter()
                    .map(plist)
                    .collect::<Result<_>>()?,
            )
        }
        "string" | "date" | "data" => {
            ensure!(node.children.is_empty(), "Nested plist scalar");
            json!(node.text)
        }
        "integer" | "real" => {
            ensure!(node.children.is_empty(), "Nested plist number");
            let value: Value = serde_json::from_str(node.text.trim())?;
            ensure!(value.is_number(), "Invalid plist number");
            value
        }
        "true" | "false" => {
            ensure!(
                node.children.is_empty() && node.text.trim().is_empty(),
                "Malformed plist boolean"
            );
            json!(node.name == "true")
        }
        _ => anyhow::bail!("Unsupported plist value type"),
    })
}
