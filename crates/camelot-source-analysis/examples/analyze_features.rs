use anyhow::{Context, Result};
use std::path::PathBuf;

fn main() -> Result<()> {
    let path = PathBuf::from(std::env::args_os().nth(1).context("audio path required")?);
    let output = camelot_source_analysis::analyze_file(&path)?;
    println!("{}", serde_json::to_string(&output)?);
    Ok(())
}
