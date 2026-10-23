//! Bounded data-only Apple archive graphs. References survive; no classes execute.
use super::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ArchiveNode {
    Null,
    Bool(bool),
    Unsigned(u64),
    Signed(i64),
    Real(f64),
    Text(String),
    Date(f64),
    Data(Vec<u8>),
    Uri(String),
    Reference(u64),
    Uid(u64),
    StringPointer(u8),
    NumberPointer(u8),
    Array(Vec<ArchiveNode>),
    Set(Vec<ArchiveNode>),
    Dictionary(Vec<(ArchiveNode, ArchiveNode)>),
    Class {
        name: String,
        fields: Vec<(String, ArchiveNode)>,
    },
    Opaque {
        kind: String,
        bytes: Vec<u8>,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchivedValue {
    pub root: ArchiveNode,
    pub objects: Vec<ArchiveNode>,
    pub plist_objects: Vec<ArchiveNode>,
    pub strings: Vec<ArchiveNode>,
    pub numbers: Vec<ArchiveNode>,
    pub metadata_root: Option<ArchiveNode>,
    pub raw: Vec<u8>,
    pub format: String,
    #[serde(skip)]
    pub(super) limits: ImportLimits,
}
impl ArchivedValue {
    pub(super) fn new(raw: &[u8], format: &str, limits: &ImportLimits) -> Self {
        Self {
            root: ArchiveNode::Null,
            objects: Vec::new(),
            plist_objects: Vec::new(),
            strings: Vec::new(),
            numbers: Vec::new(),
            metadata_root: None,
            raw: raw.to_vec(),
            format: format.into(),
            limits: limits.clone(),
        }
    }
    pub fn resolved<'a>(&'a self, mut node: &'a ArchiveNode) -> Result<&'a ArchiveNode> {
        let mut seen = BTreeSet::new();
        loop {
            let (kind, index, target) = match node {
                ArchiveNode::Reference(i) => (0, *i, self.plist_objects.get(usize::try_from(*i)?)),
                ArchiveNode::Uid(i) => {
                    if *i == 0 && !self.objects.is_empty() {
                        return Ok(&ArchiveNode::Null);
                    }
                    (1, *i, self.objects.get(usize::try_from(*i)?))
                }
                ArchiveNode::StringPointer(i) => (2, u64::from(*i), self.strings.get(*i as usize)),
                ArchiveNode::NumberPointer(i) => (3, u64::from(*i), self.numbers.get(*i as usize)),
                _ => return Ok(node),
            };
            ensure!(
                seen.insert((kind, index)) && seen.len() <= self.limits.hierarchy_depth,
                "Cyclic/deep archive reference"
            );
            node = target.context("Archive reference out of bounds")?;
        }
    }
    pub fn text<'a>(&'a self, node: &'a ArchiveNode) -> Result<Option<&'a str>> {
        Ok(match self.resolved(node)? {
            ArchiveNode::Text(text) | ArchiveNode::Uri(text) => Some(text),
            _ => None,
        })
    }
    pub fn number(&self, node: &ArchiveNode) -> Result<Option<f64>> {
        Ok(match self.resolved(node)? {
            ArchiveNode::Unsigned(n) => Some(*n as f64),
            ArchiveNode::Signed(n) => Some(*n as f64),
            ArchiveNode::Real(n) | ArchiveNode::Date(n) => Some(*n),
            _ => None,
        })
    }
    pub fn array<'a>(&'a self, node: &'a ArchiveNode) -> Result<Option<&'a [ArchiveNode]>> {
        Ok(match self.resolved(node)? {
            ArchiveNode::Array(items) => Some(items),
            _ => None,
        })
    }
    pub fn data<'a>(&'a self, node: &'a ArchiveNode) -> Result<Option<&'a [u8]>> {
        Ok(match self.resolved(node)? {
            ArchiveNode::Data(bytes) => Some(bytes),
            _ => None,
        })
    }
    pub fn field<'a>(
        &'a self,
        node: &'a ArchiveNode,
        name: &str,
    ) -> Result<Option<&'a ArchiveNode>> {
        Ok(match self.resolved(node)? {
            ArchiveNode::Class { fields, .. } => fields
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value),
            ArchiveNode::Dictionary(fields) => {
                let mut found = None;
                for (key, value) in fields {
                    if self.text(key)? == Some(name) {
                        ensure!(found.is_none(), "Duplicate archive dictionary key");
                        found = Some(value);
                    }
                }
                found
            }
            _ => None,
        })
    }
    fn class_name<'a>(&'a self, node: &'a ArchiveNode) -> Result<Option<&'a str>> {
        if let ArchiveNode::Class { name, .. } = self.resolved(node)? {
            return Ok(Some(name));
        }
        let Some(class) = self.field(node, "$class")? else {
            return Ok(None);
        };
        let Some(name) = self.field(class, "$classname")? else {
            return Ok(None);
        };
        self.text(name)
    }
    pub fn to_json(&self) -> Result<Value> {
        let mut active = BTreeSet::new();
        let mut budget = Budget { nodes: 0, bytes: 0 };
        self.json(&self.root, 0, &mut active, &mut budget)
    }
    fn json(
        &self,
        node: &ArchiveNode,
        depth: usize,
        active: &mut BTreeSet<(u8, u64)>,
        budget: &mut Budget,
    ) -> Result<Value> {
        budget.nodes += 1;
        ensure!(
            depth <= self.limits.hierarchy_depth && budget.nodes <= 1_000_000,
            "Archive expansion/depth limit"
        );
        let reference = match node {
            ArchiveNode::Reference(i) => Some((0, *i)),
            ArchiveNode::Uid(i) => Some((1, *i)),
            ArchiveNode::StringPointer(i) => Some((2, u64::from(*i))),
            ArchiveNode::NumberPointer(i) => Some((3, u64::from(*i))),
            _ => None,
        };
        if let Some(key) = reference {
            if !active.insert(key) {
                return Ok(json!({"$reference":key.1,"$namespace":key.0,"$cycle":true}));
            }
            // Walk one reference at a time so UID identity participates in cycle detection.
            let value = match key.0 {
                0 => self.plist_objects.get(usize::try_from(key.1)?),
                1 if key.1 == 0 && !self.objects.is_empty() => Some(&ArchiveNode::Null),
                1 => self.objects.get(usize::try_from(key.1)?),
                2 => self.strings.get(usize::try_from(key.1)?),
                3 => self.numbers.get(usize::try_from(key.1)?),
                _ => None,
            }
            .context("Archive reference out of bounds")?;
            let result = self.json(value, depth + 1, active, budget);
            active.remove(&key);
            return result;
        }
        let class = self.class_name(node)?;
        if let Some(class) = class {
            if matches!(class, "NSString" | "NSMutableString" | "NSCFString")
                && let Some(value) = self.field(node, "NS.string")?
            {
                return self.json(value, depth + 1, active, budget);
            }
            if matches!(class, "NSArray" | "NSMutableArray")
                && let Some(value) = self.field(node, "NS.objects")?
            {
                return self.json(value, depth + 1, active, budget);
            }
            if class == "NSDate"
                && let Some(value) = self.field(node, "NS.time")?
                && let Some(seconds) = self.number(value)?
            {
                return date_value(seconds);
            }
            if matches!(class, "NSNumber" | "__NSCFNumber" | "NSCFNumber") {
                for key in ["NS.boolval", "NS.intval", "NS.dblval", "NS.floatval"] {
                    if let Some(value) = self.field(node, key)? {
                        return self.json(value, depth + 1, active, budget);
                    }
                }
            }
            if matches!(class, "NSDictionary" | "NSMutableDictionary")
                && let (Some(keys), Some(values)) = (
                    self.field(node, "NS.keys")?,
                    self.field(node, "NS.objects")?,
                )
            {
                let keys = self.array(keys)?.context("Invalid NSDictionary keys")?;
                let values = self.array(values)?.context("Invalid NSDictionary values")?;
                ensure!(
                    keys.len() == values.len(),
                    "Dictionary key/value length mismatch"
                );
                let mut out = serde_json::Map::new();
                for (key, value) in keys.iter().zip(values) {
                    let key = self.json(key, depth + 1, active, budget)?;
                    let key = key.as_str().context("Nonstring dictionary key")?;
                    if safe_key(key) {
                        ensure!(!out.contains_key(key), "Duplicate archive key");
                        out.insert(key.into(), self.json(value, depth + 1, active, budget)?);
                    }
                }
                return Ok(Value::Object(out));
            }
        }
        Ok(match node {
            ArchiveNode::Null => Value::Null,
            ArchiveNode::Bool(v) => json!(v),
            ArchiveNode::Unsigned(v) => json!(v),
            ArchiveNode::Signed(v) => json!(v),
            ArchiveNode::Real(v) => {
                ensure!(v.is_finite(), "Nonfinite archive value");
                json!(v)
            }
            ArchiveNode::Date(v) => date_value(*v)?,
            ArchiveNode::Text(text) => {
                budget.charge(text.len(), &self.limits)?;
                json!(text)
            }
            ArchiveNode::Uri(text) => {
                budget.charge(text.len(), &self.limits)?;
                json!({"$uri":text})
            }
            ArchiveNode::Data(bytes) => {
                budget.charge(bytes.len(), &self.limits)?;
                json!({"$binary":{"sha256":digest(bytes),"bytes":bytes.len()}})
            }
            ArchiveNode::Opaque { kind, bytes } => {
                budget.charge(bytes.len(), &self.limits)?;
                json!({"$opaque":{"kind":kind,"sha256":digest(bytes),"bytes":bytes.len()}})
            }
            ArchiveNode::Array(items) => {
                let mut out = Vec::new();
                for value in items {
                    out.push(self.json(value, depth + 1, active, budget)?)
                }
                json!(out)
            }
            ArchiveNode::Set(items) => {
                let mut out = Vec::new();
                for value in items {
                    out.push(self.json(value, depth + 1, active, budget)?)
                }
                json!({"$set":out})
            }
            ArchiveNode::Class { fields, .. } => {
                let mut out = serde_json::Map::new();
                if let Some(class) = class {
                    out.insert("$class".into(), json!(class));
                }
                for (key, value) in fields {
                    if safe_key(key) {
                        ensure!(!out.contains_key(key), "Duplicate typed field");
                        out.insert(key.clone(), self.json(value, depth + 1, active, budget)?);
                    }
                }
                Value::Object(out)
            }
            ArchiveNode::Dictionary(fields) => {
                let mut out = serde_json::Map::new();
                if let Some(class) = class {
                    out.insert("$class".into(), json!(class));
                }
                for (key, value) in fields {
                    let key = self.text(key)?.context("Nonstring plist key")?;
                    if key == "$class" && class.is_some() {
                        continue;
                    }
                    if safe_key(key) {
                        ensure!(!out.contains_key(key), "Duplicate plist field");
                        out.insert(key.into(), self.json(value, depth + 1, active, budget)?);
                    }
                }
                Value::Object(out)
            }
            _ => anyhow::bail!("Unresolved archive node"),
        })
    }
}
struct Budget {
    nodes: usize,
    bytes: u64,
}
impl Budget {
    fn charge(&mut self, n: usize, limits: &ImportLimits) -> Result<()> {
        self.bytes = self
            .bytes
            .checked_add(n as u64)
            .context("Archive budget overflow")?;
        ensure!(
            self.bytes <= limits.blob_bytes.saturating_mul(4),
            "Archive expansion byte limit"
        );
        Ok(())
    }
}
pub(super) fn safe_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    ![
        "password",
        "credential",
        "license",
        "account",
        "token",
        "cookie",
        "secret",
    ]
    .iter()
    .any(|word| key.contains(word))
}
pub(super) fn date_value(seconds: f64) -> Result<Value> {
    ensure!(seconds.is_finite(), "Nonfinite Apple epoch date");
    let unix = seconds + 978307200.;
    ensure!(
        unix >= i64::MIN as f64 && unix < i64::MAX as f64,
        "Apple date out of range"
    );
    let floor = unix.floor();
    let nanos = ((unix - floor) * 1e9).round().clamp(0., 999999999.) as u32;
    let utc = chrono::DateTime::<chrono::Utc>::from_timestamp(floor as i64, nanos)
        .context("Apple date outside supported UTC range")?
        .to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true);
    Ok(json!({"$apple_seconds":seconds,"utc":utc}))
}
fn unsigned(bytes: &[u8]) -> Result<u64> {
    ensure!(
        !bytes.is_empty() && bytes.len() <= 8,
        "Invalid archive integer width"
    );
    Ok(bytes.iter().fold(0, |n, b| (n << 8) | u64::from(*b)))
}
struct Plist<'a> {
    bytes: &'a [u8],
    reference_size: usize,
    object_count: usize,
    elements: usize,
    payload: u64,
    limits: &'a ImportLimits,
}
impl Plist<'_> {
    fn length(&self, at: &mut usize, end: usize, low: u8) -> Result<usize> {
        let n = if low < 15 {
            u64::from(low)
        } else {
            let marker = *self
                .bytes
                .get(*at)
                .filter(|_| *at < end)
                .context("Missing plist length")?;
            *at += 1;
            ensure!(
                marker >> 4 == 1 && (marker & 15) <= 3,
                "Invalid plist length integer"
            );
            let count = 1usize << (marker & 15);
            let next = at
                .checked_add(count)
                .filter(|next| *next <= end)
                .context("Truncated plist length")?;
            let n = unsigned(&self.bytes[*at..next])?;
            *at = next;
            n
        };
        Ok(usize::try_from(n)?)
    }
    fn refs(&mut self, at: usize, end: usize, n: usize) -> Result<Vec<ArchiveNode>> {
        self.elements = self
            .elements
            .checked_add(n)
            .context("Reference count overflow")?;
        ensure!(self.elements <= 1_000_000, "Archive reference count limit");
        let size = n
            .checked_mul(self.reference_size)
            .context("Reference byte overflow")?;
        let next = at
            .checked_add(size)
            .filter(|next| *next <= end)
            .context("Plist reference array exceeds object")?;
        let mut out = Vec::with_capacity(n);
        for bytes in self.bytes[at..next].chunks(self.reference_size) {
            let index = unsigned(bytes)?;
            ensure!(
                index < self.object_count as u64,
                "Plist reference out of bounds"
            );
            out.push(ArchiveNode::Reference(index));
        }
        Ok(out)
    }
    fn object(&mut self, start: usize, end: usize) -> Result<ArchiveNode> {
        let marker = *self
            .bytes
            .get(start)
            .filter(|_| start < end)
            .context("Empty plist object")?;
        let kind = marker >> 4;
        let low = marker & 15;
        let mut at = start + 1;
        if matches!(kind, 4..=6) {
            let n = self.length(&mut at, end, low)?;
            let size = n
                .checked_mul(if kind == 6 { 2 } else { 1 })
                .context("Plist string overflow")?;
            let next = at
                .checked_add(size)
                .filter(|next| *next <= end)
                .context("Plist payload exceeds object")?;
            self.payload = self
                .payload
                .checked_add(size as u64)
                .context("Payload overflow")?;
            ensure!(
                self.payload <= self.limits.blob_bytes.saturating_mul(2),
                "Archive payload expansion limit"
            );
            let bytes = &self.bytes[at..next];
            return if kind == 4 {
                Ok(ArchiveNode::Data(bytes.to_vec()))
            } else {
                ensure!(size as u64 <= self.limits.text_bytes, "Archive text limit");
                Ok(ArchiveNode::Text(if kind == 5 {
                    std::str::from_utf8(bytes)?.into()
                } else {
                    String::from_utf16(
                        &bytes
                            .as_chunks::<2>()
                            .0
                            .iter()
                            .map(|p| u16::from_be_bytes(*p))
                            .collect::<Vec<_>>(),
                    )?
                }))
            };
        }
        if matches!(kind, 10..=13) {
            let n = self.length(&mut at, end, low)?;
            ensure!(n <= 1_000_000, "Plist collection count limit");
            if kind == 13 {
                let refs = self.refs(at, end, n.checked_mul(2).context("Dictionary overflow")?)?;
                return Ok(ArchiveNode::Dictionary(
                    refs[..n]
                        .iter()
                        .cloned()
                        .zip(refs[n..].iter().cloned())
                        .collect(),
                ));
            }
            let refs = self.refs(at, end, n)?;
            return Ok(if kind == 10 {
                ArchiveNode::Array(refs)
            } else {
                ArchiveNode::Set(refs)
            });
        }
        let count = match kind {
            0 => 0,
            1 => {
                ensure!(low <= 4, "Unsupported plist integer width");
                1usize << low
            }
            2 => {
                ensure!(matches!(low, 2 | 3), "Unsupported plist real width");
                1usize << low
            }
            3 => {
                ensure!(low == 3, "Unsupported plist date");
                8
            }
            8 => usize::from(low) + 1,
            _ => anyhow::bail!("Unknown binary plist object type"),
        };
        let next = at
            .checked_add(count)
            .filter(|n| *n <= end)
            .context("Truncated plist scalar")?;
        let data = &self.bytes[at..next];
        Ok(match kind {
            0 => match low {
                0 => ArchiveNode::Null,
                8 => ArchiveNode::Bool(false),
                9 => ArchiveNode::Bool(true),
                _ => ArchiveNode::Opaque {
                    kind: format!("plist-simple-{low}"),
                    bytes: vec![marker],
                },
            },
            1 => {
                if count < 8 {
                    ArchiveNode::Unsigned(unsigned(data)?)
                } else if count == 8 {
                    ArchiveNode::Signed(i64::from_be_bytes(data.try_into()?))
                } else if data[..8].iter().all(|b| *b == 0) {
                    ArchiveNode::Unsigned(unsigned(&data[8..])?)
                } else {
                    ArchiveNode::Opaque {
                        kind: "integer128".into(),
                        bytes: data.to_vec(),
                    }
                }
            }
            2 => {
                let value = if count == 4 {
                    f64::from(f32::from_be_bytes(data.try_into()?))
                } else {
                    f64::from_be_bytes(data.try_into()?)
                };
                ensure!(value.is_finite(), "Nonfinite plist real");
                ArchiveNode::Real(value)
            }
            3 => {
                let value = f64::from_be_bytes(data.try_into()?);
                ensure!(value.is_finite(), "Nonfinite plist date");
                ArchiveNode::Date(value)
            }
            8 => {
                if count <= 8 {
                    ArchiveNode::Uid(unsigned(data)?)
                } else {
                    ArchiveNode::Opaque {
                        kind: "wide-uid".into(),
                        bytes: data.to_vec(),
                    }
                }
            }
            _ => unreachable!(),
        })
    }
}
pub fn decode_keyed_archive(bytes: &[u8], limits: &ImportLimits) -> Result<ArchivedValue> {
    ensure!(
        bytes.starts_with(b"bplist00")
            && bytes.len() >= 40
            && bytes.len() as u64 <= limits.blob_bytes,
        "Invalid/oversize binary plist"
    );
    let trailer = &bytes[bytes.len() - 32..];
    let offset_size = usize::from(trailer[6]);
    let reference_size = usize::from(trailer[7]);
    ensure!(
        (1..=8).contains(&offset_size) && (1..=8).contains(&reference_size),
        "Invalid plist table widths"
    );
    let count = usize::try_from(unsigned(&trailer[8..16])?)?;
    let top = unsigned(&trailer[16..24])?;
    let table = usize::try_from(unsigned(&trailer[24..32])?)?;
    ensure!(
        count > 0 && count <= 1_000_000 && top < count as u64 && table >= 8,
        "Invalid plist object table"
    );
    let table_end = table
        .checked_add(
            count
                .checked_mul(offset_size)
                .context("Offset count overflow")?,
        )
        .filter(|end| *end <= bytes.len() - 32)
        .context("Offset table exceeds plist")?;
    let mut offsets = Vec::with_capacity(count);
    let mut ordered = BTreeSet::new();
    for value in bytes[table..table_end].chunks(offset_size) {
        let offset = usize::try_from(unsigned(value)?)?;
        ensure!(
            offset >= 8 && offset < table && ordered.insert(offset),
            "Invalid/duplicate object offset"
        );
        offsets.push(offset);
    }
    let mut ends = BTreeMap::new();
    let mut sorted = ordered.into_iter().collect::<Vec<_>>();
    sorted.push(table);
    for pair in sorted.windows(2) {
        ends.insert(pair[0], pair[1]);
    }
    let mut parser = Plist {
        bytes,
        reference_size,
        object_count: count,
        elements: 0,
        payload: 0,
        limits,
    };
    let mut graph = ArchivedValue::new(bytes, "bplist00", limits);
    for offset in offsets {
        graph
            .plist_objects
            .push(parser.object(offset, ends[&offset])?);
    }
    graph.root = ArchiveNode::Reference(top);
    if let Some(archiver) = graph.field(&graph.root, "$archiver")? {
        ensure!(
            graph.text(archiver)? == Some("NSKeyedArchiver"),
            "Unsupported archive producer"
        );
        let version = graph
            .field(&graph.root, "$version")?
            .context("Missing archive version")?;
        ensure!(
            graph.number(version)? == Some(100000.),
            "Unsupported keyed archive version"
        );
        let objects = graph
            .field(&graph.root, "$objects")?
            .context("Missing archive objects")?;
        graph.objects = graph
            .array(objects)?
            .context("Invalid archive object array")?
            .to_vec();
        let top = graph
            .field(&graph.root, "$top")?
            .context("Missing archive top")?;
        let mut root = graph
            .field(top, "root")?
            .or(graph.field(top, "NSKeyedArchiveRootObjectKey")?)
            .context("Unsupported archive root key")?
            .clone();
        // Expose the semantic NSKeyedArchiver root without flattening its UID.
        let mut seen = BTreeSet::new();
        while let ArchiveNode::Reference(index) = root {
            ensure!(
                seen.insert(index) && seen.len() <= limits.hierarchy_depth,
                "Cyclic/deep archive root reference"
            );
            root = graph
                .plist_objects
                .get(usize::try_from(index)?)
                .context("Archive root reference out of bounds")?
                .clone();
        }
        graph.metadata_root = Some(graph.root.clone());
        graph.root = root;
        for node in &graph.plist_objects {
            if let ArchiveNode::Uid(index) = node {
                ensure!(
                    *index < graph.objects.len() as u64,
                    "Archive UID out of bounds"
                );
            }
        }
    }
    Ok(graph)
}
