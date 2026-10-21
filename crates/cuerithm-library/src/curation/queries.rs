use super::*;
use anyhow::{Result, ensure};
use rusqlite::{Connection, OptionalExtension};
pub(crate) fn validate_filter(conn: &Connection, filter: Option<&CurationFilter>) -> Result<()> {
    if let Some(filter) = filter {
        ensure!(
            filter.all_tags.len()
                + filter.any_tags.len()
                + filter.exclude_tags.len()
                + filter.empty_groups.len()
                <= 4096
                && filter.notes.len() <= 65536,
            "Curate filter exceeds bound"
        );
        for id in filter
            .all_tags
            .iter()
            .chain(&filter.any_tags)
            .chain(&filter.exclude_tags)
        {
            ensure!(conn.query_row("SELECT EXISTS(SELECT 1 FROM custom_tags t LEFT JOIN curation_value_properties v ON v.tag_id=t.id LEFT JOIN curation_group_properties g ON g.category_id=t.category_id WHERE t.id=?1 AND COALESCE(v.deleted,0)=0 AND COALESCE(g.deleted,0)=0)",[id],|row|row.get::<_,bool>(0))?,"Filter references an unavailable Curate value; refresh filters");
        }
        for id in &filter.empty_groups {
            super::taxonomy::group_active(conn, id)?;
        }
        for rating in [filter.user_rating_min, filter.user_rating_max]
            .into_iter()
            .flatten()
        {
            ensure!(rating <= 5, "Curate rating filter requires0..5");
        }
        ensure!(
            filter
                .user_rating_min
                .zip(filter.user_rating_max)
                .is_none_or(|(min, max)| min <= max),
            "Invalid Curate rating range"
        );
    }
    Ok(())
}
pub(crate) fn project_summary(conn: &Connection, row: &mut crate::TrackSummary) -> Result<()> {
    let scalar:Option<(Option<u8>,String,u64)>=conn.query_row("SELECT json_extract(committed_json,'$.user_rating'),COALESCE(json_extract(committed_json,'$.notes'),''),acknowledged_revision FROM curation_tracks WHERE track_id=?1",[&row.id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
    let (rating, notes, revision) = scalar.unwrap_or((None, String::new(), 0));
    row.metadata
        .insert("user_rating".into(), serde_json::json!(rating));
    row.metadata.insert("curation_notes".into(), notes.into());
    row.metadata
        .insert("curation_revision".into(), revision.into());
    row.metadata.insert(
        "categories".into(),
        serde_json::to_value(super::storage::categories(conn, &row.id)?)?,
    );
    Ok(())
}
pub(crate) fn smart_values(
    conn: &Connection,
) -> Result<std::collections::BTreeMap<String, (Option<u8>, String)>> {
    let mut query=conn.prepare("SELECT track_id,json_extract(committed_json,'$.user_rating'),COALESCE(json_extract(committed_json,'$.notes'),'') FROM curation_tracks")?;
    Ok(query
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                (row.get::<_, Option<u8>>(1)?, row.get::<_, String>(2)?),
            ))
        })?
        .collect::<rusqlite::Result<_>>()?)
}
pub(crate) const FILTER_SQL: &str = "NOT EXISTS(SELECT 1 FROM json_each(json_extract(:curation,'$.all_tags')) f WHERE NOT EXISTS(SELECT 1 FROM track_custom_tags l WHERE l.track_id=t.id AND l.tag_id=f.value)) AND (COALESCE(json_array_length(json_extract(:curation,'$.any_tags')),0)=0 OR EXISTS(SELECT 1 FROM track_custom_tags l JOIN json_each(json_extract(:curation,'$.any_tags')) f ON l.tag_id=f.value WHERE l.track_id=t.id)) AND NOT EXISTS(SELECT 1 FROM track_custom_tags l JOIN json_each(json_extract(:curation,'$.exclude_tags')) f ON l.tag_id=f.value WHERE l.track_id=t.id) AND NOT EXISTS(SELECT 1 FROM track_custom_tags l JOIN custom_tags v ON v.id=l.tag_id JOIN json_each(json_extract(:curation,'$.empty_groups')) f ON f.value=v.category_id WHERE l.track_id=t.id) AND (json_extract(:curation,'$.user_rating_min') IS NULL OR COALESCE((SELECT json_extract(committed_json,'$.user_rating') FROM curation_tracks WHERE track_id=t.id),0)>=json_extract(:curation,'$.user_rating_min')) AND (json_extract(:curation,'$.user_rating_max') IS NULL OR COALESCE((SELECT json_extract(committed_json,'$.user_rating') FROM curation_tracks WHERE track_id=t.id),0)<=json_extract(:curation,'$.user_rating_max')) AND instr(COALESCE((SELECT notes_search FROM curation_tracks WHERE track_id=t.id),''),COALESCE(json_extract(:curation,'$.notes'),''))>0";
