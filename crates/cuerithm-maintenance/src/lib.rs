//! Read-only duplicate matching and review suggestions for local music libraries.
//! Metadata groups are candidates, not proof that decoded audio is identical.
mod normalize;
mod review;
pub use review::*;
mod merge;
pub use merge::*;
mod import;
pub use import::*;

use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum Tolerance {
    None,
    Low,
    Medium,
    High,
    #[serde(rename = "High_IgnoreParentheses")]
    HighIgnoreParentheses,
}
impl Tolerance {
    fn fuzzy(self) -> bool {
        matches!(self, Self::Low | Self::High | Self::HighIgnoreParentheses)
    }
    fn broad(self) -> bool {
        matches!(
            self,
            Self::Medium | Self::High | Self::HighIgnoreParentheses
        )
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Candidate {
    pub id: String,
    pub artist: String,
    pub title: String,
    pub duration: f64,
    pub location: String,
    pub comparison_path: String,
    pub bitrate: Option<f64>,
    pub cue_count: usize,
    pub present: bool,
    pub ignored: bool,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PreferredCopy {
    pub keeper: Option<String>,
    pub tags_from: Option<String>,
}

fn validate(tracks: &[Candidate]) -> Result<()> {
    let mut seen = HashSet::new();
    for t in tracks {
        ensure!(
            !t.id.is_empty() && seen.insert(&t.id),
            "Duplicate or empty candidate identity"
        );
        ensure!(
            t.duration.is_finite() && t.duration >= 0.,
            "Invalid candidate duration"
        );
        ensure!(
            t.bitrate.is_none_or(|v| v.is_finite() && v >= 0.),
            "Invalid candidate bitrate"
        );
        ensure!(
            t.artist.len() <= 1_048_576 && t.title.len() <= 1_048_576,
            "Candidate metadata exceeds 1 MiB"
        );
    }
    Ok(())
}
pub struct Matcher {
    comparison_limit: usize,
}
impl Default for Matcher {
    fn default() -> Self {
        Self::new()
    }
}
impl Matcher {
    pub fn new() -> Self {
        Self {
            comparison_limit: 10_000_000,
        }
    }
    pub fn artist_keys(&self, value: &str, broad: bool) -> Vec<String> {
        normalize::artists(value, broad)
    }
    pub fn simplified_text(&self, value: &str) -> String {
        normalize::simple(value)
    }
    pub fn title_trigrams(&self, value: &str, tolerance: Tolerance) -> Vec<String> {
        normalize::trigrams(value, tolerance)
    }

    /// Preserve the original greedy matching order, including overlapping artist groups.
    /// The caller resolves exact-audio precedence before offering merge actions.
    pub fn metadata_groups(
        &self,
        tracks: &[Candidate],
        tolerance: Tolerance,
        ignored_paths: &[String],
        video_ids: &[String],
    ) -> Result<Vec<Vec<String>>> {
        validate(tracks)?;
        if tolerance.fuzzy() {
            return self.fuzzy_groups(tracks, tolerance, ignored_paths, video_ids);
        }
        let mut indexes = HashMap::new();
        let mut groups: Vec<(usize, Vec<usize>)> = Vec::new();
        let mut artist_order = HashMap::new();
        for (index, track) in tracks.iter().enumerate() {
            if !eligible(track, ignored_paths, video_ids) {
                continue;
            }
            let artists = if tolerance == Tolerance::None {
                normalize::artist_parts(&track.artist, false)
            } else {
                normalize::artists(&track.artist, true)
            };
            for artist in artists {
                let title = strip_artist(&track.title, &artist);
                let key = if tolerance == Tolerance::None {
                    normalize::simple(&normalize::original(title)).to_lowercase()
                } else {
                    normalize::simple(&normalize::loose_title(title)).to_lowercase()
                };
                if key.is_empty() {
                    continue;
                }
                let next_artist = artist_order.len();
                let artist_index = *artist_order.entry(artist.clone()).or_insert(next_artist);
                let next = groups.len();
                let group = *indexes.entry((artist, key)).or_insert_with(|| {
                    groups.push((artist_index, Vec::new()));
                    next
                });
                if groups[group].1.last() != Some(&index) {
                    groups[group].1.push(index);
                }
            }
        }
        let mut result = Vec::new();
        groups.sort_by_key(|g| g.0);
        for (_, mut group) in groups {
            if group.len() < 2 {
                continue;
            }
            if tolerance == Tolerance::None {
                group.sort_by(|a, b| tracks[*a].duration.total_cmp(&tracks[*b].duration));
                let mut run: Vec<usize> = Vec::new();
                for i in group {
                    if run.last().is_some_and(|last| {
                        (tracks[i].duration - tracks[*last].duration).abs() > 5.
                    }) {
                        push_group(&mut result, &run, tracks);
                        run.clear();
                    }
                    run.push(i);
                }
                push_group(&mut result, &run, tracks);
            } else {
                push_group(&mut result, &group, tracks);
            }
        }
        Ok(result)
    }

    fn fuzzy_groups(
        &self,
        tracks: &[Candidate],
        tolerance: Tolerance,
        ignored_paths: &[String],
        video_ids: &[String],
    ) -> Result<Vec<Vec<String>>> {
        struct Entry {
            track: usize,
            trigrams: HashSet<String>,
            matches: Vec<usize>,
        }
        let mut buckets: Vec<Vec<Entry>> = Vec::new();
        let mut indexes = HashMap::new();
        for (i, t) in tracks.iter().enumerate() {
            if t.artist.is_empty() || t.title.is_empty() {
                continue;
            }
            let grams = normalize::trigrams(&t.title, tolerance);
            if grams.is_empty() {
                continue;
            }
            for artist in normalize::artists(&t.artist, tolerance.broad()) {
                let next = buckets.len();
                let bucket = *indexes.entry(artist).or_insert_with(|| {
                    buckets.push(vec![]);
                    next
                });
                buckets[bucket].push(Entry {
                    track: i,
                    trigrams: grams.iter().cloned().collect(),
                    matches: vec![],
                });
            }
        }
        let mut matched = HashSet::new();
        let mut comparisons = 0usize;
        for (i, t) in tracks.iter().enumerate() {
            if matched.contains(&i) || !eligible(t, ignored_paths, video_ids) {
                continue;
            }
            for artist in normalize::artists(&t.artist, tolerance.broad()) {
                let grams: HashSet<_> =
                    normalize::trigrams(strip_artist(&t.title, &artist), tolerance)
                        .into_iter()
                        .collect();
                if grams.is_empty() {
                    continue;
                }
                let Some(bucket) = indexes.get(&artist) else {
                    continue;
                };
                let threshold = if tolerance == Tolerance::Low {
                    0.9
                } else if grams.len() <= 6 {
                    0.5
                } else {
                    0.7
                };
                for entry in &mut buckets[*bucket] {
                    comparisons += 1;
                    ensure!(
                        comparisons <= self.comparison_limit,
                        "Duplicate comparison budget exceeded; narrow the scan scope"
                    );
                    let score = 2. * grams.intersection(&entry.trigrams).count() as f64
                        / (grams.len() + entry.trigrams.len()) as f64;
                    if score >= threshold {
                        entry.matches.push(i);
                        matched.insert(i);
                        matched.insert(entry.track);
                        break;
                    }
                }
            }
        }
        let mut result = Vec::new();
        for bucket in buckets {
            for entry in bucket {
                let mut group = vec![entry.track];
                let mut seen = HashSet::from([entry.track]);
                for i in entry.matches {
                    if seen.insert(i) {
                        group.push(i);
                    }
                }
                push_group(&mut result, &group, tracks);
            }
        }
        Ok(result)
    }

    /// Suggest a keeper. Only exact-audio groups may suggest a separate tag/cue donor.
    /// Selection never changes records, playlists, tags or source audio.
    pub fn preselect(&self, tracks: &[Candidate], exact_audio: bool) -> Result<PreferredCopy> {
        validate(tracks)?;
        let available: Vec<_> = tracks.iter().filter(|t| t.present && !t.ignored).collect();
        let most = available
            .iter()
            .copied()
            .max_by_key(|t| t.cue_count)
            .filter(|t| t.cue_count > 0);
        // max_by_key returns the last equal item; the shipped helper chooses the first.
        let most = most.and_then(|maximum| {
            available
                .iter()
                .copied()
                .find(|t| t.cue_count == maximum.cue_count)
        });
        let mut quality: Vec<_> = available
            .iter()
            .copied()
            .filter(|t| t.bitrate.is_some())
            .collect();
        quality.sort_by(|a, b| b.bitrate.unwrap().total_cmp(&a.bitrate.unwrap()));
        let mp3 = available.iter().copied().find(|t| {
            t.bitrate.is_some_and(|v| v >= 320.)
                && t.location
                    .rsplit('.')
                    .next()
                    .is_some_and(|s| s.eq_ignore_ascii_case("mp3"))
        });
        let mut donor = None;
        let chosen = if let Some(mp3) =
            mp3.filter(|mp3| most.is_none_or(|m| m.cue_count == mp3.cue_count))
        {
            Some(mp3)
        } else if quality.len() > 1 {
            if let Some(most) = most {
                if quality.iter().any(|t| t.id == most.id) {
                    Some(most)
                } else {
                    donor = Some(most);
                    preferred_path(&quality)
                }
            } else {
                preferred_path(&quality)
            }
        } else if quality.len() == 1 {
            donor = most.filter(|m| m.id != quality[0].id);
            Some(quality[0])
        } else {
            donor = most;
            most.or_else(|| available.first().copied())
        };
        Ok(PreferredCopy {
            keeper: chosen.map(|t| t.id.clone()),
            tags_from: donor.filter(|_| exact_audio).map(|t| t.id.clone()),
        })
    }
}

fn eligible(track: &Candidate, ignored_paths: &[String], video_ids: &[String]) -> bool {
    !track.artist.is_empty()
        && !track.title.is_empty()
        && !video_ids.contains(&track.id)
        && !ignored_paths
            .iter()
            .any(|p| track.location.to_lowercase().starts_with(&p.to_lowercase()))
}
fn strip_artist<'a>(title: &'a str, artist: &str) -> &'a str {
    let title = title.trim();
    let prefix = format!("{artist} - ");
    if title
        .get(..prefix.len())
        .is_some_and(|s| s.eq_ignore_ascii_case(&prefix))
    {
        &title[prefix.len()..]
    } else {
        title
    }
}
fn push_group(result: &mut Vec<Vec<String>>, indexes: &[usize], tracks: &[Candidate]) {
    if indexes.len() > 1 {
        result.push(indexes.iter().map(|i| tracks[*i].id.clone()).collect());
    }
}
fn preferred_path<'a>(tracks: &[&'a Candidate]) -> Option<&'a Candidate> {
    let first = *tracks.first()?;
    let parts: Vec<_> = tracks
        .iter()
        .map(|t| {
            let path = t.comparison_path.replace('\\', "/");
            let (dir, file) = path.rsplit_once('/').unwrap_or(("", &path));
            let stem = file.rsplit_once('.').map_or(file, |(s, _)| s);
            (dir.to_lowercase(), stem.to_owned())
        })
        .collect();
    if parts.iter().all(|p| p.0 == parts[0].0) && !parts.iter().all(|p| p.1 == parts[0].1) {
        for kind in 0..3 {
            let duplicate = |stem: &str| {
                let stem = stem.trim();
                let bytes = stem.as_bytes();
                match kind {
                    0 => stem.to_lowercase().ends_with("copy"),
                    1 => {
                        bytes.len() >= 3
                            && bytes[bytes.len() - 3] == b'('
                            && bytes[bytes.len() - 2].is_ascii_digit()
                            && bytes[bytes.len() - 1] == b')'
                    }
                    _ => {
                        bytes.len() >= 2
                            && matches!(bytes[bytes.len() - 2], b' ' | b'-')
                            && bytes[bytes.len() - 1].is_ascii_digit()
                    }
                }
            };
            if parts.iter().any(|p| duplicate(&p.1)) {
                return Some(
                    parts
                        .iter()
                        .position(|p| !duplicate(&p.1))
                        .map_or(first, |i| tracks[i]),
                );
            }
        }
    }
    Some(first)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exhausted_comparison_budget_returns_error_instead_of_partial_groups() {
        let tracks: Vec<_> = (0..4)
            .map(|i| Candidate {
                id: i.to_string(),
                artist: "North".into(),
                title: "Signal".into(),
                duration: 180.,
                location: format!("/authored/{i}.flac"),
                comparison_path: format!("/authored/{i}.flac"),
                bitrate: Some(1000.),
                cue_count: 0,
                present: true,
                ignored: false,
            })
            .collect();
        let limited = Matcher {
            comparison_limit: 2,
        };
        assert!(
            limited
                .metadata_groups(&tracks, Tolerance::Low, &[], &[])
                .unwrap_err()
                .to_string()
                .contains("budget")
        );
        assert_eq!(
            Matcher::new()
                .metadata_groups(&tracks, Tolerance::Low, &[], &[])
                .unwrap(),
            vec![vec!["0", "1", "2", "3"]]
        );
    }
}
