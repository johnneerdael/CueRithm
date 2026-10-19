use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImportField {
    Artist,
    Title,
    AlbumTitle,
    Genre,
    Comment,
    Key,
    Rating,
    Bpm,
    Color,
    Remixer,
    Mix,
    Label,
    Composer,
    Producer,
    Grouping,
    Lyricist,
    Year,
    ReleaseDate,
    TrackNumber,
    Energy,
    Extra1,
    Extra2,
}
impl ImportField {
    pub const ALL: &'static [Self] = &[
        Self::Artist,
        Self::Title,
        Self::AlbumTitle,
        Self::Genre,
        Self::Comment,
        Self::Key,
        Self::Rating,
        Self::Bpm,
        Self::Color,
        Self::Remixer,
        Self::Mix,
        Self::Label,
        Self::Composer,
        Self::Producer,
        Self::Grouping,
        Self::Lyricist,
        Self::Year,
        Self::ReleaseDate,
        Self::TrackNumber,
        Self::Energy,
        Self::Extra1,
        Self::Extra2,
    ];
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ImportRecord {
    pub fields: BTreeMap<ImportField, Value>,
    pub cues: Vec<Value>,
    pub tempo_markers: Vec<Value>,
    pub tags: Vec<Value>,
    pub identity: BTreeMap<String, Value>,
}
impl ImportRecord {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            serde_json::to_vec(self)?.len() <= 1_048_576,
            "Import record exceeds 1 MiB"
        );
        ensure!(
            self.cues.len() <= 4096 && self.tempo_markers.len() <= 4096 && self.tags.len() <= 4096,
            "Too many imported markers or tags"
        );
        for (field, value) in &self.fields {
            if value.is_null() {
                continue;
            }
            let valid = match field {
                ImportField::Bpm => value
                    .as_f64()
                    .is_some_and(|n| n.is_finite() && (0. ..1000.).contains(&n)),
                ImportField::Rating => value.as_u64().is_some_and(|n| n <= 5),
                ImportField::Year => value.as_u64().is_some_and(|n| n <= 9999),
                ImportField::TrackNumber => value.as_u64().is_some_and(|n| n <= 65535),
                ImportField::Energy => value.as_u64().is_some_and(|n| n <= 10),
                _ => value.is_string(),
            };
            ensure!(valid, "Invalid imported metadata field: {field:?}");
        }
        for cue in &self.cues {
            ensure!(
                cue.is_object()
                    && cue["position"].as_i64().is_some()
                    && cue["startTime"].as_f64().is_some()
                    && matches!(cue["type"].as_str(), Some("1" | "5")),
                "Invalid imported cue"
            );
            ensure!(
                cue.get("endTime")
                    .is_none_or(|v| v.is_null() || v.as_f64().is_some()),
                "Invalid imported loop end"
            );
        }
        for marker in &self.tempo_markers {
            ensure!(
                marker.is_object() && marker["bpm"].as_f64().is_some_and(|b| b > 0. && b < 1000.),
                "Invalid imported tempo marker"
            );
        }
        ensure!(
            self.tags
                .iter()
                .all(|id| id.as_str().is_some_and(|s| !s.is_empty())
                    || id.as_u64().is_some_and(|n| n > 0)),
            "Invalid imported tag identity"
        );
        Ok(())
    }
}
/// Copy the approved metadata projection. Technical identity stays with the target.
/// Empty donor collections retain the target; populated collections replace it.
pub fn import_record(target: &ImportRecord, donor: &ImportRecord) -> Result<ImportRecord> {
    target.validate()?;
    donor.validate()?;
    let mut result = target.clone();
    for field in ImportField::ALL {
        result.fields.insert(
            *field,
            donor.fields.get(field).cloned().unwrap_or(Value::Null),
        );
    }
    if !donor.cues.is_empty() {
        let mut cues = donor.cues.clone();
        for index in 0..cues.len() {
            if cues[index]["position"].as_i64().unwrap() < 0 {
                let next = cues
                    .iter()
                    .filter_map(|c| c["position"].as_i64())
                    .max()
                    .unwrap_or(0)
                    .max(0)
                    .checked_add(1)
                    .context("Imported cue position exhausted")?;
                cues[index]["position"] = next.into();
            }
            if cues[index]["type"] == "5"
                && cues[index]
                    .get("endTime")
                    .and_then(Value::as_f64)
                    .filter(|n| *n != 0.)
                    .is_none_or(|end| end <= cues[index]["startTime"].as_f64().unwrap())
            {
                cues[index]["type"] = "1".into();
                cues[index]["endTime"] = Value::Null;
            }
        }
        let mut seen = std::collections::BTreeSet::new();
        cues.retain(|c| seen.insert(c["position"].as_i64().unwrap()));
        result.cues = cues;
    }
    if !donor.tempo_markers.is_empty() {
        result.tempo_markers = donor.tempo_markers.clone();
        for marker in &mut result.tempo_markers {
            if marker.get("startTime").and_then(Value::as_f64).is_none() {
                marker["startTime"] = 0.into();
            }
        }
        result
            .fields
            .insert(ImportField::Bpm, result.tempo_markers[0]["bpm"].clone());
    }
    if !donor.tags.is_empty() {
        result.tags = donor.tags.clone();
    }
    result.validate()?;
    Ok(result)
}
