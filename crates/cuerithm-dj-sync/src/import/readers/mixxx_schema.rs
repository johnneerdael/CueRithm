use super::*;
use rusqlite::{Connection, OpenFlags, limits::Limit, types::ValueRef};
pub(super) const TABLES: [&str; 9] = [
    "library",
    "track_locations",
    "cues",
    "Playlists",
    "PlaylistTracks",
    "crates",
    "crate_tracks",
    "directories",
    "settings",
];
pub(super) fn scalar(value: ValueRef<'_>) -> Result<Value> {
    Ok(match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(v) => json!(v),
        ValueRef::Real(v) => {
            ensure!(v.is_finite(), "Nonfinite source SQL number");
            json!(v)
        }
        ValueRef::Text(v) => json!(std::str::from_utf8(v)?),
        ValueRef::Blob(_) => anyhow::bail!("Unexpected binary schema cell"),
    })
}
fn pragma(db: &Connection, sql: &str) -> Result<Vec<Value>> {
    let mut statement = db.prepare(sql)?;
    let names = statement
        .column_names()
        .iter()
        .map(|n| n.to_string())
        .collect::<Vec<_>>();
    let mut rows = statement.query([])?;
    let mut values = Vec::new();
    while let Some(row) = rows.next()? {
        let mut value = serde_json::Map::new();
        for (index, name) in names.iter().enumerate() {
            value.insert(
                if name == "dflt_value" {
                    "default"
                } else {
                    name
                }
                .into(),
                scalar(row.get_ref(index)?)?,
            );
        }
        values.push(Value::Object(value));
    }
    Ok(values)
}
fn quoted(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}
fn fingerprint(db: &Connection) -> Result<String> {
    let mut tables = serde_json::Map::new();
    let mut indexes = serde_json::Map::new();
    let mut foreign = serde_json::Map::new();
    for table in TABLES {
        let (kind, sql): (String, String) = db.query_row(
            "SELECT type,sql FROM sqlite_master WHERE name=?1",
            [table],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        ensure!(
            kind == "table" && !sql.to_ascii_uppercase().contains("CREATE VIRTUAL TABLE"),
            "Mixxx requires ordinary music tables"
        );
        tables.insert(
            table.into(),
            json!(pragma(
                db,
                &format!("PRAGMA table_info({})", quoted(table))
            )?),
        );
        let mut native_indexes = pragma(db, &format!("PRAGMA index_list({})", quoted(table)))?;
        for index in &mut native_indexes {
            let name = index["name"].as_str().context("Invalid index name")?;
            let columns = pragma(db, &format!("PRAGMA index_info({})", quoted(name)))?;
            index
                .as_object_mut()
                .context("Invalid index")?
                .insert("columns".into(), json!(columns));
        }
        indexes.insert(table.into(), json!(native_indexes));
        foreign.insert(
            table.into(),
            json!(pragma(
                db,
                &format!("PRAGMA foreign_key_list({})", quoted(table))
            )?),
        );
    }
    Ok(digest(&serde_json::to_vec(
        &json!({"tables":tables,"indexes":indexes,"foreign_keys":foreign}),
    )?))
}
pub(super) fn open(
    snapshot: &ImportSnapshot,
    options: &ReadOptions,
) -> Result<(Connection, SourceProfile, u32)> {
    ensure!(
        snapshot.provider == SourceProvider::Mixxx
            && snapshot.artifacts.len() == 1
            && matches!(snapshot.artifacts[0].kind, ArtifactKind::Sqlite),
        "Choose one snapshotted Mixxx SQLite library"
    );
    let db = Connection::open_with_flags(
        snapshot.artifact(&snapshot.artifacts[0].id)?,
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    let limit = options
        .limits
        .blob_bytes
        .max(options.limits.text_bytes)
        .min(i32::MAX as u64) as i32;
    db.set_limit(Limit::SQLITE_LIMIT_LENGTH, limit)?;
    db.pragma_update(None, "query_only", true)?;
    let actual = fingerprint(&db)?;
    let version: String = db.query_row(
        "SELECT value FROM settings WHERE name='mixxx.schema.version'",
        [],
        |r| r.get(0),
    )?;
    let version = version
        .parse::<u32>()
        .context("Invalid Mixxx schema setting")?;
    let expected = match version {
        39 => "26d1d5d532abd4aa903b37591c3a50e7160b862d54999f1595f84af25a2e13c0",
        40 => "48f154557323db8ff4f7c686e64464af95a92df7ba34a429c663290a7b25f9f8",
        _ => anyhow::bail!("Unqualified Mixxx schema revision"),
    };
    ensure!(
        actual == expected,
        "Mixxx music table/column/index/foreign-key profile differs from the pinned schema"
    );
    let mut profile = profile(
        SourceProvider::Mixxx,
        &format!("mixxx-schema{version}"),
        &[
            "descriptive_tags",
            "key",
            "bpm",
            "beatgrid",
            "hot_cues",
            "main_cues",
            "load_cues",
            "loops",
            "markers",
            "ratings",
            "color",
            "history",
            "collections",
        ],
    );
    profile.schema_fingerprint = actual;
    profile
        .origin
        .insert("schema_revision".into(), version.to_string());
    Ok((db, profile, version))
}
