//! Ports of the selected OneTagger metadata adapters, with shared transport.
//! GPL-3.0-only; upstream source identity is recorded in vendor/onetagger/UPSTREAM.json.
mod mapping;
mod pages;
#[cfg(test)]
mod tests;
use crate::catalog_runtime::{CatalogAuth, CatalogRuntime, CatalogService};
use crate::{AutotagConfig, CatalogMatch, CatalogTrack, ProviderBackend, ProviderId, TagInput};
use anyhow::{Context, Result, bail, ensure};
use cuerithm_credentials::{CredentialRef, ProviderId as CredentialProvider, Secret};
use onetagger_selected_providers::http::{CatalogHttp, ChallengeStore};
use onetagger_tagger::MatchingUtils;
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub struct CatalogPort {
    id: ProviderId,
    runtime: Arc<CatalogRuntime>,
    http: CatalogHttp,
    fixture_origin: Option<String>,
    challenge_store: Arc<dyn ChallengeStore>,
}
impl CatalogPort {
    pub fn new(
        id: ProviderId,
        runtime: Arc<CatalogRuntime>,
        store: Arc<dyn ChallengeStore>,
    ) -> Result<Self> {
        ensure!(
            matches!(
                id,
                ProviderId::Itunes
                    | ProviderId::Bpmsupreme
                    | ProviderId::Bandcamp
                    | ProviderId::Deezer
            ),
            "Provider has no legacy catalog port"
        );
        let client = reqwest::blocking::Client::builder()
            .user_agent(
                "Mozilla/5.0 CueRithm catalog (https://github.com/johnneerdael/Cuerithm/issues)",
            )
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let mut port = Self {
            id,
            runtime: runtime.clone(),
            http: CatalogHttp::new(client.clone(), id.as_str(), store.clone()),
            fixture_origin: None,
            challenge_store: store,
        };
        let service = port.service();
        port.http = port
            .http
            .with_start_gate(Arc::new(move |cancel| runtime.wait_start(service, cancel)));
        Ok(port)
    }
    /// Trusted Rust test hook; never accepted from installed provider settings.
    #[cfg(test)]
    pub fn with_local_server(mut self, base: &str) -> Result<Self> {
        let url = reqwest::Url::parse(base)?;
        ensure!(
            url.scheme() == "http"
                && matches!(url.host_str(), Some("127.0.0.1" | "::1" | "[::1]"))
                && url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none()
                && url.fragment().is_none()
                && url.path() == "/",
            "Fixture catalog origin must be literal loopback HTTP"
        );
        self.fixture_origin = Some(base.trim_end_matches('/').to_owned());
        Ok(self)
    }
    fn service(&self) -> CatalogService {
        match self.id {
            ProviderId::Itunes => CatalogService::ITunes,
            ProviderId::Bpmsupreme => CatalogService::BpmSupreme,
            ProviderId::Bandcamp => CatalogService::Bandcamp,
            ProviderId::Deezer => CatalogService::Deezer,
            _ => unreachable!(),
        }
    }
    fn page(&self, url: &str, config: &AutotagConfig, cancel: &Arc<AtomicBool>) -> Result<String> {
        let parsed = reqwest::Url::parse(url)?;
        let host = parsed.host_str().unwrap_or("");
        let valid = match self.id {
            ProviderId::Bandcamp => host == "bandcamp.com" || host.ends_with(".bandcamp.com"),
            _ => false,
        };
        ensure!(
            valid
                && parsed.scheme() == "https"
                && parsed.port_or_known_default() == Some(443)
                && parsed.username().is_empty()
                && parsed.password().is_none(),
            "Catalog page origin is not approved"
        );
        let target = if let Some(base) = &self.fixture_origin {
            let mut target = reqwest::Url::parse(base)?;
            target.set_path(parsed.path());
            target.set_query(parsed.query());
            target.to_string()
        } else {
            url.to_owned()
        };
        self.http.get_text(&target, &config.solver, cancel)
    }
    fn search(
        &self,
        input: &TagInput,
        config: &AutotagConfig,
        cancel: &Arc<AtomicBool>,
    ) -> Result<Vec<CatalogTrack>> {
        let info = input.audio_info(config)?;
        let query = format!(
            "{} {}",
            info.artist()?,
            MatchingUtils::clean_title(info.title()?)
        );
        let params = |pairs: &[(&str, &str)]| {
            pairs
                .iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect::<Vec<_>>()
        };
        let body = match self.id {
            ProviderId::Itunes => self.runtime.get_json(
                self.service(),
                "https://itunes.apple.com/search",
                &params(&[
                    ("term", &query),
                    ("media", "music"),
                    ("entity", "song"),
                    ("limit", "50"),
                ]),
                CatalogAuth::None,
                cancel,
            )?,
            ProviderId::Deezer => self.runtime.get_json(
                self.service(),
                "https://api.deezer.com/search/track",
                &params(&[("q", &query), ("limit", "50")]),
                CatalogAuth::None,
                cancel,
            )?,
            ProviderId::Bpmsupreme => {
                let status = self
                    .runtime
                    .credential_status(CredentialProvider::BpmSupreme)?;
                let token = pool_session(&self.runtime, &status.reference, cancel)?;
                self.runtime.get_json(
                    self.service(),
                    "https://api.download.bpmsupreme.com/v1/albums",
                    &params(&[
                        ("term", &query),
                        ("limit", "50"),
                        ("skip", "0"),
                        ("library", "music"),
                        ("hide_remix", "0"),
                    ]),
                    CatalogAuth::Session {
                        token,
                        guard: status.reference,
                    },
                    cancel,
                )?
            }
            ProviderId::Bandcamp => {
                use onetagger_selected_providers::http::SolverFailure;
                if self.challenge_store.requires_solver("bandcamp")? {
                    return Err(SolverFailure("Bandcamp requires a solver, but its JSON search operation cannot be replayed by the configured GET-only solver transport".into()).into());
                }
                let result = self.runtime.post_json(
                    self.service(),
                    "https://bandcamp.com/api/bcsearch_public_api/1/autocomplete_elastic",
                    &json!({"fan_id":null,"full_page":false,"search_filter":"t","search_text":query}),
                    CatalogAuth::None,
                    cancel,
                );
                if result.as_ref().err().is_some_and(|error| {
                    error
                        .downcast_ref::<crate::catalog_runtime::CatalogError>()
                        .is_some_and(|error| {
                            error.kind
                                == crate::catalog_runtime::CatalogErrorKind::CloudflareProtected
                        })
                }) {
                    self.challenge_store.mark_requires_solver("bandcamp")?;
                    return Err(SolverFailure("Bandcamp requires a solver, but JSON POST search is unsupported by the GET-only solver transport".into()).into());
                }
                result?
            }
            _ => bail!("Unsupported catalog port"),
        };
        mapping::search(self.id, &body)
    }
}
impl ProviderBackend for CatalogPort {
    fn id(&self) -> ProviderId {
        self.id
    }
    fn stage_timeout(&self, config: &AutotagConfig) -> std::time::Duration {
        let pages = if self.id == ProviderId::Bandcamp {
            5
        } else {
            1
        };
        let solver = if self.id == ProviderId::Bandcamp
            && config
                .solver
                .endpoint
                .as_ref()
                .is_some_and(|s| !s.trim().is_empty())
        {
            2 * (config.solver.timeout_seconds.min(300) + 10) + 1
        } else {
            0
        };
        std::time::Duration::from_secs(pages * (95 + solver))
    }
    fn candidates(&self, input: &TagInput, config: &AutotagConfig) -> Result<Vec<CatalogMatch>> {
        self.candidates_cancellable(input, config, &Arc::new(AtomicBool::new(false)))
    }
    fn candidates_cancellable(
        &self,
        input: &TagInput,
        config: &AutotagConfig,
        cancel: &Arc<AtomicBool>,
    ) -> Result<Vec<CatalogMatch>> {
        ensure!(!cancel.load(Ordering::Relaxed), "Tagging is stopping");
        let info = input.audio_info(config)?;
        if self.id == ProviderId::Deezer
            && config.options.match_by_id
            && let Some(isrc) = info.isrc.as_deref()
        {
            let isrc = isrc.replace('-', "").to_ascii_uppercase();
            ensure!(
                isrc.len() == 12 && isrc.bytes().all(|c| c.is_ascii_alphanumeric()),
                "Invalid source ISRC"
            );
            let body = self.runtime.get_json(
                self.service(),
                &format!("https://api.deezer.com/track/isrc:{isrc}"),
                &[],
                CatalogAuth::None,
                cancel,
            )?;
            if body["error"]["code"].as_i64() != Some(800) {
                let tracks = mapping::search(self.id, &json!({"data":[body]}))?;
                let mut found = Vec::new();
                for track in tracks {
                    ensure!(
                        track.isrc.as_deref().is_some_and(|value| value
                            .replace('-', "")
                            .eq_ignore_ascii_case(&isrc)),
                        "Deezer returned another ISRC recording"
                    );
                    found.push(CatalogMatch::new_isrc(track));
                }
                return Ok(found);
            }
        }
        let tracks = self.search(input, config, cancel)?;
        let tracks = if self.id == ProviderId::Bandcamp {
            let mut title_matches =
                MatchingUtils::match_track(&info, &tracks, &config.options, false);
            title_matches.sort_by(|a, b| b.accuracy.total_cmp(&a.accuracy));
            let mut expanded = Vec::new();
            let mut seen = std::collections::BTreeSet::new();
            for candidate in title_matches {
                if !seen.insert(candidate.track.url.clone()) {
                    continue;
                }
                if expanded.len() == 3 {
                    break;
                }
                let detail = pages::bandcamp_detail(
                    &self.page(&candidate.track.url, config, cancel)?,
                    &candidate.track,
                )?;
                ensure!(
                    identity_title(&detail.title) == identity_title(&candidate.track.title),
                    "Bandcamp expanded a different recording"
                );
                expanded.push(detail);
            }
            expanded
        } else {
            tracks
        };
        Ok(
            MatchingUtils::match_track(&info, &tracks, &config.options, true)
                .into_iter()
                .map(CatalogMatch::from)
                .collect(),
        )
    }
    fn extend_cancellable(
        &self,
        track: &mut CatalogTrack,
        config: &AutotagConfig,
        cancel: &Arc<AtomicBool>,
    ) -> Result<()> {
        ensure!(!cancel.load(Ordering::Relaxed), "Tagging is stopping");
        if self.id == ProviderId::Bandcamp {
            let expanded = pages::bandcamp_detail(&self.page(&track.url, config, cancel)?, track)?;
            // Expansion must not replace the chosen recording/artist with an unrelated uploader.
            ensure!(
                identity_title(&expanded.title) == identity_title(&track.title),
                "Bandcamp expanded a different recording"
            );
            ensure!(
                track.artists.iter().all(|artist| expanded
                    .artists
                    .iter()
                    .any(|credit| identity_title(credit) == identity_title(artist))),
                "Bandcamp changed the selected artist credit"
            );
            *track = expanded;
        } else if self.id == ProviderId::Deezer {
            let id = track
                .track_id
                .as_deref()
                .context("Deezer track identifier missing")?;
            ensure!(
                id.bytes().all(|b| b.is_ascii_digit()),
                "Invalid Deezer track identifier"
            );
            let body = self.runtime.get_json(
                self.service(),
                &format!("https://api.deezer.com/track/{id}"),
                &[],
                CatalogAuth::None,
                cancel,
            )?;
            let mut expanded = mapping::search(self.id, &json!({"data":[body]}))?
                .pop()
                .context("Deezer track missing")?;
            ensure!(
                expanded.track_id == track.track_id,
                "Deezer changed recording identity"
            );
            ensure!(
                track
                    .release_id
                    .as_ref()
                    .is_none_or(|id| expanded.release_id.as_ref() == Some(id)),
                "Deezer changed the selected release identity"
            );
            ensure!(
                track.isrc.as_ref().is_none_or(|isrc| expanded
                    .isrc
                    .as_ref()
                    .is_some_and(|value| value.eq_ignore_ascii_case(isrc))),
                "Deezer changed the selected ISRC"
            );
            let album = expanded
                .release_id
                .as_deref()
                .context("Deezer release identifier missing")?;
            ensure!(
                album.bytes().all(|b| b.is_ascii_digit()),
                "Invalid Deezer release identifier"
            );
            let body = self.runtime.get_json(
                self.service(),
                &format!("https://api.deezer.com/album/{album}"),
                &[],
                CatalogAuth::None,
                cancel,
            )?;
            ensure!(body.get("error").is_none(), "Deezer album unavailable");
            ensure!(
                body.get("id")
                    .and_then(|id| id
                        .as_u64()
                        .map(|n| n.to_string())
                        .or_else(|| id.as_str().map(str::to_owned)))
                    .as_deref()
                    == Some(album),
                "Deezer returned another album identity"
            );
            ensure!(
                expanded.album.as_ref().is_none_or(|title| body
                    .get("title")
                    .and_then(Value::as_str)
                    .is_none_or(|current| current == title)),
                "Deezer album title changed during enrichment"
            );
            expanded.label = body["label"].as_str().map(str::to_owned);
            expanded.track_total = body["nb_tracks"]
                .as_u64()
                .and_then(|n| u16::try_from(n).ok());
            expanded.album_artists = body["contributors"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|v| v["name"].as_str().map(str::to_owned))
                .take(64)
                .collect();
            expanded.genres = body["genres"]["data"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|v| v["name"].as_str().map(str::to_owned))
                .take(64)
                .collect();
            *track = expanded;
        }
        Ok(())
    }
}
fn pool_session(
    runtime: &CatalogRuntime,
    reference: &CredentialRef,
    cancel: &Arc<AtomicBool>,
) -> Result<Secret> {
    runtime.with_credential(reference,|secret,revocation|{
        ensure!(!revocation.is_revoked(),"BPM Supreme account changed");
        let account:Value=serde_json::from_str(secret).map_err(|_|anyhow::anyhow!("Stored BPM Supreme account is invalid"))?;
        let email=account["email"].as_str().context("BPM Supreme email missing")?;
        let password=account["password"].as_str().context("BPM Supreme password missing")?;
        let body=runtime.post_json(CatalogService::BpmSupreme,"https://api.bpmsupreme.com/v4/login",&json!({"email":email,"password":password,"from":"global-login","device":{"app_type":"web_landing","app_version":"2.0","build_version":"1","debug":false,"device_data_os":"web","device_data_device_name":"CueRithm","device_uuid":reference.generation.simple().to_string(),"language":"en-US"}}),CatalogAuth::Guard(reference.clone()),cancel)?;
        Secret::new(body["data"]["session_token"].as_str().context("BPM Supreme login did not return a session")?.into()).map_err(Into::into)
    })
}
pub fn test_pool_account(
    runtime: &CatalogRuntime,
    reference: &CredentialRef,
    cancel: &Arc<AtomicBool>,
) -> Result<()> {
    pool_session(runtime, reference, cancel).map(|_| ())
}

fn identity_title(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}
