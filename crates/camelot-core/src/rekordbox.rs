//! Rekordbox XML interchange. Rendering never opens or modifies audio files.
//!
//! Format: https://cdn.rekordbox.com/files/20200410160904/xml_format_list.pdf
use crate::{Cue, MusicalKey};
use anyhow::{Result, ensure};
use std::{collections::BTreeSet, fmt::Write, path::Path};

/// A caller-verified constant 4/4 grid; start is a first beat in a bar.
/// An estimated BPM or a cue point alone does not establish this information.
#[derive(Debug, Clone, Copy)]
pub struct VerifiedBeatGrid {
    pub start_seconds: f64,
    pub bpm: f64,
}

pub struct ExportTrack<'a> {
    pub id: i32,
    pub path: &'a Path,
    pub title: &'a str,
    pub artist: &'a str,
    pub duration_seconds: f64,
    pub key: Option<MusicalKey>,
    pub bpm: Option<f64>,
    pub cues: &'a [Cue],
    pub verified_grid: Option<VerifiedBeatGrid>,
}

pub struct ExportDocument {
    pub xml: String,
    /// Values in [-10ms, 0) are clamped in XML only. Source cues stay unchanged.
    pub clamped_negative_cues: usize,
}

fn attribute(value: &str) -> Result<String> {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        ensure!(
            matches!(ch, '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}'),
            "Metadata contains an invalid XML 1.0 character"
        );
        escaped.push_str(match ch {
            '&' => "&amp;",
            '<' => "&lt;",
            '>' => "&gt;",
            '\'' => "&apos;",
            '"' => "&quot;",
            '\n' => "&#10;",
            '\r' => "&#13;",
            '\t' => "&#9;",
            _ => {
                escaped.push(ch);
                continue;
            }
        });
    }
    Ok(escaped)
}

fn location(path: &Path) -> Result<String> {
    let url = url::Url::from_file_path(path)
        .map_err(|()| anyhow::anyhow!("Export requires an absolute local audio path"))?;
    ensure!(
        url.host_str().is_none(),
        "Rekordbox export currently requires a local or mounted path rather than a UNC host"
    );
    Ok(format!("file://localhost{}", url.path()))
}

fn valid_bpm(bpm: f64) -> bool {
    bpm.is_finite() && (30.0..=300.0).contains(&bpm)
}

/// Render one collection and playlist, with chronological hot cue slots A–H.
/// Reject unsupported/invalid values instead of silently dropping cues or tracks.
/// Beatgrid elements are emitted only when explicitly supplied by the caller.
pub fn render(tracks: &[ExportTrack<'_>], playlist_name: &str) -> Result<ExportDocument> {
    ensure!(tracks.len() <= i32::MAX as usize, "Too many export tracks");
    let playlist_name = attribute(playlist_name)?;
    let mut xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<DJ_PLAYLISTS Version=\"1.0.0\">\n  <PRODUCT Name=\"CamelotWheel\" Version=\"{}\" Company=\"CamelotWheel contributors\"/>\n  <COLLECTION Entries=\"{}\">\n",
        env!("CARGO_PKG_VERSION"),
        tracks.len()
    );
    let mut ids = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut clamped_negative_cues = 0;
    for track in tracks {
        ensure!(
            track.id > 0 && ids.insert(track.id),
            "Track IDs must be positive and unique"
        );
        let uri = location(track.path)?;
        ensure!(
            paths.insert(uri.clone()),
            "Duplicate exported audio location"
        );
        let duration = track.duration_seconds;
        ensure!(
            duration.is_finite() && duration > 0.0,
            "Invalid export duration"
        );
        ensure!(
            track.cues.len() <= 8,
            "Export supports at most eight hot cues per track"
        );
        write!(
            xml,
            "    <TRACK TrackID=\"{}\" Name=\"{}\" Artist=\"{}\" TotalTime=\"{}\" Location=\"{}\"",
            track.id,
            attribute(track.title)?,
            attribute(track.artist)?,
            duration.floor(),
            attribute(&uri)?
        )?;
        if let Some(key) = track.key {
            write!(xml, " Tonality=\"{}\"", key.camelot())?;
        }
        if let Some(bpm) = track.bpm {
            ensure!(valid_bpm(bpm), "Invalid export BPM");
            write!(xml, " AverageBpm=\"{bpm}\"")?;
        }
        xml.push_str(">\n");
        if let Some(grid) = track.verified_grid {
            ensure!(
                grid.start_seconds.is_finite()
                    && (0.0..duration).contains(&grid.start_seconds)
                    && valid_bpm(grid.bpm),
                "Invalid verified beatgrid"
            );
            writeln!(
                xml,
                "      <TEMPO Inizio=\"{}\" Bpm=\"{}\" Metro=\"4/4\" Battito=\"1\"/>",
                grid.start_seconds, grid.bpm
            )?;
        }
        let mut cues: Vec<_> = track.cues.iter().collect();
        cues.sort_by(|a, b| a.time_seconds.total_cmp(&b.time_seconds));
        for (index, cue) in cues.into_iter().enumerate() {
            ensure!(
                cue.time_seconds.is_finite() && (-0.01..duration).contains(&cue.time_seconds),
                "Cue outside exportable track range"
            );
            clamped_negative_cues += usize::from(cue.time_seconds < 0.0);
            writeln!(
                xml,
                "      <POSITION_MARK Name=\"{}\" Type=\"0\" Start=\"{}\" Num=\"{}\"/>",
                attribute(&cue.name)?,
                cue.time_seconds.max(0.0),
                index
            )?;
        }
        xml.push_str("    </TRACK>\n");
    }
    write!(
        xml,
        "  </COLLECTION>\n  <PLAYLISTS>\n    <NODE Type=\"0\" Name=\"ROOT\" Count=\"1\">\n      <NODE Type=\"1\" Name=\"{playlist_name}\" Entries=\"{}\" KeyType=\"0\">\n",
        tracks.len()
    )?;
    for track in tracks {
        writeln!(xml, "        <TRACK Key=\"{}\"/>", track.id)?;
    }
    xml.push_str("      </NODE>\n    </NODE>\n  </PLAYLISTS>\n</DJ_PLAYLISTS>\n");
    Ok(ExportDocument {
        xml,
        clamped_negative_cues,
    })
}
