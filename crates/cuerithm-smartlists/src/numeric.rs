use crate::{FieldValue, Operator, RuleValue, SmartTrack};
use anyhow::{Result, ensure};
fn rounded(value: f64, digits: u32) -> Result<f64> {
    let scale = 10_f64.powi(digits as i32);
    let product = value.abs() * scale;
    ensure!(product.is_finite(), "Numeric rounding overflow");
    let precise: f64 = format!("{product:.14e}").parse()?;
    Ok(((precise + 0.5).floor() / scale).copysign(value))
}
pub(crate) fn matches(
    track: &SmartTrack,
    field: &str,
    op: Operator,
    value: &RuleValue,
) -> Result<bool> {
    let raw = track.value(field);
    let number = match raw {
        FieldValue::Number(n) => n,
        FieldValue::Null => 0.,
        _ => anyhow::bail!("Non-numeric track field: {field}"),
    };
    ensure!(number.is_finite(), "Nonfinite numeric field: {field}");
    let present = number != 0. || matches!(field, "cuepoints" | "tempomarkers");
    if matches!(op, Operator::BpmExactNumber | Operator::BpmNotExactNumber) {
        let exact = number != 0. && rounded(number, 3)?.fract() == 0.;
        return Ok(number != 0.
            && if op == Operator::BpmNotExactNumber {
                !exact
            } else {
                exact
            });
    }
    let n = if field == "bpm" && present {
        rounded(number, 2)?
    } else {
        number
    };
    use Operator::*;
    if let RuleValue::NumberRange { start, end } = value {
        return Ok(present && n >= start.min(*end) && n <= start.max(*end));
    }
    let RuleValue::Number(q) = value else {
        unreachable!()
    };
    Ok(match op {
        NumberEquals => n == *q,
        NumberNotEquals => n != *q,
        NumberGreaterThan => n > *q,
        NumberGreaterThanEquals => n >= *q,
        NumberLessThan => n < *q,
        NumberLessThanEquals => n <= *q,
        _ => unreachable!(),
    })
}
