//! Accepted musical observations are independent of measured DSP results.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CueKind {
    Cue,
    FadeIn,
    FadeOut,
    Load,
    Loop,
    Unknown,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AcceptedCue {
    pub id: String,
    pub kind: CueKind,
    pub start_seconds: f64,
    pub end_seconds: Option<f64>,
    pub slot: Option<u32>,
    pub memory: bool,
    pub name: Option<String>,
    pub color: Option<String>,
    pub active_loop: bool,
    #[serde(default)]
    pub source_data: BTreeMap<String, serde_json::Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TempoMarker {
    pub id: String,
    pub start_seconds: f64,
    pub bpm: f64,
    pub beat_number: Option<u32>,
    pub beats_per_bar: Option<u32>,
    #[serde(default)]
    pub source_data: BTreeMap<String, serde_json::Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AcceptedMusicalData {
    pub id: String,
    pub source: String,
    pub cues: Vec<AcceptedCue>,
    pub tempo_markers: Vec<TempoMarker>,
}
impl AcceptedMusicalData {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.id.is_empty() && self.id.len() <= 256,
            "Invalid musical revision ID"
        );
        ensure!(
            self.cues.len() <= 4096 && self.tempo_markers.len() <= 4096,
            "Too many musical markers"
        );
        let mut ids = BTreeSet::new();
        for c in &self.cues {
            ensure!(
                !c.id.is_empty() && ids.insert(&c.id),
                "Duplicate/empty cue ID"
            );
            ensure!(
                c.start_seconds.is_finite() && c.end_seconds.is_none_or(f64::is_finite),
                "Nonfinite cue time"
            );
            // Invalid source loops remain inspectable data, never executable loops.
            ensure!(
                serde_json::to_vec(&c.source_data)?.len() <= 65536,
                "Cue source data exceeds limit"
            );
        }
        ids.clear();
        let mut anchors = BTreeMap::new();
        for m in &self.tempo_markers {
            ensure!(
                !m.id.is_empty() && ids.insert(&m.id),
                "Duplicate/empty tempo marker ID"
            );
            ensure!(
                m.start_seconds.is_finite() && m.bpm.is_finite() && m.bpm > 0.,
                "Invalid tempo marker"
            );
            ensure!(
                m.beat_number != Some(0) && m.beats_per_bar != Some(0),
                "Invalid beat phase"
            );
            let anchor = if m.start_seconds == 0. {
                0
            } else {
                m.start_seconds.to_bits()
            };
            if let Some(bpm) = anchors.insert(anchor, m.bpm) {
                ensure!(bpm == m.bpm, "Conflicting tempo anchors require review");
            }
            ensure!(
                serde_json::to_vec(&m.source_data)?.len() <= 65536,
                "Tempo source data exceeds limit"
            );
        }
        Ok(())
    }
    /// Piecewise constant tempo; no invented beat-one phase.
    pub fn tempo_at(&self, seconds: f64) -> Option<&TempoMarker> {
        self.tempo_markers
            .iter()
            .filter(|m| m.start_seconds <= seconds)
            .max_by(|a, b| {
                a.start_seconds
                    .total_cmp(&b.start_seconds)
                    .then(a.id.cmp(&b.id))
            })
    }
}
impl crate::Library {
    pub fn accepted_musical_data(&self, id: &str) -> Result<Option<AcceptedMusicalData>> {
        use rusqlite::OptionalExtension;
        let json:Option<String>=self.conn()?.query_row("SELECT m.json FROM tracks t JOIN accepted_musical_revisions m ON m.id=t.selected_musical_revision AND m.track_id=t.id WHERE t.id=?1",[id],|r|r.get(0)).optional()?;
        match json {
            Some(s) => Ok(Some(serde_json::from_str(&s)?)),
            None => {
                let conn = self.conn()?;
                accepted_snapshot_from_analysis(&conn, id)
            }
        }
    }
    pub fn accept_musical_data(&self, id: &str, data: &AcceptedMusicalData) -> Result<()> {
        data.validate()?;
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure!(
            tx.query_row("SELECT COUNT(*) FROM tracks WHERE id=?1", [id], |r| r
                .get::<_, i64>(0))?
                == 1,
            "Unknown track"
        );
        tx.execute(
            "INSERT INTO accepted_musical_revisions(id,track_id,json,created) VALUES(?1,?2,?3,?4)",
            rusqlite::params![data.id, id, serde_json::to_string(data)?, crate::now()],
        )?;
        tx.execute(
            "UPDATE tracks SET selected_musical_revision=?1,updated=?2 WHERE id=?3",
            rusqlite::params![data.id, crate::now(), id],
        )?;
        tx.execute(
            "UPDATE meta SET value=value+1 WHERE key IN ('revision','smart_input_revision')",
            [],
        )?;
        tx.commit()?;
        Ok(())
    }
}

