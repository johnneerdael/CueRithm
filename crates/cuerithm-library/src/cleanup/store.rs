use crate::Library;
use anyhow::{Context, Result, ensure};
use cuerithm_cleanup::{CleanupConfig, CleanupReport, Metadata, MetadataField};
use rusqlite::{Connection, params};
pub(crate) fn store_native(conn: &Connection, track: &str, metadata: &Metadata) -> Result<()> {
    metadata.validate()?;
    let mut native = metadata.clone();
    native.custom_tags.clear();
    conn.execute(
        "UPDATE tracks SET native_metadata_json=?1 WHERE id=?2",
        params![serde_json::to_string(&native)?, track],
    )?;
    Ok(())
}
pub(crate) fn validate_report(
    conn: &Connection,
    track: &str,
    config: Option<&CleanupConfig>,
    report: Option<&CleanupReport>,
) -> Result<()> {
    ensure!(
        config.is_some() == report.is_some(),
        "Required cleanup stage differs from its configuration"
    );
    if let Some(config) = config {
        let report = report.context("Required cleanup has not completed")?;
        let expected = cuerithm_cleanup::apply(config, &report.original)?;
        ensure!(
            expected == *report,
            "Cleanup result differs from the configured rules"
        );
        let stable=|tags:&[cuerithm_cleanup::CustomTag]|->Result<std::collections::BTreeSet<(String,String)>>{tags.iter().map(|tag|crate::curation::resolve_category_value(conn,&tag.category,&tag.value,false)).collect()};
        ensure!(
            stable(&super::custom_tags(conn, track)?)? == stable(&report.original.custom_tags)?,
            "Categorized tags changed during preparation"
        );
    }
    Ok(())
}
pub(crate) fn apply_report(conn: &Connection, track: &str, report: &CleanupReport) -> Result<()> {
    use MetadataField as F;
    report.metadata.validate()?;
    for (field, column) in [
        (F::Title, "title"),
        (F::Artist, "artist"),
        (F::Album, "album"),
        (F::Genre, "genre"),
    ] {
        if report.changes.iter().any(|c| c.field == field) {
            let value = report
                .metadata
                .fields
                .get(&field)
                .map(|v| v.join(", "))
                .unwrap_or_default();
            conn.execute(
                &format!("UPDATE tracks SET {column}=?1 WHERE id=?2"),
                params![value, track],
            )?;
        }
    }
    conn.execute(
        "UPDATE tracks SET cleanup_json=?1 WHERE id=?2",
        params![serde_json::to_string(report)?, track],
    )?;
    let (title, artist, genre): (String, String, String) = conn.query_row(
        "SELECT title,artist,genre FROM tracks WHERE id=?1",
        [track],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    conn.execute(
        "UPDATE tracks SET search_text=?1,sort_title=?2,sort_artist=?3 WHERE id=?4",
        params![
            format!("{title} {artist} {genre}").to_lowercase(),
            title.to_lowercase(),
            artist.to_lowercase(),
            track
        ],
    )?;
    super::journal::set_custom_tags(conn, track, &report.metadata.custom_tags)?;
    store_native(conn, track, &report.metadata)?;
    Ok(())
}
impl Library {
    pub(crate) fn preparation_cleanup_config(
        snapshot: &crate::SettingsSnapshot,
    ) -> Result<Option<&CleanupConfig>> {
        if snapshot.settings.cleanup_enabled {
            Ok(Some(
                snapshot
                    .settings
                    .cleanup
                    .as_ref()
                    .context("Missing cleanup settings")?,
            ))
        } else {
            Ok(None)
        }
    }
}
