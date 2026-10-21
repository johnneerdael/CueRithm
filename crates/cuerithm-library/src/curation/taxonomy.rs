use super::storage::*;
use super::*;
use crate::{Library, model::now};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::BTreeSet;
fn name(value: &str) -> Result<String> {
    let value = value.trim();
    ensure!(
        !value.is_empty() && value.len() <= 256 && !value.contains('\0'),
        "Category names require1..256bytes without NUL"
    );
    Ok(value.into())
}
pub(super) fn catalog(service: &CurationService) -> Result<CurationCatalog> {
    let (revision, groups, values, mappings) = {
        let conn = service.library.conn()?;
        let revision = tax_revision(&conn)?;
        let mut query=conn.prepare("SELECT c.id,c.name,COALESCE(g.mode,'multiple'),COALESCE(g.position,0) FROM custom_tag_categories c LEFT JOIN curation_group_properties g ON g.category_id=c.id WHERE COALESCE(g.deleted,0)=0 ORDER BY COALESCE(g.position,0),c.name,c.id")?;
        let groups = query
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, u32>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut query=conn.prepare("SELECT t.id,t.category_id,t.name,COALESCE(v.position,0) FROM custom_tags t LEFT JOIN curation_value_properties v ON v.tag_id=t.id LEFT JOIN curation_group_properties g ON g.category_id=t.category_id WHERE COALESCE(v.deleted,0)=0 AND COALESCE(g.deleted,0)=0 ORDER BY COALESCE(v.position,0),t.name,t.id")?;
        let values = query
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, u32>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut query = conn.prepare("SELECT payload_json FROM curation_mappings ORDER BY id")?;
        let mappings = query
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        (revision, groups, values, mappings)
    };
    let groups = groups
        .into_iter()
        .map(|(id, name, mode, position)| CurationGroup {
            values: values
                .iter()
                .filter(|value| value.1 == id)
                .map(|(id, _, name, position)| CurationValue {
                    id: id.clone(),
                    name: name.clone(),
                    position: *position,
                })
                .collect(),
            id,
            name,
            mode: if mode == "single" {
                CurationGroupMode::Single
            } else {
                CurationGroupMode::Multiple
            },
            position,
        })
        .collect();
    let result=CurationCatalog{revision,groups,mappings:mappings.into_iter().map(|text|{ensure!(text.len()<=128*1024,"Curate mapping exceeds bound");Ok(serde_json::from_str(&text)?)}).collect::<Result<_>>()?,capabilities:CurationCapabilities{native_available:cfg!(feature="autotag")&&service.runtime.is_some(),containers:vec![CurationContainer::Id3,CurationContainer::Vorbis,CurationContainer::Mp4,CurationContainer::Asf],text_fields:vec![CurationTextField::Genre,CurationTextField::Style,CurationTextField::Grouping,CurationTextField::Label,CurationTextField::Version,CurationTextField::Composer,CurationTextField::Lyricist],native_dj_reason:Some("Native DJ category mappings require a tested adapter contract; file projections are available explicitly".into())}};
    json(&result)?;
    Ok(result)
}
pub(super) fn group_active(conn: &Connection, id: &str) -> Result<()> {
    ensure!(conn.query_row("SELECT EXISTS(SELECT 1 FROM custom_tag_categories c LEFT JOIN curation_group_properties g ON g.category_id=c.id WHERE c.id=?1 AND COALESCE(g.deleted,0)=0)",[id],|row|row.get::<_,bool>(0))?,"Unknown or retired category group");
    Ok(())
}
fn value_group(conn: &Connection, id: &str) -> Result<String> {
    conn.query_row("SELECT t.category_id FROM custom_tags t LEFT JOIN curation_value_properties v ON v.tag_id=t.id LEFT JOIN curation_group_properties g ON g.category_id=t.category_id WHERE t.id=?1 AND COALESCE(v.deleted,0)=0 AND COALESCE(g.deleted,0)=0",[id],|row|row.get(0)).context("Unknown or retired category value")
}
pub(super) fn target_key(mapping: &CurationMapping) -> Result<String> {
    Ok(match &mapping.target {
        CurationMappingTarget::Text { field, .. } => {
            format!("text:{}", serde_json::to_string(field)?)
        }
        CurationMappingTarget::Comment {
            language,
            description,
            ..
        } => format!("comment:{}:{}", language, description),
        CurationMappingTarget::Rating { owner } => format!("rating:{owner}"),
    })
}
fn validate_mapping(catalog: &CurationCatalog, mapping: &CurationMapping) -> Result<()> {
    valid_request(&mapping.id)?;
    ensure!(
        !mapping.containers.is_empty()
            && mapping.containers.len() <= 4
            && mapping.containers.iter().collect::<BTreeSet<_>>().len() == mapping.containers.len(),
        "Choose unique native container families"
    );
    if let CurationMappingSource::Group { group_id } = &mapping.source {
        ensure!(
            catalog.groups.iter().any(|group| group.id == *group_id),
            "Unknown mapped category"
        );
    }
    match (&mapping.source, &mapping.target) {
        (CurationMappingSource::Rating, CurationMappingTarget::Rating { owner }) => {
            ensure!(
                !owner.is_empty() && owner.len() <= 1024 && !owner.contains('\0'),
                "Choose a valid rating owner"
            );
            ensure!(
                mapping
                    .containers
                    .iter()
                    .all(|c| *c == CurationContainer::Id3)
                    || owner == "native",
                "Non-ID3 ratings require the native rating identity; use separate owner mappings"
            );
        }
        (_, CurationMappingTarget::Rating { .. }) | (CurationMappingSource::Rating, _) => {
            anyhow::bail!("Ratings map only to an explicit native rating owner")
        }
        (
            _,
            CurationMappingTarget::Comment {
                language,
                description,
                ..
            },
        ) => {
            ensure!(
                description.len() <= 1024 && !description.contains('\0'),
                "Invalid named comment description"
            );
            if mapping.containers.contains(&CurationContainer::Id3) {
                ensure!(
                    language.len() == 3 && language.bytes().all(|c| c.is_ascii_alphabetic()),
                    "ID3 comments require a3letter language"
                );
            }
            if mapping
                .containers
                .iter()
                .any(|container| *container != CurationContainer::Id3)
            {
                ensure!(
                    language.is_empty() && description.is_empty(),
                    "Named/language comments are ID3-specific; use separate container mappings"
                );
            }
        }
        _ => {}
    }
    if mapping.enabled {
        let key = target_key(mapping)?;
        for other in &catalog.mappings {
            if other.id != mapping.id
                && other.enabled
                && other
                    .containers
                    .iter()
                    .any(|container| mapping.containers.contains(container))
            {
                ensure!(
                    target_key(other)? != key,
                    "Native mapping collision: two sources claim the same field/comment/rating identity"
                );
            }
        }
    }
    Ok(())
}
fn referenced(
    conn: &Connection,
    id: &str,
    category: bool,
) -> Result<Vec<(String, cuerithm_smartlists::SmartDefinition)>> {
    let mut query = conn
        .prepare("SELECT id,definition_json FROM playlists WHERE kind='smartlist' AND deleted=0")?;
    let rows = query
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    rows.into_iter().filter_map(|(playlist,text)|{let parsed=serde_json::from_str::<cuerithm_smartlists::SmartDefinition>(&text);match parsed{Ok(definition)=>definition.rules.iter().any(|rule|{let is_category=matches!(rule.operator,cuerithm_smartlists::Operator::TagCategoryNone|cuerithm_smartlists::Operator::TagCategoryAtleastOneAnyCat|cuerithm_smartlists::Operator::TagCategoryAtleastOneAllCAt);is_category==category&&matches!(&rule.value,cuerithm_smartlists::RuleValue::Ids(ids)if ids.iter().any(|value|value==id))}).then_some(Ok((playlist,definition))),Err(error)=>Some(Err(error.into()))}}).collect()
}
fn replace_rules(
    conn: &Connection,
    id: &str,
    replacement: Option<&str>,
    category: bool,
) -> Result<()> {
    let rows = referenced(conn, id, category)?;
    ensure!(
        rows.is_empty() || replacement.is_some(),
        "Category is used by smart playlists; choose a replacement before retiring it"
    );
    if let Some(replacement) = replacement {
        for (playlist, mut definition) in rows {
            for rule in &mut definition.rules {
                let is_category = matches!(
                    rule.operator,
                    cuerithm_smartlists::Operator::TagCategoryNone
                        | cuerithm_smartlists::Operator::TagCategoryAtleastOneAnyCat
                        | cuerithm_smartlists::Operator::TagCategoryAtleastOneAllCAt
                );
                if is_category == category
                    && let cuerithm_smartlists::RuleValue::Ids(ids) = &mut rule.value
                {
                    for value in ids.iter_mut() {
                        if value == id {
                            *value = replacement.into()
                        }
                    }
                    let mut seen = BTreeSet::new();
                    ids.retain(|value| seen.insert(value.clone()));
                }
            }
            conn.execute(
                "UPDATE playlists SET definition_json=?1,last_edit_id=?3 WHERE id=?2",
                params![
                    serde_json::to_string(&definition)?,
                    playlist,
                    uuid::Uuid::new_v4().to_string()
                ],
            )?;
            conn.execute(
                "UPDATE meta SET value=value+1 WHERE key='playlist_revision'",
                [],
            )?;
        }
    }
    Ok(())
}
fn no_pending(conn: &Connection, ids: &[String]) -> Result<()> {
    for id in ids {
        ensure!(!conn.query_row("SELECT EXISTS(SELECT 1 FROM curation_tracks WHERE track_id=?1 AND (revision!=acknowledged_revision OR active_publication_id IS NOT NULL))",[id],|row|row.get::<_,bool>(0))?,"Finish or discard pending Curate changes before retiring assigned taxonomy");
    }
    Ok(())
}
fn affected(conn: &Connection, group: Option<&str>, value: Option<&str>) -> Result<Vec<String>> {
    let mut query=conn.prepare("SELECT DISTINCT l.track_id FROM track_custom_tags l JOIN custom_tags t ON t.id=l.tag_id WHERE (?1 IS NULL OR t.category_id=?1) AND (?2 IS NULL OR t.id=?2) UNION SELECT c.track_id FROM curation_tracks c,json_each(c.desired_json,'$.categories') g,json_each(g.value) v WHERE (?1 IS NULL OR g.key=?1) AND (?2 IS NULL OR v.value=?2)")?;
    Ok(query
        .query_map(params![group, value], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?)
}
fn mark_stale(conn: &Connection, source: Option<&CurationMappingSource>) -> Result<u64> {
    let affected = match source {
        Some(CurationMappingSource::Group { group_id }) => affected(conn, Some(group_id), None)?,
        Some(CurationMappingSource::Rating) => {
            let mut query=conn.prepare("SELECT track_id FROM curation_tracks WHERE json_extract(desired_json,'$.user_rating') IS NOT NULL")?;
            query
                .query_map([], |row| row.get(0))?
                .collect::<rusqlite::Result<Vec<String>>>()?
        }
        Some(CurationMappingSource::Notes) => {
            let mut query=conn.prepare("SELECT track_id FROM curation_tracks WHERE json_extract(desired_json,'$.notes')!=''")?;
            query
                .query_map([], |row| row.get(0))?
                .collect::<rusqlite::Result<Vec<String>>>()?
        }
        None => vec![],
    };
    let mut count = 0;
    for track in affected {
        conn.execute(
            "INSERT OR IGNORE INTO curation_native_stale VALUES(?1)",
            [&track],
        )?;
        conn.execute(
            "UPDATE curation_tracks SET native_stale=1 WHERE track_id=?1",
            [track],
        )?;
        count += 1;
    }
    Ok(count)
}
pub(super) fn edit(
    service: &CurationService,
    request: CurationTaxonomyRequest,
) -> Result<CurationTaxonomyReceipt> {
    valid_request(&request.request_id)?;
    let hash = digest(&request)?;
    if let Some(receipt) = mutation(&service.library, &request.request_id, "taxonomy", &hash)? {
        return Ok(serde_json::from_str(&receipt)?);
    }
    let catalog = catalog(service)?;
    ensure!(
        catalog.revision == request.expected_revision,
        "Category catalog changed; refresh before editing"
    );
    if let CurationTaxonomyChange::PutMapping { mapping } = &request.change {
        validate_mapping(&catalog, mapping)?;
    }
    let mut created = None;
    let mut changed_tracks = vec![];
    let mut stale = 0;
    let mut conn = service.library.conn()?;
    let tx = conn.transaction()?;
    ensure!(
        tax_revision(&tx)? == request.expected_revision,
        "Category catalog changed; refresh before editing"
    );
    match &request.change {
        CurationTaxonomyChange::CreateGroup { name: label, mode } => {
            let label = name(label)?;
            let existing: Option<String> = tx
                .query_row(
                    "SELECT category_id FROM curation_category_aliases WHERE name=?1",
                    [&label],
                    |row| row.get(0),
                )
                .optional()?;
            let id = if let Some(id) = existing {
                ensure!(tx.query_row("SELECT COALESCE((SELECT deleted FROM curation_group_properties WHERE category_id=?1),0)",[&id],|row|row.get::<_,bool>(0))?,"Category name already exists or belongs to a retained alias");
                id
            } else {
                let id = uuid::Uuid::new_v4().to_string();
                tx.execute(
                    "INSERT INTO custom_tag_categories(id,name)VALUES(?1,?2)",
                    params![id, label],
                )?;
                tx.execute(
                    "INSERT OR IGNORE INTO curation_category_aliases VALUES(?1,?2)",
                    params![label, id],
                )?;
                id
            };
            tx.execute(
                "UPDATE custom_tag_categories SET name=?1 WHERE id=?2",
                params![label, id],
            )?;
            tx.execute("INSERT INTO curation_group_properties(category_id,mode)VALUES(?1,?2) ON CONFLICT(category_id)DO UPDATE SET mode=excluded.mode,deleted=0",params![id,if *mode==CurationGroupMode::Single{"single"}else{"multiple"}])?;
            created = Some(id);
        }
        CurationTaxonomyChange::RenameGroup { id, name: label } => {
            group_active(&tx, id)?;
            let label = name(label)?;
            let collision: Option<String> = tx
                .query_row(
                    "SELECT category_id FROM curation_category_aliases WHERE name=?1",
                    [&label],
                    |row| row.get(0),
                )
                .optional()?;
            ensure!(
                collision.as_ref().is_none_or(|value| value == id),
                "Name belongs to another category or retained alias"
            );
            tx.execute(
                "INSERT OR IGNORE INTO curation_category_aliases VALUES(?1,?2)",
                params![label, id],
            )?;
            tx.execute(
                "UPDATE custom_tag_categories SET name=?1 WHERE id=?2",
                params![label, id],
            )?;
            changed_tracks = affected(&tx, Some(id), None)?;
            if catalog.mappings.iter().any(|mapping| {
                mapping.enabled
                    && mapping.source
                        == CurationMappingSource::Group {
                            group_id: id.clone(),
                        }
            }) {
                stale = mark_stale(
                    &tx,
                    Some(&CurationMappingSource::Group {
                        group_id: id.clone(),
                    }),
                )?;
            }
        }
        CurationTaxonomyChange::SetGroupMode { id, mode } => {
            group_active(&tx, id)?;
            if *mode == CurationGroupMode::Single {
                ensure!(!tx.query_row("SELECT EXISTS(SELECT 1 FROM curation_tracks c,json_each(c.desired_json,'$.categories') g WHERE g.key=?1 AND json_array_length(g.value)>1)",[id],|row|row.get::<_,bool>(0))?,"Pending Curate revisions have multiple values; reconcile them before switching to single selection");
            }
            if *mode == CurationGroupMode::Single {
                ensure!(!tx.query_row("SELECT EXISTS(SELECT 1 FROM track_custom_tags l JOIN custom_tags t ON t.id=l.tag_id WHERE t.category_id=?1 GROUP BY l.track_id HAVING COUNT(*)>1)",[id],|row|row.get::<_,bool>(0))?,"Existing tracks have multiple values; reconcile them before switching to single selection");
            }
            tx.execute("INSERT INTO curation_group_properties(category_id,mode)VALUES(?1,?2)ON CONFLICT(category_id)DO UPDATE SET mode=excluded.mode",params![id,if *mode==CurationGroupMode::Single{"single"}else{"multiple"}])?;
        }
        CurationTaxonomyChange::CreateValue {
            group_id,
            name: label,
        } => {
            group_active(&tx, group_id)?;
            let label = name(label)?;
            let existing: Option<String> = tx
                .query_row(
                    "SELECT tag_id FROM curation_value_aliases WHERE category_id=?1 AND name=?2",
                    params![group_id, label],
                    |row| row.get(0),
                )
                .optional()?;
            let id = if let Some(id) = existing {
                ensure!(tx.query_row("SELECT COALESCE((SELECT deleted FROM curation_value_properties WHERE tag_id=?1),0)",[&id],|row|row.get::<_,bool>(0))?,"Value name already exists or belongs to a retained alias");
                id
            } else {
                let id = uuid::Uuid::new_v4().to_string();
                tx.execute(
                    "INSERT INTO custom_tags(id,category_id,name)VALUES(?1,?2,?3)",
                    params![id, group_id, label],
                )?;
                tx.execute(
                    "INSERT OR IGNORE INTO curation_value_aliases VALUES(?1,?2,?3)",
                    params![group_id, label, id],
                )?;
                id
            };
            tx.execute(
                "UPDATE custom_tags SET name=?1 WHERE id=?2",
                params![label, id],
            )?;
            tx.execute("INSERT INTO curation_value_properties(tag_id)VALUES(?1)ON CONFLICT(tag_id)DO UPDATE SET deleted=0",[&id])?;
            created = Some(id);
        }
        CurationTaxonomyChange::RenameValue { id, name: label } => {
            let group = value_group(&tx, id)?;
            let label = name(label)?;
            let collision: Option<String> = tx
                .query_row(
                    "SELECT tag_id FROM curation_value_aliases WHERE category_id=?1 AND name=?2",
                    params![group, label],
                    |row| row.get(0),
                )
                .optional()?;
            ensure!(
                collision.as_ref().is_none_or(|value| value == id),
                "Name belongs to another value or retained alias"
            );
            tx.execute(
                "INSERT OR IGNORE INTO curation_value_aliases VALUES(?1,?2,?3)",
                params![group, label, id],
            )?;
            tx.execute(
                "UPDATE custom_tags SET name=?1 WHERE id=?2",
                params![label, id],
            )?;
            changed_tracks = affected(&tx, None, Some(id))?;
            if catalog.mappings.iter().any(|mapping| {
                mapping.enabled
                    && mapping.source
                        == CurationMappingSource::Group {
                            group_id: group.clone(),
                        }
            }) {
                stale = mark_stale(&tx, Some(&CurationMappingSource::Group { group_id: group }))?;
            }
        }
        CurationTaxonomyChange::DeleteValue { id, replacement_id } => {
            let group = value_group(&tx, id)?;
            ensure!(
                replacement_id.as_ref() != Some(id),
                "Choose another replacement value"
            );
            if let Some(replacement) = replacement_id {
                ensure!(
                    value_group(&tx, replacement)? == group,
                    "Replacement value must belong to the same group"
                );
            }
            changed_tracks = affected(&tx, None, Some(id))?;
            no_pending(&tx, &changed_tracks)?;
            replace_rules(&tx, id, replacement_id.as_deref(), false)?;
            if let Some(replacement) = replacement_id {
                tx.execute("INSERT OR IGNORE INTO track_custom_tags SELECT track_id,?1 FROM track_custom_tags WHERE tag_id=?2",params![replacement,id])?;
                tx.execute(
                    "UPDATE curation_value_aliases SET tag_id=?1 WHERE tag_id=?2",
                    params![replacement, id],
                )?;
            }
            tx.execute("DELETE FROM track_custom_tags WHERE tag_id=?1", [id])?;
            tx.execute("INSERT INTO curation_value_properties(tag_id,deleted)VALUES(?1,1)ON CONFLICT(tag_id)DO UPDATE SET deleted=1",[id])?;
        }
        CurationTaxonomyChange::DeleteGroup { id, replacement_id } => {
            group_active(&tx, id)?;
            ensure!(
                replacement_id.as_ref() != Some(id),
                "Choose another replacement category"
            );
            if let Some(replacement) = replacement_id {
                group_active(&tx, replacement)?;
                ensure!(
                    affected(&tx, Some(id), None)?.is_empty(),
                    "Move assigned values explicitly before replacing a nonempty group"
                );
            }
            changed_tracks = affected(&tx, Some(id), None)?;
            no_pending(&tx, &changed_tracks)?;
            replace_rules(&tx, id, replacement_id.as_deref(), true)?;
            let values = catalog
                .groups
                .iter()
                .find(|group| group.id == *id)
                .context("Missing category")?;
            for value in &values.values {
                replace_rules(&tx, &value.id, None, false)?;
            }
            tx.execute("DELETE FROM track_custom_tags WHERE tag_id IN (SELECT id FROM custom_tags WHERE category_id=?1)",[id])?;
            tx.execute("INSERT INTO curation_group_properties(category_id,deleted)VALUES(?1,1)ON CONFLICT(category_id)DO UPDATE SET deleted=1",[id])?;
        }
        CurationTaxonomyChange::PutMapping { mapping } => {
            let payload = json(mapping)?;
            tx.execute("INSERT INTO curation_mappings VALUES(?1,?2,?3)ON CONFLICT(id)DO UPDATE SET payload_json=excluded.payload_json,updated=excluded.updated",params![mapping.id,payload,now()])?;
            if let Some(previous) = catalog
                .mappings
                .iter()
                .find(|previous| previous.id == mapping.id)
            {
                stale += mark_stale(&tx, Some(&previous.source))?;
            }
            if mapping.enabled {
                stale += mark_stale(&tx, Some(&mapping.source))?;
            }
            created = Some(mapping.id.clone());
        }
        CurationTaxonomyChange::DeleteMapping { id } => {
            let old = catalog
                .mappings
                .iter()
                .find(|mapping| mapping.id == *id)
                .context("Unknown mapping")?;
            stale = mark_stale(&tx, Some(&old.source))?;
            tx.execute("DELETE FROM curation_mappings WHERE id=?1", [id])?;
        }
    }
    // Assignment removals are reflected in every already-acknowledged head. They
    // never rewrite pending intent, which was rejected above.
    if matches!(
        request.change,
        CurationTaxonomyChange::DeleteGroup { .. } | CurationTaxonomyChange::DeleteValue { .. }
    ) {
        for track in &changed_tracks {
            tx.execute(
                "INSERT OR IGNORE INTO curation_native_stale VALUES(?1)",
                [track],
            )?;
            let values = CurationValues {
                categories: categories(&tx, track)?,
                ..Default::default()
            };
            let existing: Option<String> = tx
                .query_row(
                    "SELECT committed_json FROM curation_tracks WHERE track_id=?1",
                    [track],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(text) = existing {
                let mut original: CurationValues = serde_json::from_str(&text)?;
                original.categories = values.categories;
                let text = json(&original)?;
                tx.execute("UPDATE curation_tracks SET desired_json=?1,committed_json=?1,revision=revision+1,acknowledged_revision=acknowledged_revision+1,native_stale=1,updated=?2 WHERE track_id=?3",params![text,now(),track])?;
            }
        }
    }
    tx.execute(
        "UPDATE meta SET value=value+1 WHERE key='curation_taxonomy_revision'",
        [],
    )?;
    Library::bump(&tx)?;
    let receipt = CurationTaxonomyReceipt {
        revision: tax_revision(&tx)?,
        created_id: created,
        affected_tracks: changed_tracks.len() as u64,
        reexport_required: stale,
    };
    save_mutation(
        &tx,
        &request.request_id,
        "taxonomy",
        &hash,
        &json(&receipt)?,
    )?;
    tx.commit()?;
    drop(conn);
    service.emit(CurationEvent {
        track_id: None,
        revision: receipt.revision,
        state: "taxonomy".into(),
    });
    service.wake();
    Ok(receipt)
}
