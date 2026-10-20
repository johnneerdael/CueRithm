//! Read-only live integration diagnostic. Uses public metadata; never opens audio.
//! cargo run -p cuerithm-autotag --example solver_probe -- http://localhost:8191
use anyhow::{Context, Result, ensure};
use cuerithm_autotag::{
    AutotagConfig, CatalogEndpoints, ChallengeStore, MatchStatus, MemoryChallengeStore,
    SupportedTag, TagInput, Tagger,
};
use cuerithm_media::MediaInfo;
use std::{sync::Arc, time::Instant};

fn input(artist: &str, title: &str) -> TagInput {
    TagInput {
        path: "public-catalog-probe.flac".into(),
        source_sha256: "0".repeat(64),
        duration_seconds: 346.,
        media: MediaInfo {
            codec: "flac".into(),
            sample_rate: 44100,
            channels: 2,
            bits_per_sample: Some(16),
            properties: Default::default(),
            tags: [
                ("artist".into(), artist.into()),
                ("title".into(), title.into()),
            ]
            .into(),
        },
    }
}
fn main() -> Result<()> {
    let endpoint = std::env::args()
        .nth(1)
        .context("Pass a solver server URL or /v1 endpoint")?;
    let store = Arc::new(MemoryChallengeStore::default());
    let tagger = Tagger::with_endpoints_and_store(CatalogEndpoints::default(), store.clone())?;
    let mut config = AutotagConfig::default();
    config.options.platforms = vec!["traxsource".into()];
    config.options.tags = vec![
        SupportedTag::CatalogNumber,
        SupportedTag::AlbumArtist,
        SupportedTag::TrackNumber,
    ];
    let track = input("Dennis Ferrer", "Night Flight to Oslo");
    let direct = tagger.match_track(&track, &config);
    println!(
        "direct_success={} requires_solver={}",
        direct.is_ok(),
        store.requires_solver("traxsource")?
    );
    config.solver.endpoint = Some(endpoint);
    let started = Instant::now();
    let result = tagger.match_track(&track, &config)?;
    ensure!(
        result.status == MatchStatus::Matched,
        "Expected a known public catalog match"
    );
    for item in result.matches {
        ensure!(
            item.track.release_id.is_some() && item.track.catalog_number.is_some(),
            "Missing release metadata"
        );
        println!(
            "match title={:?} bpm={:?} key={:?} album={:?} catalog={:?} track_number={:?} elapsed={:.2}s",
            item.track.title,
            item.track.bpm,
            item.track.key,
            item.track.album,
            item.track.catalog_number,
            item.track.track_number,
            started.elapsed().as_secs_f64()
        );
    }
    let started = Instant::now();
    let empty = tagger.match_track(&input("Daft Punk", "One More Time"), &config)?;
    ensure!(
        empty.status == MatchStatus::NoMatch,
        "Expected a genuine empty search"
    );
    println!(
        "empty_search={:?} elapsed={:.2}s",
        empty.status,
        started.elapsed().as_secs_f64()
    );
    Ok(())
}
