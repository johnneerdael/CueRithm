use super::{
    container::open_zip,
    projection::{key_value, track_value},
    *,
};
use anyhow::{Context, Result, bail, ensure};
use rusqlite::{Connection, OpenFlags, types::ValueRef};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Read, Seek, Write},
    path::Path,
};
use uuid::Uuid;
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ImportReport {
    pub warnings: Vec<String>,
    pub excluded: Vec<Value>,
    pub counts: BTreeMap<String, u64>,
}
#[derive(Debug, Clone)]
pub struct ImportPreview {
    pub archive: LogicalArchive,
    pub report: ImportReport,
}
const PROFILE: &str =
    include_str!("../../../../docs/research/relocation-backup-handover/lexicon-v26-profile.json");
fn id(ns: &Uuid, table: &str, source: &Value) -> Result<String> {
    let n = source
        .as_i64()
        .context("Source ID must be a signed 64-bit integer")?;
    Ok(Uuid::new_v5(ns, format!("{table}:{n}").as_bytes()).to_string())
}
fn source_object(v: &Value, l: &ArchiveLimits) -> Result<Value> {
    if v.is_null() {
        return Ok(json!({}));
    }
    let s = v.as_str().context("Source JSON must be text")?;
    ensure!(s.len() <= l.record_bytes, "Source JSON size limit");
    let value = parse_strict_json(s.as_bytes(), l.depth)?;
    ensure!(value.is_object(), "Source data must be object");
    Ok(value)
}
fn schema(conn: &Connection) -> Result<Value> {
    let p: Value = serde_json::from_str(PROFILE)?;
    let user: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    ensure!(
        user == p["pragma_user_version"].as_i64().unwrap(),
        "Unsupported Lexicon pragma version"
    );
    let mut stmt = conn.prepare(
        "SELECT type,name,tbl_name,sql FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'",
    )?;
    let mut seen = BTreeSet::new();
    for row in stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, Option<String>>(3)?,
        ))
    })? {
        let (kind, name, table, sql) = row?;
        seen.insert(name.clone());
        if kind == "index" {
            continue;
        }
        if kind == "trigger" || kind == "view" || name.starts_with("Track_FTS") {
            let expected = if kind == "trigger" || kind == "view" {
                p["inert_schema_objects"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|v| v["kind"] == kind && v["name"] == name && v["table"] == table)
            } else {
                p["inert_fts_tables"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|v| v["name"] == name)
            }
            .context("Unexpected executable/virtual Lexicon schema object")?;
            let sql = sql.context("Missing schema SQL")?;
            let normalized = sql.split_ascii_whitespace().collect::<Vec<_>>().join(" ");
            ensure!(
                format!("{:x}", Sha256::digest(normalized.as_bytes()))
                    == expected["normalized_sql_sha256"],
                "Unsupported Lexicon schema fingerprint"
            );
            continue;
        }
        ensure!(
            kind == "table"
                && sql
                    .as_deref()
                    .is_some_and(|s| !s.to_ascii_uppercase().contains("VIRTUAL TABLE")),
            "Unexpected source object"
        );
        ensure!(
            p["tables"].get(&name).is_some()
                || [
                    "Waveform",
                    "AlbumartPreview",
                    "HistorySession",
                    "HistoryTrack",
                    "ChartItem",
                    "CloudFile"
                ]
                .contains(&name.as_str()),
            "Unexpected Lexicon table"
        );
    }
    for (table, profile) in p["tables"].as_object().unwrap() {
        ensure!(seen.contains(table), "Missing Lexicon table {table}");
        let mut q = conn.prepare(&format!("PRAGMA table_info(\"{table}\")"))?;
        let cols = q
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, bool>(3)?,
                    r.get::<_, i64>(5)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let expected = profile["columns"].as_array().unwrap();
        ensure!(
            cols.len() == expected.len(),
            "Unsupported columns in {table}"
        );
        for (actual, expected) in cols.iter().zip(expected) {
            ensure!(
                actual.0 == expected["name"]
                    && actual.1.to_ascii_uppercase() == expected["declared_type"]
                    && actual.2 == expected["not_null"].as_bool().unwrap()
                    && actual.3 == expected["primary_key_position"].as_i64().unwrap(),
                "Column profile mismatch in {table}"
            );
        }
        let mut fk = conn.prepare(&format!("PRAGMA foreign_key_list(\"{table}\")"))?;
        let mut actual=fk.query_map([],|r|Ok(json!({"target_table":r.get::<_,String>(2)?,"from":r.get::<_,String>(3)?,"to":r.get::<_,String>(4)?,"on_update":r.get::<_,String>(5)?,"on_delete":r.get::<_,String>(6)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;
        let mut expected = profile["foreign_keys"].as_array().unwrap().clone();
        actual.sort_by_key(Value::to_string);
        expected.sort_by_key(Value::to_string);
        ensure!(
            actual == expected,
            "Foreign-key profile mismatch in {table}"
        );
    }
    Ok(p)
}
fn rows(conn: &Connection, p: &Value, table: &str, l: &ArchiveLimits) -> Result<Vec<Value>> {
    let columns = p["tables"][table]["columns"]
        .as_array()
        .context("No table profile")?;
    let names = columns
        .iter()
        .map(|v| v["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    let sql = format!(
        "SELECT {} FROM \"{table}\"",
        names
            .iter()
            .map(|n| format!("\"{n}\""))
            .collect::<Vec<_>>()
            .join(",")
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut query = stmt.query([])?;
    let mut result = vec![];
    let mut memory = 0u64;
    while let Some(row) = query.next()? {
        ensure!(
            result.len()
                < if table == "Track" {
                    l.tracks
                } else {
                    l.relations
                },
            "Source row limit"
        );
        let mut value = serde_json::Map::new();
        let mut bytes = 0usize;
        for (index, name) in names.iter().enumerate() {
            let v = match row.get_ref(index)? {
                ValueRef::Null => {
                    ensure!(
                        !columns[index]["not_null"].as_bool().unwrap(),
                        "NULL required source value in {table}.{name}"
                    );
                    Value::Null
                }
                ValueRef::Integer(v) => json!(v),
                ValueRef::Real(v) => {
                    ensure!(v.is_finite(), "Nonfinite source number");
                    json!(v)
                }
                ValueRef::Text(s) => {
                    bytes += s.len();
                    ensure!(bytes <= l.record_bytes, "Source record size limit");
                    json!(std::str::from_utf8(s)?)
                }
                ValueRef::Blob(_) => bail!("Unexpected source blob in {table}.{name}"),
            };
            value.insert(name.to_string(), v);
        }
        memory = memory
            .checked_add((serde_json::to_vec(&value)?.len() as u64).saturating_mul(8))
            .context("Source memory overflow")?;
        ensure!(
            memory <= l.memory_bytes,
            "Lexicon table exceeds in-memory staging budget"
        );
        result.push(Value::Object(value));
    }
    Ok(result)
}
fn string(v: &Value) -> Result<&str> {
    v.as_str()
        .context("Required source text has wrong storage type")
}
fn number(v: &Value) -> Result<f64> {
    v.as_f64()
        .context("Required source number has wrong storage type")
}
fn optional_positive(v: &Value) -> Result<Value> {
    let n = number(v)?;
    Ok(if n > 0.0 { json!(n) } else { Value::Null })
}
fn timestamp(v: &Value, tz: chrono_tz::Tz, report: &mut ImportReport) -> Option<i64> {
    use chrono::{LocalResult, TimeZone};
    let text = v.as_str()?;
    if text.is_empty() {
        return None;
    }
    if let Ok(d) = chrono::DateTime::parse_from_rfc3339(text) {
        return Some(d.timestamp_millis());
    }
    for fmt in ["%Y-%m-%d %H:%M:%S%.f", "%Y-%m-%dT%H:%M:%S%.f"] {
        if let Ok(d) = chrono::NaiveDateTime::parse_from_str(text, fmt) {
            return match tz.from_local_datetime(&d) {
                LocalResult::Single(t) => Some(t.timestamp_millis()),
                _ => {
                    report.warnings.push(
                        "Ambiguous or nonexistent source date retained without an epoch value"
                            .into(),
                    );
                    None
                }
            };
        }
    }
    report
        .warnings
        .push("Invalid source date retained without an epoch value".into());
    None
}
fn locator(a: &mut LogicalArchive, id: String, row: &Value) -> Result<()> {
    let kind = string(&row["type"])?;
    let v = if kind == "0" {
        let path = string(&row["location"])?;
        let windows = path.as_bytes().get(1) == Some(&b':')
            && path.as_bytes().first().is_some_and(u8::is_ascii_alphabetic);
        let unc = path.starts_with("\\\\") || path.starts_with("//");
        let flavor = if windows {
            "windows-drive"
        } else if unc {
            "windows-unc"
        } else {
            "posix"
        };
        let normalized = if windows || unc {
            path.replace('\\', "/")
        } else {
            path.to_string()
        };
        let safe = !path.contains('\0') && !normalized.split('/').any(|p| p == "." || p == "..");
        let device = normalized.starts_with("//?/") || normalized.starts_with("//./");
        let absolute = safe
            && !device
            && if windows {
                normalized.as_bytes().get(2) == Some(&b'/')
            } else {
                unc || path.starts_with('/')
            };
        let (root, components) = if absolute {
            let parts = normalized
                .split('/')
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>();
            let root_count = if windows {
                2.min(parts.len())
            } else if unc {
                3.min(parts.len())
            } else {
                1.min(parts.len())
            };
            let prefix = if windows {
                ""
            } else if unc {
                "//"
            } else {
                "/"
            };
            let root = format!("{prefix}{}", parts[..root_count].join("/"));
            let cs = parts[root_count..]
                .iter()
                .map(|s| json!(s))
                .collect::<Vec<_>>();
            (Some(root), Some(cs))
        } else {
            (None, None)
        };
        let root_id = root.as_ref().map(|root| {
            Uuid::new_v5(
                &Uuid::NAMESPACE_URL,
                format!("lexicon-root:{flavor}:{root}").as_bytes(),
            )
            .to_string()
        });
        if let (Some(rid), Some(root)) = (&root_id, &root)
            && !a.records["roots"].iter().any(|r| &r.id == rid)
        {
            a.push("roots",LogicalRecord::new(rid,json!({"label":"Imported music root","source_flavor":flavor,"source_root":{"encoding":"utf8","data":root},"case_rule":"unknown"})));
        }
        json!({"kind":"local","source_flavor":flavor,"original":{"encoding":"utf8","data":path},"root_id":root_id,"components":components})
    } else if kind == "1" {
        let service = string(&row["streamingService"])?;
        let remote = string(&row["streamingId"])?;
        ensure!(
            !service.is_empty() && !remote.is_empty(),
            "Streaming reference missing service/remote ID"
        );
        json!({"kind":"streaming","service":service,"remote_id":remote})
    } else {
        json!({"kind":"unsupported","source_kind":kind,"source_value":{"location":row["location"],"streamingService":row["streamingService"],"streamingId":row["streamingId"]}})
    };
    a.push("locators", LogicalRecord::new(id, v));
    Ok(())
}
fn smart(
    value: &Value,
    playlist: &str,
    report: &mut ImportReport,
    l: &ArchiveLimits,
) -> Result<Option<Value>> {
    if value.is_null() {
        return Ok(None);
    }
    let raw = source_object(value, l)?;
    let rules = raw["rules"]
        .as_array()
        .context("Smart rules must be array")?;
    ensure!(rules.len() <= 256, "Source smart rule limit");
    let mut out = vec![];
    for (index, r) in rules.iter().enumerate() {
        let field = r["field"].as_str().unwrap_or("");
        let op = r["operator"].as_str().unwrap_or("");
        let values = r["values"].as_array();
        let one = values.and_then(|v| v.first());
        let val = match op {
            "StringContains" | "StringEquals" | "StringEndsWith" | "KeySimilar"
                if one.is_some_and(Value::is_string) =>
            {
                json!({"kind":"text","value":one.unwrap()})
            }
            "NumberGreaterThanEquals" if one.is_some_and(Value::is_number) => {
                json!({"kind":"number","value":one.unwrap()})
            }
            "NumberBetween"
                if values.is_some_and(|v| v.len() >= 2 && v[0].is_number() && v[1].is_number()) =>
            {
                let v = values.unwrap();
                let x = v[0].as_f64().unwrap();
                let y = v[1].as_f64().unwrap();
                json!({"kind":"number_range","value":{"start":x.min(y),"end":x.max(y)}})
            }
            _ => {
                report.warnings.push(format!(
                    "Smartlist {playlist}: unsupported rule; entire definition disabled"
                ));
                return Ok(None);
            }
        };
        if ![
            "key", "bpm", "remixer", "artist", "genre", "comment", "energy",
        ]
        .contains(&field)
            || rules.len() > 128
        {
            report.warnings.push(format!(
                "Smartlist {playlist}: unsupported field or rule count; disabled"
            ));
            return Ok(None);
        }
        out.push(json!({"id":format!("source-rule-{index}"),"field":field,"operator":op,"value":val,"or":r["or"].as_bool().unwrap_or(false)}));
    }
    let native = json!({"match_all":raw["matchAll"].as_bool().context("Invalid source matchAll")?,"rules":out});
    let _: cuerithm_smartlists::SmartDefinition = serde_json::from_value(native.clone())?;
    Ok(Some(native))
}
pub fn read_lexicon_archive(
    path: &Path,
    time_zone: &str,
    l: &ArchiveLimits,
) -> Result<ImportPreview> {
    read_lexicon_archive_with_control(path, time_zone, l, &|_, _| Ok(()))
}
/// Compatibility helper for small callers and authored fixtures. Production
/// inspection/restoration consumes `read_lexicon_spooled` without materializing.
pub fn read_lexicon_archive_with_control(
    path: &Path,
    time_zone: &str,
    l: &ArchiveLimits,
    control: &dyn Fn(&str, u64) -> Result<()>,
) -> Result<ImportPreview> {
    let preview = read_lexicon_spooled(path, time_zone, l, control)?;
    let mut archive = LogicalArchive::empty(time_zone)?;
    archive.manifest = preview.archive.manifest.clone();
    let mut resident = 0usize;
    for (kind, _) in KINDS {
        let output = archive.records.get_mut(*kind).unwrap();
        output.clear();
        preview.archive.for_each(kind, &mut |record| {
            control("materialize", resident as u64)?;
            resident = resident
                .saturating_add(record.id.capacity())
                .saturating_add(128)
                .saturating_add(value_allocation(&record.value))
                .saturating_add(
                    record
                        .extensions
                        .iter()
                        .map(|(key, value)| {
                            128usize
                                .saturating_add(key.capacity().saturating_mul(2))
                                .saturating_add(value_allocation(value))
                        })
                        .sum::<usize>(),
                );
            ensure!(
                resident as u64 <= l.memory_bytes,
                "Materialized Lexicon view exceeds memory budget; use the streaming API"
            );
            output.push(record);
            Ok(())
        })?;
    }
    validate_logical_archive(&archive, l)?;
    Ok(ImportPreview {
        archive,
        report: preview.report,
    })
}
fn value_allocation(value: &Value) -> usize {
    64usize.saturating_add(match value {
        Value::String(text) => text.capacity().saturating_mul(2),
        Value::Array(items) => items.iter().map(value_allocation).sum(),
        Value::Object(fields) => fields
            .iter()
            .map(|(key, value)| {
                128usize
                    .saturating_add(key.capacity().saturating_mul(2))
                    .saturating_add(value_allocation(value))
            })
            .sum(),
        _ => 0,
    })
}

#[path = "lexicon_stream.rs"]
mod stream;
pub use stream::{ImportPreviewSpooled, read_lexicon_spooled};
