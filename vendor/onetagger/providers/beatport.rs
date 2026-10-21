use crate::BoundedResponse;
use anyhow::Error;
use chrono::NaiveDate;
use onetagger_tag::FrameName;
use onetagger_tagger::{
    supported_tags, Album, AudioFileInfo, AutotaggerSource, AutotaggerSourceBuilder, MatchingUtils,
    PlatformCustomOptionValue, PlatformCustomOptions, PlatformInfo, SupportedTag, TaggerConfig,
    Track, TrackMatch, TrackNumber,
};
use reqwest::blocking::Client;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Condvar, Mutex,
};
use std::time::{Duration, Instant};

const INVALID_ART: &'static str = "ab2d1d04-233d-4b08-8234-9782b34dcab8";

#[derive(Debug, Default)]
struct TokenState {
    token: Option<BeatportOAuth>,
    refreshing: bool,
    failure: Option<TokenRefreshFailure>,
}
/// Share the exact refresh cause with concurrent callers and let the native
/// retry scheduler honor the same coalescing window before another request.
#[derive(Debug, Clone)]
pub struct TokenRefreshFailure {
    cause: Arc<anyhow::Error>,
    retry_after: Instant,
}
impl TokenRefreshFailure {
    pub fn retry_after(&self) -> Instant {
        self.retry_after
    }
}
impl std::fmt::Display for TokenRefreshFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Recent catalog token refresh failed: {}", self.cause)
    }
}
impl std::error::Error for TokenRefreshFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.cause.as_ref().as_ref())
    }
}
#[derive(Debug, Default)]
pub struct BeatportTokenCache {
    state: Mutex<TokenState>,
    changed: Condvar,
}
impl BeatportTokenCache {
    pub fn with_token(token: BeatportOAuth) -> Self {
        Self {
            state: Mutex::new(TokenState {
                token: Some(token),
                ..Default::default()
            }),
            ..Default::default()
        }
    }
}
pub struct Beatport {
    client: Client,
    access_token: Arc<BeatportTokenCache>,
    api: String,
    auth: String,
    cancel: Arc<AtomicBool>,
}

