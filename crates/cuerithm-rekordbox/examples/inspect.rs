//! Research diagnostics: aggregate schema/integrity output, never rows or key material.
use anyhow::{Context, Result};
use clap::Parser;
use cuerithm_media::CancellationToken;
use cuerithm_rekordbox::{DatabaseKey, ReadOnlyDatabase};
use std::path::PathBuf;
#[derive(Parser)]
struct Args {
    #[arg(long)]
    database: PathBuf,
    #[arg(long)]
    key_file: PathBuf,
    #[arg(long)]
    snapshot_cache: Option<PathBuf>,
}
fn main() -> Result<()> {
    let args = Args::parse();
    let bytes = zeroize::Zeroizing::new(
        std::fs::read(args.key_file).context("Read private database key file")?,
    );
    let key = DatabaseKey::new(bytes.as_slice())?;
    let db = ReadOnlyDatabase::open(&args.database, &key)?;
    let schema = db.schema()?;
    let checks = db.checks()?;
    let mut report = serde_json::json!({"cipher_version":db.cipher_version(),"user_version":schema.user_version,"application_id":schema.application_id,"schema_sha256":schema.fingerprint,"objects":schema.objects.len(),"tables":schema.objects.iter().filter(|o|o.kind=="table").count(),"checks":checks});
    if let Some(cache) = args.snapshot_cache {
        let copy = db.capture(&cache, &key, &CancellationToken::default())?;
        report["snapshot"] = serde_json::json!({"sha256":copy.sha256,"schema_sha256":copy.schema.fingerprint,"checks":copy.checks,"owned_temporary":true});
    }
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
