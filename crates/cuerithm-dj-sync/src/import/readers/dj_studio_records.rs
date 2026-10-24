//! Data-only filesystem-record names and strict bounded JSON; no renderer execution.
use super::*;
use serde::de::{DeserializeSeed, Deserializer, Error, MapAccess, SeqAccess, Visitor};
pub fn shard(key: &str) -> Result<String> {
    validate_key(key)?;
    Ok(format!(
        "{:x}",
        key.encode_utf16()
            .fold(0u8, |sum, unit| sum.wrapping_add(unit as u8))
    ))
}
pub fn validate_key(key: &str) -> Result<()> {
    ensure!(
        !key.is_empty()
            && key.len() <= 4096
            && !key.starts_with('/')
            && !key
                .chars()
                .any(|c| c.is_control()
                    || matches!(c, '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')),
        "Unsafe DJ.Studio record key"
    );
    ensure!(
        key.split('/')
            .all(|part| !part.is_empty() && part != "." && part != ".."),
        "Unsafe DJ.Studio record key components"
    );
    Ok(())
}
pub fn record_path(binding: &str, key: &str) -> Result<PathBuf> {
    let binding = binding.replace('.', "-");
    ensure!(
        matches!(
            binding.as_str(),
            "audio-library"
                | "track-structures"
                | "projects"
                | "projects-meta"
                | "mix-data"
                | "automation-data"
        ),
        "Unsupported DJ.Studio record binding"
    );
    Ok(PathBuf::from("Database")
        .join(format!("{binding}-table"))
        .join(shard(key)?)
        .join(key))
}
struct Seed<'a> {
    limits: &'a ImportLimits,
    depth: usize,
    nodes: &'a mut usize,
}
impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = Value;
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> std::result::Result<Value, D::Error> {
        *self.nodes += 1;
        if self.depth > self.limits.hierarchy_depth || *self.nodes > 1_000_000 {
            return Err(D::Error::custom("DJ.Studio JSON depth/node budget"));
        }
        d.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for Seed<'_> {
    type Value = Value;
    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("bounded JSON data")
    }
    fn visit_unit<E: Error>(self) -> std::result::Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_bool<E: Error>(self, v: bool) -> std::result::Result<Value, E> {
        Ok(json!(v))
    }
    fn visit_i64<E: Error>(self, v: i64) -> std::result::Result<Value, E> {
        Ok(json!(v))
    }
    fn visit_u64<E: Error>(self, v: u64) -> std::result::Result<Value, E> {
        Ok(json!(v))
    }
    fn visit_f64<E: Error>(self, v: f64) -> std::result::Result<Value, E> {
        serde_json::Number::from_f64(v)
            .map(Value::Number)
            .ok_or_else(|| E::custom("Nonfinite DJ.Studio JSON number"))
    }
    fn visit_str<E: Error>(self, v: &str) -> std::result::Result<Value, E> {
        self.visit_string(v.to_owned())
    }
    fn visit_string<E: Error>(self, v: String) -> std::result::Result<Value, E> {
        if v.len() as u64 > self.limits.text_bytes {
            return Err(E::custom("DJ.Studio JSON string budget"));
        }
        Ok(Value::String(v))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> std::result::Result<Value, A::Error> {
        let mut values = Vec::new();
        while let Some(value) = seq.next_element_seed(Seed {
            limits: self.limits,
            depth: self.depth + 1,
            nodes: &mut *self.nodes,
        })? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> std::result::Result<Value, A::Error> {
        let mut values = serde_json::Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if key.len() as u64 > self.limits.text_bytes || values.contains_key(&key) {
                return Err(A::Error::custom("Duplicate/oversize DJ.Studio JSON key"));
            }
            let value = map.next_value_seed(Seed {
                limits: self.limits,
                depth: self.depth + 1,
                nodes: &mut *self.nodes,
            })?;
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}
pub fn decode_record(bytes: &[u8], limits: &ImportLimits) -> Result<Value> {
    ensure!(
        bytes.len() as u64 <= limits.text_bytes,
        "DJ.Studio record byte budget"
    );
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    let mut nodes = 0;
    let value = Seed {
        limits,
        depth: 0,
        nodes: &mut nodes,
    }
    .deserialize(&mut decoder)?;
    decoder.end()?;
    ensure!(value.is_object(), "DJ.Studio record is not an object");
    Ok(value)
}
pub(super) fn numeric_key(value: &Value) -> Option<String> {
    let n = value.as_i64().filter(|n| (0..24).contains(n))? as usize;
    Some(
        [
            "8B", "3B", "10B", "5B", "12B", "7B", "2B", "9B", "4B", "11B", "6B", "1B", "8A", "3A",
            "10A", "5A", "12A", "7A", "2A", "9A", "4A", "11A", "6A", "1A",
        ][n]
            .into(),
    )
}
