use crate::http::{CatalogHttp, MemoryChallengeStore, SolverConfig};
use anyhow::Error;
use chrono::NaiveDate;
use onetagger_tagger::{
    supported_tags, Album, AudioFileInfo, AutotaggerSource, AutotaggerSourceBuilder, MatchingUtils,
    PlatformInfo, SupportedTag, TaggerConfig, Track, TrackMatch, TrackNumber,
};
use reqwest::blocking::Client;
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::{atomic::AtomicBool, Arc};

pub struct Traxsource {
    client: Client,
    web: String,
    http: CatalogHttp,
    solver: SolverConfig,
    cancel: Arc<AtomicBool>,
}

impl Traxsource {
    // Create new instance
    pub fn new() -> Traxsource {
        let client = Client::builder()
            .user_agent(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:85.0) Gecko/20100101 Firefox/85.0",
            )
            .build()
            .unwrap();
        Traxsource {
            http: CatalogHttp::new(
                client.clone(),
                "traxsource",
                Arc::new(MemoryChallengeStore::default()),
            ),
            client,
            web: "https://www.traxsource.com".into(),
            solver: SolverConfig::default(),
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn with_endpoints(client: Client, web: &str) -> Self {
        Self {
            http: CatalogHttp::new(
                client.clone(),
                "traxsource",
                Arc::new(MemoryChallengeStore::default()),
            ),
            client,
            web: web.trim_end_matches('/').into(),
            solver: SolverConfig::default(),
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn with_http(
        mut self,
        http: CatalogHttp,
        solver: SolverConfig,
        cancel: Arc<AtomicBool>,
    ) -> Self {
        self.http = http;
        self.solver = solver;
        self.cancel = cancel;
        self
    }

    pub fn search_tracks(&self, query: &str) -> Result<Vec<Track>, Error> {
        let mut url = reqwest::Url::parse(&format!("{}/search/tracks", self.web))?;
        url.query_pairs_mut().append_pair("term", query);
        let data = self
            .http
            .get_text(url.as_str(), &self.solver, &self.cancel)?;
        let document = page(&data)?;
        let list = document.select(&selector("div#searchTrackList")).next();
        let Some(list) = list else {
            if empty_search(&document) {
                return Ok(vec![]);
            }
            return Err(anyhow!("Malformed Traxsource page: missing search results"));
        };
        list.select(&selector("div.trk-row"))
            .map(|row| parse_track(row, &self.web, true))
            .collect()
    }

    /// Fetch one explicit result page; callers decide whether to request more pages.
    pub fn search_releases(
        &self,
        query: &str,
        page_number: u32,
    ) -> Result<Vec<TraxsourceRelease>, Error> {
        if page_number == 0 {
            return Err(anyhow!("Traxsource pages start at 1"));
        }
        let mut url = reqwest::Url::parse(&format!("{}/scripts/search.php/titles", self.web))?;
        url.query_pairs_mut()
            .append_pair("term", query)
            .append_pair("page", &page_number.to_string())
            .append_pair("rpc", "1");
        let data = self
            .http
            .get_text(url.as_str(), &self.solver, &self.cancel)?;
        let document = page(&data)?;
        let container = document.select(&selector("#searchResCont")).next();
        let Some(container) = container else {
            if empty_search(&document) {
                return Ok(vec![]);
            }
            return Err(anyhow!(
                "Malformed Traxsource page: missing release search results"
            ));
        };
        let mut found = Vec::new();
        for card in container.select(&selector(".grid-item.play-ttl")) {
            let title = card
                .select(&selector("a.com-title"))
                .next()
                .ok_or_else(|| anyhow!("Missing release title"))?;
            let href = title
                .attr("href")
                .ok_or_else(|| anyhow!("Missing release link"))?;
            found.push(TraxsourceRelease {
                id: catalog_id(href, "title")?,
                title: nonempty(text(title), "release title")?,
                url: format!("{}{}", self.web, href),
                label: optional_text(card, "a.com-label"),
                thumbnail: card
                    .select(&selector(".grid-image img"))
                    .next()
                    .and_then(|e| e.attr("src"))
                    .map(str::to_owned),
            });
        }
        if found.is_empty() && !empty_search(&document) {
            return Err(anyhow!(
                "Malformed Traxsource page: missing release results"
            ));
        }
        Ok(found)
    }

    pub fn get_release(&self, id: &str) -> Result<Album, Error> {
        numeric_id(id)?;
        let data = self.http.get_text(
            &format!("{}/title/{id}/", self.web),
            &self.solver,
            &self.cancel,
        )?;
        self.parse_release(id, &data)
    }

    fn parse_release(&self, id: &str, data: &str) -> Result<Album, Error> {
        let document = page(data)?;
        if let Some(actual) = document
            .select(&selector("#ttlPageCont[data-tid]"))
            .next()
            .and_then(|e| e.attr("data-tid"))
        {
            if actual != id {
                return Err(anyhow!("Traxsource returned a different release"));
            }
        }
        let release = document
            .select(&selector(".ttl-cont"))
            .next()
            .ok_or_else(|| anyhow!("Malformed Traxsource page: missing release"))?;
        let name = required_text(release, "h1.title")?;
        let artists_element = release
            .select(&selector("h1.artists"))
            .next()
            .ok_or_else(|| anyhow!("Missing release artists"))?;
        let mut artists: Vec<_> = artists_element
            .select(&selector("a"))
            .map(text)
            .filter(|v| !v.is_empty())
            .collect();
        if artists.is_empty() {
            artists = text(artists_element)
                .split(',')
                .map(|v| v.trim().to_owned())
                .filter(|v| !v.is_empty())
                .collect();
        }
        let cat_date = required_text(release, ".cat-rdate")?;
        let parts: Vec<_> = cat_date.split('|').map(str::trim).collect();
        let date_text = parts
            .last()
            .ok_or_else(|| anyhow!("Missing release date"))?
            .replace("Pre-order for ", "");
        let date = NaiveDate::parse_from_str(date_text.trim(), "%Y-%m-%d")
            .map_err(|_| anyhow!("Invalid Traxsource release date"))?;
        let catalog = (parts.len() > 1)
            .then(|| parts[0].to_owned())
            .filter(|v| !v.is_empty());
        let label = optional_text(release, "a.com-label");
        let art = release
            .select(&selector(".t-image img"))
            .next()
            .and_then(|e| e.attr("src"))
            .map(str::to_owned);
        let list = release
            .select(&selector("#trkListCont .trklist"))
            .next()
            .ok_or_else(|| anyhow!("Malformed Traxsource page: missing release tracklist"))?;
        let mut tracks = Vec::new();
        let mut ids = HashSet::new();
        for row in list.select(&selector(".trk-row.play-trk")) {
            let mut track = parse_track(row, &self.web, false)?;
            if !ids.insert(track.track_id.clone()) {
                return Err(anyhow!("Duplicate Traxsource release track"));
            }
            let number: i32 = required_text(row, ".tnum")?.parse()?;
            if number != tracks.len() as i32 + 1 {
                return Err(anyhow!("Incomplete or unordered Traxsource tracklist"));
            }
            track.track_number = Some(TrackNumber::Number(number));
            track.album = Some(name.clone());
            track.album_artists = artists.clone();
            track.release_id = Some(id.to_owned());
            track.catalog_number = catalog.clone();
            track.release_date = Some(date);
            track.label = label.clone();
            track.art = art.clone();
            tracks.push(track);
        }
        if tracks.is_empty() {
            return Err(anyhow!("Malformed Traxsource release: no tracks"));
        }
        if let Some(expected) = release
            .select(&selector("a.com-play-all[data-tracks]"))
            .next()
            .and_then(|element| element.attr("data-tracks"))
        {
            let advertised: Vec<_> = expected.split(',').map(str::trim).collect();
            let actual: Vec<_> = tracks
                .iter()
                .filter_map(|track| track.track_id.as_deref())
                .collect();
            if advertised != actual {
                return Err(anyhow!("Incomplete Traxsource release tracklist"));
            }
        }
        let count = u16::try_from(tracks.len())?;
        for track in &mut tracks {
            track.track_total = Some(count);
        }
        Ok(Album {
            id: id.to_owned(),
            name,
            tracks,
        })
    }

    // Preserve the exact release identity linked from the matched track.
    pub fn extend_track_traxsource(
        &self,
        track: &mut Track,
        album_meta: bool,
        album_art: bool,
    ) -> Result<(), Error> {
        let data = self.http.get_text(&track.url, &self.solver, &self.cancel)?;
        let document = page(&data)?;
        let link = document
            .select(&selector("div.ttl-info.ellip a[href^='/title/']"))
            .next()
            .ok_or_else(|| anyhow!("Missing Traxsource track release link"))?;
        let href = link
            .attr("href")
            .ok_or_else(|| anyhow!("Missing release link"))?;
        let id = catalog_id(href, "title")?;
        track.album = Some(nonempty(text(link), "release title")?);
        track.release_id = Some(id.clone());
        if !album_meta {
            return Ok(());
        }
        let album = self.get_release(&id)?;
        let found = album
            .tracks
            .iter()
            .find(|candidate| candidate.track_id == track.track_id)
            .ok_or_else(|| anyhow!("Matched track is absent from its Traxsource release"))?;
        track.album = found.album.clone();
        track.catalog_number = found.catalog_number.clone();
        track.album_artists = found.album_artists.clone();
        track.track_number = found.track_number.clone();
        track.track_total = found.track_total;
        track.art = found.art.clone();
        if album_art {
            if let Some(art) = track.art.as_deref() {
                self.client
                    .get(art)
                    .header("Referer", "https://www.traxsource.com/")
                    .send()
                    .ok();
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraxsourceRelease {
    pub id: String,
    pub title: String,
    pub url: String,
    pub label: Option<String>,
    pub thumbnail: Option<String>,
}

fn selector(css: &str) -> Selector {
    Selector::parse(css).expect("static selector")
}
fn text(element: ElementRef<'_>) -> String {
    element.text().collect::<String>().trim().to_owned()
}
fn optional_text(root: ElementRef<'_>, css: &str) -> Option<String> {
    root.select(&selector(css))
        .next()
        .map(text)
        .filter(|v| !v.is_empty())
}
fn nonempty(value: String, field: &str) -> Result<String, Error> {
    if value.is_empty() {
        Err(anyhow!("Missing Traxsource {field}"))
    } else {
        Ok(value)
    }
}
fn required_text(root: ElementRef<'_>, css: &str) -> Result<String, Error> {
    optional_text(root, css).ok_or_else(|| anyhow!("Missing Traxsource element: {css}"))
}
fn numeric_id(id: &str) -> Result<(), Error> {
    if id.is_empty() || !id.bytes().all(|v| v.is_ascii_digit()) {
        return Err(anyhow!("Invalid Traxsource catalog ID"));
    }
    Ok(())
}
fn catalog_id(href: &str, kind: &str) -> Result<String, Error> {
    let suffix = href
        .strip_prefix(&format!("/{kind}/"))
        .ok_or_else(|| anyhow!("Invalid Traxsource catalog link"))?;
    let id = suffix.split('/').next().unwrap_or_default();
    numeric_id(id)?;
    Ok(id.to_owned())
}
fn page(data: &str) -> Result<Html, Error> {
    let body = if data.trim_start().starts_with("<?xml") || data.trim_start().starts_with("<root>")
    {
        let xml = roxmltree::Document::parse(data)
            .map_err(|_| anyhow!("Malformed Traxsource RPC response"))?;
        xml.root_element()
            .children()
            .find(|n| n.has_tag_name("data"))
            .and_then(|n| n.text())
            .ok_or_else(|| anyhow!("Missing Traxsource RPC data"))?
            .to_owned()
    } else {
        data.to_owned()
    };
    Ok(Html::parse_document(&body))
}
fn empty_search(document: &Html) -> bool {
    document
        .select(&selector("#bodyBay .search-res-cont h2, #searchResCont h2"))
        .any(|e| text(e).starts_with("No results found for "))
}
fn parse_track(row: ElementRef<'_>, web: &str, search: bool) -> Result<Track, Error> {
    let title_element = row
        .select(&selector("div.title"))
        .next()
        .ok_or_else(|| anyhow!("Missing Traxsource track title"))?;
    let link = title_element
        .select(&selector("a[href^='/track/']"))
        .next()
        .ok_or_else(|| anyhow!("Missing Traxsource track link"))?;
    let href = link
        .attr("href")
        .ok_or_else(|| anyhow!("Missing Traxsource track URL"))?;
    let duration_text = optional_text(title_element, ".duration")
        .or_else(|| {
            title_element
                .text()
                .map(str::trim)
                .filter(|v| v.contains(':'))
                .last()
                .map(str::to_owned)
        })
        .ok_or_else(|| anyhow!("Missing Traxsource duration"))?;
    let duration = MatchingUtils::parse_duration(&duration_text)?;
    let version = title_element
        .select(&selector(".version"))
        .next()
        .map(|e| {
            e.text()
                .collect::<String>()
                .replace(&duration_text, "")
                .trim()
                .to_owned()
        })
        .filter(|v| !v.is_empty());
    let artists: Vec<_> = row
        .select(&selector("div.artists a"))
        .map(text)
        .filter(|v| !v.is_empty())
        .collect();
    if artists.is_empty() {
        return Err(anyhow!("Missing Traxsource track artists"));
    }
    let label = optional_text(row, "div.label");
    let release_date = optional_text(row, "div.r-date")
        .map(|v| NaiveDate::parse_from_str(v.replace("Pre-order for ", "").trim(), "%Y-%m-%d"))
        .transpose()?;
    let key_bpm: Vec<_> = row
        .select(&selector("div.key-bpm"))
        .next()
        .map(|e| {
            e.text()
                .map(|v| v.trim().to_owned())
                .filter(|v| !v.is_empty())
                .collect()
        })
        .unwrap_or_default();
    if search
        && (label.is_none()
            || release_date.is_none()
            || row.select(&selector("div.key-bpm")).next().is_none())
    {
        return Err(anyhow!("Malformed Traxsource search track metadata"));
    }
    let (key, bpm) = if key_bpm.len() == 2 {
        (
            Some(key_bpm[0].replace("maj", "").replace("min", "m")),
            key_bpm[1].parse().ok(),
        )
    } else {
        (None, None)
    };
    Ok(Track {
        platform: "traxsource".into(),
        title: nonempty(text(link), "track title")?,
        version,
        artists,
        key,
        bpm,
        track_id: Some(catalog_id(href, "track")?),
        url: format!("{web}{href}"),
        duration,
        label,
        release_date,
        genres: optional_text(row, "div.genre").into_iter().collect(),
        thumbnail: row
            .select(&selector("div.thumb img"))
            .next()
            .and_then(|e| e.attr("src"))
            .map(str::to_owned),
        ..Default::default()
    })
}

impl AutotaggerSource for Traxsource {
    fn get_album(&mut self, id: &str, _config: &TaggerConfig) -> Result<Option<Album>, Error> {
        self.get_release(id).map(Some)
    }
    fn match_track(
        &mut self,
        info: &AudioFileInfo,
        config: &TaggerConfig,
    ) -> Result<Vec<TrackMatch>, Error> {
        // Search
        let query = format!(
            "{} {}",
            info.artist()?,
            MatchingUtils::clean_title(info.title()?)
        );
        let tracks = self.search_tracks(&query)?;
        Ok(MatchingUtils::match_track(&info, &tracks, &config, true))
    }

    fn extend_track(&mut self, track: &mut Track, config: &TaggerConfig) -> Result<(), Error> {
        Self::extend_track_traxsource(
            &self,
            track,
            config.any_tag_enabled(&supported_tags!(
                CatalogNumber,
                TrackNumber,
                AlbumArt,
                TrackTotal,
                AlbumArtist
            )),
            config.tag_enabled(SupportedTag::AlbumArt),
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct TraxsourceBuilder;

impl AutotaggerSourceBuilder for TraxsourceBuilder {
    fn new() -> TraxsourceBuilder {
        TraxsourceBuilder
    }

    fn get_source(&mut self, _config: &TaggerConfig) -> Result<Box<dyn AutotaggerSource>, Error> {
        Ok(Box::new(Traxsource::new()))
    }

    fn info(&self) -> PlatformInfo {
        PlatformInfo {
            id: "traxsource".to_string(),
            name: "Traxsource".to_string(),
            description: "Overall more specialized in House".to_string(),
            icon: b"",
            max_threads: 1,
            version: "1.0.0".to_string(),
            custom_options: Default::default(),
            requires_auth: false,
            supported_tags: supported_tags!(
                Version,
                Artist,
                BPM,
                Key,
                Title,
                URL,
                Label,
                ReleaseDate,
                Genre,
                TrackId,
                Duration,
                Album,
                ReleaseId,
                CatalogNumber,
                AlbumArtist,
                TrackNumber,
                TrackTotal,
                AlbumArt
            ),
        }
    }
}
