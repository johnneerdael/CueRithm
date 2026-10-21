use crate::{Library, TrackSummary};
use anyhow::Result;
use cuerithm_smartlists::KeyMixingMode;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CompatibleTrackRequest {
    pub source_id: String,
    pub compatible_key: bool,
    pub key_mode: Option<KeyMixingMode>,
    pub bpm_tolerance: Option<f64>,
    pub include_half_double: bool,
    pub energy_tolerance: Option<u8>,
    pub genre: Option<String>,
    pub custom_tag_ids: Vec<String>,
}
impl Default for CompatibleTrackRequest {
    fn default() -> Self {
        Self {
            source_id: String::new(),
            compatible_key: true,
            key_mode: None,
            bpm_tolerance: Some(4.0),
            include_half_double: false,
            energy_tolerance: None,
            genre: None,
            custom_tag_ids: vec![],
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibleTrackMatch {
    pub track: TrackSummary,
    pub reasons: Vec<String>,
    pub bpm_delta: Option<f64>,
    pub bpm_multiplier: Option<f64>,
    pub energy_delta: Option<u8>,
    pub same_key: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibleTrackResult {
    pub source: TrackSummary,
    pub key_mode: KeyMixingMode,
    pub matches: Vec<CompatibleTrackMatch>,
    pub checked: usize,
    pub excluded_unknown: usize,
    pub library_revision: u64,
}
const SUMMARY_COLUMNS: &str = "id,path,title,artist,album,genre,state,musical_key,bpm,energy,duration,source_hash,metadata_json,created,updated,archived,json_extract(tagging_json,'$.status')";
fn key(track: &TrackSummary) -> Option<String> {
    track
        .key
        .as_deref()
        .and_then(|k| cuerithm_smartlists::key_to_camelot(k.trim()))
}
fn bpm(track: &TrackSummary) -> Option<f64> {
    track.bpm.filter(|n| n.is_finite() && *n > 0.0)
}
fn energy(track: &TrackSummary) -> Option<u8> {
    track.energy.filter(|n| (1..=10).contains(n))
}
impl Library {
    /// Find metadata-compatible prepared tracks from one consistent database snapshot.
    /// This reads the published library measurements; it never analyzes or writes audio.
    pub fn find_compatible_tracks(
        &self,
        request: CompatibleTrackRequest,
    ) -> Result<CompatibleTrackResult> {
        use anyhow::{Context, ensure};
        use std::collections::{BTreeMap, BTreeSet};
        ensure!(
            request
                .bpm_tolerance
                .is_none_or(|n| n.is_finite() && (0.0..=300.0).contains(&n)),
            "Choose a BPM tolerance from 0 to 300"
        );
        ensure!(
            request.energy_tolerance.is_none_or(|n| n <= 9),
            "Choose an energy tolerance from 0 to 9"
        );
        ensure!(
            request.custom_tag_ids.len() <= 4096
                && request
                    .custom_tag_ids
                    .iter()
                    .all(|s| !s.is_empty() && s.len() <= 256),
            "Invalid custom tag selection"
        );
        ensure!(
            request.genre.as_ref().is_none_or(|s| s.len() <= 4096),
            "Genre restriction is too long"
        );
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let source = tx
            .query_row(
                &format!("SELECT {SUMMARY_COLUMNS} FROM tracks WHERE id=?1"),
                [&request.source_id],
                crate::store::summary_row,
            )
            .context("Source track is unavailable. Choose another prepared track.")?;
        ensure!(
            source.state == "ready" && !source.archived,
            "Choose a prepared source track from Tracks"
        );
        let source_key = key(&source);
        ensure!(
            !request.compatible_key || source_key.is_some(),
            "Source key is unknown. Prepare or correct the source, or disable key matching."
        );
        ensure!(
            request.bpm_tolerance.is_none() || bpm(&source).is_some(),
            "Source BPM is unknown. Prepare or correct the source, or disable BPM matching."
        );
        ensure!(
            request.energy_tolerance.is_none() || energy(&source).is_some(),
            "Source energy is unknown. Prepare the source, or disable energy matching."
        );
        let preferences: String = tx.query_row(
            "SELECT json FROM library_preferences WHERE singleton=1",
            [],
            |r| r.get(0),
        )?;
        let preferences: crate::PlaylistPreferences = serde_json::from_str(&preferences)?;
        let mode = request.key_mode.unwrap_or(preferences.key_mixing_mode);
        let library_revision =
            tx.query_row("SELECT value FROM meta WHERE key='revision'", [], |r| {
                r.get(0)
            })?;
        let mut tag_names = BTreeMap::<String, String>::new();
        {
            let mut q = tx.prepare("SELECT t.id,c.name,t.name FROM custom_tags t JOIN custom_tag_categories c ON c.id=t.category_id")?;
            for row in q.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })? {
                let (id, category, name) = row?;
                tag_names.insert(id, format!("{category}: {name}"));
            }
        }
        ensure!(
            request
                .custom_tag_ids
                .iter()
                .all(|id| tag_names.contains_key(id)),
            "A selected custom tag was removed. Reload the compatible tracks workspace."
        );
        let mut tags = BTreeMap::<String, BTreeSet<String>>::new();
        if !request.custom_tag_ids.is_empty() {
            let mut q = tx.prepare("SELECT track_id,tag_id FROM track_custom_tags")?;
            for row in q.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
                let (track, tag) = row?;
                tags.entry(track).or_default().insert(tag);
            }
        }
        let tracks = {
            let mut q = tx.prepare(&format!(
                "SELECT {SUMMARY_COLUMNS} FROM tracks WHERE state='ready' AND archived=0 AND id<>?1"
            ))?;
            q.query_map([&request.source_id], crate::store::summary_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        let mut result = CompatibleTrackResult {
            source: source.clone(),
            key_mode: mode,
            matches: vec![],
            checked: tracks.len(),
            excluded_unknown: 0,
            library_revision,
        };
        let genre = request
            .genre
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        for track in tracks {
            let track_key = key(&track);
            if (request.compatible_key && track_key.is_none())
                || (request.bpm_tolerance.is_some() && bpm(&track).is_none())
                || (request.energy_tolerance.is_some() && energy(&track).is_none())
            {
                result.excluded_unknown += 1;
                continue;
            }
            let mut matched = CompatibleTrackMatch {
                track,
                reasons: vec![],
                bpm_delta: None,
                bpm_multiplier: None,
                energy_delta: None,
                same_key: false,
            };
            if request.compatible_key {
                // Required values were checked above. Canonical notation also makes aliases equivalent.
                let a = source_key.as_deref().unwrap();
                let b = track_key.as_deref().unwrap();
                if !cuerithm_smartlists::compatible_keys(a, b, mode) {
                    continue;
                }
                matched.same_key = a == b;
                let mode_name = match mode {
                    KeyMixingMode::Harmonic => "harmonic",
                    KeyMixingMode::Fuzzy => "fuzzy",
                };
                matched.reasons.push(if matched.same_key {
                    format!("Same key: {a}")
                } else {
                    format!("Compatible key: {a} → {b} ({mode_name})")
                });
            }
            if let Some(tolerance) = request.bpm_tolerance {
                let candidate = bpm(&matched.track).unwrap();
                let source = bpm(&source).unwrap();
                let factors: &[f64] = if request.include_half_double {
                    &[1.0, 0.5, 2.0]
                } else {
                    &[1.0]
                };
                let (multiplier, delta) = factors
                    .iter()
                    .map(|factor| (*factor, (candidate * factor - source).abs()))
                    .min_by(|a, b| {
                        a.1.total_cmp(&b.1)
                            .then_with(|| (a.0 != 1.0).cmp(&(b.0 != 1.0)))
                    })
                    .unwrap();
                if delta > tolerance {
                    continue;
                }
                matched.bpm_delta = Some(delta);
                matched.bpm_multiplier = Some(multiplier);
                matched.reasons.push(format!(
                    "BPM: {candidate} × {multiplier} = {}; difference {delta:.2} (±{tolerance})",
                    candidate * multiplier
                ));
            }
            if let Some(tolerance) = request.energy_tolerance {
                let candidate = energy(&matched.track).unwrap();
                let source = energy(&source).unwrap();
                let delta = source.abs_diff(candidate);
                if delta > tolerance {
                    continue;
                }
                matched.energy_delta = Some(delta);
                matched.reasons.push(format!("Whole-track energy: {candidate}/10; source {source}/10, difference {delta} (±{tolerance})"));
            }
            if let Some(genre) = genre {
                if matched.track.genre.trim().to_lowercase() != genre.to_lowercase() {
                    continue;
                }
                matched
                    .reasons
                    .push(format!("Genre: {}", matched.track.genre));
            }
            if !request.custom_tag_ids.iter().all(|id| {
                tags.get(&matched.track.id)
                    .is_some_and(|set| set.contains(id))
            }) {
                continue;
            }
            for id in &request.custom_tag_ids {
                matched
                    .reasons
                    .push(format!("Custom tag: {}", tag_names[id]));
            }
            result.matches.push(matched);
        }
        result.matches.sort_by(|a, b| {
            b.same_key
                .cmp(&a.same_key)
                .then_with(|| {
                    a.bpm_delta
                        .unwrap_or_default()
                        .total_cmp(&b.bpm_delta.unwrap_or_default())
                })
                .then_with(|| a.energy_delta.cmp(&b.energy_delta))
                .then_with(|| {
                    a.track
                        .artist
                        .to_lowercase()
                        .cmp(&b.track.artist.to_lowercase())
                })
                .then_with(|| {
                    a.track
                        .title
                        .to_lowercase()
                        .cmp(&b.track.title.to_lowercase())
                })
                .then_with(|| a.track.id.cmp(&b.track.id))
        });
        tx.commit()?;
        Ok(result)
    }
}
#[cfg(test)]
mod tests;
