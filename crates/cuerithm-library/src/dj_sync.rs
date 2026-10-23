//! Project prepared library data into native DJ adapters without running DSP.
use crate::{Library, PlaylistKind, PlaylistNode};
use anyhow::{Context, Result, ensure};
use camelot_core::lexicon_publication::{Publication, PublishedType};
use cuerithm_dj_sync::{SyncBeat, SyncCue, SyncLibrary, SyncPlaylist, SyncTrack};
use cuerithm_media::MediaInfo;
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Modified scope accepts IDs calculated from a durable target baseline. It never
/// guesses whether a track changed, and never publishes partial playlists.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DjSyncScope {
    Full,
    /// Resolve changes against the durable target baseline in the sync service.
    Changed,
    Playlist {
        id: String,
    },
    Modified {
        track_ids: Vec<String>,
    },
}

struct RawTrack {
    id: String,
    path: Vec<u8>,
    title: String,
    artist: String,
    album: String,
    genre: String,
    key: Option<String>,
    duration: Option<f64>,
    analysis: Option<String>,
    metadata: String,
    cues: Option<String>,
}
#[derive(Deserialize)]
struct SavedAnalysis {
    #[serde(default)]
    media: Option<camelot_core::prepared::PreparedMediaIdentity>,
    publication: Option<Publication>,
    source_sample_rate: u32,
}

impl Library {
    /// Capture prepared tracks, saved publication timing and ordered memberships.
    /// Smartlists use their materialized membership, as the playlist UI does.
    /// No source files are read, hashed, decoded or tagged by this projection.
    pub fn dj_sync_library(&self, scope: DjSyncScope) -> Result<SyncLibrary> {
        ensure!(
            !matches!(scope, DjSyncScope::Changed),
            "Changed scope requires a target baseline; use preview_dj_sync"
        );
        if let DjSyncScope::Modified { track_ids } = &scope {
            for id in track_ids {
                let imported = self.conn()?.query_row(
                    "SELECT EXISTS(SELECT 1 FROM import_items WHERE id=?1)",
                    [id],
                    |r| r.get::<_, bool>(0),
                )?;
                if imported {
                    let readiness = self.import_export_readiness(id)?;
                    ensure!(readiness.ready, "{}", readiness.reasons.join("; "));
                }
            }
        }
        let (raw, mut playlists) = {
            // Keep one coherent DB snapshot, then release the library mutex before
            // parsing the potentially large per-track analysis JSON.
            let connection = self.conn()?;
            let playlists = project_playlists(&connection, &scope)?;
            let selected: Option<BTreeSet<String>> = match &scope {
                DjSyncScope::Full | DjSyncScope::Changed => None,
                DjSyncScope::Playlist { .. } => Some(
                    playlists
                        .iter()
                        .flat_map(|playlist| playlist.track_ids.iter().cloned())
                        .collect(),
                ),
                DjSyncScope::Modified { track_ids } => {
                    let ids: BTreeSet<_> = track_ids.iter().cloned().collect();
                    ensure!(
                        ids.iter().all(|id| !id.is_empty()),
                        "Modified sync contains an empty track ID"
                    );
                    Some(ids)
                }
            };
            let mut query = connection.prepare(
                "SELECT id,path,title,artist,album,genre,musical_key,duration,analysis_json,metadata_json,
                 (SELECT json FROM cue_revisions c WHERE c.track_id=t.id ORDER BY c.created DESC,c.rowid DESC LIMIT 1)
                 FROM tracks t WHERE state='ready' AND archived=0 ORDER BY sort_title,id"
            )?;
            let rows = query.query_map([], |row| {
                Ok(RawTrack {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    title: row.get(2)?,
                    artist: row.get(3)?,
                    album: row.get(4)?,
                    genre: row.get(5)?,
                    key: row.get(6)?,
                    duration: row.get(7)?,
                    analysis: row.get(8)?,
                    metadata: row.get(9)?,
                    cues: row.get(10)?,
                })
            })?;
            let mut raw = Vec::new();
            for row in rows {
                let row = row?;
                if selected.as_ref().is_none_or(|ids| ids.contains(&row.id)) {
                    raw.push(row);
                }
            }
            // Explicit IDs may include tracks which became Incoming or archived
            // since the baseline was captured. They are intentionally excluded.
            (raw, playlists)
        };
        for row in &raw {
            let imported: bool = self.conn()?.query_row(
                "SELECT EXISTS(SELECT 1 FROM import_items WHERE id=?1)",
                [&row.id],
                |r| r.get(0),
            )?;
            if imported {
                let readiness = self.import_export_readiness(&row.id)?;
                ensure!(readiness.ready, "{}", readiness.reasons.join("; "));
            }
        }
        let tracks: Vec<SyncTrack> = raw.into_iter().map(project_track).collect::<Result<_>>()?;
        let ids: BTreeSet<_> = tracks.iter().map(|track| track.id.as_str()).collect();
        for playlist in &mut playlists {
            playlist.track_ids.retain(|id| ids.contains(id.as_str()));
        }
        Ok(SyncLibrary { tracks, playlists })
    }
}

