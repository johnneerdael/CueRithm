//! Owned before/after data for coordinated donor publication.
use crate::model::{bytes_path, path_bytes};
use anyhow::{Context, Result, ensure};
use cuerithm_autotag::{
    Metadata, MetadataField as F, TransferDate, TransferSnapshot, TransferText,
};
use cuerithm_maintenance::{ImportField as I, ImportRecord};
use rusqlite::{
    Connection, params, params_from_iter,
    types::{Value as SqlValue, ValueRef},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::PathBuf};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(super) enum Cell {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}
impl Cell {
    fn capture(value: ValueRef<'_>) -> Result<Self> {
        Ok(match value {
            ValueRef::Null => Self::Null,
            ValueRef::Integer(n) => Self::Integer(n),
            ValueRef::Real(n) => {
                ensure!(n.is_finite(), "Nonfinite library value");
                Self::Real(n)
            }
            ValueRef::Text(t) => Self::Text(std::str::from_utf8(t)?.into()),
            ValueRef::Blob(b) => Self::Blob(b.into()),
        })
    }
    fn sql(&self) -> SqlValue {
        match self {
            Self::Null => SqlValue::Null,
            Self::Integer(n) => SqlValue::Integer(*n),
            Self::Real(n) => SqlValue::Real(*n),
            Self::Text(s) => SqlValue::Text(s.clone()),
            Self::Blob(b) => SqlValue::Blob(b.clone()),
        }
    }
    fn json(&self) -> Value {
        match self {
            Self::Null => Value::Null,
            Self::Integer(n) => json!(n),
            Self::Real(n) => json!(n),
            Self::Text(s) => json!(s),
            Self::Blob(b) => json!(b),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct RowImage {
    pub columns: Vec<String>,
    pub values: Vec<Cell>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct TrackImage {
    pub row: RowImage,
    pub cues: Vec<RowImage>,
    pub tags: Vec<String>,
}
fn capture_rows(conn: &Connection, table: &str, key: &str, id: &str) -> Result<Vec<RowImage>> {
    ensure!(
        matches!(
            (table, key),
            ("tracks", "id") | ("cue_revisions", "track_id")
        ),
        "Invalid snapshot table"
    );
    let mut stmt = conn.prepare(&format!(
        "SELECT * FROM {table} WHERE {key}=?1 ORDER BY rowid"
    ))?;
    let columns = stmt
        .column_names()
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut rows = stmt.query([id])?;
    let mut result = vec![];
    while let Some(row) = rows.next()? {
        let mut values = vec![];
        for i in 0..columns.len() {
            values.push(Cell::capture(row.get_ref(i)?)?);
        }
        result.push(RowImage {
            columns: columns.clone(),
            values,
        });
    }
    Ok(result)
}
impl RowImage {
    pub fn get(&self, name: &str) -> Result<&Cell> {
        let i = self
            .columns
            .iter()
            .position(|c| c == name)
            .context("Snapshot column missing")?;
        self.values.get(i).context("Snapshot value missing")
    }
    pub fn text(&self, name: &str) -> Result<Option<&str>> {
        Ok(match self.get(name)? {
            Cell::Text(s) => Some(s),
            Cell::Null => None,
            _ => anyhow::bail!("Invalid text column"),
        })
    }
    pub fn value(&self, name: &str) -> Result<Value> {
        Ok(self.get(name)?.json())
    }
    pub fn id(&self) -> Result<&str> {
        self.text("id")?.context("Missing track identity")
    }
    pub fn path(&self) -> Result<PathBuf> {
        match self.get("path")? {
            Cell::Blob(b) => bytes_path(b),
            _ => anyhow::bail!("Invalid track path"),
        }
    }
    pub fn hash(&self) -> Result<&str> {
        self.text("source_hash")?.context("Missing source identity")
    }
}
impl TrackImage {
    pub fn capture(conn: &Connection, id: &str) -> Result<Self> {
        let row = capture_rows(conn, "tracks", "id", id)?
            .into_iter()
            .next()
            .context("Track no longer exists")?;
        let cues = capture_rows(conn, "cue_revisions", "track_id", id)?;
        let mut q =
            conn.prepare("SELECT tag_id FROM track_custom_tags WHERE track_id=?1 ORDER BY tag_id")?;
        let tags = q
            .query_map([id], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(Self { row, cues, tags })
    }
    pub fn publication(&self) -> Result<Option<Value>> {
        if let Some(last) = self.cues.last() {
            let v: Value =
                serde_json::from_str(last.text("json")?.context("Missing cue payload")?)?;
            if v["schema"] == "lexicon-publication-v1" {
                return Ok(Some(v));
            }
            if v.get("cues")
                .and_then(Value::as_array)
                .is_some_and(|c| !c.is_empty())
            {
                anyhow::bail!("Donor cue data needs reanalysis before lossless import");
            }
        }
        let analysis: Value = self
            .row
            .text("analysis_json")?
            .map(serde_json::from_str)
            .transpose()?
            .unwrap_or(Value::Null);
        Ok(analysis
            .get("publication")
            .filter(|p| !p.is_null())
            .cloned())
    }
    pub fn text_values(&self, native: &TransferSnapshot) -> Result<BTreeMap<F, Vec<String>>> {
        let stored: Option<Metadata> = self
            .row
            .text("native_metadata_json")?
            .map(serde_json::from_str)
            .transpose()?;
        let mut result = stored
            .map(|m| m.fields)
            .unwrap_or_else(|| native.text.clone());
        for (f, column) in [
            (F::Title, "title"),
            (F::Artist, "artist"),
            (F::Album, "album"),
            (F::Genre, "genre"),
        ] {
            let display = self.row.text(column)?.unwrap_or("");
            if result.get(&f).map(|v| v.join(", ")).as_deref() != Some(display) {
                if display.is_empty() {
                    result.remove(&f);
                } else {
                    result.insert(f, vec![display.into()]);
                }
            }
        }
        Ok(result)
    }
    pub fn record(&self, native: &TransferSnapshot) -> Result<ImportRecord> {
        let text = self.text_values(native)?;
        let mut fields = BTreeMap::new();
        for (source, target) in field_pairs() {
            fields.insert(
                source,
                text.get(&target)
                    .map(|v| json!(v.join(", ")))
                    .unwrap_or(Value::Null),
            );
        }
        for (f, column) in [
            (I::Key, "musical_key"),
            (I::Bpm, "bpm"),
            (I::Energy, "energy"),
        ] {
            fields.insert(f, self.row.value(column)?);
        }
        for (f, n) in [
            (I::Color, TransferText::Color),
            (I::Extra1, TransferText::Extra1),
            (I::Extra2, TransferText::Extra2),
        ] {
            fields.insert(
                f,
                native
                    .extra
                    .get(&n)
                    .map(|v| json!(v.join(", ")))
                    .unwrap_or(Value::Null),
            );
        }
        fields.insert(
            I::Rating,
            native.rating.map(|n| json!(n)).unwrap_or(Value::Null),
        );
        fields.insert(
            I::TrackNumber,
            native.track_number.map(|n| json!(n)).unwrap_or(Value::Null),
        );
        fields.insert(
            I::Year,
            native
                .date
                .as_ref()
                .map(|d| json!(d.year))
                .unwrap_or(Value::Null),
        );
        fields.insert(
            I::ReleaseDate,
            native
                .date
                .as_ref()
                .map(|d| {
                    json!(match (d.month, d.day) {
                        (Some(m), Some(day)) => format!("{:04}-{m:02}-{day:02}", d.year),
                        _ => format!("{:04}", d.year),
                    })
                })
                .unwrap_or(Value::Null),
        );
        let pubdata = self.publication()?;
        let cues = pubdata
            .as_ref()
            .and_then(|p| p["cues"].as_array())
            .cloned()
            .unwrap_or_default();
        let tempo_markers = pubdata
            .as_ref()
            .map(|p| vec![json!({"bpm":p["grid"]["bpm"],"startTime":p["grid"]["startTime"]})])
            .unwrap_or_default();
        let record = ImportRecord {
            fields,
            cues,
            tempo_markers,
            tags: self.tags.iter().map(|id| json!(id)).collect(),
            identity: BTreeMap::from([
                ("id".into(), json!(self.row.id()?)),
                ("source_hash".into(), json!(self.row.hash()?)),
                ("path".into(), json!(path_bytes(&self.row.path()?))),
            ]),
        };
        record.validate()?;
        Ok(record)
    }
}
fn field_pairs() -> [(I, F); 12] {
    [
        (I::Title, F::Title),
        (I::Artist, F::Artist),
        (I::AlbumTitle, F::Album),
        (I::Genre, F::Genre),
        (I::Comment, F::Comment),
        (I::Grouping, F::Grouping),
        (I::Label, F::Label),
        (I::Remixer, F::Remixer),
        (I::Mix, F::Mix),
        (I::Producer, F::Producer),
        (I::Composer, F::Composer),
        (I::Lyricist, F::Lyricist),
    ]
}
pub(super) fn native_after(
    target: &TransferSnapshot,
    donor: &TransferSnapshot,
    source: &TrackImage,
    record: &ImportRecord,
) -> Result<TransferSnapshot> {
    let mut result = target.clone();
    let vectors = source.text_values(donor)?;
    for (f, n) in field_pairs() {
        let display = record.fields.get(&f).and_then(Value::as_str).unwrap_or("");
        if display.is_empty() {
            result.text.remove(&n);
        } else {
            let values = vectors
                .get(&n)
                .filter(|v| v.join(", ") == display)
                .cloned()
                .unwrap_or_else(|| vec![display.into()]);
            result.text.insert(n, values);
        }
    }
    for (f, n) in [
        (I::Key, TransferText::Key),
        (I::Color, TransferText::Color),
        (I::Extra1, TransferText::Extra1),
        (I::Extra2, TransferText::Extra2),
    ] {
        let value = record
            .fields
            .get(&f)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty());
        if let Some(v) = value {
            let values = donor
                .extra
                .get(&n)
                .filter(|values| values.join(", ") == v)
                .cloned()
                .unwrap_or_else(|| vec![v.into()]);
            result.extra.insert(n, values);
        } else {
            result.extra.remove(&n);
        }
    }
    if let Some(energy) = record
        .fields
        .get(&I::Energy)
        .and_then(Value::as_u64)
        .filter(|v| *v > 0)
    {
        result
            .extra
            .insert(TransferText::Energy, vec![energy.to_string()]);
    } else {
        result.extra.remove(&TransferText::Energy);
    }
    result.bpm = record
        .fields
        .get(&I::Bpm)
        .and_then(Value::as_f64)
        .filter(|v| *v > 0.);
    result.rating = record
        .fields
        .get(&I::Rating)
        .and_then(Value::as_u64)
        .filter(|v| *v > 0)
        .map(|v| v as u8);
    result.track_number = record
        .fields
        .get(&I::TrackNumber)
        .and_then(Value::as_u64)
        .filter(|v| *v > 0)
        .map(|v| v as u16);
    result.date = if let Some(value) = record
        .fields
        .get(&I::ReleaseDate)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        let pieces = value.split('-').collect::<Vec<_>>();
        match pieces.as_slice() {
            [year] => Some(TransferDate {
                year: year.parse()?,
                month: None,
                day: None,
            }),
            [year, month, day] => Some(TransferDate {
                year: year.parse()?,
                month: Some(month.parse()?),
                day: Some(day.parse()?),
            }),
            _ => anyhow::bail!("Invalid imported release date"),
        }
    } else {
        record
            .fields
            .get(&I::Year)
            .and_then(Value::as_u64)
            .filter(|v| *v > 0)
            .map(|year| TransferDate {
                year: year as i32,
                month: None,
                day: None,
            })
    };
    result.validate()?;
    Ok(result)
}
pub(super) fn write_import(
    conn: &Connection,
    before: &TrackImage,
    record: &ImportRecord,
    native: &TransferSnapshot,
    info: &cuerithm_media::MediaInfo,
    hash: &str,
    donor: &str,
) -> Result<()> {
    let id = before.row.id()?;
    let string = |f| {
        record
            .fields
            .get(&f)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned()
    };
    let title = string(I::Title);
    let artist = string(I::Artist);
    let album = string(I::AlbumTitle);
    let genre = string(I::Genre);
    let key = record
        .fields
        .get(&I::Key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty());
    let bpm = record
        .fields
        .get(&I::Bpm)
        .and_then(Value::as_f64)
        .filter(|b| *b > 0.);
    let energy = record
        .fields
        .get(&I::Energy)
        .and_then(Value::as_u64)
        .filter(|e| *e > 0);
    let mut analysis: Option<Value> = before
        .row
        .text("analysis_json")?
        .map(serde_json::from_str)
        .transpose()?;
    let publication = if let Some(marker) = record.tempo_markers.first() {
        let implementation = format!(
            "{:x}",
            Sha256::digest(include_bytes!(
                "../../../cuerithm-maintenance/src/import.rs"
            ))
        );
        let inputs = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&(id, donor, record))?)
        );
        let publication = json!({"grid":{"bpm":marker["bpm"],"startTime":marker["startTime"]},"cues":record.cues,"implementation_sha256":implementation,"inputs_sha256":inputs,"origin":{"kind":"donor_import","track_id":donor}});
        let _: camelot_core::lexicon_publication::Publication =
            serde_json::from_value(publication.clone())?;
        Some(publication)
    } else {
        ensure!(
            record.cues.is_empty(),
            "Imported cues have no usable beatgrid"
        );
        None
    };
    if let Some(a) = &mut analysis {
        ensure!(a.is_object(), "Recipient analysis is not an object");
        if let Some(p) = &publication {
            a["publication"] = p.clone();
        }
    }
    let native_metadata = Metadata {
        fields: native.text.clone(),
        custom_tags: vec![],
    };
    conn.execute("UPDATE tracks SET title=?1,artist=?2,album=?3,genre=?4,musical_key=?5,bpm=?6,energy=?7,source_hash=?8,metadata_json=?9,native_metadata_json=?10,analysis_json=?11,search_text=?12,sort_title=?13,sort_artist=?14,updated=?15 WHERE id=?16",params![title,artist,album,genre,key,bpm,energy,hash,serde_json::to_string(info)?,serde_json::to_string(&native_metadata)?,analysis.map(|v|v.to_string()),format!("{title} {artist} {genre}").to_lowercase(),title.to_lowercase(),artist.to_lowercase(),crate::now(),id])?;
    conn.execute("DELETE FROM track_custom_tags WHERE track_id=?1", [id])?;
    for tag in &record.tags {
        let tag = tag.as_str().context("Imported tag identity is not local")?;
        conn.execute(
            "INSERT INTO track_custom_tags VALUES(?1,?2)",
            params![id, tag],
        )?;
    }
    if let Some(mut p) = publication {
        p["schema"] = "lexicon-publication-v1".into();
        conn.execute(
            "INSERT INTO cue_revisions VALUES(?1,?2,NULL,?3,?4)",
            params![
                uuid::Uuid::new_v4().to_string(),
                id,
                p.to_string(),
                crate::now()
            ],
        )?;
    }
    Ok(())
}
pub(super) fn restore(conn: &Connection, image: &TrackImage) -> Result<()> {
    let id = image.row.id()?;
    let stmt = conn.prepare("SELECT * FROM tracks LIMIT 0")?;
    let columns = stmt
        .column_names()
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    drop(stmt);
    ensure!(
        columns == image.row.columns,
        "Unsupported track snapshot schema"
    );
    let assignments = columns
        .iter()
        .enumerate()
        .filter(|(_, c)| c.as_str() != "id")
        .map(|(i, c)| format!("\"{c}\"=?{}", i + 1))
        .collect::<Vec<_>>()
        .join(",");
    let mut values = image.row.values.iter().map(Cell::sql).collect::<Vec<_>>();
    values.push(SqlValue::Text(id.into()));
    conn.execute(
        &format!("UPDATE tracks SET {assignments} WHERE id=?{}", values.len()),
        params_from_iter(values),
    )?;
    conn.execute("DELETE FROM cue_revisions WHERE track_id=?1", [id])?;
    for row in &image.cues {
        ensure!(
            row.columns == vec!["id", "track_id", "job_id", "json", "created"],
            "Unsupported cue snapshot"
        );
        conn.execute(
            "INSERT INTO cue_revisions VALUES(?1,?2,?3,?4,?5)",
            params_from_iter(row.values.iter().map(Cell::sql)),
        )?;
    }
    conn.execute("DELETE FROM track_custom_tags WHERE track_id=?1", [id])?;
    for tag in &image.tags {
        conn.execute(
            "INSERT INTO track_custom_tags VALUES(?1,?2)",
            params![id, tag],
        )?;
    }
    Ok(())
}
pub(super) fn epochs(
    conn: &Connection,
    tracks: &[String],
    playlists: &[String],
) -> Result<BTreeMap<String, u64>> {
    let mut values = BTreeMap::new();
    for (prefix, table, ids) in [
        ("t", "maintenance_track_epochs", tracks),
        ("p", "maintenance_playlist_epochs", playlists),
    ] {
        for id in ids {
            let value: u64 = conn.query_row(
                &format!("SELECT epoch FROM {table} WHERE id=?1"),
                [id],
                |r| r.get(0),
            )?;
            values.insert(format!("{prefix}:{id}"), value);
        }
    }
    Ok(values)
}
