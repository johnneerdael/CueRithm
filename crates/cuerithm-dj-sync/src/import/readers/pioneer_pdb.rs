//! Data-only PDB row projection. Every row also retains its original byte blob.
use super::*;
use crate::pioneer_usb::pdb;
pub(super) struct Row {
    pub table: &'static str,
    pub id: String,
    pub raw: Value,
}
fn number(
    raw: &mut serde_json::Map<String, Value>,
    row: &[u8],
    name: &str,
    at: usize,
    short: bool,
) -> Result<()> {
    raw.insert(
        name.into(),
        json!(if short {
            u32::from(pdb::read16(row, at)?)
        } else {
            pdb::read32(row, at)?
        }),
    );
    Ok(())
}
fn string(
    raw: &mut serde_json::Map<String, Value>,
    row: &[u8],
    name: &str,
    offset: Result<usize>,
    issues: &mut Vec<String>,
) {
    match offset.and_then(|at| {
        ensure!(row.get(at) != Some(&1), "Invalid PDB short string tag");
        pdb::decode_string(row, at)
    }) {
        Ok(text) => {
            raw.insert(name.into(), json!(text));
        }
        Err(_) => issues.push(format!("unresolved_pdb_{name}")),
    }
}
pub(super) fn decode(kind: u32, row: &[u8], identity: &str) -> Result<Option<Row>> {
    let mut raw = serde_json::Map::new();
    let mut issues = Vec::new();
    let (table, id) = match kind {
        0 => {
            let id = pdb::read32(row, 72)?;
            raw.insert("content_id".into(), json!(id));
            if row.len() < 136 {
                issues.push("truncated_pdb_track".into());
            } else {
                for (name, at) in [
                    ("subtype", 0),
                    ("indexShift", 2),
                    ("discNo", 76),
                    ("djPlayCount", 78),
                    ("releaseYear", 80),
                    ("bitDepth", 82),
                    ("length", 84),
                    ("unknown86", 86),
                    ("fileType", 90),
                    ("stringOffsetPrefix", 92),
                ] {
                    number(&mut raw, row, name, at, true)?;
                }
                for (name, at) in [
                    ("bitmask", 4),
                    ("samplingRate", 8),
                    ("artist_id_composer", 12),
                    ("fileSize", 16),
                    ("masterContentId", 20),
                    ("masterDbId", 24),
                    ("image_id", 28),
                    ("key_id", 32),
                    ("artist_id_originalArtist", 36),
                    ("label_id", 40),
                    ("artist_id_remixer", 44),
                    ("bitrate", 48),
                    ("trackNo", 52),
                    ("bpmx100", 56),
                    ("genre_id", 60),
                    ("album_id", 64),
                    ("artist_id_artist", 68),
                ] {
                    number(&mut raw, row, name, at, false)?;
                }
                raw.insert("color_id".into(), json!(row[88]));
                raw.insert("rating".into(), json!(row[89]));
                let names = [
                    "isrc",
                    "texter",
                    "unknown2",
                    "unknown3",
                    "unknown4",
                    "message",
                    "kuvoPublic",
                    "autoloadHotcues",
                    "unknown5",
                    "unknown6",
                    "dateAdded",
                    "releaseDate",
                    "subtitle",
                    "unknown7",
                    "analysisDataFilePath",
                    "analyzeDate",
                    "djComment",
                    "title",
                    "unknown8",
                    "fileName",
                    "path",
                ];
                for (index, name) in names.into_iter().enumerate() {
                    string(
                        &mut raw,
                        row,
                        name,
                        pdb::read16(row, 94 + index * 2).map(usize::from),
                        &mut issues,
                    );
                }
            }
            ("content", id.to_string())
        }
        1 | 2 | 3 | 4 | 5 | 6 | 13 => {
            let (table, id, offset) = match kind {
                2 => {
                    let subtype = pdb::read16(row, 0)?;
                    let id = pdb::read32(row, 4)?;
                    let offset = match subtype {
                        96 => row
                            .get(9)
                            .copied()
                            .map(usize::from)
                            .context("Missing artist name pointer"),
                        100 => pdb::read16(row, 10).map(usize::from),
                        _ => Err(anyhow::anyhow!("Unknown artist subtype")),
                    };
                    raw.insert("subtype".into(), json!(subtype));
                    ("artist", id, offset)
                }
                3 => {
                    let subtype = pdb::read16(row, 0)?;
                    let id = pdb::read32(row, 12)?;
                    number(&mut raw, row, "artist_id", 8, false)?;
                    let offset = match subtype {
                        128 => row
                            .get(21)
                            .copied()
                            .map(usize::from)
                            .context("Missing album name pointer"),
                        132 => pdb::read16(row, 22).map(usize::from),
                        _ => Err(anyhow::anyhow!("Unknown album subtype")),
                    };
                    raw.insert("subtype".into(), json!(subtype));
                    ("album", id, offset)
                }
                5 => {
                    number(&mut raw, row, "second_id", 4, false)?;
                    ("key", pdb::read32(row, 0)?, Ok(8))
                }
                6 => (
                    "color",
                    u32::from(*row.get(4).context("Missing color identity")?),
                    Ok(8),
                ),
                13 => ("image", pdb::read32(row, 0)?, Ok(4)),
                1 => ("genre", pdb::read32(row, 0)?, Ok(4)),
                _ => ("label", pdb::read32(row, 0)?, Ok(4)),
            };
            raw.insert(format!("{table}_id"), json!(id));
            string(
                &mut raw,
                row,
                if kind == 13 { "path" } else { "name" },
                offset,
                &mut issues,
            );
            (table, id.to_string())
        }
        7 => {
            for (name, at) in [
                ("playlist_id_parent", 0),
                ("unknown4", 4),
                ("sequenceNo", 8),
                ("playlist_id", 12),
                ("attribute", 16),
            ] {
                number(&mut raw, row, name, at, false)?;
            }
            string(&mut raw, row, "name", Ok(20), &mut issues);
            ("playlist", pdb::read32(row, 12)?.to_string())
        }
        8 => {
            for (name, at) in [("sequenceNo", 0), ("content_id", 4), ("playlist_id", 8)] {
                number(&mut raw, row, name, at, false)?;
            }
            ("playlist_content", identity.into())
        }
        11 => {
            number(&mut raw, row, "history_id", 0, false)?;
            string(&mut raw, row, "name", Ok(4), &mut issues);
            ("history", pdb::read32(row, 0)?.to_string())
        }
        12 => {
            for (name, at) in [("content_id", 0), ("history_id", 4), ("sequenceNo", 8)] {
                number(&mut raw, row, name, at, false)?;
            }
            ("history_content", identity.into())
        }
        _ => return Ok(None),
    };
    if !issues.is_empty() {
        raw.insert("parse_issues".into(), json!(issues));
    }
    Ok(Some(Row {
        table,
        id,
        raw: Value::Object(raw),
    }))
}
