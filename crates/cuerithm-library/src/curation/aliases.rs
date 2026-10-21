use anyhow::{Result, ensure};
use rusqlite::{Connection, OptionalExtension, params};
pub(crate) fn resolve_category_value(
    conn: &Connection,
    category: &str,
    value: &str,
    create: bool,
) -> Result<(String, String)> {
    let group:Option<(String,bool)>=conn.query_row("SELECT c.id,COALESCE(p.deleted,0) FROM curation_category_aliases a JOIN custom_tag_categories c ON c.id=a.category_id LEFT JOIN curation_group_properties p ON p.category_id=c.id WHERE a.name=?1",[category],|row|Ok((row.get(0)?,row.get(1)?))).optional()?;
    let category_id = if let Some((id, deleted)) = group {
        ensure!(
            !deleted,
            "Captured cleanup category was retired; review cleanup before retrying"
        );
        id
    } else if let Some(id) = conn
        .query_row(
            "SELECT id FROM custom_tag_categories WHERE name=?1",
            [category],
            |row| row.get::<_, String>(0),
        )
        .optional()?
    {
        conn.execute(
            "INSERT OR IGNORE INTO curation_category_aliases VALUES(?1,?2)",
            params![category, id],
        )?;
        id
    } else {
        ensure!(create, "Captured cleanup category is unavailable");
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO custom_tag_categories(id,name)VALUES(?1,?2)",
            params![id, category],
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO curation_category_aliases VALUES(?1,?2)",
            params![category, id],
        )?;
        id
    };
    ensure!(!conn.query_row("SELECT COALESCE((SELECT deleted FROM curation_group_properties WHERE category_id=?1),0)",[&category_id],|row|row.get::<_,bool>(0))?,"Captured cleanup category was retired; review cleanup before retrying");
    let tag:Option<(String,bool)>=conn.query_row("SELECT t.id,COALESCE(p.deleted,0) FROM curation_value_aliases a JOIN custom_tags t ON t.id=a.tag_id LEFT JOIN curation_value_properties p ON p.tag_id=t.id WHERE a.category_id=?1 AND a.name=?2",params![category_id,value],|row|Ok((row.get(0)?,row.get(1)?))).optional()?;
    let tag_id = if let Some((id, deleted)) = tag {
        ensure!(
            !deleted,
            "Captured cleanup value was retired; review cleanup before retrying"
        );
        id
    } else if let Some(id) = conn
        .query_row(
            "SELECT id FROM custom_tags WHERE category_id=?1 AND name=?2",
            params![category_id, value],
            |row| row.get::<_, String>(0),
        )
        .optional()?
    {
        conn.execute(
            "INSERT OR IGNORE INTO curation_value_aliases VALUES(?1,?2,?3)",
            params![category_id, value, id],
        )?;
        id
    } else {
        ensure!(create, "Captured cleanup value is unavailable");
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO custom_tags(id,category_id,name)VALUES(?1,?2,?3)",
            params![id, category_id, value],
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO curation_value_aliases VALUES(?1,?2,?3)",
            params![category_id, value, id],
        )?;
        id
    };
    ensure!(
        !conn.query_row(
            "SELECT COALESCE((SELECT deleted FROM curation_value_properties WHERE tag_id=?1),0)",
            [&tag_id],
            |row| row.get::<_, bool>(0)
        )?,
        "Captured cleanup value was retired; review cleanup before retrying"
    );
    Ok((category_id, tag_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;
    #[test]
    fn captured_cleanup_names_keep_stable_ids_across_legacy_rename_and_tombstone() {
        let temp = tempfile::tempdir().unwrap();
        let library = Library::open(&temp.path().join("library.sqlite")).unwrap();
        {
            let conn = library.conn().unwrap();
            conn.execute(
                "INSERT INTO custom_tag_categories(id,name)VALUES('mood','Old mood')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO custom_tags(id,category_id,name)VALUES('warm','mood','Warm')",
                [],
            )
            .unwrap();
            conn.execute(
                "UPDATE custom_tag_categories SET name='New mood' WHERE id='mood'",
                [],
            )
            .unwrap();
            conn.execute("UPDATE custom_tags SET name='Bright' WHERE id='warm'", [])
                .unwrap();
            assert_eq!(
                resolve_category_value(&conn, "Old mood", "Warm", false).unwrap(),
                ("mood".into(), "warm".into())
            );
        }
        let service = CurationService::new(library.clone(), None);
        service
            .edit_taxonomy(CurationTaxonomyRequest {
                request_id: "delete".into(),
                expected_revision: service.catalog().unwrap().revision,
                change: CurationTaxonomyChange::DeleteValue {
                    id: "warm".into(),
                    replacement_id: None,
                },
            })
            .unwrap();
        assert!(
            resolve_category_value(&library.conn().unwrap(), "Old mood", "Warm", true).is_err()
        );
        let restored = service
            .edit_taxonomy(CurationTaxonomyRequest {
                request_id: "restore".into(),
                expected_revision: service.catalog().unwrap().revision,
                change: CurationTaxonomyChange::CreateValue {
                    group_id: "mood".into(),
                    name: "Warm".into(),
                },
            })
            .unwrap();
        assert_eq!(restored.created_id.as_deref(), Some("warm"));
        assert_eq!(service.catalog().unwrap().groups[0].values[0].name, "Warm");
    }
}
