//! CueRithm tracks that are not in rekordbox yet, as tracks to add to it.
//!
//! A track qualifies when it is ready and active, its audio file exists in a format rekordbox plays, and no track
//! with that file is in rekordbox already. Nothing is added unless the caller asks for it.
use crate::native_reconcile::canonical_fields;
use crate::native_sync::local_fields;
use crate::*;
use anyhow::Result;
use camelot_core::lexicon_publication::{Publication, PublishedCue};
use cuerithm_rekordbox::{ContentFields, MyTagRef, NewTrack, RekordboxCollection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewTrackPlanItem {
    pub local_track_id: String,
    pub title: String,
    pub artist: String,
    pub path: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkippedNewTrack {
    pub local_track_id: String,
    pub title: String,
    pub reason: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NewTrackSummary {
    pub items: Vec<NewTrackPlanItem>,
    pub skipped: Vec<SkippedNewTrack>,
}

struct LocalRow {
    id: String,
    path: Vec<u8>,
    title: String,
    artist: String,
    bpm: Option<f64>,
    duration: Option<f64>,
    info: String,
    analysis: Option<String>,
    created: i64,
}

/// rekordbox's file type number for a file, or `None` for a format it does not play.
fn file_type(path: &std::path::Path, codec: &str) -> Option<i64> {
    let extension = path.extension()?.to_string_lossy().to_lowercase();
    Some(match extension.as_str() {
        "mp3" => 1,
        "aac" => 2,
        "mp4" => 3,
        "m4a" if codec.eq_ignore_ascii_case("alac") => 6,
        "m4a" => 4,
        "flac" => 5,
        "wav" | "wave" => 11,
        "aiff" | "aif" | "aifc" => 12,
        _ => return None,
    })
}
fn text(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(str::to_owned)
}
fn int(value: Option<&Value>) -> Option<i64> {
    value.and_then(Value::as_i64)
}

impl Library {
    /// Local tracks to add to the library in `collection`, with the new-track record for each.
    pub(crate) fn plan_native_new_tracks(
        &self,
        target: &str,
        collection: &RekordboxCollection,
    ) -> Result<(Vec<NewTrack>, NewTrackSummary)> {
        let in_rekordbox: BTreeSet<&str> = collection
            .tracks
            .iter()
            .filter(|t| t.deleted != Some(true))
            .filter_map(|t| t.source.path.as_deref())
            .collect();
        let rows: Vec<LocalRow> = {
            let conn = self.conn()?;
            let mut q = conn.prepare(
                "SELECT t.id,t.path,t.title,t.artist,t.bpm,t.duration,t.metadata_json,t.analysis_json,t.created FROM tracks t WHERE t.state='ready' AND t.archived=0 AND NOT EXISTS (SELECT 1 FROM native_track_links l WHERE l.target_id=?1 AND l.local_track_id=t.id) ORDER BY t.created,t.id",
            )?;
            q.query_map([target], |r| {
                Ok(LocalRow {
                    id: r.get(0)?,
                    path: r.get(1)?,
                    title: r.get(2)?,
                    artist: r.get(3)?,
                    bpm: r.get(4)?,
                    duration: r.get(5)?,
                    info: r.get(6)?,
                    analysis: r.get(7)?,
                    created: r.get(8)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?
        };
        let (mut tracks, mut summary) = (vec![], NewTrackSummary::default());
        for LocalRow {
            id,
            path,
            title,
            artist,
            bpm,
            duration,
            info,
            analysis,
            created,
        } in rows
        {
            let skip = |summary: &mut NewTrackSummary, reason: &str| {
                summary.skipped.push(SkippedNewTrack {
                    local_track_id: id.clone(),
                    title: title.clone(),
                    reason: reason.into(),
                })
            };
            let path = crate::model::bytes_path(&path)?;
            let shown = path.display().to_string();
            let media: cuerithm_media::MediaInfo =
                serde_json::from_str(&info).unwrap_or_else(|_| cuerithm_media::MediaInfo {
                    codec: String::new(),
                    sample_rate: 0,
                    channels: 0,
                    bits_per_sample: None,
                    tags: Default::default(),
                    properties: Default::default(),
                });
            let Some(kind) = file_type(&path, &media.codec) else {
                skip(&mut summary, "format_not_supported_by_rekordbox");
                continue;
            };
            let Ok(meta) = std::fs::metadata(&path) else {
                skip(&mut summary, "file_missing");
                continue;
            };
            if in_rekordbox.contains(shown.as_str()) {
                skip(&mut summary, "already_in_rekordbox");
                continue;
            }
            let Some(length) = duration.filter(|d| d.is_finite() && *d > 0.) else {
                skip(&mut summary, "not_analysed");
                continue;
            };
            let local = {
                let conn = self.conn()?;
                local_fields(&conn, &id)?
            };
            let canonical = canonical_fields(&local, true);
            let cues = serde_json::from_value::<Vec<PublishedCue>>(
                local.get("cues").cloned().unwrap_or(Value::Null),
            )
            .map(|cues| {
                rekordbox_cue_rows(&cues, bpm.unwrap_or(0.), CueExportOptions::default()).rows
            })
            .unwrap_or_default();
            let first_beat = analysis
                .and_then(|a| serde_json::from_str::<Value>(&a).ok())
                .and_then(|a| {
                    serde_json::from_value::<Publication>(a["publication"].clone())
                        .ok()
                        .map(|p| p.grid.start_time)
                        .or_else(|| a["analysis"]["tempo"]["downbeat_time"].as_f64())
                });
            let tags = local
                .get("tag_ids")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .filter_map(|t| t.split_once(crate::native_sync::TAG_SEPARATOR))
                        .map(|(category, tag)| MyTagRef {
                            category: category.into(),
                            tag: tag.into(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            let file_name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            tracks.push(NewTrack {
                key: id.clone(),
                folder_path: shown.clone(),
                file_name,
                fields: ContentFields {
                    title: text(canonical.get("title")).or(Some(title.clone())),
                    artist: text(canonical.get("artist")).or(Some(artist.clone())),
                    album: text(canonical.get("album")),
                    genre: text(canonical.get("genre")),
                    label: text(canonical.get("label")),
                    remixer: text(canonical.get("remixer")),
                    composer: text(canonical.get("composer")),
                    key: text(canonical.get("key")),
                    comment: text(canonical.get("comment")),
                    subtitle: text(canonical.get("mix_name")),
                    lyricist: text(canonical.get("lyricist")),
                    bpm_hundredths: int(canonical.get("bpm")),
                    release_year: int(canonical.get("release_year")),
                    release_date: text(canonical.get("release_date")),
                    track_number: int(canonical.get("track_number")),
                    disc_number: int(canonical.get("disc_number")),
                    rating: int(canonical.get("rating")),
                },
                length_seconds: length.floor() as i64,
                bit_rate: Some((meta.len() as f64 * 8. / length / 1000.).round() as i64),
                sample_rate: (media.sample_rate > 0).then_some(i64::from(media.sample_rate)),
                file_size: i64::try_from(meta.len()).ok(),
                file_type: kind,
                date_added: cuerithm_smartlists::epoch_to_calendar_day(created, "UTC")?,
                color_id: None,
                first_beat_seconds: first_beat,
                cues,
                tags,
            });
            summary.items.push(NewTrackPlanItem {
                local_track_id: id.clone(),
                title,
                artist,
                path: shown,
            });
        }
        Ok((tracks, summary))
    }
}
