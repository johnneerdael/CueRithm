use crate::{DateUnit, EvaluationContext, FieldValue, Operator, RuleValue, SmartTrack};
use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Duration, LocalResult, Months, NaiveDateTime, Offset, TimeZone, Utc};
/// Resolve a calendar input using the same IANA gap/fold policy as date rules.
pub fn calendar_day_epoch(day: &str, time_zone: &str) -> Result<i64> {
    let date = chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d")?;
    ensure!(
        date.format("%Y-%m-%d").to_string() == day,
        "Use YYYY-MM-DD for calendar dates"
    );
    let context = EvaluationContext::new(0, time_zone)?;
    Ok(resolve(
        &context,
        date.and_hms_opt(0, 0, 0).context("Invalid calendar day")?,
    )?
    .timestamp_millis())
}
/// The calendar day (`YYYY-MM-DD`) of an instant in an IANA time zone, the inverse of [`calendar_day_epoch`].
pub fn epoch_to_calendar_day(time: i64, time_zone: &str) -> Result<String> {
    let context = EvaluationContext::new(0, time_zone)?;
    Ok(local(&context, time)?.format("%Y-%m-%d").to_string())
}
fn local(context: &EvaluationContext, time: i64) -> Result<DateTime<chrono_tz::Tz>> {
    Ok(DateTime::from_timestamp_millis(time)
        .context("Date outside supported range")?
        .with_timezone(&context.time_zone))
}
fn resolve(context: &EvaluationContext, time: NaiveDateTime) -> Result<DateTime<chrono_tz::Tz>> {
    match context.time_zone.from_local_datetime(&time) {
        LocalResult::Single(t) => Ok(t),
        LocalResult::Ambiguous(a, b) => Ok(a.min(b)),
        LocalResult::None => {
            // JavaScript calendar setters move gaps forward by their actual offset jump,
            // including whole-day changes such as Pacific/Apia in December 2011.
            let gap = chrono_tz::GapInfo::new(&time, &context.time_zone)
                .context("Calendar gap unavailable")?;
            let (_, before) = gap.begin.context("Calendar gap has no lower boundary")?;
            let utc = time
                .checked_sub_signed(Duration::seconds(i64::from(before.fix().local_minus_utc())))
                .context("Calendar offset overflow")?;
            Ok(DateTime::<Utc>::from_naive_utc_and_offset(utc, Utc)
                .with_timezone(&context.time_zone))
        }
    }
}
fn cutoff(context: &EvaluationContext, count: i64, unit: DateUnit) -> Result<i64> {
    let now = local(context, context.now_millis)?.naive_local();
    let naive = match unit {
        DateUnit::Days | DateUnit::Weeks => {
            let days = count
                .checked_mul(if unit == DateUnit::Weeks { 7 } else { 1 })
                .context("Calendar duration overflow")?;
            now.checked_sub_signed(Duration::try_days(days).context("Calendar duration overflow")?)
        }
        DateUnit::Months => {
            let months = Months::new(u32::try_from(count.unsigned_abs())?);
            if count >= 0 {
                now.checked_sub_months(months)
            } else {
                now.checked_add_months(months)
            }
        }
    }
    .context("Calendar subtraction overflow")?;
    Ok(resolve(context, naive)?.timestamp_millis())
}
pub(crate) fn matches(
    track: &SmartTrack,
    field: &str,
    op: Operator,
    value: &RuleValue,
    context: &EvaluationContext,
) -> Result<bool> {
    let timestamp = match track.value(field) {
        FieldValue::Null => None,
        FieldValue::Date(t) => Some(t),
        _ => anyhow::bail!("Non-date track field: {field}"),
    };
    use Operator::*;
    if let RuleValue::Relative { count, unit } = value {
        let found = match timestamp.filter(|t| *t != 0) {
            Some(t) if *count != 0 => t > cutoff(context, *count, *unit)?,
            _ => false,
        };
        return Ok(if op == DateNotRecent { !found } else { found });
    }
    if let RuleValue::DateRange { start, end } = value {
        let Some(t) = timestamp else {
            return Ok(false);
        };
        let day = local(context, t)?.date_naive();
        let a = local(context, *start)?.date_naive();
        let b = local(context, *end)?.date_naive();
        return Ok(day >= a.min(b) && day <= a.max(b));
    }
    let RuleValue::Date(query) = value else {
        unreachable!()
    };
    let Some(t) = timestamp.filter(|t| *t != 0) else {
        return Ok(op == DateNotEquals);
    };
    let day = local(context, t)?.date_naive();
    let chosen = local(context, *query)?.date_naive();
    Ok(match op {
        DateEquals => day == chosen,
        DateNotEquals => day != chosen,
        DateAfter => day >= chosen,
        DateBefore => day < chosen,
        _ => unreachable!(),
    })
}
