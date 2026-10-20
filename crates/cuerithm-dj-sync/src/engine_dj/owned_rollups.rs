use super::state::{Catalog, direct_members};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, params, params_from_iter, types::Value};
use std::collections::{BTreeMap, BTreeSet};

type Member = (i64, String);
pub(super) fn rebuild(
    db: &Connection,
    state: &Catalog,
    affected: &BTreeSet<i64>,
    known: &BTreeMap<String, BTreeSet<Member>>,
    uuid: &str,
) -> Result<()> {
    if affected.is_empty() {
        return Ok(());
    }
    let mut counts: BTreeMap<i64, BTreeMap<Member, i64>> =
        affected.iter().map(|id| (*id, BTreeMap::new())).collect();
    fn add(
        state: &Catalog,
        mut parent: i64,
        members: &BTreeSet<Member>,
        counts: &mut BTreeMap<i64, BTreeMap<Member, i64>>,
    ) -> Result<()> {
        let mut seen = BTreeSet::new();
        while parent != 0 {
            ensure!(seen.insert(parent), "Engine rollup parent cycle");
            if let Some(rows) = counts.get_mut(&parent) {
                for member in members {
                    *rows.entry(member.clone()).or_default() += 1;
                }
            }
            parent = state
                .ordinary
                .get(&parent)
                .context("Engine rollup parent is missing")?
                .parent;
        }
        Ok(())
    }
    for node in state.ordinary.values() {
        let members = direct_members(db, node.id)?.into_iter().collect();
        add(state, node.parent, &members, &mut counts)?;
    }
    let paths = state.path_ids()?;
    for smart in state.smart.values() {
        let Some(parent) = paths.get(&smart.parent).copied() else {
            continue;
        };
        let mut ancestor = parent;
        let mut needed = false;
        while ancestor != 0 {
            needed |= affected.contains(&ancestor);
            ancestor = state
                .ordinary
                .get(&ancestor)
                .context("Engine smart rollup parent missing")?
                .parent;
        }
        if !needed {
            continue;
        }
        let members = if let Some(members) = known.get(&smart.id) {
            members.clone()
        } else {
            evaluate(db,&smart.rules,uuid).with_context(||format!("Cannot safely refresh an unselected Engine smartlist '{}'; its rules are unsupported",smart.name))?
        };
        add(state, parent, &members, &mut counts)?;
    }
    for (folder, mut rows) in counts {
        let direct = direct_members(db, folder)?;
        for member in &direct {
            rows.remove(member);
        }
        // The last direct link may currently point into the soon-to-be-replaced rollup.
        db.execute("UPDATE PlaylistEntity SET nextEntityId=0 WHERE listId=?1 AND COALESCE(membershipReference,0)=0 AND nextEntityId NOT IN(SELECT id FROM PlaylistEntity WHERE listId=?1 AND COALESCE(membershipReference,0)=0)",[folder])?;
        db.execute(
            "DELETE FROM PlaylistEntity WHERE listId=?1 AND membershipReference>0",
            [folder],
        )?;
        let mut prior = None;
        let mut first = None;
        for ((track, uuid), count) in rows {
            db.execute("INSERT INTO PlaylistEntity(listId,trackId,databaseUuid,nextEntityId,membershipReference) VALUES(?1,?2,?3,0,?4)",params![folder,track,uuid,count])?;
            let id = db.last_insert_rowid();
            first.get_or_insert(id);
            if let Some(prior) = prior {
                db.execute(
                    "UPDATE PlaylistEntity SET nextEntityId=?1 WHERE id=?2",
                    params![id, prior],
                )?;
            }
            prior = Some(id);
        }
        if let Some(first) = first {
            db.execute("UPDATE PlaylistEntity SET nextEntityId=?1 WHERE listId=?2 AND COALESCE(membershipReference,0)=0 AND nextEntityId=0",params![first,folder])?;
        }
    }
    Ok(())
}
/// Bind all values and accept only known native column expressions/operators.
/// Unknown target-only extensions cause a conflict instead of losing their rollup.
fn evaluate(db: &Connection, rules: &str, uuid: &str) -> Result<BTreeSet<Member>> {
    let rules: serde_json::Value = serde_json::from_str(rules)?;
    let all = match rules["match"].as_str() {
        Some("all") => true,
        Some("one") => false,
        _ => anyhow::bail!("Unknown Engine smart conjunction"),
    };
    let mut conditions = Vec::new();
    let mut values = Vec::new();
    for rule in rules["rules"]
        .as_array()
        .context("Missing Engine smart predicates")?
    {
        let column = rule["col"]
            .as_str()
            .context("Missing Engine smart column")?;
        let numeric = matches!(
            column,
            "key"
                | "IFNULL(key,-1)"
                | "IFNULL(rating,0)"
                | "IFNULL(year,0)"
                | "IFNULL(bpmAnalyzed,0)"
                | "ROUND(bpmAnalyzed)"
                | "bpmAnalyzed"
                | "rating"
                | "year"
                | "length"
                | "dateAdded"
                | "NULLIF(dateAdded,0)"
                | "isAnalyzed"
                | "isPlayed"
                | "CAST(CAST(printf('%.14e',IFNULL(bpmAnalyzed,0)*100) AS REAL)+0.5 AS INTEGER)/100.0"
        );
        ensure!(
            numeric
                || matches!(
                    column,
                    "artist"
                        | "album"
                        | "title"
                        | "genre"
                        | "comment"
                        | "label"
                        | "composer"
                        | "remixer"
                        | "fileType"
                        | "streamingSource"
                ),
            "Unsupported Engine smart column"
        );
        let operator = rule["con"]
            .as_str()
            .context("Missing Engine smart operator")?;
        let param = rule["param"]
            .as_str()
            .context("Missing Engine smart parameter")?;
        if operator == "IN" {
            ensure!(
                matches!(column, "key" | "IFNULL(key,-1)"),
                "Unsupported Engine smart set column"
            );
            let list = param
                .strip_prefix('(')
                .and_then(|s| s.strip_suffix(')'))
                .context("Invalid Engine key set")?;
            let keys = list
                .split(',')
                .map(|s| s.trim().parse::<u8>())
                .collect::<std::result::Result<Vec<_>, _>>()?;
            ensure!(
                !keys.is_empty() && keys.len() <= 24 && keys.iter().all(|n| *n < 24),
                "Invalid Engine harmonic key set"
            );
            conditions.push(format!(
                "({column}) IN ({})",
                vec!["?"; keys.len()].join(",")
            ));
            values.extend(keys.into_iter().map(|n| Value::Integer(i64::from(n))));
        } else if matches!(operator, "BETWEEN" | "NOT BETWEEN") {
            ensure!(numeric, "Text Engine ranges are unsupported");
            let (first, last) = param
                .split_once(" AND ")
                .context("Invalid Engine numeric range")?;
            values.push(number(first)?);
            values.push(number(last)?);
            conditions.push(format!("({column}) {operator} ? AND ?"));
        } else {
            ensure!(
                matches!(
                    operator,
                    "=" | "!="
                        | "<>"
                        | ">"
                        | ">="
                        | "<"
                        | "<="
                        | "LIKE"
                        | "NOT LIKE"
                        | "BEGINSWITH"
                        | "ENDSWITH"
                ),
                "Unsupported Engine smart operator"
            );
            let op = if matches!(operator, "BEGINSWITH" | "ENDSWITH") {
                "LIKE"
            } else {
                operator
            };
            values.push(if numeric {
                number(param)?
            } else {
                let text = if let Some(value) =
                    param.strip_prefix('\'').and_then(|s| s.strip_suffix('\''))
                {
                    value.replace("''", "'")
                } else {
                    param.to_owned()
                };
                Value::Text(text)
            });
            conditions.push(format!("({column}) {op} ?"));
        }
    }
    let condition = if conditions.is_empty() {
        if all { "1".into() } else { "0".into() }
    } else {
        conditions.join(if all { " AND " } else { " OR " })
    };
    let ids = db
        .prepare(&format!("SELECT id FROM Track WHERE {condition}"))?
        .query_map(params_from_iter(values), |r| r.get::<_, i64>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(ids.into_iter().map(|id| (id, uuid.to_owned())).collect())
}
fn number(value: &str) -> Result<Value> {
    let value: f64 = value.parse()?;
    ensure!(value.is_finite(), "Non-finite Engine smart threshold");
    Ok(Value::Real(value))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_date_and_harmonic_key_rules_use_bound_values_and_exclude_missing_dates() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE Track(id INTEGER,key INTEGER,dateAdded INTEGER);INSERT INTO Track VALUES(1,1,1640995200),(2,1,0),(3,2,1640995200);").unwrap();
        let rules=serde_json::json!({"match":"all","rules":[{"col":"NULLIF(dateAdded,0)","con":"BETWEEN","param":"1640995200 AND 1641081599"},{"col":"key","con":"IN","param":"(1,3)"}]}).to_string();
        assert_eq!(
            evaluate(&db, &rules, "db").unwrap(),
            BTreeSet::from([(1, "db".into())])
        );
        let unsafe_rules=serde_json::json!({"match":"all","rules":[{"col":"key);DROP TABLE Track;--","con":"=","param":"1"}]}).to_string();
        assert!(evaluate(&db, &unsafe_rules, "db").is_err());
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM Track", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            3
        );
    }
}
