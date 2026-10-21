use crate::{
    CatalogFacts, CatalogMatch, CatalogTrack, SourceCatalogFacts, SourceCatalogIdentity,
    SourceCatalogIdentityKind, TransferDate,
};
use anyhow::{Context, Result, ensure};
use scraper::{Html, Selector};
use serde_json::Value;
use std::time::Duration;

pub(super) fn parse(
    body: &str,
    expected_id: u64,
    expected_isrc: Option<&str>,
) -> Result<CatalogMatch> {
    ensure!(
        body.len() <= crate::MAX_CATALOG_JSON_BYTES,
        "Volumo page exceeds metadata bound"
    );
    let document = Html::parse_document(body);
    let selector = Selector::parse("script#__NEXT_DATA__").expect("static Volumo selector");
    let scripts = document.select(&selector).take(2).collect::<Vec<_>>();
    ensure!(
        scripts.len() == 1,
        "Volumo public metadata is missing or ambiguous; layout may have changed"
    );
    let value: Value = serde_json::from_str(&scripts[0].text().collect::<String>())
        .context("Malformed Volumo public metadata")?;
    ensure!(
        value["page"] == "/track/[trackId]",
        "Volumo returned another page type"
    );
    let route = value["query"]["trackId"]
        .as_str()
        .context("Missing Volumo page identity")?;
    ensure!(
        numeric_id(route.split('-').next().unwrap_or(""))? == expected_id,
        "Volumo page belongs to another track"
    );
    let queries = value["props"]["pageProps"]["dehydratedState"]["queries"]
        .as_array()
        .context("Volumo public query state missing")?;
    ensure!(
        queries.len() <= 128,
        "Volumo page query state exceeds bound"
    );
    let albums = queries
        .iter()
        .filter(|query| {
            scope(query, "Album")
                && query["queryKey"].as_array().is_some_and(|keys| {
                    keys.iter()
                        .any(|key| key["trackId"].as_u64() == Some(expected_id))
                })
        })
        .collect::<Vec<_>>();
    ensure!(
        albums.len() == 1,
        "Volumo selected album metadata is missing or ambiguous"
    );
    let album = &albums[0]["state"]["data"];
    let album_id = id(album, "id")?;
    let tracks = album["tracks"]
        .as_array()
        .context("Volumo album track list missing")?;
    ensure!(tracks.len() <= 1000, "Volumo track list exceeds bound");
    let selected = tracks
        .iter()
        .enumerate()
        .filter(|(_, track)| track["id"].as_u64() == Some(expected_id))
        .collect::<Vec<_>>();
    ensure!(
        selected.len() == 1,
        "Volumo selected recording is missing or ambiguous"
    );
    let (index, track) = selected[0];
    ensure!(
        id(&track["album"], "id")? == album_id,
        "Volumo recording changed album identity"
    );
    let title = text(track, "title").context("Volumo recording title missing")?;
    let mut artists = names(track, "artists")?;
    ensure!(!artists.is_empty(), "Volumo recording credits missing");
    let recording = recording(&document, expected_id)?;
    let (json_id, url) = track_url(
        recording["url"]
            .as_str()
            .or_else(|| recording["@id"].as_str())
            .context("Volumo recording URL missing")?,
    )?;
    ensure!(
        json_id == expected_id,
        "Volumo structured recording changed identity"
    );
    if let Some(url) = recording["@id"].as_str() {
        ensure!(
            track_url(url)?.0 == expected_id,
            "Volumo structured recording changed identity"
        );
    }
    if let Some(name) = text(&recording["recordingOf"], "name") {
        ensure!(name == title, "Volumo structured recording changed title");
    }
    let isrc = text(track, "isrc")
        .map(|isrc| super::normalize_isrc(&isrc))
        .transpose()?;
    if let Some(expected) = expected_isrc {
        ensure!(
            isrc.as_deref() == Some(super::normalize_isrc(expected)?.as_str()),
            "Volumo public recording does not prove the requested ISRC"
        );
    }
    if let Some(json_isrc) = text(&recording, "isrcCode") {
        ensure!(
            isrc.as_deref() == Some(super::normalize_isrc(&json_isrc)?.as_str()),
            "Volumo structured ISRC changed recording"
        );
    }
    ensure!(
        track["available"].as_bool() != Some(false),
        "Volumo track is not available"
    );
    for artist in names(track, "featured_artists")? {
        if !artists.contains(&artist) {
            artists.push(artist);
        }
    }
    // The explicit release date is authoritative. UTC release_start_at and
    // schema.org dateCreated can fall on the preceding calendar day.
    let release_date = text(album, "original_release_date")
        .map(|date| date_value(&date))
        .transpose()?;
    let genres = genres(queries, track, &recording)?;
    let duration = match track.get("duration").filter(|value| !value.is_null()) {
        Some(value) => {
            let milliseconds = value
                .as_f64()
                .filter(|milliseconds| {
                    milliseconds.is_finite() && (0.0..=86_400_000.).contains(milliseconds)
                })
                .context("Invalid Volumo recording duration")?;
            Duration::from_secs_f64(milliseconds / 1000.)
        }
        None => Duration::ZERO,
    };
    let total = album["tracks_total"]
        .as_u64()
        .and_then(|total| u16::try_from(total).ok())
        .filter(|total| *total > 0);
    let position = total
        .filter(|total| usize::from(*total) == tracks.len())
        .map(|_| index + 1);
    let native = CatalogTrack {
        platform: "volumo".into(),
        title,
        version: text(track, "version"),
        artists,
        remixers: names(track, "remixers")?,
        album: text(album, "title"),
        label: text(&track["recordlabel"], "name").or_else(|| text(&album["recordlabel"], "name")),
        catalog_number: text(album, "catalog_number"),
        track_id: Some(expected_id.to_string()),
        release_id: Some(album_id.to_string()),
        isrc,
        duration,
        genres,
        bpm: track["bpm"].as_i64().filter(|bpm| (1..=1000).contains(bpm)),
        key: text(track, "keysign"),
        url,
        art: text(&recording, "image")
            .map(|image| image_url(&image))
            .transpose()?,
        track_number: position.map(|position| (position as i32).into()),
        track_total: total,
        ..Default::default()
    };
    let mut matched = CatalogMatch::new_id(native);
    matched.facts = Some(CatalogFacts::Source(SourceCatalogFacts {
        schema_version: 1,
        provider: "volumo".into(),
        release_id: Some(album_id.to_string()),
        release_date,
        raw_position: position.map(|position| position.to_string()),
        identities: vec![SourceCatalogIdentity {
            source: "volumo".into(),
            kind: SourceCatalogIdentityKind::PlatformTrack,
            id: expected_id.to_string(),
        }],
        links: vec![],
    }));
    matched.validate()?;
    Ok(matched)
}
pub(super) fn numeric_id(value: &str) -> Result<u64> {
    ensure!(
        !value.is_empty() && value.len() <= 20 && value.bytes().all(|byte| byte.is_ascii_digit()),
        "Invalid Volumo track identity"
    );
    let id = value.parse::<u64>()?;
    ensure!(id > 0, "Invalid Volumo track identity");
    Ok(id)
}
pub(super) fn track_url(value: &str) -> Result<(u64, String)> {
    ensure!(
        value.len() <= 4096 && !value.chars().any(char::is_control),
        "Invalid Volumo recording URL"
    );
    let mut url =
        reqwest::Url::parse(value).map_err(|_| anyhow::anyhow!("Invalid Volumo recording URL"))?;
    ensure!(
        url.scheme() == "https"
            && url.host_str() == Some("volumo.com")
            && url.port_or_known_default() == Some(443)
            && url.username().is_empty()
            && url.password().is_none(),
        "Volumo recording URL origin is not approved"
    );
    let parts = url
        .path_segments()
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    ensure!(
        parts.len() == 2 && parts[0] == "track" && !url.path().contains('%'),
        "Volumo URL does not identify a recording"
    );
    let id = numeric_id(parts[1].split('-').next().unwrap_or(""))?;
    // Sharing/tracking parameters do not establish recording identity and must
    // never be sent onward to public metadata fetches or retained in evidence.
    url.set_query(None);
    url.set_fragment(None);
    Ok((id, url.to_string()))
}
fn scope(query: &Value, name: &str) -> bool {
    query["queryKey"]
        .as_array()
        .is_some_and(|keys| keys.iter().any(|key| key["scope"] == name))
}
fn id(value: &Value, key: &str) -> Result<u64> {
    value[key]
        .as_u64()
        .filter(|id| *id > 0)
        .context("Volumo metadata identity missing")
}
fn text(value: &Value, key: &str) -> Option<String> {
    value[key]
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty() && text.len() <= 4096)
        .map(str::to_owned)
}
fn names(value: &Value, key: &str) -> Result<Vec<String>> {
    let Some(rows) = value[key].as_array() else {
        return Ok(vec![]);
    };
    ensure!(rows.len() <= 64, "Volumo credits exceed bound");
    Ok(rows.iter().filter_map(|row| text(row, "name")).collect())
}
fn date_value(value: &str) -> Result<TransferDate> {
    let parts = value.split('-').collect::<Vec<_>>();
    ensure!(
        (1..=3).contains(&parts.len()),
        "Invalid Volumo release date"
    );
    let date = TransferDate {
        year: parts[0].parse()?,
        month: parts.get(1).map(|part| part.parse()).transpose()?,
        day: parts.get(2).map(|part| part.parse()).transpose()?,
    };
    crate::validate_date(&date)?;
    Ok(date)
}
fn image_url(value: &str) -> Result<String> {
    let url = reqwest::Url::parse("https://volumo.com")?.join(value)?;
    ensure!(
        url.scheme() == "https"
            && url.host_str() == Some("volumo.com")
            && url.port_or_known_default() == Some(443)
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
            && url.path().starts_with("/img/size/"),
        "Volumo artwork origin is not approved"
    );
    Ok(url.to_string())
}
fn genres(queries: &[Value], track: &Value, recording: &Value) -> Result<Vec<String>> {
    let mut genres = Vec::new();
    let genre = track["genre_id"]
        .as_u64()
        .or_else(|| track["genre"].as_u64());
    if let (Some(a), Some(b)) = (track["genre_id"].as_u64(), track["genre"].as_u64()) {
        ensure!(a == b, "Volumo genre identities disagree");
    }
    if let Some(genre) = genre {
        for query in queries.iter().filter(|query| scope(query, "Genres")) {
            if let Some(rows) = query["state"]["data"].as_array() {
                ensure!(rows.len() <= 1000, "Volumo genre list exceeds bound");
                for row in rows.iter().filter(|row| row["id"].as_u64() == Some(genre)) {
                    if let Some(name) = text(row, "name")
                        && !genres.contains(&name)
                    {
                        genres.push(name);
                    }
                }
            }
        }
    }
    if genres.is_empty() {
        genres = text(recording, "genre").into_iter().collect();
    }
    Ok(genres)
}
fn recording(document: &Html, expected_id: u64) -> Result<Value> {
    let selector =
        Selector::parse("script[type='application/ld+json']").expect("static Volumo selector");
    let mut found = None;
    for script in document.select(&selector).take(20) {
        let value: Value = serde_json::from_str(&script.text().collect::<String>())
            .context("Malformed Volumo recording metadata")?;
        let mut pending = vec![value];
        let mut visited = 0;
        while let Some(value) = pending.pop() {
            visited += 1;
            ensure!(visited <= 128, "Volumo structured metadata exceeds bound");
            if let Some(rows) = value.as_array() {
                ensure!(rows.len() <= 64, "Volumo structured metadata exceeds bound");
                pending.extend(rows.iter().cloned());
                continue;
            }
            if let Some(rows) = value["@graph"].as_array() {
                ensure!(rows.len() <= 64, "Volumo structured metadata exceeds bound");
                pending.extend(rows.iter().cloned());
            }
            let recording = value["@type"] == "MusicRecording"
                || value["@type"]
                    .as_array()
                    .is_some_and(|types| types.iter().any(|kind| kind == "MusicRecording"));
            if recording && value["volumo:trackId"].as_u64() == Some(expected_id) {
                ensure!(found.is_none(), "Volumo recording proof is ambiguous");
                found = Some(value);
            }
        }
    }
    found.context("Volumo selected structured recording is missing; layout may have changed")
}
