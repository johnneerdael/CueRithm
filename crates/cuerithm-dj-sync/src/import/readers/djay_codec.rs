//! Data-only TSAF records; alignment is absolute within the original blob.
use super::keyed_archive::{ArchiveNode, ArchivedValue};
use super::*;
struct Decoder<'a> {
    bytes: &'a [u8],
    at: usize,
    graph: ArchivedValue,
    nodes: usize,
}
impl Decoder<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8]> {
        let end = self
            .at
            .checked_add(n)
            .filter(|end| *end <= self.bytes.len())
            .context("djay field exceeds record")?;
        let out = &self.bytes[self.at..end];
        self.at = end;
        Ok(out)
    }
    fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn aligned<const N: usize>(&mut self) -> Result<[u8; N]> {
        self.at = self.at.checked_add(N - 1).context("Alignment overflow")? / N * N;
        Ok(self.take(N)?.try_into()?)
    }
    fn count(&mut self) -> Result<usize> {
        let n = u32::from_le_bytes(self.aligned()?) as usize;
        ensure!(
            n <= 1_000_000 && n <= self.bytes.len().saturating_sub(self.at),
            "djay count exceeds record/limit"
        );
        Ok(n)
    }
    fn string(&mut self, register: bool) -> Result<String> {
        let n = self
            .bytes
            .get(self.at..)
            .context("Truncated djay string")?
            .iter()
            .position(|b| *b == 0)
            .context("Unterminated djay string")?;
        ensure!(
            n as u64 <= self.graph.limits.text_bytes,
            "djay text byte limit"
        );
        let value = std::str::from_utf8(self.take(n)?)?.to_owned();
        self.take(1)?;
        if register {
            ensure!(
                self.graph.strings.len() < 1_000_000,
                "djay string count limit"
            );
            self.graph.strings.push(ArchiveNode::Text(value.clone()));
        }
        Ok(value)
    }
    fn field_name(&mut self, depth: usize) -> Result<String> {
        let node = self.value(depth, false)?;
        Ok(self
            .graph
            .text(&node)?
            .context("Nonstring djay field/class name")?
            .to_owned())
    }
    fn fields(&mut self, count: Option<usize>, depth: usize) -> Result<Vec<(String, ArchiveNode)>> {
        let mut out = Vec::new();
        while count.is_none_or(|n| out.len() < n) {
            if count.is_none() && self.bytes.get(self.at) == Some(&0) {
                self.at += 1;
                break;
            }
            ensure!(out.len() < 1_000_000, "djay field count limit");
            let value = self.value(depth + 1, false)?;
            let name = self.field_name(depth + 1)?;
            out.push((name, value));
        }
        Ok(out)
    }
    fn value(&mut self, depth: usize, counted: bool) -> Result<ArchiveNode> {
        self.nodes += 1;
        ensure!(
            depth <= self.graph.limits.hierarchy_depth && self.nodes <= 1_000_000,
            "djay nesting/node limit"
        );
        let tag = self.byte()?;
        Ok(match tag {
            0 => ArchiveNode::Null,
            8 => ArchiveNode::Text(self.string(true)?),
            5 => {
                let index = self.byte()?;
                ensure!(
                    (index as usize) < self.graph.strings.len(),
                    "djay string pointer out of bounds"
                );
                ArchiveNode::StringPointer(index)
            }
            2 => {
                let index = self.byte()?;
                ensure!(
                    (index as usize) < self.graph.numbers.len(),
                    "djay number pointer out of bounds"
                );
                ArchiveNode::NumberPointer(index)
            }
            20 => {
                let value = f64::from_le_bytes(self.aligned()?);
                ensure!(value.is_finite(), "Nonfinite djay double");
                ArchiveNode::Real(value)
            }
            19 => {
                let value = f64::from(f32::from_le_bytes(self.aligned()?));
                ensure!(value.is_finite(), "Nonfinite djay float");
                let node = ArchiveNode::Real(value);
                self.graph.numbers.push(node.clone());
                node
            }
            48 => {
                let value = f64::from_le_bytes(self.aligned()?);
                ensure!(value.is_finite(), "Nonfinite djay date");
                let node = ArchiveNode::Date(value);
                self.graph.numbers.push(node.clone());
                node
            }
            16 => ArchiveNode::Unsigned(u64::from(u16::from_le_bytes(self.aligned()?))),
            17 => ArchiveNode::Unsigned(u64::from(u32::from_le_bytes(self.aligned()?))),
            18 => ArchiveNode::Unsigned(u64::from_le_bytes(self.aligned()?)),
            13 => ArchiveNode::Bool(true),
            14 => ArchiveNode::Bool(false),
            46 => ArchiveNode::Unsigned(0),
            45 => ArchiveNode::Unsigned(1),
            15 => ArchiveNode::Unsigned(u64::from(self.byte()?)),
            33 => {
                ensure!(self.byte()? == 8, "Unknown djay URI envelope");
                let uri = self.string(false)?;
                ensure!(self.byte()? == 0, "Missing djay URI terminator");
                ArchiveNode::Uri(uri)
            }
            21 => {
                let n = self.count()?;
                ensure!(
                    n as u64 <= self.graph.limits.blob_bytes,
                    "djay binary block limit"
                );
                let bytes = self.take(n)?.to_vec();
                self.at = self.at.checked_add(3).context("Block alignment overflow")? / 4 * 4;
                self.take(0)?;
                ArchiveNode::Data(bytes)
            }
            43 => {
                let name = self.field_name(depth + 1)?;
                let fields = self.fields(None, depth + 1)?;
                ArchiveNode::Class { name, fields }
            }
            24 => {
                if counted {
                    let n = self.count()?;
                    let fields = self.fields(Some(n), depth + 1)?;
                    ArchiveNode::Dictionary(
                        fields
                            .into_iter()
                            .map(|(name, value)| (ArchiveNode::Text(name), value))
                            .collect(),
                    )
                } else {
                    ArchiveNode::Opaque {
                        kind: "tsaf-control".into(),
                        bytes: self.aligned::<4>()?.to_vec(),
                    }
                }
            }
            27 | 12 | 10 | 11 | 26 => {
                let n = self.count()?;
                let mut out = Vec::with_capacity(n);
                for _ in 0..n {
                    out.push(self.value(depth + 1, true)?)
                }
                ArchiveNode::Array(out)
            }
            _ => anyhow::bail!("Unsupported djay value tag {tag:#x}"),
        })
    }
}
pub fn decode_djay_stream(bytes: &[u8], limits: &ImportLimits) -> Result<ArchivedValue> {
    ensure!(
        bytes.len() >= 21
            && bytes.len() as u64 <= limits.blob_bytes
            && bytes[..8] == *b"TSAF\x03\0\x03\0"
            && bytes[20] == 43,
        "Unqualified djay TSAF header/class marker"
    );
    let mut decoder = Decoder {
        bytes,
        at: 20,
        graph: ArchivedValue::new(bytes, "TSAF-3.0-3.0", limits),
        nodes: 0,
    };
    let root = decoder.value(0, false)?;
    ensure!(decoder.at == bytes.len(), "Trailing/truncated djay record");
    decoder.graph.root = root;
    Ok(decoder.graph)
}