fn project_playlists(connection: &Connection, scope: &DjSyncScope) -> Result<Vec<SyncPlaylist>> {
    if matches!(scope, DjSyncScope::Modified { .. }) {
        return Ok(Vec::new());
    }
    let tree = crate::playlists::load(connection)?;
    let mut nodes: BTreeMap<String, PlaylistNode> = tree
        .iter()
        .filter(|(_, stored)| !stored.node.deleted)
        .map(|(id, stored)| (id.clone(), stored.node.clone()))
        .collect();
    // A child of a removed folder must not become a new root during export.
    let active: BTreeSet<_> = nodes
        .keys()
        .filter(|id| {
            let mut parent = nodes[*id].parent_id.as_ref();
            let mut seen = BTreeSet::new();
            while let Some(id) = parent {
                if !seen.insert(id) {
                    return false;
                }
                let Some(node) = nodes.get(id) else {
                    return false;
                };
                parent = node.parent_id.as_ref();
            }
            true
        })
        .cloned()
        .collect();
    nodes.retain(|id, _| active.contains(id));
    let selected: BTreeSet<String> = match scope {
        DjSyncScope::Full | DjSyncScope::Changed => active,
        DjSyncScope::Playlist { id } => {
            ensure!(
                nodes.contains_key(id),
                "Selected sync playlist does not exist or is removed"
            );
            let mut selected = BTreeSet::from([id.clone()]);
            loop {
                let children: Vec<_> = nodes
                    .values()
                    .filter(|node| {
                        node.parent_id
                            .as_ref()
                            .is_some_and(|parent| selected.contains(parent))
                    })
                    .map(|node| node.id.clone())
                    .filter(|id| !selected.contains(id))
                    .collect();
                if children.is_empty() {
                    break;
                }
                selected.extend(children);
            }
            let mut parent = nodes[id].parent_id.as_ref();
            while let Some(id) = parent {
                selected.insert(id.clone());
                parent = nodes[id].parent_id.as_ref();
            }
            selected
        }
        DjSyncScope::Modified { .. } => unreachable!(),
    };
    nodes.retain(|id, _| selected.contains(id));
    let mut ordered = Vec::new();
    append_children(&nodes, None, &mut ordered);
    ensure!(
        ordered.len() == selected.len(),
        "Invalid playlist hierarchy in sync scope"
    );
    let mut result = Vec::new();
    for id in ordered {
        let node = &nodes[&id];
        let folder = node.kind == PlaylistKind::Folder;
        let members =
            if folder {
                Vec::new()
            } else {
                if node.kind == PlaylistKind::Smartlist {
                    let materialized: Option<u64> = connection.query_row(
                    "SELECT last_good_revision FROM smart_playlist_status WHERE playlist_id=?1",
                    [&id], |row| row.get(0)).optional()?.flatten();
                    ensure!(
                        materialized.is_some(),
                        "Smartlist '{}' has no materialized result; refresh it before sync",
                        node.name
                    );
                }
                crate::playlists::queries::scoped_members(connection, &tree, &id)?
                    .into_iter()
                    .map(|(id, _, _)| id)
                    .collect()
            };
        result.push(SyncPlaylist {
            smart: node.definition.clone(),
            id: id.clone(),
            parent_id: node.parent_id.clone(),
            name: node.name.clone(),
            folder,
            track_ids: members,
        });
    }
    Ok(result)
}
fn append_children(
    nodes: &BTreeMap<String, PlaylistNode>,
    parent: Option<&str>,
    output: &mut Vec<String>,
) {
    let mut children: Vec<_> = nodes
        .values()
        .filter(|node| node.parent_id.as_deref() == parent)
        .collect();
    children.sort_by(|a, b| (a.position, &a.id).cmp(&(b.position, &b.id)));
    for node in children {
        output.push(node.id.clone());
        append_children(nodes, Some(&node.id), output);
    }
}
fn project_track(row: RawTrack) -> Result<SyncTrack> {
    let analysis: SavedAnalysis = serde_json::from_str(
        row.analysis
            .as_deref()
            .with_context(|| format!("Prepared track '{}' has no saved analysis", row.title))?,
    )
    .with_context(|| format!("Read saved analysis for '{}'", row.title))?;
    let mut publication = analysis.publication.with_context(|| {
        format!(
            "Prepared track '{}' has no saved cue/grid publication; prepare it before sync",
            row.title
        )
    })?;
    if let Some(cues) = row.cues {
        let value: Value = serde_json::from_str(&cues).context("Read latest cue revision")?;
        if value["schema"] == "lexicon-publication-v1" {
            publication =
                serde_json::from_value(value).context("Read latest published cue revision")?;
        }
    }
    ensure!(
        publication.grid.bpm.is_finite()
            && publication.grid.bpm > 0.0
            && publication.grid.start_time.is_finite()
            && publication.grid.start_time >= 0.0,
        "Prepared track contains invalid saved grid timing"
    );
    let info: MediaInfo = serde_json::from_str(&row.metadata)
        .with_context(|| format!("Read prepared metadata for '{}'", row.title))?;
    let columns = info.columns();
    let text = |keys: &[&str]| -> String {
        keys.iter()
            .find_map(|key| columns.get(*key).and_then(Value::as_str))
            .unwrap_or("")
            .to_owned()
    };
    let year = ["tag.year", "tag.date"].iter().find_map(|key| {
        let value = columns.get(*key)?;
        if let Some(year) = value.as_u64().and_then(|year| u16::try_from(year).ok()) {
            return Some(year);
        }
        let text = value.as_str()?.trim();
        let year = if text.as_bytes().get(4) == Some(&b'-') {
            text.get(..4)?
        } else {
            text
        };
        year.parse::<u16>().ok()
    });
    let rating = ["tag.rating", "rating", "tag.stars"]
        .iter()
        .find_map(|key| {
            let value = columns.get(*key)?;
            let number = value.as_u64().or_else(|| value.as_str()?.parse().ok())?;
            (number <= 5).then_some(number as u8)
        })
        .unwrap_or(0);
    let color = columns.get("tag.color").and_then(rgb);
    let cues = publication
        .cues
        .into_iter()
        .map(|cue| {
            ensure!(
                cue.start_time.is_finite() && cue.start_time >= 0.0,
                "Invalid saved cue time"
            );
            if let Some(end) = cue.end_time {
                ensure!(
                    end.is_finite() && end > cue.start_time,
                    "Invalid saved loop time"
                );
            }
            let index =
                u8::try_from(cue.position).context("Saved cue position exceeds target model")?;
            let end_seconds = if cue.kind == PublishedType::Loop {
                Some(cue.end_time.context("Saved loop has no end time")?)
            } else {
                None
            };
            Ok(SyncCue {
                active_loop: end_seconds.is_some() && cue.active_loop.unwrap_or(false),
                index,
                name: cue.name,
                start_seconds: cue.start_time,
                end_seconds,
                color: rgb(&cue.color),
            })
        })
        .collect::<Result<_>>()?;
    let duration_seconds = row.duration.context("Prepared track has no duration")?;
    ensure!(
        duration_seconds.is_finite() && duration_seconds > 0.0,
        "Prepared track duration is invalid"
    );
    Ok(SyncTrack {
        native_path: None,
        id: row.id,
        path: crate::model::bytes_path(&row.path)?,
        title: row.title,
        artist: row.artist,
        album: row.album,
        genre: row.genre,
        label: text(&["tag.label", "tag.publisher"]),
        comment: text(&["tag.comment", "tag.comments"]),
        key: row.key.context("Prepared track has no key")?,
        bpm: publication.grid.bpm,
        duration_seconds,
        sample_rate: if info.sample_rate > 0 {
            info.sample_rate
        } else {
            analysis.source_sample_rate
        },
        export_timing: analysis.media.and_then(|media| media.export_timing),
        native_timeline: None,
        attributes: Default::default(),
        artwork: None,
        rating,
        year,
        color,
        cues,
        beatgrid: vec![SyncBeat {
            seconds: publication.grid.start_time,
            bpm: publication.grid.bpm,
            beat: 1,
        }],
    })
}
fn rgb(value: &Value) -> Option<u32> {
    if let Some(number) = value.as_u64() {
        return u32::try_from(number)
            .ok()
            .filter(|value| *value <= 0xffffff);
    }
    let text = value.as_str()?.trim().to_lowercase();
    if let Some(hex) = text.strip_prefix('#') {
        return u32::from_str_radix(hex, 16)
            .ok()
            .filter(|value| *value <= 0xffffff);
    }
    // Saved publication uses these named colors. Keep the palette in the same
    // RGB space used by its source template, leaving adapter quantization there.
    Some(match text.as_str() {
        "red_dark" => 0x9e1006,
        "red" => 0xe60e0e,
        "red_light" => 0xf2665c,
        "red_orange" => 0xef590f,
        "orange" => 0xe88915,
        "beige" => 0xffe194,
        "yellow_dark" => 0xf5d000,
        "yellow" => 0xf5f50c,
        "lime" => 0xbae815,
        "green_light" => 0xadf55f,
        "green" => 0x7fe80e,
        "green_dark" => 0x4b8c06,
        "teal" => 0x12de78,
        "aqua" => 0x12deca,
        "aqua_dark" => 0x099486,
        "blue_light" => 0x2fa8ed,
        "blue" => 0x1058de,
        "blue_dark" => 0x0028ab,
        "blue_violet" => 0x8400ff,
        "violet" => 0xaa3bff,
        "violet_light" => 0xc68cf3,
        "magenta" => 0xe60ede,
        "magenta_dark" => 0xaa00bd,
        "magenta_red" => 0xde105c,
        "grey_light" => 0xadadad,
        "grey_dark" => 0x5c5c5c,
        "black" => 0x303030,
        "white" => 0xffffff,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use camelot_core::lexicon_publication::{PublishedCue, SavedGrid};
    use rusqlite::params;
    use serde_json::json;

    fn publication() -> Publication {
        Publication {
            grid: SavedGrid {
                bpm: 127.43,
                start_time: 0.234567,
            },
            cues: vec![
                PublishedCue {
                    name: "Drop".into(),
                    start_time: 32.123456,
                    kind: PublishedType::Cue,
                    position: 0,
                    color: json!("red"),
                    end_time: None,
                    active_loop: None,
                    data: None,
                },
                PublishedCue {
                    name: "S.O.S.".into(),
                    start_time: 220.25,
                    kind: PublishedType::Loop,
                    position: 1,
                    color: json!("magenta"),
                    end_time: Some(227.783),
                    active_loop: Some(true),
                    data: None,
                },
            ],
            implementation_sha256: "synthetic-implementation".into(),
            inputs_sha256: "synthetic-inputs".into(),
        }
    }
    fn fixture() -> (tempfile::TempDir, Library) {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(&dir.path().join("library.db")).unwrap();
        for (id, state, archived) in [
            ("a", "ready", false),
            ("b", "ready", false),
            ("c", "ready", false),
            ("incoming", "incoming", false),
            ("archive", "ready", true),
        ] {
            let info = MediaInfo {
                codec: "mp3".into(),
                sample_rate: 48000,
                channels: 2,
                bits_per_sample: None,
                tags: BTreeMap::from([
                    ("label".into(), "Fixture label".into()),
                    ("comment".into(), "Fixture comment".into()),
                    ("date".into(), "2026-10-20".into()),
                    ("rating".into(), "4".into()),
                    ("color".into(), "#123456".into()),
                ]),
                properties: BTreeMap::new(),
            };
            let analysis = json!({"publication":publication(),"source_sample_rate":48000,
                "analysis":{"tempo":{"bpm":127.431234,"beats_seconds":[0.22,0.7,1.17]}}});
            let path = crate::model::path_bytes(&dir.path().join(format!("{id}-not-present.mp3")));
            library.conn().unwrap().execute(
                "INSERT INTO tracks(id,path,title,artist,album,genre,source_hash,state,archived,analysis_json,metadata_json,musical_key,bpm,duration,created,updated,sort_title)
                 VALUES(?1,?2,?1,'Artist','Album','House','synthetic',?3,?4,?5,?6,'8A',127.43,300,1,1,?1)",
                params![id,path,state,archived,analysis.to_string(),serde_json::to_string(&info).unwrap()]).unwrap();
        }
        (dir, library)
    }
    fn playlists(library: &Library) {
        let conn = library.conn().unwrap();
        for (id, parent, kind, position) in [
            ("outside", None, "playlist", 0),
            ("folder", None, "folder", 1),
            ("manual", Some("folder"), "playlist", 1),
            ("smart", Some("folder"), "smartlist", 0),
        ] {
            conn.execute("INSERT INTO playlists(id,parent_id,kind,name,position,last_edit_id) VALUES(?1,?2,?3,?1,?4,'fixture')",params![id,parent,kind,position]).unwrap();
        }
        for (position, id) in ["a", "archive", "incoming"].iter().enumerate() {
            conn.execute(
                "INSERT INTO playlist_members VALUES('manual',?1,?2)",
                params![id, position],
            )
            .unwrap();
        }
        conn.execute("INSERT INTO playlist_members VALUES('outside','c',0)", [])
            .unwrap();
        conn.execute("INSERT INTO smart_playlist_status(playlist_id,state,last_good_revision) VALUES('smart','ready',1)",[]).unwrap();
        conn.execute(
            "INSERT INTO smart_playlist_members VALUES('smart','b',0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO smart_playlist_members VALUES('smart','a',1)",
            [],
        )
        .unwrap();
    }
    #[test]
    fn projects_saved_publication_and_current_metadata_without_reading_sources() {
        let (_dir, library) = fixture();
        let result = library.dj_sync_library(DjSyncScope::Full).unwrap();
        assert_eq!(
            result
                .tracks
                .iter()
                .map(|track| track.id.as_str())
                .collect::<Vec<_>>(),
            ["a", "b", "c"]
        );
        let track = &result.tracks[0];
        assert!(!track.path.exists());
        assert_eq!(track.bpm, 127.43);
        assert_eq!(track.beatgrid.len(), 1);
        assert_eq!(track.beatgrid[0].seconds, 0.234567);
        assert_eq!(track.cues[0].start_seconds, 32.123456);
        assert_eq!(track.cues[1].end_seconds, Some(227.783));
        assert!(!track.cues[0].active_loop);
        assert!(track.cues[1].active_loop);
        assert_eq!(track.cues[1].color, Some(0xe60ede));
        assert_eq!(track.label, "Fixture label");
        assert_eq!(track.comment, "Fixture comment");
        assert_eq!(track.year, Some(2026));
        assert_eq!(track.rating, 4);
        assert_eq!(track.color, Some(0x123456));
        assert_eq!(track.sample_rate, 48000);
    }
    #[test]
    fn playlist_scope_preserves_tree_order_members_and_materialized_smartlists() {
        let (_dir, library) = fixture();
        playlists(&library);
        let result = library
            .dj_sync_library(DjSyncScope::Playlist {
                id: "folder".into(),
            })
            .unwrap();
        assert_eq!(
            result
                .playlists
                .iter()
                .map(|playlist| playlist.id.as_str())
                .collect::<Vec<_>>(),
            ["folder", "smart", "manual"]
        );
        assert_eq!(
            result
                .tracks
                .iter()
                .map(|track| track.id.as_str())
                .collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert_eq!(result.playlists[1].track_ids, ["b", "a"]);
        assert_eq!(result.playlists[2].track_ids, ["a"]);
        assert_eq!(result.playlists[2].parent_id.as_deref(), Some("folder"));
        let single = library
            .dj_sync_library(DjSyncScope::Playlist {
                id: "manual".into(),
            })
            .unwrap();
        assert_eq!(
            single
                .playlists
                .iter()
                .map(|playlist| playlist.id.as_str())
                .collect::<Vec<_>>(),
            ["folder", "manual"]
        );
        assert_eq!(single.tracks.len(), 1);
        let full = library.dj_sync_library(DjSyncScope::Full).unwrap();
        assert_eq!(
            full.playlists
                .iter()
                .map(|playlist| playlist.id.as_str())
                .collect::<Vec<_>>(),
            ["outside", "folder", "smart", "manual"]
        );
    }
    #[test]
    fn modified_scope_requires_explicit_ids_and_never_sends_partial_playlists() {
        let (_dir, library) = fixture();
        playlists(&library);
        let result = library
            .dj_sync_library(DjSyncScope::Modified {
                track_ids: vec!["b".into(), "archive".into(), "incoming".into()],
            })
            .unwrap();
        assert_eq!(result.tracks.len(), 1);
        assert_eq!(result.tracks[0].id, "b");
        assert!(result.playlists.is_empty());
        assert!(
            library
                .dj_sync_library(DjSyncScope::Modified { track_ids: vec![] })
                .unwrap()
                .tracks
                .is_empty()
        );
    }
    #[test]
    fn latest_published_revision_wins_over_original_analysis() {
        let (_dir, library) = fixture();
        let settings = library.settings().unwrap().revision;
        let mut revised = publication();
        revised.grid.bpm = 130.0;
        revised.cues[0].start_time = 50.5;
        let mut json = serde_json::to_value(revised).unwrap();
        json["schema"] = "lexicon-publication-v1".into();
        {
            let conn = library.conn().unwrap();
            conn.execute("INSERT INTO jobs(id,track_id,source_hash,settings_revision,state,stage,created,updated) VALUES('revision-job','a','synthetic',?1,'complete','complete',1,1)",[settings]).unwrap();
            conn.execute(
                "INSERT INTO cue_revisions VALUES('revised','a','revision-job',?1,2)",
                [json.to_string()],
            )
            .unwrap();
        }
        let result = library
            .dj_sync_library(DjSyncScope::Modified {
                track_ids: vec!["a".into()],
            })
            .unwrap();
        assert_eq!(result.tracks[0].bpm, 130.0);
        assert_eq!(result.tracks[0].cues[0].start_seconds, 50.5);
    }
    #[test]
    fn unprepared_publication_and_never_materialized_smartlists_block_sync() {
        let (_dir, library) = fixture();
        playlists(&library);
        library.conn().unwrap().execute("UPDATE smart_playlist_status SET last_good_revision=NULL WHERE playlist_id='smart'",[]).unwrap();
        assert!(
            library
                .dj_sync_library(DjSyncScope::Full)
                .unwrap_err()
                .to_string()
                .contains("materialized")
        );
        library
            .conn()
            .unwrap()
            .execute("UPDATE tracks SET analysis_json=NULL WHERE id='a'", [])
            .unwrap();
        assert!(
            library
                .dj_sync_library(DjSyncScope::Modified {
                    track_ids: vec!["a".into()]
                })
                .unwrap_err()
                .to_string()
                .contains("saved analysis")
        );
    }
}
