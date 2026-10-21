//! Public anonymous Discogs catalog with optional verified personal authorization.
use crate::catalog_runtime::{CatalogAuth, CatalogError, CatalogErrorKind, CatalogService};
use crate::sonovault::{date, matched, numeric_id, strings, text};
use crate::{
    AutotagConfig, CatalogFacts, CatalogMatch, CatalogRuntime, CatalogTrack, ProviderBackend,
    ProviderId, SourceCatalogFacts, SourceCatalogIdentity, SourceCatalogIdentityKind, TagInput,
};
use anyhow::{Context, Result, ensure};
use cuerithm_credentials::ProviderId as CredentialProviderId;
use serde_json::Value;
use std::sync::{Arc, atomic::AtomicBool};
pub struct DiscogsBackend {
    runtime: Arc<CatalogRuntime>,
}
impl DiscogsBackend {
    pub fn new(runtime: Arc<CatalogRuntime>) -> Self {
        Self { runtime }
    }
    fn auth(&self) -> Result<CatalogAuth> {
        if let Ok(status) = self
            .runtime
            .credential_status(CredentialProviderId::Discogs)
            && self.runtime.is_authenticated(&status.reference)?
        {
            return Ok(CatalogAuth::Credential(status.reference));
        }
        Ok(CatalogAuth::None)
    }
    fn get(
        &self,
        url: &str,
        query: &[(String, String)],
        cancel: &Arc<AtomicBool>,
    ) -> Result<Value> {
        self.runtime
            .get_json(CatalogService::Discogs, url, query, self.auth()?, cancel)
    }
}
impl ProviderBackend for DiscogsBackend {
    fn id(&self) -> ProviderId {
        ProviderId::Discogs
    }
    fn candidates(&self, i: &TagInput, c: &AutotagConfig) -> Result<Vec<CatalogMatch>> {
        self.candidates_cancellable(i, c, &Arc::new(AtomicBool::new(false)))
    }
    fn candidates_cancellable(
        &self,
        i: &TagInput,
        c: &AutotagConfig,
        cancel: &Arc<AtomicBool>,
    ) -> Result<Vec<CatalogMatch>> {
        let info = i.audio_info(c)?;
        let Some(title) = info.title.as_ref().filter(|s| !s.trim().is_empty()) else {
            return Ok(vec![]);
        };
        if info.artists.is_empty() {
            return Ok(vec![]);
        }
        let value = self.get(
            "https://api.discogs.com/database/search",
            &[
                ("type".into(), "release".into()),
                ("artist".into(), info.artists.join(", ")),
                ("track".into(), title.clone()),
                ("per_page".into(), "5".into()),
                ("page".into(), "1".into()),
            ],
            cancel,
        )?;
        let rows = value
            .get("results")
            .and_then(Value::as_array)
            .context("Malformed Discogs search response")?;
        ensure!(rows.len() <= 100, "Discogs search exceeds limit");
        let mut matches = Vec::new();
        for row in rows.iter().take(5) {
            let id = row
                .get("id")
                .and_then(Value::as_u64)
                .filter(|id| *id > 0)
                .context("Invalid Discogs search identity")?;
            match self.get(
                &format!("https://api.discogs.com/releases/{id}"),
                &[],
                cancel,
            ) {
                Ok(value) => {
                    ensure!(
                        value.get("id").and_then(Value::as_u64) == Some(id),
                        "Discogs returned another release"
                    );
                    matches.extend(parse_release(&value)?);
                }
                Err(e)
                    if e.downcast_ref::<CatalogError>()
                        .is_some_and(|e| e.kind == CatalogErrorKind::NotFound) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(matched(&info, &c.options, matches))
    }
    fn extend_candidate_cancellable(
        &self,
        m: &mut CatalogMatch,
        _: &AutotagConfig,
        cancel: &Arc<AtomicBool>,
    ) -> Result<()> {
        m.validate()?;
        ensure!(m.platform == "discogs", "Wrong provider identity");
        let id = m
            .release_id
            .as_ref()
            .context("Missing Discogs release identity")?;
        numeric_id(id)?;
        let value = self.get(
            &format!("https://api.discogs.com/releases/{id}"),
            &[],
            cancel,
        )?;
        ensure!(
            value
                .get("id")
                .and_then(Value::as_u64)
                .map(|n| n.to_string())
                .as_ref()
                == Some(id),
            "Discogs returned another release"
        );
        let mut next = parse_release(&value)?
            .into_iter()
            .find(|candidate| candidate.track_id == m.track_id)
            .context("Discogs release no longer contains selected track")?;
        ensure!(
            next.title == m.title && next.artists == m.artists,
            "Discogs selected track changed"
        );
        next.validate_extension_from(m)?;
        next.accuracy = m.accuracy;
        next.reason = m.reason;
        *m = next;
        Ok(())
    }
}
fn credits(v: &Value) -> Vec<String> {
    v.get("artists")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(64)
        .filter_map(|a| text(a, "anv").or_else(|| text(a, "name")))
        .collect()
}
fn duration(s: &str) -> Result<std::time::Duration> {
    let parts = s
        .split(':')
        .map(str::parse::<u64>)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    ensure!(
        (2..=3).contains(&parts.len()) && parts.iter().skip(1).all(|n| *n < 60),
        "Invalid Discogs duration"
    );
    let seconds = parts
        .into_iter()
        .try_fold(0u64, |sum, n| sum.checked_mul(60)?.checked_add(n))
        .context("Invalid Discogs duration")?;
    ensure!(seconds <= 86400, "Invalid Discogs duration");
    Ok(std::time::Duration::from_secs(seconds))
}
fn parse_release(v: &Value) -> Result<Vec<CatalogMatch>> {
    let id = v
        .get("id")
        .and_then(Value::as_u64)
        .filter(|v| *v > 0)
        .context("Missing Discogs release identity")?
        .to_string();
    let album = text(v, "title").context("Missing Discogs release title")?;
    let release_date = text(v, "released")
        .map(|s| date(&s, None))
        .transpose()?
        .or_else(|| {
            v.get("year")
                .and_then(Value::as_i64)
                .filter(|y| *y > 0)
                .map(|year| crate::TransferDate {
                    year: i32::try_from(year).unwrap_or(0),
                    month: None,
                    day: None,
                })
        });
    if let Some(d) = &release_date {
        crate::validate_date(d)?;
    }
    let release_artists = credits(v);
    let tracks = v
        .get("tracklist")
        .and_then(Value::as_array)
        .context("Malformed Discogs tracklist")?;
    ensure!(tracks.len() <= 500, "Discogs tracklist exceeds limit");
    let mut output = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for (index, row) in tracks.iter().enumerate() {
        if text(row, "type_").is_some_and(|s| s != "track") {
            continue;
        }
        let title = text(row, "title").context("Missing Discogs track title")?;
        let raw = text(row, "position");
        let identity = format!(
            "{id}-{}",
            raw.as_deref()
                .map(str::to_owned)
                .unwrap_or_else(|| format!("index-{}", index + 1))
        );
        ensure!(
            seen.insert(identity.clone()),
            "Ambiguous Discogs track position"
        );
        let artists = credits(row);
        let artists = if artists.is_empty() {
            release_artists.clone()
        } else {
            artists
        };
        ensure!(!artists.is_empty(), "Missing Discogs artist credits");
        let label = v
            .get("labels")
            .and_then(Value::as_array)
            .and_then(|rows| rows.first());
        let image = v
            .get("images")
            .and_then(Value::as_array)
            .and_then(|rows| {
                rows.iter()
                    .find(|r| text(r, "type").as_deref() == Some("primary"))
                    .or_else(|| rows.first())
            })
            .and_then(|r| text(r, "uri"));
        let mut track = CatalogTrack {
            platform: "discogs".into(),
            title,
            artists,
            album_artists: release_artists.clone(),
            album: Some(album.clone()),
            track_id: Some(identity.clone()),
            release_id: Some(id.clone()),
            track_number: raw
                .as_ref()
                .map(|s| onetagger_tagger::TrackNumber::Custom(s.clone())),
            genres: strings(v, "genres"),
            styles: strings(v, "styles"),
            label: label.and_then(|l| text(l, "name")),
            catalog_number: label.and_then(|l| text(l, "catno")),
            art: image,
            url: format!("https://www.discogs.com/release/{id}"),
            ..Default::default()
        };
        if let Some(d) = text(row, "duration") {
            track.duration = duration(&d)?;
        }
        let mut identities = vec![SourceCatalogIdentity {
            source: "discogs".into(),
            kind: SourceCatalogIdentityKind::ReleaseTrack,
            id: identity,
        }];
        if let Some(master) = v
            .get("master_id")
            .and_then(Value::as_u64)
            .filter(|v| *v > 0)
        {
            identities.push(SourceCatalogIdentity {
                source: "discogs-master".into(),
                kind: SourceCatalogIdentityKind::Release,
                id: master.to_string(),
            });
        }
        let mut candidate = CatalogMatch::new(0., track);
        candidate.facts = Some(CatalogFacts::Source(SourceCatalogFacts {
            schema_version: 1,
            provider: "discogs".into(),
            release_id: Some(id.clone()),
            release_date: release_date.clone(),
            raw_position: raw,
            identities,
            ..Default::default()
        }));
        candidate.validate()?;
        output.push(candidate);
    }
    Ok(output)
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn anonymous_release_preserves_raw_vinyl_position_and_partial_date() {
        let m=parse_release(&json!({"id":42,"master_id":9,"title":"Edition","released":"2021-07-00","artists":[{"name":"Credit","id":1}],"genres":["Electronic"],"styles":["House"],"labels":[{"name":"Label","catno":"CAT42"}],"tracklist":[{"position":"A1","type_":"track","title":"Signal (Dub)","duration":"5:00"}]})).unwrap().remove(0);
        m.validate().unwrap();
        assert_eq!(m.release_id.as_deref(), Some("42"));
        assert_eq!(
            m.track_number,
            Some(onetagger_tagger::TrackNumber::Custom("A1".into()))
        );
        assert_eq!(
            m.selected_release_date(),
            Some(&crate::TransferDate {
                year: 2021,
                month: Some(7),
                day: None
            })
        );
        assert_eq!(m.track_id.as_deref(), Some("42-A1"));
        assert_eq!(m.duration.as_secs(), 300);
    }

    #[test]
    fn anonymous_search_and_selected_release_lookup_use_shared_runtime() {
        use crate::catalog_runtime::*;
        struct Fixture;
        impl CatalogTransport for Fixture {
            fn execute(
                &self,
                r: CatalogHttpRequest,
                _: &Arc<AtomicBool>,
                _: Option<&cuerithm_credentials::Revocation>,
            ) -> Result<CatalogHttpResponse> {
                assert!(
                    !r.headers
                        .iter()
                        .any(|(k, _)| k.eq_ignore_ascii_case("Authorization"))
                );
                let body = if r.url.contains("/database/search?") {
                    json!({"results":[{"id":42}]})
                } else {
                    assert!(r.url.ends_with("/releases/42"));
                    json!({"id":42,"title":"Edition","released":"2021-07-00","artists":[{"name":"Credit"}],"tracklist":[{"position":"A1","type_":"track","title":"Signal (Dub)","duration":"5:00"}]})
                };
                Ok(CatalogHttpResponse {
                    status: 200,
                    body: serde_json::to_vec(&body).unwrap(),
                    headers: vec![],
                })
            }
        }
        let runtime = CatalogRuntime::for_local_server("http://127.0.0.1:9")
            .unwrap()
            .with_transport(Arc::new(Fixture));
        let backend = DiscogsBackend::new(Arc::new(runtime));
        let m = backend
            .candidates(
                &crate::catalog_runtime::tests::provider_input(),
                &AutotagConfig::default(),
            )
            .unwrap()
            .remove(0);
        assert_eq!(m.track_id.as_deref(), Some("42-A1"));
        assert!(matches!(m.facts, Some(crate::CatalogFacts::Source(_))));
    }
}
