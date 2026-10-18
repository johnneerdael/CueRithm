use super::*;
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Template {
    pub name: String,
    pub definition: SmartDefinition,
}
pub(super) fn definition(mut rules: Vec<SmartRule>) -> SmartDefinition {
    for (i, rule) in rules.iter_mut().enumerate() {
        rule.id = format!("generated-{i}");
    }
    SmartDefinition {
        match_all: true,
        rules,
    }
}
fn rule(field: &str, operator: Operator, value: RuleValue) -> SmartRule {
    SmartRule {
        id: String::new(),
        field: field.into(),
        operator,
        value,
        or: false,
    }
}
pub(super) fn scalar(field: &str, values: Vec<FieldValue>) -> Result<Vec<Template>> {
    values
        .into_iter()
        .map(|value| {
            let (name, operator, value) = match value {
                FieldValue::Number(n) => {
                    ensure!(n.is_finite(), "Nonfinite generation value");
                    let name = if field == "energy" {
                        format!("{:02} Energy", n.round().clamp(0., 10.) as u32)
                    } else {
                        n.to_string()
                    };
                    (name, Operator::NumberEquals, RuleValue::Number(n))
                }
                FieldValue::Text(s) => (s.clone(), Operator::StringEquals, RuleValue::Text(s)),
                FieldValue::Color(s) => {
                    (s.clone(), Operator::ColorEquals, RuleValue::Color(Some(s)))
                }
                _ => anyhow::bail!("Unsupported scalar generation value"),
            };
            Ok(Template {
                name,
                definition: definition(vec![rule(field, operator, value)]),
            })
        })
        .collect()
}
pub(super) fn ranges(field: &str, values: &[f64], decade: bool, tail: f64) -> Vec<Template> {
    let starts = values
        .iter()
        .filter(|v| v.is_finite() && **v > 0.)
        .map(|v| ((v / 10.).floor() * 10.).to_bits())
        .collect::<BTreeSet<_>>();
    starts
        .into_iter()
        .map(|bits| {
            let start = f64::from_bits(bits);
            let end = start + 10. - tail;
            Template {
                name: if decade {
                    format!("{start}s")
                } else {
                    format!("{start} - {end}")
                },
                definition: definition(vec![rule(
                    field,
                    Operator::NumberBetween,
                    RuleValue::NumberRange { start, end },
                )]),
            }
        })
        .collect()
}
pub(super) fn buckets(
    field: &str,
    values: &[f64],
    cuts: &[f64],
    scale: f64,
    unit: &str,
) -> Vec<Template> {
    let mut items = vec![];
    for i in 0..=cuts.len() {
        let lower = i.checked_sub(1).map(|j| cuts[j]);
        let upper = cuts.get(i).copied();
        if !values.iter().any(|v| {
            v.is_finite() && lower.map_or(*v > 0., |l| *v >= l) && upper.is_none_or(|h| *v < h)
        }) {
            continue;
        }
        let name = match (lower, upper) {
            (None, Some(h)) => format!("< {} {unit}", h / scale),
            (Some(l), None) => format!("{} {unit}+", l / scale),
            (Some(l), Some(h)) => format!("{}-{} {unit}", l / scale, h / scale),
            _ => unreachable!(),
        };
        let mut rules = vec![rule(
            field,
            if lower.is_some() {
                Operator::NumberGreaterThanEquals
            } else {
                Operator::NumberGreaterThan
            },
            RuleValue::Number(lower.unwrap_or(0.)),
        )];
        if let Some(high) = upper {
            rules.push(rule(
                field,
                Operator::NumberLessThan,
                RuleValue::Number(high),
            ));
        }
        items.push(Template {
            name,
            definition: definition(rules),
        });
    }
    items
}
pub(super) fn tags(ids: &[String], labels: &BTreeMap<String, String>) -> Vec<Template> {
    ids.iter()
        .filter_map(|id| {
            labels.get(id).map(|name| Template {
                name: name.clone(),
                definition: definition(vec![rule(
                    "tags",
                    Operator::TagsAtleastOne,
                    RuleValue::Ids(vec![id.clone()]),
                )]),
            })
        })
        .collect()
}
pub(super) fn and(left: &Template, right: &Template) -> Template {
    Template {
        name: format!("{} {}", left.name, right.name),
        definition: definition(
            left.definition
                .rules
                .iter()
                .chain(&right.definition.rules)
                .cloned()
                .collect(),
        ),
    }
}
