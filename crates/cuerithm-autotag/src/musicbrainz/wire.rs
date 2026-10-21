use crate::*;
use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::time::Duration;
fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key)?
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}
fn id(v: &Value, key: &str) -> Result<Option<Mbid>> {
    text(v, key).map(Mbid::new).transpose()
}
fn array<'a>(v: &'a Value, key: &str, max: usize) -> Result<&'a [Value]> {
    match v.get(key) {
        None | Some(Value::Null) => Ok(&[]),
        Some(Value::Array(a)) => {
            ensure!(a.len() <= max, "MusicBrainz collection exceeds bounds");
            Ok(a)
        }
        _ => anyhow::bail!("Malformed MusicBrainz collection"),
    }
}
pub(super) fn date(v: Option<&str>) -> Result<Option<TransferDate>> {
    let Some(v) = v.filter(|s| !s.trim().is_empty()) else {
        return Ok(None);
    };
    ensure!(
        matches!(v.len(), 4 | 7 | 10)
            && v.bytes().enumerate().all(|(i, c)| if i == 4 || i == 7 {
                c == b'-'
            } else {
                c.is_ascii_digit()
            }),
        "Malformed MusicBrainz date"
    );
    let p: Vec<_> = v.split('-').collect();
    let d = TransferDate {
        year: p[0].parse()?,
        month: p.get(1).map(|n| n.parse()).transpose()?,
        day: p.get(2).map(|n| n.parse()).transpose()?,
    };
    crate::validate_date(&d)?;
    Ok(Some(d))
}
fn credits(v: &Value) -> Result<Vec<CatalogArtistCredit>> {
    array(v, "artist-credit", 64)?
        .iter()
        .map(|c| {
            let artist = &c["artist"];
            let name = text(artist, "name").context("Missing MusicBrainz artist name")?;
            Ok(CatalogArtistCredit {
                artist_id: id(artist, "id")?.context("Missing MusicBrainz artist ID")?,
                credited_name: text(c, "name").unwrap_or_else(|| name.clone()),
                name,
                join_phrase: c
                    .get("joinphrase")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .into(),
            })
        })
        .collect()
}
fn isrcs(v: &Value) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for code in array(v, "isrcs", 64)? {
        let code = code
            .as_str()
            .or_else(|| code.get("id")?.as_str())
            .context("Malformed MusicBrainz ISRC")?
            .to_ascii_uppercase();
        ensure!(
            code.len() == 12 && code.bytes().all(|c| c.is_ascii_alphanumeric()),
            "Malformed MusicBrainz ISRC"
        );
        if !out.contains(&code) {
            out.push(code)
        }
    }
    Ok(out)
}
fn base(v: &Value) -> Result<MusicBrainzFacts> {
    Ok(MusicBrainzFacts {
        schema_version: 1,
        recording_id: id(v, "id")?.context("Missing MusicBrainz recording ID")?,
        release_id: None,
        release_group_id: None,
        release_track_id: None,
        scope: CatalogIdentityScope::RecordingOnly,
        recording_title: text(v, "title").context("Missing MusicBrainz recording title")?,
        track_title: None,
        recording_disambiguation: text(v, "disambiguation"),
        recording_credit: credits(v)?,
        track_credit: vec![],
        release_credit: vec![],
        isrcs: isrcs(v)?,
        recording_length_ms: v.get("length").and_then(Value::as_u64),
        track_length_ms: None,
        selected_release_date: None,
        recording_first_release_date: date(v.get("first-release-date").and_then(Value::as_str))?,
        release_group_first_release_date: None,
        release_events: vec![],
        position: CatalogPosition::default(),
        release: None,
        cover: None,
        source_policy: CatalogSourcePolicyStamp {
            adapter_revision: "musicbrainz-json-v2-1".into(),
            policy_revision: "musicbrainz-core-v1".into(),
            data_class: "core".into(),
        },
    })
}
fn legacy_date(d: Option<&TransferDate>) -> (Option<i16>, Option<chrono::NaiveDate>) {
    match d {
        Some(d) if d.month.is_none() => (i16::try_from(d.year).ok(), None),
        Some(d) if d.day.is_some() => (
            None,
            chrono::NaiveDate::from_ymd_opt(d.year, d.month.unwrap().into(), d.day.unwrap().into()),
        ),
        _ => (None, None),
    }
}
fn one(values: impl Iterator<Item = String>) -> Option<String> {
    let mut values: Vec<_> = values.collect();
    values.sort();
    values.dedup();
    (values.len() == 1).then(|| values.remove(0))
}
fn project(f: MusicBrainzFacts) -> Result<CatalogMatch> {
    f.validate()?;
    let credit = if f.track_credit.is_empty() {
        &f.recording_credit
    } else {
        &f.track_credit
    };
    let (year, date) = legacy_date(f.selected_release_date.as_ref());
    let (pyear, pdate) = legacy_date(f.recording_first_release_date.as_ref());
    let mut track = CatalogTrack {
        platform: "musicbrainz".into(),
        track_id: Some(f.recording_id.to_string()),
        release_id: f.release_id.as_ref().map(ToString::to_string),
        title: f
            .track_title
            .clone()
            .unwrap_or_else(|| f.recording_title.clone()),
        artists: credit.iter().map(|c| c.credited_name.clone()).collect(),
        album_artists: f
            .release_credit
            .iter()
            .map(|c| c.credited_name.clone())
            .collect(),
        album: f.release.as_ref().map(|r| r.title.clone()),
        isrc: (f.isrcs.len() == 1).then(|| f.isrcs[0].clone()),
        duration: Duration::from_millis(f.track_length_ms.or(f.recording_length_ms).unwrap_or(0)),
        url: format!("https://musicbrainz.org/recording/{}", f.recording_id),
        release_year: year,
        release_date: date,
        publish_year: pyear,
        publish_date: pdate,
        disc_number: f.position.medium_position,
        track_total: f
            .position
            .medium_track_count
            .and_then(|n| u16::try_from(n).ok()),
        ..Default::default()
    };
    if let Some(number) = &f.position.printed_number {
        track.track_number = Some(match number.parse::<i32>() {
            Ok(n) if n > 0 => onetagger_tagger::TrackNumber::Number(n),
            _ => onetagger_tagger::TrackNumber::Custom(number.clone()),
        });
    }
    if let Some(r) = &f.release {
        track.label = one(r.labels.iter().filter_map(|l| l.name.clone()));
        track.catalog_number = one(r.labels.iter().filter_map(|l| l.catalog_number.clone()));
    }
    if let Some(c) = &f.cover {
        track.art = Some(format!(
            "https://coverartarchive.org/release/{}/front-500",
            c.release_id
        ));
    }
    let m = CatalogMatch {
        facts: Some(CatalogFacts::MusicBrainz(Box::new(f))),
        ..CatalogMatch::new(1., track)
    };
    m.validate()?;
    Ok(m)
}
fn with_release(
    recording: &MusicBrainzFacts,
    r: &Value,
    complete: bool,
) -> Result<Vec<CatalogMatch>> {
    let release_id = id(r, "id")?.context("Missing MusicBrainz release ID")?;
    let title = text(r, "title").context("Missing MusicBrainz release title")?;
    let release_date = date(r.get("date").and_then(Value::as_str))?;
    let labels: Vec<MusicBrainzLabel> = array(r, "label-info", 64)?
        .iter()
        .map(|l| {
            Ok(MusicBrainzLabel {
                id: id(&l["label"], "id")?,
                name: text(&l["label"], "name"),
                catalog_number: text(l, "catalog-number"),
            })
        })
        .collect::<Result<_>>()?;
    let group = &r["release-group"];
    let release_credit = credits(r)?;
    let media = array(r, "media", 100)?;
    let mut out = Vec::new();
    for medium in media {
        let medium_position = medium
            .get("position")
            .and_then(Value::as_u64)
            .and_then(|n| u16::try_from(n).ok());
        let tracks = if medium.get("tracks").is_some() {
            array(medium, "tracks", 1000)?
        } else {
            array(medium, "track", 1000)?
        };
        for (index, t) in tracks.iter().enumerate() {
            if let Some(id) = id(&t["recording"], "id")?
                && id != recording.recording_id
            {
                continue;
            }
            let Some(track_id) = id(t, "id")? else {
                continue;
            };
            let position = t
                .get("position")
                .and_then(Value::as_u64)
                .and_then(|n| u32::try_from(n).ok())
                .or_else(|| {
                    medium
                        .get("track-offset")
                        .and_then(Value::as_u64)
                        .and_then(|offset| offset.checked_add(index as u64 + 1))
                        .and_then(|n| u32::try_from(n).ok())
                })
                .or_else(|| complete.then_some(index as u32 + 1));
            if medium_position.is_none() || position.is_none() {
                continue;
            }
            let mut f = recording.clone();
            f.scope = CatalogIdentityScope::ReleaseTrack;
            f.release_id = Some(release_id.clone());
            f.release_group_id = id(group, "id")?;
            f.release_track_id = Some(track_id);
            f.track_title = text(t, "title");
            f.track_credit = credits(t)?;
            f.release_credit = release_credit.clone();
            f.track_length_ms = t.get("length").and_then(Value::as_u64);
            f.selected_release_date = release_date.clone();
            f.release_group_first_release_date =
                date(group.get("first-release-date").and_then(Value::as_str))?;
            f.position = CatalogPosition {
                medium_position,
                track_position: position,
                printed_number: text(t, "number"),
                medium_track_count: medium
                    .get("track-count")
                    .and_then(Value::as_u64)
                    .and_then(|n| u32::try_from(n).ok()),
                medium_count: complete.then(|| u16::try_from(media.len()).ok()).flatten(),
            };
            f.release = Some(MusicBrainzRelease {
                title: title.clone(),
                status: text(r, "status"),
                country: text(r, "country"),
                barcode: text(r, "barcode"),
                media_format: text(medium, "format"),
                labels: labels.clone(),
            });
            f.cover = Some(MusicBrainzCoverRef {
                release_id: release_id.clone(),
                image_id: None,
            });
            f.release_events = array(r, "release-events", 64)?
                .iter()
                .map(|e| {
                    Ok(MusicBrainzReleaseEvent {
                        date: date(e.get("date").and_then(Value::as_str))?,
                        area_id: id(&e["area"], "id")?,
                        country: array(&e["area"], "iso-3166-1-codes", 8)?
                            .first()
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                    })
                })
                .collect::<Result<_>>()?;
            out.push(project(f)?);
            if out.len() >= 11 {
                return Ok(out);
            }
        }
    }
    Ok(out)
}
pub(super) fn recordings(value: Value) -> Result<Vec<CatalogMatch>> {
    let rows = if value.get("recordings").is_some() {
        array(&value, "recordings", 100)?
    } else if value.get("id").is_some() {
        std::slice::from_ref(&value)
    } else {
        anyhow::bail!("Malformed MusicBrainz recording response")
    };
    let mut out = Vec::new();
    for r in rows {
        let f = base(r)?;
        let mut releases = Vec::new();
        for release in array(r, "releases", 100)? {
            releases.extend(with_release(&f, release, false)?);
            if releases.len() >= 11 {
                break;
            }
        }
        if releases.is_empty() {
            out.push(project(f)?)
        } else {
            out.extend(releases)
        }
        if out.len() >= 11 {
            out.truncate(11);
            break;
        }
    }
    Ok(out)
}
pub(super) fn release(value: Value, selected: &CatalogMatch) -> Result<CatalogMatch> {
    let f = selected
        .musicbrainz()
        .context("Selected candidate lacks MusicBrainz facts")?;
    ensure!(
        id(&value, "id")?.as_ref() == f.release_id.as_ref(),
        "MusicBrainz returned another release"
    );
    let mut candidates = with_release(f, &value, true)?;
    let at = candidates
        .iter()
        .position(|m| {
            m.musicbrainz().is_some_and(|n| {
                n.release_track_id == f.release_track_id
                    && n.position.medium_position == f.position.medium_position
                    && n.position.track_position == f.position.track_position
            })
        })
        .context("Selected recording/track is absent from release")?;
    let m = candidates.remove(at);
    m.validate_extension_from(selected)?;
    Ok(m)
}
pub(super) fn release_candidates(
    value: Value,
    recording: Option<&Mbid>,
    release_track: Option<&Mbid>,
) -> Result<Vec<CatalogMatch>> {
    let rows = if value.get("releases").is_some() {
        array(&value, "releases", 100)?
    } else {
        std::slice::from_ref(&value)
    };
    let mut out = Vec::new();
    for release in rows {
        for medium in array(release, "media", 100)? {
            for track in array(medium, "tracks", 1000)? {
                let r = &track["recording"];
                let Some(id) = id(r, "id")? else { continue };
                if recording.is_some_and(|expected| expected != &id)
                    || release_track.is_some_and(|expected| {
                        track.get("id").and_then(Value::as_str) != Some(expected.as_str())
                    })
                {
                    continue;
                }
                let facts = base(r)?;
                for candidate in with_release(&facts, release, true)? {
                    if release_track.is_none_or(|id| {
                        candidate
                            .musicbrainz()
                            .and_then(|f| f.release_track_id.as_ref())
                            == Some(id)
                    }) && !out
                        .iter()
                        .any(|m: &CatalogMatch| m.entity_key() == candidate.entity_key())
                    {
                        out.push(candidate)
                    }
                    if out.len() >= 11 {
                        return Ok(out);
                    }
                }
            }
        }
    }
    Ok(out)
}
pub(super) fn releases_for_recording(
    value: Value,
    selected: &CatalogMatch,
) -> Result<Vec<CatalogMatch>> {
    let facts = selected
        .musicbrainz()
        .context("Missing MusicBrainz recording facts")?;
    let mut out = Vec::new();
    for release in array(&value, "releases", 100)? {
        out.extend(with_release(facts, release, true)?);
        if out.len() >= 11 {
            out.truncate(11);
            break;
        }
    }
    Ok(out)
}
