//! Research diagnostics: aggregate schema/integrity output, never rows or key material.
use anyhow::{Context, Result};
use clap::Parser;
use cuerithm_media::CancellationToken;
use cuerithm_rekordbox::{DatabaseKey, ReadOnlyDatabase};
use std::path::PathBuf;
#[derive(Parser)]
struct Args {
    #[arg(long, requires = "key_file")]
    database: Option<PathBuf>,
    #[arg(long, requires = "database")]
    key_file: Option<PathBuf>,
    #[arg(long, conflicts_with = "database")]
    discover: bool,
    #[arg(long, requires = "database")]
    snapshot_cache: Option<PathBuf>,
    #[arg(long, requires = "database")]
    collection: bool,
}
fn main() -> Result<()> {
    let args = Args::parse();
    if args.discover {
        let paths = cuerithm_rekordbox::DiscoveryPaths::current()?;
        let found = cuerithm_rekordbox::discover_targets(&paths, &CancellationToken::default())?;
        let output = serde_json::json!({"discovery":{"platform_supported":found.platform_supported,"installations":found.installations.iter().map(|v|serde_json::json!({"version":v.version,"major":v.major,"version_source":v.version_source})).collect::<Vec<_>>(),"database_state":found.database.as_ref().map(|d|&d.state),"database_source":found.database.as_ref().map(|d|&d.source),"issue_codes":found.issues.iter().map(|i|&i.code).collect::<Vec<_>>()}});
        println!("{}", serde_json::to_string_pretty(&output)?);
        if args.database.is_none() {
            return Ok(());
        }
    }
    let database = args
        .database
        .context("Choose --discover or --database with --key-file")?;
    let bytes = zeroize::Zeroizing::new(
        std::fs::read(args.key_file.context("A private key file is required")?)
            .context("Read private database key file")?,
    );
    let key = DatabaseKey::new(bytes.as_slice())?;
    let db = ReadOnlyDatabase::open(&database, &key)?;
    let schema = db.schema()?;
    let checks = db.checks()?;
    let mut report = serde_json::json!({"cipher_version":db.cipher_version(),"user_version":schema.user_version,"application_id":schema.application_id,"schema_sha256":schema.fingerprint,"objects":schema.objects.len(),"tables":schema.objects.iter().filter(|o|o.kind=="table").count(),"checks":checks});
    if args.collection {
        use sha2::{Digest, Sha256};
        let collection = db.read_collection(Default::default(), &CancellationToken::default())?;
        let mut tracks = collection.tracks.iter().collect::<Vec<_>>();
        tracks.sort_by(|a, b| a.identity.content_id.cmp(&b.identity.content_id));
        let projection = tracks
            .iter()
            .map(|t| {
                serde_json::json!([
                    t.identity.content_id,
                    t.metadata.title,
                    t.metadata.artist,
                    t.metadata.album,
                    t.metadata.genre,
                    t.metadata.label,
                    t.metadata.key,
                    t.metadata.comment,
                    t.metadata.mix_name,
                    t.metadata.remixer,
                    t.metadata.composer,
                    t.metadata.lyricist,
                    t.bpm.map(|v| format!("{v:.2}")).unwrap_or_default()
                ])
            })
            .collect::<Vec<_>>();
        let mut issues_by_code = std::collections::BTreeMap::new();
        for issue in &collection.issues {
            *issues_by_code.entry(&issue.code).or_insert(0usize) += 1;
        }
        report["collection"] = serde_json::json!({"tracks":collection.tracks.len(),"active_local":collection.active_local_tracks().count(),"playlists":collection.playlists.len(),"playlist_links":collection.playlist_links.len(),"tags":collection.tags.len(),"tag_links":collection.tag_links.len(),"cues":collection.tracks.iter().map(|t|t.cues.len()).sum::<usize>(),"issues":collection.issues.len(),"issues_by_code":issues_by_code,"metadata_projection_sha256":format!("{:x}",Sha256::digest(serde_json::to_vec(&projection)?))});
    }
    if let Some(cache) = args.snapshot_cache {
        let copy = db.capture(&cache, &key, &CancellationToken::default())?;
        report["snapshot"] = serde_json::json!({"sha256":copy.sha256,"schema_sha256":copy.schema.fingerprint,"checks":copy.checks,"owned_temporary":true});
    }
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
