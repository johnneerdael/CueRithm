//! Bounded local metadata matching. A candidate is never proof of recording identity.
use crate::{Library, PlaylistChange, TrackSummary};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use unicode_normalization::UnicodeNormalization;

pub const TRACKLIST_MAX_BYTES: usize = 1_048_576;
pub const TRACKLIST_MAX_ROWS: usize = 1000;
pub const TRACKLIST_MAX_FIELD_BYTES: usize = 4096;
pub const TRACKLIST_MAX_COMPARISONS: usize = 10_000_000;
pub const TRACKLIST_MAX_CANDIDATES: usize = 10;
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TracklistFormat {
    #[default]
    Text,
    Csv,
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TracklistMode {
    #[default]
    Exact,
    Fuzzy,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TracklistOptions {
    pub format: TracklistFormat,
    pub headers: bool,
    pub swap_artist_title: bool,
    pub mode: TracklistMode,
}
impl Default for TracklistOptions {
    fn default() -> Self {
        Self {
            format: TracklistFormat::Text,
            headers: true,
            swap_artist_title: false,
            mode: TracklistMode::Exact,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracklistEntry {
    pub row: usize,
    pub artist: String,
    pub title: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TracklistReason {
    SameTags,
    SimilarTags,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracklistCandidate {
    pub track: TrackSummary,
    pub reason: TracklistReason,
    pub score: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracklistRow {
    pub entry: TracklistEntry,
    pub candidates: Vec<TracklistCandidate>,
    pub candidate_count: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracklistResult {
    pub rows: Vec<TracklistRow>,
    pub library_revision: u64,
    pub playlist_revision: u64,
    pub candidate_limit: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracklistSaveRequest {
    pub name: String,
    pub track_ids: Vec<String>,
    pub expected_library_revision: u64,
    pub expected_playlist_revision: u64,
}

fn entry(row: usize, artist: &str, title: &str, swap: bool) -> Result<TracklistEntry> {
    ensure!(
        artist.len() <= TRACKLIST_MAX_FIELD_BYTES && title.len() <= TRACKLIST_MAX_FIELD_BYTES,
        "Row {row}: fields must be at most 4,096 bytes"
    );
    let (artist, title) = if swap {
        (title.trim(), artist.trim())
    } else {
        (artist.trim(), title.trim())
    };
    ensure!(!title.is_empty(), "Row {row}: title is empty");
    Ok(TracklistEntry {
        row,
        artist: artist.into(),
        title: title.into(),
    })
}
// csv intentionally accepts some non-RFC malformed quoting. Validate quoting first,
// then delegate record boundaries, escaping, UTF-8 and column widths to csv.
fn validate_csv_quotes(input: &str) -> Result<()> {
    #[derive(Clone, Copy)]
    enum State {
        Start,
        Plain,
        Quoted,
        Closed,
    }
    let mut state = State::Start;
    for b in input.bytes() {
        state = match (state, b) {
            (State::Start, b'"') => State::Quoted,
            (State::Start | State::Plain | State::Closed, b',' | b'\n' | b'\r') => State::Start,
            (State::Quoted, b'"') => State::Closed,
            (State::Closed, b'"') => State::Quoted,
            (State::Quoted, _) => State::Quoted,
            (State::Plain, b'"') | (State::Closed, _) => anyhow::bail!(
                "Malformed CSV quoting; quote the complete field and escape quotes as two quotes"
            ),
            _ => State::Plain,
        };
    }
    ensure!(
        !matches!(state, State::Quoted),
        "Malformed CSV: unclosed quoted field"
    );
    Ok(())
}
pub fn parse_tracklist(input: &str, options: &TracklistOptions) -> Result<Vec<TracklistEntry>> {
    ensure!(
        input.len() <= TRACKLIST_MAX_BYTES,
        "Tracklist exceeds 1 MiB; choose a smaller file or paste fewer rows"
    );
    let input = input.strip_prefix('\u{feff}').unwrap_or(input);
    let mut rows = vec![];
    match options.format {
        TracklistFormat::Text => {
            for (i, line) in input.lines().enumerate() {
                if line.trim().is_empty() {
                    continue;
                }
                let (artist, title) = line.split_once(" - ").unwrap_or(("", line));
                // A title-only line stays title-only when swapping paired columns.
                rows.push(entry(
                    i + 1,
                    artist,
                    title,
                    options.swap_artist_title && !artist.is_empty(),
                )?);
                ensure!(
                    rows.len() <= TRACKLIST_MAX_ROWS,
                    "Tracklist exceeds 1,000 entries"
                );
            }
        }
        TracklistFormat::Csv => {
            validate_csv_quotes(input)?;
            let mut reader = csv::ReaderBuilder::new()
                .has_headers(options.headers)
                .from_reader(input.as_bytes());
            let (artist, title) = if options.headers && !input.trim().is_empty() {
                let headers = reader.headers()?;
                ensure!(
                    headers.iter().all(|f| f.len() <= TRACKLIST_MAX_FIELD_BYTES),
                    "CSV fields must be at most 4,096 bytes"
                );
                let artists: Vec<_> = headers
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.trim().eq_ignore_ascii_case("artist"))
                    .map(|(i, _)| i)
                    .collect();
                let titles: Vec<_> = headers
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.trim().eq_ignore_ascii_case("title"))
                    .map(|(i, _)| i)
                    .collect();
                ensure!(
                    artists.len() == 1 && titles.len() == 1,
                    "CSV needs one artist column and one title column; check the header setting"
                );
                (artists[0], titles[0])
            } else {
                (0, 1)
            };
            for record in reader.records() {
                let record = record?;
                ensure!(
                    record.iter().all(|f| f.len() <= TRACKLIST_MAX_FIELD_BYTES),
                    "CSV fields must be at most 4,096 bytes"
                );
                ensure!(
                    record.len() > artist.max(title),
                    "CSV needs artist and title columns"
                );
                let row = record
                    .position()
                    .map_or(rows.len() + 1, |p| p.line() as usize);
                rows.push(entry(
                    row,
                    &record[artist],
                    &record[title],
                    options.swap_artist_title,
                )?);
                ensure!(
                    rows.len() <= TRACKLIST_MAX_ROWS,
                    "Tracklist exceeds 1,000 entries"
                );
            }
        }
    }
    Ok(rows)
}
fn normalized(value: &str) -> String {
    value
        .nfkc()
        .flat_map(char::to_lowercase)
        .map(|c| {
            if c.is_alphanumeric() || unicode_normalization::char::is_combining_mark(c) {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
fn trigrams(value: &str) -> BTreeSet<[char; 3]> {
    let chars: Vec<_> = format!("  {value}  ").chars().collect();
    chars.windows(3).map(|c| [c[0], c[1], c[2]]).collect()
}
fn similarity(a: &BTreeSet<[char; 3]>, b: &BTreeSet<[char; 3]>) -> f64 {
    2.0 * a.intersection(b).count() as f64 / (a.len() + b.len()) as f64
}
pub fn match_tracklist(
    entries: &[TracklistEntry],
    tracks: &[TrackSummary],
    mode: TracklistMode,
) -> Result<Vec<TracklistRow>> {
    ensure!(
        entries.len() <= TRACKLIST_MAX_ROWS,
        "Tracklist exceeds 1,000 entries"
    );
    let eligible = tracks
        .iter()
        .filter(|t| t.state == "ready" && !t.archived)
        .count();
    ensure!(
        entries.len().saturating_mul(eligible) <= TRACKLIST_MAX_COMPARISONS,
        "Matching exceeds 10 million comparisons; match fewer tracklist rows at a time"
    );
    let tracks: Vec<_> = tracks
        .iter()
        .filter(|t| t.state == "ready" && !t.archived)
        .map(|t| {
            let a = normalized(&t.artist);
            let n = normalized(&t.title);
            (t, a.clone(), n.clone(), trigrams(&a), trigrams(&n))
        })
        .collect();
    entries
        .iter()
        .map(|e| {
            let artist = normalized(&e.artist);
            let title = normalized(&e.title);
            let ag = trigrams(&artist);
            let tg = trigrams(&title);
            ensure!(
                !title.is_empty(),
                "Row {}: title has no searchable characters",
                e.row
            );
            let mut candidates = vec![];
            let mut candidate_count = 0;
            for (track, a, t, a_grams, t_grams) in &tracks {
                let same = title == *t && (artist.is_empty() || artist == *a);
                if !same && matches!(mode, TracklistMode::Exact) {
                    continue;
                }
                let title_score = if same { 1.0 } else { similarity(&tg, t_grams) };
                let artist_score = if same || artist.is_empty() {
                    1.0
                } else {
                    similarity(&ag, a_grams)
                };
                // Conservative thresholds: title >= .72, supplied artist >= .60,
                // weighted similarity >= .80. Version words remain in every comparison.
                let score = if artist.is_empty() {
                    title_score
                } else {
                    0.8 * title_score + 0.2 * artist_score
                };
                if same
                    || (matches!(mode, TracklistMode::Fuzzy)
                        && title_score >= 0.72
                        && artist_score >= 0.60
                        && score >= 0.80)
                {
                    candidate_count += 1;
                    candidates.push(TracklistCandidate {
                        track: (*track).clone(),
                        reason: if same {
                            TracklistReason::SameTags
                        } else {
                            TracklistReason::SimilarTags
                        },
                        score: if same { 1.0 } else { score },
                    });
                    candidates.sort_by(|a, b| {
                        b.score.total_cmp(&a.score).then_with(|| {
                            (
                                normalized(&a.track.artist),
                                normalized(&a.track.title),
                                &a.track.id,
                            )
                                .cmp(&(
                                    normalized(&b.track.artist),
                                    normalized(&b.track.title),
                                    &b.track.id,
                                ))
                        })
                    });
                    candidates.truncate(TRACKLIST_MAX_CANDIDATES);
                }
            }
            Ok(TracklistRow {
                entry: e.clone(),
                candidates,
                candidate_count,
            })
        })
        .collect()
}
const SUMMARY_COLUMNS: &str = "id,path,title,artist,album,genre,state,musical_key,bpm,energy,duration,source_hash,metadata_json,created,updated,archived,json_extract(tagging_json,'$.status')";
impl Library {
    pub fn match_tracklist(
        &self,
        input: String,
        options: TracklistOptions,
    ) -> Result<TracklistResult> {
        let entries = parse_tracklist(&input, &options)?;
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let library_revision =
            tx.query_row("SELECT value FROM meta WHERE key='revision'", [], |r| {
                r.get(0)
            })?;
        let playlist_revision = tx.query_row(
            "SELECT value FROM meta WHERE key='playlist_revision'",
            [],
            |r| r.get(0),
        )?;
        let tracks=tx.prepare(&format!("SELECT {SUMMARY_COLUMNS} FROM tracks WHERE state='ready' AND archived=0 ORDER BY id"))?.query_map([],crate::store::summary_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
        let rows = match_tracklist(&entries, &tracks, options.mode)?;
        tx.commit()?;
        Ok(TracklistResult {
            rows,
            library_revision,
            playlist_revision,
            candidate_limit: TRACKLIST_MAX_CANDIDATES,
        })
    }
    pub fn save_tracklist_playlist(&self, request: TracklistSaveRequest) -> Result<PlaylistChange> {
        ensure!(
            !request.track_ids.is_empty() && request.track_ids.len() <= TRACKLIST_MAX_ROWS,
            "Choose 1 to 1,000 reviewed rows"
        );
        self.save_prepared_playlist(crate::PreparedPlaylistSaveRequest {
            name: request.name,
            track_ids: request.track_ids,
            expected_library_revision: request.expected_library_revision,
            expected_playlist_revision: request.expected_playlist_revision,
        })
    }
}