/// Project an existing saved publication without running or claiming new DSP.
/// This remains available when a file replacement makes measured analysis stale.
pub(crate) fn accepted_snapshot_from_analysis(
    conn: &rusqlite::Connection,
    id: &str,
) -> Result<Option<AcceptedMusicalData>> {
    use rusqlite::OptionalExtension;
    use sha2::{Digest, Sha256};
    let raw: Option<String> =
        conn.query_row(
            "SELECT analysis_json FROM tracks WHERE id=?1 UNION ALL SELECT NULL FROM import_items WHERE id=?1 AND local_track_id IS NULL AND state='imported' LIMIT 1",
            [id],
            |r| r.get(0),
        )?;
    let saved:Option<String>=conn.query_row("SELECT json FROM cue_revisions WHERE track_id=?1 ORDER BY created DESC,rowid DESC LIMIT 1",[id],|r|r.get(0)).optional()?;
    let analysis = raw
        .as_deref()
        .map(serde_json::from_str::<serde_json::Value>)
        .transpose()?;
    let saved = saved
        .as_deref()
        .map(serde_json::from_str::<serde_json::Value>)
        .transpose()?;
    let publication = saved
        .as_ref()
        .filter(|v| v["schema"] == "lexicon-publication-v1")
        .cloned()
        .or_else(|| {
            analysis
                .as_ref()
                .and_then(|v| v.get("publication").filter(|v| !v.is_null()).cloned())
        });
    let mut cues = Vec::new();
    let mut tempo_markers = Vec::new();
    if let Some(value) = publication {
        let publication: camelot_core::lexicon_publication::Publication =
            serde_json::from_value(value)?;
        for (index, c) in publication.cues.into_iter().enumerate() {
            let mut source_data = BTreeMap::new();
            if let Some(data) = &c.data {
                source_data.insert("publication_data".into(), data.clone());
            }
            if !c.color.is_null() && c.color.as_str().is_none() {
                source_data.insert("publication_color".into(), c.color.clone());
            }
            cues.push(AcceptedCue {
                id: format!("saved-cue:{index}"),
                kind: match c.kind {
                    camelot_core::lexicon_publication::PublishedType::Cue => CueKind::Cue,
                    camelot_core::lexicon_publication::PublishedType::Loop => CueKind::Loop,
                },
                start_seconds: c.start_time,
                end_seconds: c.end_time,
                slot: Some(c.position),
                memory: c
                    .data
                    .as_ref()
                    .is_some_and(|d| d["rekordbox"]["originalNum"] == -1),
                name: Some(c.name),
                color: c.color.as_str().map(str::to_owned),
                active_loop: c.active_loop.unwrap_or(false),
                source_data,
            });
        }
        tempo_markers.push(TempoMarker {
            id: "saved-tempo:0".into(),
            start_seconds: publication.grid.start_time,
            bpm: publication.grid.bpm,
            beat_number: None,
            beats_per_bar: None,
            source_data: BTreeMap::new(),
        });
    } else {
        let structural = saved
            .as_ref()
            .filter(|v| v.get("cues").is_some())
            .or_else(|| {
                analysis
                    .as_ref()
                    .and_then(|v| v.pointer("/analysis/lexicon_cues"))
                    .filter(|v| !v.is_null())
            });
        if let Some(value) = structural {
            let structural: camelot_core::lexicon_cues::StructuralCueResult =
                serde_json::from_value(value.clone())?;
            if let Some(point) = &structural.emergency_loop {
                let mut source_data = BTreeMap::new();
                source_data.insert("loop_beats".into(), point.loop_beats.into());
                cues.push(AcceptedCue {
                    id: "saved-emergency-loop".into(),
                    kind: CueKind::Loop,
                    start_seconds: point.time_seconds,
                    end_seconds: None,
                    slot: None,
                    memory: false,
                    name: Some("Emergency loop".into()),
                    color: None,
                    active_loop: false,
                    source_data,
                });
            }
            for (index, c) in structural.cues.into_iter().enumerate() {
                cues.push(AcceptedCue {
                    id: format!("saved-cue:{index}"),
                    kind: CueKind::Cue,
                    start_seconds: c.time_seconds,
                    end_seconds: None,
                    slot: None,
                    memory: false,
                    name: Some(c.name),
                    color: Some(c.color),
                    active_loop: false,
                    source_data: BTreeMap::new(),
                });
            }
        }
        if let Some(tempo) = analysis
            .as_ref()
            .and_then(|v| v.pointer("/analysis/tempo"))
            .filter(|v| !v.is_null())
            && let (Some(bpm), Some(start_seconds)) = (
                tempo["bpm"].as_f64(),
                tempo["downbeat_time"]
                    .as_f64()
                    .or_else(|| tempo["beats_seconds"].get(0).and_then(|v| v.as_f64())),
            )
        {
            tempo_markers.push(TempoMarker {
                id: "saved-tempo:0".into(),
                start_seconds,
                bpm,
                beat_number: None,
                beats_per_bar: None,
                source_data: BTreeMap::new(),
            });
        }
    }
    if cues.is_empty() && tempo_markers.is_empty() {
        return Ok(None);
    }
    let digest = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(id, &cues, &tempo_markers))?)
    );
    let data = AcceptedMusicalData {
        id: format!("saved:{digest}"),
        source: "cuerithm_saved_publication".into(),
        cues,
        tempo_markers,
    };
    data.validate()?;
    Ok(Some(data))
}
