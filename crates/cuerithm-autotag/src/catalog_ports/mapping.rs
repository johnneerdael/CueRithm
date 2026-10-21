//! Adapted from OneTagger's pinned catalog mappings (GPL-3.0-only).
//! Missing optional values stay missing; malformed collections are never no-match.
use crate::{CatalogTrack, ProviderId};
use anyhow::{Context, Result, bail, ensure};
use chrono::NaiveDate;
use serde_json::Value;
use std::time::Duration;

fn string(v: &Value, key: &str) -> Option<String> {
    v.get(key)?
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .map(str::to_owned)
}
fn id(v: &Value, key: &str) -> Option<String> {
    let v = v.get(key)?;
    v.as_u64()
        .filter(|n| *n > 0)
        .map(|n| n.to_string())
        .or_else(|| {
            v.as_str()
                .filter(|s| {
                    !s.is_empty() && s.len() <= 128 && s.bytes().all(|c| c.is_ascii_alphanumeric())
                })
                .map(str::to_owned)
        })
}
fn names(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(64)
        .filter_map(|a| string(a, "name"))
        .collect()
}
fn date(s: Option<String>) -> Option<NaiveDate> {
    s.as_deref()
        .and_then(|s| s.get(..10))
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
}
fn number(v: &Value, key: &str) -> Option<u16> {
    v.get(key)?
        .as_u64()
        .and_then(|n| u16::try_from(n).ok())
        .filter(|n| *n > 0)
}
fn seconds(v: &Value, key: &str) -> Duration {
    Duration::from_secs(
        v.get(key)
            .and_then(Value::as_u64)
            .filter(|n| *n <= 86_400)
            .unwrap_or(0),
    )
}
pub(super) fn search(provider: ProviderId, body: &Value) -> Result<Vec<CatalogTrack>> {
    if body.get("error").is_some() {
        let code = body["error"]["code"].as_i64().unwrap_or(0);
        return Err(crate::BackendFailure::new(
            if code == 4 {
                crate::ProviderFailureKind::RateLimited
            } else {
                crate::ProviderFailureKind::Unavailable
            },
            format!(
                "{} catalog returned an API error ({code})",
                provider.as_str()
            ),
        )
        .into());
    }
    let array = match provider {
        ProviderId::Itunes => body.get("results"),
        ProviderId::Deezer | ProviderId::Bpmsupreme => body.get("data"),
        ProviderId::Bandcamp => body.get("auto").and_then(|v| v.get("results")),
        _ => bail!("Unsupported catalog response"),
    }
    .and_then(Value::as_array)
    .context("Catalog response has no expected result collection")?;
    ensure!(
        array.len() <= 1000,
        "Catalog result collection exceeds limit"
    );
    let mut tracks = Vec::new();
    for row in array.iter().take(100) {
        if provider == ProviderId::Itunes && row["kind"] != "song" {
            continue;
        }
        let title_key = match provider {
            ProviderId::Itunes => "trackName",
            ProviderId::Bandcamp => "name",
            ProviderId::Deezer => "title_short",
            _ => "title",
        };
        let title = string(row, title_key)
            .or_else(|| string(row, "title"))
            .context("Catalog track has no title")?;
        let track_id = id(
            row,
            if provider == ProviderId::Itunes {
                "trackId"
            } else {
                "id"
            },
        )
        .context("Catalog track has no valid identifier")?;
        let mut t = CatalogTrack {
            platform: provider.as_str().into(),
            title,
            track_id: Some(track_id.clone()),
            ..Default::default()
        };
        match provider {
            ProviderId::Itunes => {
                t.artists = string(row, "artistName").into_iter().collect();
                t.album = string(row, "collectionName");
                t.release_id = id(row, "collectionId");
                t.url = string(row, "trackViewUrl")
                    .unwrap_or_else(|| format!("https://music.apple.com/song/{track_id}"));
                t.duration = Duration::from_millis(
                    row["trackTimeMillis"].as_u64().unwrap_or(0).min(86_400_000),
                );
                t.genres = string(row, "primaryGenreName").into_iter().collect();
                t.release_date = date(string(row, "releaseDate"));
                t.track_number = number(row, "trackNumber").map(|n| (n as i32).into());
                t.track_total = number(row, "trackCount");
                t.disc_number = number(row, "discNumber");
                t.art = string(row, "artworkUrl100").map(|s| s.replace("100x100bb", "1000x1000bb"));
                t.explicit = row["trackExplicitness"].as_str().map(|s| s == "explicit");
            }
            ProviderId::Deezer => {
                // #359: contributors retain all credited artists; never join them into one name.
                t.artists = names(row, "contributors");
                if t.artists.is_empty() {
                    t.artists = string(&row["artist"], "name").into_iter().collect();
                }
                t.version = string(row, "title_version");
                t.album = string(&row["album"], "title");
                t.release_id = id(&row["album"], "id");
                t.duration = seconds(row, "duration");
                t.url = format!("https://www.deezer.com/track/{track_id}");
                t.art = string(&row["album"], "cover_xl")
                    .or_else(|| string(&row["album"], "cover_big"));
                t.isrc = string(row, "isrc");
                t.release_date = date(string(row, "release_date"));
                t.track_number = number(row, "track_position").map(|n| (n as i32).into());
                t.disc_number = number(row, "disk_number");
                t.bpm = row["bpm"]
                    .as_f64()
                    .filter(|b| b.is_finite() && *b > 1. && *b < 1000.)
                    .map(|b| b.round() as i64);
                t.explicit = row["explicit_lyrics"].as_bool();
                // Track IDs are not label catalog numbers (#493 mapping regression).
            }
            ProviderId::Bandcamp => {
                t.artists = string(row, "band_name").into_iter().collect();
                t.album = string(row, "album_name");
                t.release_id = id(row, "album_id");
                t.url = string(row, "item_url_path").context("Bandcamp result has no track URL")?;
                let url = reqwest::Url::parse(&t.url)?;
                ensure!(
                    url.scheme() == "https"
                        && url.host_str().is_some_and(|h| h.ends_with(".bandcamp.com"))
                        && url.path().starts_with("/track/")
                        && url.port_or_known_default() == Some(443)
                        && url.username().is_empty()
                        && url.password().is_none(),
                    "Bandcamp result is not an approved track"
                );
                // #334: uploader/label may differ from artist. Do not split arbitrary titles.
            }
            ProviderId::Bpmsupreme => {
                t.artists = string(row, "artist").into_iter().collect();
                t.bpm = row["bpm"].as_i64();
                t.genres = string(&row["genre"], "name").into_iter().collect();
                t.key = string(row, "key");
                t.label = string(row, "label");
                t.release_date = date(string(row, "created_at"));
                t.url = format!("https://app.bpmsupreme.com/d/album/{track_id}");
                t.art = string(row, "cover_url").filter(|s| !s.contains("default_cover.png"));
                // #114: never fabricate a base-version match when distinct downloadable editions exist.
                let media = row["media"]
                    .as_array()
                    .filter(|a| !a.is_empty())
                    .context("BPM Supreme edition collection is unavailable")?;
                ensure!(
                    media.len() <= 100,
                    "BPM Supreme edition collection exceeds limit"
                );
                ensure!(!t.artists.is_empty(), "BPM Supreme artist credit missing");
                for m in media {
                    let mut edition = t.clone();
                    edition.release_id = Some(track_id.clone());
                    edition.title = string(m, "name").context("BPM Supreme edition has no name")?;
                    edition.version = string(m, "version").filter(|version| {
                        !edition
                            .title
                            .to_lowercase()
                            .ends_with(&format!("({})", version.trim().to_lowercase()))
                            && !edition
                                .title
                                .to_lowercase()
                                .ends_with(&format!("[{}]", version.trim().to_lowercase()))
                    });
                    edition.track_id = id(m, "id").map(|id| format!("{track_id}-{id}"));
                    tracks.push(edition);
                }
                continue;
            }
            _ => unreachable!(),
        }
        ensure!(!t.artists.is_empty(), "Catalog track has no artist credit");
        tracks.push(t);
    }
    Ok(tracks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn deezer_multiple_contributors_and_no_fake_catalog_number() {
        let t = search(ProviderId::Deezer, &json!({"data":[{"id":1,"title_short":"Example","contributors":[{"name":"A"},{"name":"B"}],"album":{"id":2,"title":"Release"},"duration":123}]})).unwrap().remove(0);
        assert_eq!(t.artists, ["A", "B"]);
        assert_eq!(t.catalog_number, None);
    }
    #[test]
    fn itunes_ignores_non_music_and_short_unicode_dates_are_safe() {
        let body = json!({"results":[{"kind":"podcast"},{"kind":"song","trackId":1,"trackName":"Example","artistName":"A","releaseDate":"é"}]});
        let tracks = search(ProviderId::Itunes, &body).unwrap();
        assert_eq!(tracks.len(), 1);
        assert!(tracks[0].release_date.is_none());
    }
    #[test]
    fn api_error_and_schema_change_are_not_no_match() {
        assert!(search(ProviderId::Deezer, &json!({"error":{"code":4}})).is_err());
        assert!(search(ProviderId::Itunes, &json!({"changed":[]})).is_err());
        assert!(
            search(ProviderId::Itunes, &json!({"results":[]}))
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn pool_editions_do_not_collapse_to_invented_base() {
        let tracks=search(ProviderId::Bpmsupreme,&json!({"data":[{"id":1,"title":"Example","artist":"A","media":[{"id":2,"name":"Example (Clean Intro)"},{"id":3,"name":"Example (Dirty)"}]}]})).unwrap();
        assert_eq!(tracks.len(), 2);
        assert_ne!(tracks[0].track_id, tracks[1].track_id);
        assert!(tracks[0].title.contains("Clean"));
    }
}

#[cfg(test)]
mod issue_regressions {
    use super::*;
    use serde_json::json;
    #[test]
    fn pool_missing_edition_collection_is_not_a_fabricated_base_recording() {
        assert!(
            search(
                ProviderId::Bpmsupreme,
                &json!({"data":[{"id":1,"title":"Signal","artist":"Artist","media":[]}]})
            )
            .is_err()
        );
    }
}

#[cfg(test)]
mod edition_regressions {
    use super::*;
    use serde_json::json;
    #[test]
    fn pool_titles_are_not_duplicated_and_unidentified_editions_have_no_fake_track_id() {
        let tracks=search(ProviderId::Bpmsupreme,&json!({"data":[{"id":1,"title":"Signal","artist":"Artist","media":[{"name":"Signal (Clean Intro)","version":"Clean Intro","type":"mp3"},{"name":"Signal (Dirty)","type":"mp3"}]}]})).unwrap();
        assert_eq!(tracks[0].full_title(), "Signal (Clean Intro)");
        assert_eq!(tracks[1].full_title(), "Signal (Dirty)");
        assert!(tracks.iter().all(|track| track.track_id.is_none()));
        assert!(
            tracks
                .iter()
                .all(|track| track.release_id.as_deref() == Some("1"))
        );
    }
}