impl Beatport {
    /// Create new instance
    pub fn new(access_token: Arc<BeatportTokenCache>) -> Beatport {
        let client = Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36")
            .timeout(Duration::from_secs(60))
            .build()
            .unwrap();
        Beatport {
            client,
            access_token,
            api: "https://api.beatport.com".into(),
            auth: "https://account.beatport.com".into(),
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn with_endpoints(
        access_token: Arc<BeatportTokenCache>,
        client: Client,
        _web: &str,
        api: &str,
        auth: &str,
    ) -> Self {
        Self {
            client,
            access_token,
            api: api.trim_end_matches('/').into(),
            auth: auth.trim_end_matches('/').into(),
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn with_cancel(mut self, cancel: Arc<AtomicBool>) -> Self {
        self.cancel = cancel;
        self
    }

    /// Search for tracks on beatport
    // Catalog API transport adapted from OneTagger PR526; matching remains pinned upstream.
    pub fn search(
        &self,
        query: &str,
        page: i32,
        results_per_page: usize,
    ) -> Result<BeatportTrackResults, Error> {
        let query = Self::clear_search_query(query);
        let token = self.update_token()?;
        Ok(self
            .client
            .get(format!("{}/v4/catalog/search/", self.api))
            .bearer_auth(token)
            .query(&[
                ("q", query.as_str()),
                ("type", "tracks"),
                ("page", &page.to_string()),
                ("per_page", &results_per_page.to_string()),
            ])
            .send()?
            .bounded_json()?)
    }

    /// Update embed auth token
    pub fn update_token(&self) -> Result<String, Error> {
        let mut state = self
            .access_token
            .state
            .lock()
            .map_err(|_| anyhow!("Token refresh interrupted"))?;
        loop {
            if self.cancel.load(Ordering::Relaxed) {
                return Err(anyhow!("Tagging is stopping"));
            }
            if let Some(token) = state.token.as_ref().filter(|t| t.expires_in > timestamp!()) {
                return Ok(token.access_token.clone());
            }
            if let Some(failure) = &state.failure {
                if Instant::now() < failure.retry_after {
                    return Err(failure.clone().into());
                }
            }
            if !state.refreshing {
                state.refreshing = true;
                break;
            }
            state = self
                .access_token
                .changed
                .wait_timeout(state, Duration::from_millis(50))
                .map_err(|_| anyhow!("Token refresh interrupted"))?
                .0;
        }
        drop(state);
        let fetched = (|| -> Result<BeatportOAuth, Error> {
            let mut response: BeatportOAuth = self.client.post(format!("{}/o/token/",self.auth))
                .form(&json!({
                    "client_id": "2tiTbKxmQFwnbFjMONU4k7njMRZmV3ZMwRBndiZs",
                    "client_secret": "RDUJyAk4zFEGtQ8rsTmylDSfxmALRNBn3D1BsRr7MKi3oa1TL9Mq9QxqUPK7loiumXolEWbJcWa4IGAhtwnTz1cSXClGJ1tkkNCNWwRwjxIKTZJKOJxbwaNt0Rm3WG0v",
                    "grant_type": "client_credentials"
                }))
                .send()?.bounded_json()?;
            response.expires_in = response
                .expires_in
                .saturating_mul(1000)
                .saturating_add(timestamp!())
                .saturating_sub(10_000);
            Ok(response)
        })();
        let mut state = self
            .access_token
            .state
            .lock()
            .map_err(|_| anyhow!("Token refresh interrupted"))?;
        state.refreshing = false;
        let fetched = match fetched {
            Ok(token) => {
                state.token = Some(token.clone());
                state.failure = None;
                Ok(token)
            }
            Err(error) => {
                state.token = None;
                let failure = TokenRefreshFailure {
                    cause: Arc::new(error),
                    retry_after: Instant::now() + Duration::from_secs(2),
                };
                state.failure = Some(failure.clone());
                Err(Error::new(failure))
            }
        };
        drop(state);
        self.access_token.changed.notify_all();
        if self.cancel.load(Ordering::Relaxed) {
            return Err(anyhow!("Tagging is stopping"));
        }
        fetched.map(|token| token.access_token)
    }

    /// Fetch track using API
    pub fn track(&self, id: i64) -> Result<Option<BeatportTrack>, Error> {
        let token = self.update_token()?;
        let response = self
            .client
            .get(&format!("{}/v4/catalog/tracks/{}", self.api, id))
            .bearer_auth(token)
            .send()?;

        // Restricted / deleted track
        if response.status() == StatusCode::FORBIDDEN {
            return Ok(None);
        }

        Ok(response.bounded_json()?)
    }

    /// Fetch release using API
    pub fn release(&self, id: i64) -> Result<BeatportRelease, Error> {
        let token = self.update_token()?;
        let response = self
            .client
            .get(&format!("{}/v4/catalog/releases/{}", self.api, id))
            .bearer_auth(token)
            .send()?
            .bounded_json()?;
        Ok(response)
    }

    /// Get tracks from release
    pub fn release_tracks(&self, id: i64) -> Result<Vec<BeatportTrack>, Error> {
        let token = self.update_token()?;
        let response: BeatportPagination<BeatportTrack> = self
            .client
            .get(&format!(
                "{}/v4/catalog/releases/{}/tracks?per_page=200",
                self.api, id
            ))
            .bearer_auth(token)
            .send()?
            .bounded_json()?;
        Ok(response.results)
    }

    /// Beatport returns 403 if you have more than single () pair
    pub fn clear_search_query(query: &str) -> String {
        query
            .replace(['(', ')', '[', ']', ','], " ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeatportOAuth {
    pub access_token: String,
    pub expires_in: u128,
}

/// When searching for tracks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeatportTrackResults {
    #[serde(rename = "tracks")]
    pub data: Vec<BeatportTrack>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeatportTrackResult {
    pub track_id: i64,
    pub track_name: String,
    pub artists: Option<Vec<BeatportArtist>>,
    pub isrc: Option<String>,
    pub length: Option<u64>,
    pub mix_name: Option<String>,
    pub release: Option<BeatportTrackResultRelease>,
    pub genre: Option<Vec<BeatportTrackResultsGenre>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeatportTrackResultRelease {
    pub release_id: i64,
    pub release_image_uri: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeatportTrackResultsGenre {
    pub genre_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeatportArtist {
    pub artist_id: i64,
    pub artist_name: String,
    pub artist_type_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeatportTrack {
    pub artists: Vec<BeatportGeneric>,
    pub bpm: Option<i64>,
    pub catalog_number: Option<String>,
    pub exclusive: bool,
    pub genre: BeatportGeneric,
    pub id: i64,
    pub image: Option<BeatportImage>,
    pub isrc: Option<String>,
    pub key: Option<BeatportGeneric>,
    pub length_ms: Option<u64>,
    pub mix_name: String,
    pub name: String,
    pub number: Option<i64>,
    pub publish_date: Option<String>,
    pub release: BeatportRelease,
    pub remixers: Vec<BeatportGeneric>,
    pub slug: String,
    pub sub_genre: Option<BeatportGeneric>,
    pub new_release_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeatportGeneric {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeatportImage {
    pub id: i64,
    pub dynamic_uri: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeatportRelease {
    pub id: i64,
    pub name: String,
    pub label: BeatportGeneric,
    pub image: BeatportImage,
    pub upc: Option<String>,
    pub track_count: Option<u16>,
    pub artists: Option<Vec<BeatportGeneric>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BeatportPagination<T> {
    pub results: Vec<T>,
}

impl BeatportTrackResult {
    pub fn to_track(self, include_version: bool) -> Track {
        Track {
            platform: "beatport".to_string(),
            url: format!(
                "https://www.beatport.com/track/{}/{}",
                self.track_name.to_lowercase().replace(" ", "-"),
                self.track_id
            ),
            title: self.track_name,
            track_id: Some(self.track_id.to_string()),
            artists: self
                .artists
                .unwrap_or(vec![])
                .into_iter()
                .map(|a| a.artist_name)
                .collect(),
            version: match include_version {
                true => self.mix_name,
                false => None,
            },
            duration: Duration::from_millis(self.length.unwrap_or(0)).into(),
            isrc: self.isrc,
            thumbnail: self.release.map(|r| r.release_image_uri).flatten(),
            genres: self
                .genre
                .map(|g| g.into_iter().map(|g| g.genre_name).collect())
                .unwrap_or(vec![]),
            ..Default::default()
        }
    }
}

impl BeatportTrack {
    pub fn to_track(self, art_resolution: u32) -> Track {
        let art = self.get_art(art_resolution);
        let thumbnail = self.get_art(150);

        let mut track = Track {
            platform: "beatport".to_string(),
            title: self.name,
            version: Some(self.mix_name),
            artists: self.artists.into_iter().map(|a| a.name).collect(),
            album: Some(self.release.name),
            key: self
                .key
                .map(|k| k.name.replace(" Major", "").replace(" Minor", "m")),
            bpm: self.bpm,
            genres: vec![self.genre.name],
            styles: match self.sub_genre {
                Some(s) => vec![s.name],
                None => vec![],
            },
            art,
            url: format!("https://www.beatport.com/track/{}/{}", self.slug, self.id),
            label: Some(self.release.label.name),
            catalog_number: self.catalog_number,
            other: vec![(
                FrameName::same("UNIQUEFILEID"),
                vec![format!("https://beatport.com|{}", &self.id)],
            )],
            track_id: Some(self.id.to_string()),
            release_id: Some(self.release.id.to_string()),
            duration: Duration::from_millis(self.length_ms.unwrap_or(0)).into(),
            remixers: self.remixers.into_iter().map(|r| r.name).collect(),
            track_number: self.number.map(|n| TrackNumber::Number(n as i32)),
            isrc: self.isrc,
            release_year: self
                .new_release_date
                .as_ref()
                .map(|d| d.chars().take(4).collect::<String>().parse().ok())
                .flatten(),
            publish_year: self
                .publish_date
                .as_ref()
                .map(|d| d.chars().take(4).collect::<String>().parse().ok())
                .flatten(),
            release_date: self
                .new_release_date
                .as_ref()
                .map_or(None, |d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()),
            publish_date: self
                .publish_date
                .as_ref()
                .map_or(None, |d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()),
            thumbnail,
            ..Default::default()
        };

        // Exclusive
        if self.exclusive {
            track
                .other
                .push((FrameName::same("BEATPORT_EXCLUSIVE"), vec!["1".to_string()]));
        }

        track
    }

    /// Get album art URL
    pub fn get_art(&self, art_resolution: u32) -> Option<String> {
        if self.release.image.dynamic_uri.contains(&INVALID_ART) {
            return None;
        }
        let r = art_resolution.to_string();
        Some(
            self.release
                .image
                .dynamic_uri
                .replace("{w}", &r)
                .replace("{h}", &r)
                .replace("{x}", &r)
                .replace("{y}", &r),
        )
    }
}

// Match track
impl AutotaggerSource for Beatport {
    fn match_track(
        &mut self,
        info: &AudioFileInfo,
        config: &TaggerConfig,
    ) -> Result<Vec<TrackMatch>, Error> {
        // Load custom config
        let custom_config: BeatportConfig = config.get_custom("beatport")?;
        let mut output = vec![];

        // Fetch by ID
        if let Some(id) = info
            .tags
            .get("BEATPORT_TRACK_ID")
            .map(|t| {
                t.first()
                    .map(|id| id.trim().replace("\0", "").parse().ok())
                    .flatten()
            })
            .flatten()
        {
            info!("Fetching by ID: {}", id);
            match self.track(id) {
                Ok(Some(api_track)) => {
                    let track =
                        TrackMatch::new_id(api_track.to_track(custom_config.art_resolution));
                    if !config.fetch_all_results {
                        return Ok(vec![track]);
                    }
                    output.push(track);
                }
                Ok(None) => warn!("Matching by ID failed, track restricted, matching normally"),
                Err(e) => {
                    warn!("Matching by ID failed, matching normally: {e}");
                }
            }
        }

        // Fetch by ISRC
        if let Some(isrc) = info.isrc.as_ref() {
            match self.search(isrc, 1, 25) {
                Ok(results) => {
                    if !results.data.is_empty() {
                        let track = self.track(results.data[0].id)?;
                        match track {
                            Some(track) => {
                                let track = TrackMatch::new_isrc(
                                    track.to_track(custom_config.art_resolution),
                                );
                                if !config.fetch_all_results {
                                    return Ok(vec![track]);
                                }
                                output.push(track);
                            }
                            None => {
                                warn!("Matching by ISRC failed, track restricted, trying normal.")
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed fetching track by ISRC: {e}");
                }
            }
        }

        // Search
        let query = format!(
            "{} {}",
            info.artist()?,
            MatchingUtils::clean_title(info.title()?)
        );
        debug!("BP Query: {}", query);
        for page in 1..custom_config.max_pages + 1 {
            match self.search(&query, page, 50) {
                Ok(res) => {
                    // Match
                    let tracks = res
                        .data
                        .into_iter()
                        .map(|t| t.to_track(custom_config.art_resolution))
                        .collect::<Vec<_>>();

                    // Ignore version match
                    let tracks = if custom_config.ignore_version {
                        let t = tracks
                            .clone()
                            .into_iter()
                            .map(|mut t| {
                                t.version = None;
                                t
                            })
                            .collect();
                        // Copy back versions
                        MatchingUtils::match_track(info, &t, config, true)
                            .into_iter()
                            .map(|mut t| {
                                if let Some(ot) = tracks.iter().find(|ot| ot.url == t.track.url) {
                                    t.track.version = ot.version.to_owned();
                                }
                                t
                            })
                            .collect()
                    } else {
                        MatchingUtils::match_track(info, &tracks, config, true)
                    };

                    // Return
                    output.extend(tracks.into_iter().filter(|candidate| {
                        MatchingUtils::match_duration(info, &candidate.track, config)
                    }));
                    if config.fetch_all_results
                        || output.is_empty()
                        || config.multiple_matches != onetagger_tagger::MultipleMatchesSort::Default
                    {
                        continue;
                    }
                    return Ok(output);
                }
                Err(e) => {
                    warn!("Beatport search failed, query: {}. {}", query, e);
                    return Err(e);
                }
            }
        }
        Ok(output)
    }

    fn extend_track(&mut self, track: &mut Track, config: &TaggerConfig) -> Result<(), Error> {
        let custom_config: BeatportConfig = config.get_custom("beatport")?;

        // Extend search results track
        if track.other.is_empty() {
            let id = track.track_id.as_ref().unwrap().parse().unwrap();
            *track = self
                .track(id)?
                .ok_or(anyhow!("Restricted track"))?
                .to_track(custom_config.art_resolution);
        }

        // Ignore extending track
        if !config.tag_enabled(SupportedTag::AlbumArtist)
            && !config.tag_enabled(SupportedTag::TrackTotal)
        {
            return Ok(());
        }

        let release = self.release(
            track
                .release_id
                .as_ref()
                .ok_or(anyhow!("Missing release_id"))?
                .parse()?,
        )?;
        track.track_total = release.track_count;
        track.album_artists = match release.artists {
            Some(a) => a.into_iter().map(|a| a.name).collect(),
            None => vec![],
        };
        Ok(())
    }

    fn get_album(&mut self, id: &str, config: &TaggerConfig) -> Result<Option<Album>, Error> {
        let custom_config: BeatportConfig = config.get_custom("beatport")?;
        let id: i64 = id.trim().parse()?;
        let release = self.release(id)?;
        let tracks = self.release_tracks(id)?;

        let album = Album {
            id: id.to_string(),
            name: release.name,
            tracks: tracks
                .into_iter()
                .map(|t| t.to_track(custom_config.art_resolution))
                .collect(),
        };

        Ok(Some(album))
    }
}

/// For creating Beatport instances
#[derive(Debug, Clone)]
pub struct BeatportBuilder {
    access_token: Arc<BeatportTokenCache>,
}

impl AutotaggerSourceBuilder for BeatportBuilder {
    fn new() -> BeatportBuilder {
        BeatportBuilder {
            access_token: Arc::new(BeatportTokenCache::default()),
        }
    }

    fn get_source(&mut self, _config: &TaggerConfig) -> Result<Box<dyn AutotaggerSource>, Error> {
        Ok(Box::new(Beatport::new(self.access_token.clone())))
    }

    fn info(&self) -> PlatformInfo {
        PlatformInfo {
            id: "beatport".to_string(),
            name: "Beatport".to_string(),
            description: "Overall more specialized in Techno, can match using ISRC".to_string(),
            icon: b"",
            max_threads: 0,
            version: "1.0.0".to_string(),
            requires_auth: false,
            supported_tags: supported_tags!(
                Title,
                Version,
                Artist,
                AlbumArtist,
                Album,
                BPM,
                Genre,
                Style,
                Label,
                URL,
                ReleaseDate,
                PublishDate,
                Key,
                AlbumArt,
                OtherTags,
                TrackId,
                ReleaseId,
                Duration,
                Remixer,
                CatalogNumber,
                TrackTotal,
                ISRC,
                TrackNumber
            ),
            custom_options: PlatformCustomOptions::new()
                // Album art resolution
                .add(
                    "art_resolution",
                    "Album art resolution",
                    PlatformCustomOptionValue::Number {
                        min: 200,
                        max: 1600,
                        step: 100,
                        value: 500,
                    },
                )
                // Max pages to search
                .add_tooltip(
                    "max_pages",
                    "Max pages",
                    "How many pages of search results to scan for tracks",
                    PlatformCustomOptionValue::Number {
                        min: 1,
                        max: 10,
                        step: 1,
                        value: 1,
                    },
                )
                // Ignore version
                .add_tooltip(
                    "ignore_version",
                    "Ignore version when matching",
                    "Ignores (Extended Mix), (Original Mix) and such",
                    PlatformCustomOptionValue::Boolean { value: false },
                ),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BeatportConfig {
    pub art_resolution: u32,
    pub max_pages: i32,
    pub ignore_version: bool,
}

#[test]
#[ignore = "requires live catalog access; covered by adapter HTTP fixtures"]
fn test_album() {
    let mut builder = BeatportBuilder::new();
    let mut config = TaggerConfig::default();
    let custom_config = builder.info().custom_options.get_defaults();
    config
        .custom
        .0
        .insert("beatport".to_string(), custom_config);

    let mut bp = builder.get_source(&config).unwrap();
    bp.get_album("2174307", &config).unwrap();
}
