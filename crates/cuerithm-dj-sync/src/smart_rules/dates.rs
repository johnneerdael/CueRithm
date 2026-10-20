use super::*;
use chrono::{DateTime, NaiveDate};
fn day(timestamp: i64, zone: &str) -> CompileResult<NaiveDate> {
    let zone: chrono_tz::Tz = zone.parse().map_err(|_| "invalid calendar timezone")?;
    DateTime::from_timestamp_millis(timestamp)
        .map(|t| t.with_timezone(&zone).date_naive())
        .ok_or("date outside supported range".into())
}
pub(super) fn bounds(timestamp: i64, zone: &str) -> CompileResult<(i64, i64, NaiveDate)> {
    let day = day(timestamp, zone)?;
    let next = day.succ_opt().ok_or("date overflow")?;
    let resolve = |date: NaiveDate| {
        cuerithm_smartlists::calendar_day_epoch(&date.format("%Y-%m-%d").to_string(), zone)
            .map_err(|e| e.to_string())
    };
    Ok((resolve(day)?, resolve(next)?, day))
}
pub(super) fn virtual_dj(op: Op, value: &RuleValue, zone: &str) -> CompileResult<Vec<String>> {
    let RuleValue::Date(timestamp) = value else {
        return Err("VirtualDJ has no verified relative/range date translation".into());
    };
    let (_, _, day) = bounds(*timestamp, zone)?;
    let operator = match op {
        Op::DateEquals => "=",
        Op::DateNotEquals => "!=",
        Op::DateAfter => ">=",
        Op::DateBefore => "<",
        _ => return Err("VirtualDJ cannot preserve this date operator".into()),
    };
    Ok(vec![format!(
        "First Seen {operator} \"{}\"",
        day.format("%Y/%m/%d")
    )])
}
pub(super) fn engine(
    op: Op,
    value: &RuleValue,
    zone: &str,
) -> CompileResult<Vec<serde_json::Value>> {
    let RuleValue::Date(timestamp) = value else {
        return Err("Engine has no verified relative/range date translation".into());
    };
    let (start, next, _) = bounds(*timestamp, zone)?;
    let (con, param) = match op {
        Op::DateEquals => (
            "BETWEEN",
            format!(
                "{} AND {}",
                start.div_euclid(1000),
                next.div_euclid(1000) - 1
            ),
        ),
        Op::DateAfter => (">=", start.div_euclid(1000).to_string()),
        Op::DateBefore => ("<", start.div_euclid(1000).to_string()),
        _ => return Err("Engine cannot preserve this date operator".into()),
    };
    Ok(vec![
        json!({"col":"NULLIF(dateAdded,0)","con":con,"param":param,"v":"2.20.0"}),
    ])
}
pub(super) fn serato(
    op: Op,
    value: &RuleValue,
    zone: &str,
) -> CompileResult<Vec<serde_json::Value>> {
    let RuleValue::Date(timestamp) = value else {
        return Err("Serato has no verified relative/range date translation".into());
    };
    let (_, _, day) = bounds(*timestamp, zone)?;
    let (operation, date) = match op {
        // The reference mapping does not establish whether native AFTER includes
        // the selected calendar day. Keep exact captured membership.
        Op::DateAfter => {
            return Err("Serato AFTER calendar-day inclusivity is not established".into());
        }
        Op::DateBefore => ("BEFORE", day),
        _ => return Err("Serato only exports before/after date rules".into()),
    };
    Ok(vec![
        json!({"attribute":"time_added","operation":operation,"type":"TIME","version":1,"partial_date":date.format("%Y%m%d").to_string()}),
    ])
}
pub(super) fn djay(op: Op, value: &RuleValue, zone: &str) -> CompileResult<DjayFilter> {
    const EPOCH: f64 = 978307200.0;
    let cf = |millis: i64| millis as f64 / 1000.0 - EPOCH;
    let (operator, date) = match value {
        RuleValue::Relative { count, unit } if *count > 0 && *count <= i32::MAX as i64 => {
            let operator = match op {
                Op::DateRecent => 4,
                Op::DateNotRecent => 5,
                _ => return Err("invalid relative date rule".into()),
            };
            let period = match unit {
                cuerithm_smartlists::DateUnit::Days => 0,
                cuerithm_smartlists::DateUnit::Weeks => 1,
                cuerithm_smartlists::DateUnit::Months => 2,
            };
            (
                operator,
                DjayValue::Date {
                    start: None,
                    end: None,
                    relative: Some((*count, period)),
                },
            )
        }
        RuleValue::Date(timestamp) => {
            let (start, next, _) = bounds(*timestamp, zone)?;
            let start = cf(start);
            let end = cf(next).next_down();
            match op {
                Op::DateEquals => (
                    6,
                    DjayValue::Date {
                        start: Some(start),
                        end: Some(end),
                        relative: None,
                    },
                ),
                // Use the immediate predecessor, not an approximate time epsilon.
                Op::DateAfter => (
                    2,
                    DjayValue::Date {
                        start: Some(start.next_down()),
                        end: None,
                        relative: None,
                    },
                ),
                Op::DateBefore => (
                    3,
                    DjayValue::Date {
                        start: Some(start),
                        end: None,
                        relative: None,
                    },
                ),
                _ => {
                    return Err(
                        "djay cannot express calendar-day inequality in one conjunctive filter"
                            .into(),
                    );
                }
            }
        }
        RuleValue::DateRange { start, end } => {
            let a = day(*start, zone)?;
            let b = day(*end, zone)?;
            let low = cuerithm_smartlists::calendar_day_epoch(
                &a.min(b).format("%Y-%m-%d").to_string(),
                zone,
            )
            .map_err(|e| e.to_string())?;
            let high = bounds(if a > b { *start } else { *end }, zone)?.1;
            (
                6,
                DjayValue::Date {
                    start: Some(cf(low)),
                    end: Some(cf(high).next_down()),
                    relative: None,
                },
            )
        }
        _ => return Err("djay cannot preserve this date period".into()),
    };
    Ok(DjayFilter {
        field: "addedDate",
        name: "Added",
        multiple: false,
        operator,
        value: date,
    })
}
