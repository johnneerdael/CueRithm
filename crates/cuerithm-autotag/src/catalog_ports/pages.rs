//! Bounded Bandcamp parsing adapted from OneTagger (GPL-3.0-only).
use crate::CatalogTrack;
use anyhow::{Context, Result, ensure};
use chrono::NaiveDate;
use scraper::{Html, Selector};
use serde_json::Value;

fn selector(value: &str) -> Selector {
    Selector::parse(value).expect("static catalog selector")
}
pub(super) fn bandcamp_detail(body: &str, original: &CatalogTrack) -> Result<CatalogTrack> {
    let document = Html::parse_document(body);
    let scripts = selector("script[type='application/ld+json']");
    let mut found = None;
    for script in document.select(&scripts).take(20) {
        let value: Value = serde_json::from_str(&script.text().collect::<String>())?;
        let mut nodes = vec![value];
        let mut visited = 0;
        while let Some(v) = nodes.pop() {
            visited += 1;
            ensure!(
                visited <= 128,
                "Bandcamp structured metadata exceeds bounds"
            );
            if let Some(a) = v.as_array() {
                ensure!(a.len() <= 64, "Bandcamp graph exceeds bounds");
                nodes.extend(a.iter().cloned());
                continue;
            }
            if let Some(a) = v.get("@graph").and_then(Value::as_array) {
                ensure!(a.len() <= 64, "Bandcamp graph exceeds bounds");
                nodes.extend(a.iter().cloned());
            }
            let recording = v["@type"] == "MusicRecording"
                || v["@type"]
                    .as_array()
                    .is_some_and(|types| types.iter().any(|kind| kind == "MusicRecording"));
            if recording {
                if let Some(url) = v
                    .get("url")
                    .or_else(|| v.get("@id"))
                    .and_then(Value::as_str)
                {
                    let expected = reqwest::Url::parse(&original.url)?;
                    let actual = reqwest::Url::parse(url)?;
                    if expected.host_str() != actual.host_str() || expected.path() != actual.path()
                    {
                        continue;
                    }
                }
                found = Some(v);
                break;
            }
        }
        if found.is_some() {
            break;
        }
    }
    let v = found.context("Bandcamp recording metadata missing; layout may have changed")?;
    let mut t = original.clone();
    let title = v["name"]
        .as_str()
        .context("Bandcamp recording title missing")?;
    let credits = if let Some(artists) = v["byArtist"].as_array() {
        ensure!(artists.len() <= 64, "Bandcamp artist list exceeds bounds");
        artists
            .iter()
            .filter_map(|artist| artist["name"].as_str().map(str::to_owned))
            .collect::<Vec<_>>()
    } else {
        v["byArtist"]["name"]
            .as_str()
            .map(str::to_owned)
            .into_iter()
            .collect()
    };
    ensure!(!credits.is_empty(), "Bandcamp recording artist missing");
    // A label uploader is not silently replaced by a title-split guess (#334).
    t.title = title.into();
    t.artists = credits;
    t.album = v["inAlbum"]["name"].as_str().map(str::to_owned).or(t.album);
    t.art = v["image"].as_str().map(str::to_owned);
    t.release_date = v["datePublished"].as_str().and_then(|s| {
        s.get(..10)
            .and_then(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").ok())
            .or_else(|| {
                let date = s.split_whitespace().take(3).collect::<Vec<_>>().join(" ");
                NaiveDate::parse_from_str(&date, "%d %b %Y").ok()
            })
    });
    if let Some(keywords) = v["keywords"].as_array() {
        t.genres = keywords
            .iter()
            .take(64)
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
    }
    if let Some(s) = v["duration"].as_str() {
        let re = regex::Regex::new(r"^PT(?:(\d+)H)?(?:(\d+)M)?(?:(\d+(?:\.\d+)?)S)?$")?;
        if let Some(c) = re.captures(s) {
            let unit = |i| {
                c.get(i)
                    .and_then(|m| m.as_str().parse::<f64>().ok())
                    .unwrap_or(0.)
            };
            let secs = unit(1) * 3600. + unit(2) * 60. + unit(3);
            if secs.is_finite() && (0.0..=86_400.).contains(&secs) {
                t.duration = std::time::Duration::from_secs_f64(secs);
            }
        }
    }
    Ok(t)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bandcamp_artist_is_not_guessed_from_title() {
        let track = CatalogTrack {
            title: "Artist - Track".into(),
            artists: vec!["Label".into()],
            ..Default::default()
        };
        let html = r#"<script type="application/ld+json">{"@type":"MusicRecording","name":"Artist - Track","byArtist":{"name":"Credited Artist"},"duration":"PT4M30S"}</script>"#;
        let t = bandcamp_detail(html, &track).unwrap();
        assert_eq!(t.title, "Artist - Track");
        assert_eq!(t.artists, ["Credited Artist"]);
        assert_eq!(t.duration, std::time::Duration::from_secs(270));
    }
}
