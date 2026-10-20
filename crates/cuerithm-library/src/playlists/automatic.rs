use super::*;
use cuerithm_smartlists::{Operator, RuleValue, SmartRule};
use rusqlite::OptionalExtension;
use std::collections::BTreeMap;

fn rule(field: &str, operator: Operator, value: RuleValue) -> SmartRule {
    SmartRule {
        id: field.into(),
        field: field.into(),
        operator,
        value,
        or: false,
    }
}
fn definition(rules: Vec<SmartRule>) -> Option<SmartDefinition> {
    Some(SmartDefinition {
        match_all: true,
        rules,
    })
}
struct Builder {
    tree: Tree,
    managed: BTreeMap<String, String>,
    added: Vec<(String, String)>,
}
impl Builder {
    fn node(
        &mut self,
        key: &str,
        parent: Option<&str>,
        name: &str,
        definition: Option<SmartDefinition>,
    ) -> String {
        if let Some(id) = self.managed.get(key) {
            // Preserve names and organization while enabled categories retain
            // ownership of their rules. Deleted nodes are explicit opt-outs.
            if let Some(row) = self.tree.get_mut(id)
                && !row.node.deleted
                && row.node.kind == PlaylistKind::Smartlist
                && row.node.definition != definition
            {
                row.node.definition = definition;
                // Keep the user edit identity so Undo can distinguish this rule
                // repair from any later user edit, including an ABA change.
            }
            return id.clone();
        }
        let id = uuid::Uuid::new_v4().to_string();
        let position = tree::children(&self.tree, &parent.map(str::to_owned), true).len() as u32;
        self.tree.insert(
            id.clone(),
            StoredNode {
                last_edit_id: "automatic".into(),
                members: vec![],
                node: PlaylistNode {
                    id: id.clone(),
                    parent_id: parent.map(str::to_owned),
                    kind: if definition.is_some() {
                        PlaylistKind::Smartlist
                    } else {
                        PlaylistKind::Folder
                    },
                    name: name.chars().take(255).collect(),
                    position,
                    favorite_position: None,
                    definition,
                    deleted: false,
                    deleted_by: None,
                },
            },
        );
        self.managed.insert(key.into(), id.clone());
        self.added.push((key.into(), id.clone()));
        id
    }
    fn active(&self, id: &str) -> bool {
        self.tree.get(id).is_some_and(|r| !r.node.deleted)
    }
}

/// Reconcile only observed grouping values. This runs inside the caller's write
/// transaction, so no captured metadata or preference revision can become stale.
pub(super) fn reconcile_in_transaction(conn: &Connection) -> Result<()> {
    let stamp = preferences::stamp(conn)?;
    let previous: Option<(u64, u64, u64)> = conn.query_row("SELECT input_revision,preference_revision,playlist_revision FROM automatic_playlist_state WHERE singleton=1", [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
    if previous
        == Some((
            stamp.input_revision,
            stamp.preference_revision,
            stamp.playlist_revision,
        ))
    {
        return Ok(());
    }
    let settings = preferences::preferences(conn)?.preferences.automatic;
    let before = load(conn)?;
    let managed = conn
        .prepare("SELECT managed_key,playlist_id FROM automatic_playlist_nodes")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<BTreeMap<String, String>>>()?;
    let mut builder = Builder {
        tree: before.clone(),
        managed,
        added: vec![],
    };
    if settings.artists
        || settings.bpm
        || settings.release_dates
        || settings.genres
        || settings.keys
    {
        let root = builder.node("root", None, "Automatic Smart Playlists", None);
        if builder.active(&root) {
            let mut artists = BTreeMap::<String, String>::new();
            let mut genres = BTreeMap::<String, String>::new();
            let mut months = BTreeSet::new();
            if settings.artists || settings.genres || settings.release_dates {
                let mut statement = conn.prepare("SELECT artist,title,genre,metadata_json,native_metadata_json FROM tracks WHERE state='ready' AND archived=0 ORDER BY id")?;
                let rows = statement.query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, Option<String>>(3)?,
                        r.get::<_, Option<String>>(4)?,
                    ))
                })?;
                for row in rows {
                    let (mut artist, mut title, genre, info, native) = row?;
                    let info = info
                        .and_then(|s| serde_json::from_str::<cuerithm_media::MediaInfo>(&s).ok());
                    let tags = info.map(|v| v.tags).unwrap_or_default();
                    let mut remixer = tags.get("remixer").cloned().unwrap_or_default();
                    let mut track_genres = vec![genre];
                    if let Some(native) = native {
                        let metadata: cuerithm_cleanup::Metadata = serde_json::from_str(&native)?;
                        metadata.validate()?;
                        use cuerithm_cleanup::MetadataField as F;
                        if let Some(values) = metadata.fields.get(&F::Artist) {
                            artist = values.join("; ");
                        }
                        if let Some(values) = metadata.fields.get(&F::Title) {
                            title = values.join("; ");
                        }
                        if let Some(values) = metadata.fields.get(&F::Remixer) {
                            remixer = values.join("; ");
                        }
                        if let Some(values) = metadata.fields.get(&F::Genre) {
                            track_genres = values.clone();
                        }
                    }
                    if settings.artists {
                        for name in credits::artists(&artist, &remixer, &title) {
                            artists.entry(name.to_lowercase()).or_insert(name);
                        }
                    }
                    if settings.genres {
                        for name in credits::unique(
                            track_genres
                                .iter()
                                .flat_map(|g| g.split(';').map(str::to_owned)),
                        ) {
                            genres.entry(name.to_lowercase()).or_insert(name);
                        }
                    }
                    if settings.release_dates
                        && let Some(month) = credits::release_month(&tags)
                    {
                        months.insert(month);
                    }
                }
            }
            // Retained artist/genre/month lists stay managed even when their
            // current grouping value disappears from the prepared library.
            for key in builder.managed.keys() {
                if settings.artists
                    && let Some(name) = key.strip_prefix("artists/")
                {
                    artists.entry(name.into()).or_insert_with(|| name.into());
                }
                if settings.genres
                    && let Some(name) = key.strip_prefix("genres/")
                {
                    genres.entry(name.into()).or_insert_with(|| name.into());
                }
                if settings.release_dates
                    && let Some((month, _)) =
                        key.strip_prefix("dates/").and_then(|s| s.split_once('/'))
                    && let Ok(month) = month.parse::<u32>()
                {
                    months.insert(month);
                }
            }
            for (enabled, key, name, field, names) in [
                (
                    settings.artists,
                    "artists",
                    "Artists",
                    "artistCredit",
                    artists,
                ),
                (settings.genres, "genres", "Genres", "genreCredit", genres),
            ] {
                if !enabled {
                    continue;
                }
                let folder = builder.node(key, Some(&root), name, None);
                if !builder.active(&folder) {
                    continue;
                }
                for (canonical, name) in names {
                    builder.node(
                        &format!("{key}/{canonical}"),
                        Some(&folder),
                        &name,
                        definition(vec![rule(
                            field,
                            Operator::StringEquals,
                            RuleValue::Text(name.clone()),
                        )]),
                    );
                }
            }
            if settings.bpm {
                let folder = builder.node("bpm", Some(&root), "BPM", None);
                if builder.active(&folder) {
                    for range in settings.bpm_ranges {
                        let name = format!("{}–{} BPM", range.start, range.end);
                        builder.node(
                            &format!("bpm/{}/{}", range.start, range.end),
                            Some(&folder),
                            &name,
                            definition(vec![rule(
                                "bpm",
                                Operator::NumberBetween,
                                RuleValue::NumberRange {
                                    start: range.start,
                                    end: range.end,
                                },
                            )]),
                        );
                    }
                }
            }
            if settings.keys {
                let folder = builder.node("keys", Some(&root), "Keys", None);
                if builder.active(&folder) {
                    for number in 1..=12 {
                        for mode in ['A', 'B'] {
                            let key = format!("{number}{mode}");
                            builder.node(
                                &format!("keys/{key}"),
                                Some(&folder),
                                &key,
                                definition(vec![rule(
                                    "key",
                                    Operator::KeySimilar,
                                    RuleValue::Text(key.clone()),
                                )]),
                            );
                        }
                    }
                }
            }
            if settings.release_dates {
                let folder = builder.node("dates", Some(&root), "Release dates", None);
                if builder.active(&folder) {
                    const MONTHS: [&str; 12] = [
                        "January",
                        "February",
                        "March",
                        "April",
                        "May",
                        "June",
                        "July",
                        "August",
                        "September",
                        "October",
                        "November",
                        "December",
                    ];
                    for month in months {
                        let year = month / 100;
                        let name = MONTHS[(month % 100 - 1) as usize];
                        let year_id = builder.node(
                            &format!("dates/{year}"),
                            Some(&folder),
                            &year.to_string(),
                            None,
                        );
                        if !builder.active(&year_id) {
                            continue;
                        }
                        let month_id =
                            builder.node(&format!("dates/{month}"), Some(&year_id), name, None);
                        if !builder.active(&month_id) {
                            continue;
                        }
                        for (prefix, energy) in [
                            ("", None),
                            ("Warm-Up ", Some((1., 5.))),
                            ("Dancefloor ", Some((6., 7.))),
                            ("Anthems ", Some((8., 10.))),
                        ] {
                            let mut rules = vec![rule(
                                "releaseMonth",
                                Operator::NumberEquals,
                                RuleValue::Number(month.into()),
                            )];
                            if let Some((start, end)) = energy {
                                rules.push(rule(
                                    "energy",
                                    Operator::NumberBetween,
                                    RuleValue::NumberRange { start, end },
                                ));
                            }
                            builder.node(
                                &format!("dates/{month}/{prefix}"),
                                Some(&month_id),
                                &format!("{prefix}{name}, {year}"),
                                definition(rules),
                            );
                        }
                    }
                }
            }
        }
    }
    tree::normalize(&mut builder.tree);
    tree::validate(&builder.tree, &tree::known_tracks(conn)?)?;
    let (a, b) = diff(&before, &builder.tree);
    if !a.is_empty() {
        persist(conn, &a, &b)?;
    }
    for (key, id) in builder.added {
        conn.execute(
            "INSERT INTO automatic_playlist_nodes VALUES(?1,?2)",
            params![key, id],
        )?;
    }
    let stamp = preferences::stamp(conn)?;
    conn.execute("INSERT INTO automatic_playlist_state VALUES(1,?1,?2,?3) ON CONFLICT(singleton) DO UPDATE SET input_revision=excluded.input_revision,preference_revision=excluded.preference_revision,playlist_revision=excluded.playlist_revision",params![stamp.input_revision,stamp.preference_revision,stamp.playlist_revision])?;
    Ok(())
}
impl Library {
    pub fn reconcile_automatic_playlists(&self) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        reconcile_in_transaction(&tx)?;
        tx.commit()?;
        Ok(())
    }
}
