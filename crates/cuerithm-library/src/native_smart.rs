//! CueRithm smart rules as rekordbox intelligent-playlist rules.
//!
//! rekordbox supports a subset: a fixed list of fields with equality, text, number and date comparisons. The
//! mapping follows the reference exporter's tables, and a list is written natively only when every rule maps
//! exactly; otherwise it is written as an ordinary playlist of its current members.
use anyhow::Result;
use cuerithm_rekordbox::{SmartCondition, SmartRuleSet};
use cuerithm_smartlists::{DateUnit, Operator, RuleValue, SmartDefinition, epoch_to_calendar_day};

/// Track colours in rekordbox's order; a colour rule stores the 1-based position.
fn color_index(name: &str) -> Option<usize> {
    [
        "pink", "red", "orange", "yellow", "green", "aqua", "blue", "purple",
    ]
    .iter()
    .position(|c| {
        let name = name.to_lowercase();
        name == *c || (*c == "pink" && name == "magenta") || (*c == "purple" && name == "violet")
    })
    .map(|i| i + 1)
}
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Text,
    Number,
    Date,
    Color,
}
fn property(field: &str) -> Option<(&'static str, Kind)> {
    Some(match field {
        "title" => ("name", Kind::Text),
        "artist" => ("artist", Kind::Text),
        "album" => ("album", Kind::Text),
        "genre" => ("genre", Kind::Text),
        "comment" => ("comments", Kind::Text),
        "label" => ("label", Kind::Text),
        "remixer" => ("remixedBy", Kind::Text),
        "mix" => ("mixName", Kind::Text),
        "composer" => ("producer", Kind::Text),
        "key" => ("key", Kind::Text),
        "location" => ("fileName", Kind::Text),
        "bpm" => ("bpm", Kind::Number),
        "duration" => ("duration", Kind::Number),
        "year" => ("year", Kind::Number),
        "rating" => ("rating", Kind::Number),
        "dateAdded" => ("stockDate", Kind::Date),
        "color" => ("grouping", Kind::Color),
        _ => return None,
    })
}
fn operator(kind: Kind, op: Operator) -> Option<&'static str> {
    use Operator::*;
    Some(match (kind, op) {
        (Kind::Text, StringEquals) => "1",
        (Kind::Text, StringNotEquals) => "2",
        (Kind::Text, StringContains) => "8",
        (Kind::Text, StringNotContains) => "9",
        (Kind::Text, StringStartsWith) => "10",
        (Kind::Text, StringEndsWith) => "11",
        (Kind::Number, NumberEquals) => "1",
        (Kind::Number, NumberNotEquals) => "2",
        (Kind::Number, NumberGreaterThan) => "3",
        (Kind::Number, NumberLessThan) => "4",
        (Kind::Number, NumberBetween) => "5",
        (Kind::Date, DateEquals) => "1",
        (Kind::Date, DateNotEquals) => "2",
        (Kind::Date, DateAfter) => "3",
        (Kind::Date, DateBefore) => "4",
        (Kind::Date, DateBetween) => "5",
        (Kind::Date, DateRecent) => "6",
        (Kind::Date, DateNotRecent) => "7",
        (Kind::Color, ColorEquals) | (Kind::Color, ExistsNot) => "1",
        (Kind::Color, ColorNotEquals) => "2",
        _ => return None,
    })
}
fn number(value: f64, hundredths: bool) -> Option<String> {
    let value = if hundredths {
        (value * 100.).round()
    } else {
        value
    };
    value.is_finite().then(|| {
        if value.fract() == 0. {
            format!("{}", value as i64)
        } else {
            format!("{value}")
        }
    })
}

/// The rekordbox rules for a definition, or `None` when any part of it cannot be represented exactly.
/// `time_zone` turns calendar dates into the day rekordbox compares against.
pub fn rekordbox_rules(
    definition: &SmartDefinition,
    time_zone: &str,
) -> Result<Option<SmartRuleSet>> {
    if definition.rules.is_empty()
        || definition.rules.len() > 64
        || definition.rules.iter().any(|r| r.or)
    {
        return Ok(None);
    }
    let mut conditions = vec![];
    for rule in &definition.rules {
        let Some((property, kind)) = property(&rule.field) else {
            return Ok(None);
        };
        let Some(op) = operator(kind, rule.operator) else {
            return Ok(None);
        };
        let hundredths = property == "bpm";
        let (left, right, unit) = match (&rule.value, kind) {
            (RuleValue::Text(text), Kind::Text) => (text.clone(), String::new(), String::new()),
            (RuleValue::Number(n), Kind::Number) => match number(*n, hundredths) {
                Some(n) => (
                    n,
                    String::new(),
                    if hundredths {
                        "0".into()
                    } else {
                        String::new()
                    },
                ),
                None => return Ok(None),
            },
            (RuleValue::NumberRange { start, end }, Kind::Number) => {
                match (number(*start, hundredths), number(*end, hundredths)) {
                    (Some(a), Some(b)) => (
                        a,
                        b,
                        if hundredths {
                            "0".into()
                        } else {
                            String::new()
                        },
                    ),
                    _ => return Ok(None),
                }
            }
            (RuleValue::Date(ms), Kind::Date) => (
                epoch_to_calendar_day(*ms, time_zone)?,
                String::new(),
                String::new(),
            ),
            (RuleValue::DateRange { start, end }, Kind::Date) => (
                epoch_to_calendar_day(*start, time_zone)?,
                epoch_to_calendar_day(*end, time_zone)?,
                String::new(),
            ),
            (RuleValue::Relative { count, unit }, Kind::Date) => match unit {
                DateUnit::Days => (count.to_string(), String::new(), "day".into()),
                DateUnit::Weeks => ((count * 7).to_string(), String::new(), "day".into()),
                DateUnit::Months => (count.to_string(), String::new(), "month".into()),
            },
            (RuleValue::Color(Some(name)), Kind::Color) => match color_index(name) {
                Some(index) => (index.to_string(), String::new(), String::new()),
                None => return Ok(None),
            },
            // "Has no colour" is the equals rule against colour zero.
            (RuleValue::None | RuleValue::Color(None), Kind::Color)
                if rule.operator == Operator::ExistsNot =>
            {
                ("0".into(), String::new(), String::new())
            }
            _ => return Ok(None),
        };
        conditions.push(SmartCondition {
            operator: op.into(),
            property: property.into(),
            left,
            right,
            unit,
        });
    }
    Ok(Some(SmartRuleSet {
        match_all: definition.match_all,
        conditions,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuerithm_smartlists::SmartRule;

    fn rule(field: &str, operator: Operator, value: RuleValue) -> SmartRule {
        SmartRule {
            id: "r".into(),
            field: field.into(),
            operator,
            value,
            or: false,
        }
    }
    fn definition(match_all: bool, rules: Vec<SmartRule>) -> SmartDefinition {
        SmartDefinition { match_all, rules }
    }
    fn conditions(rules: Vec<SmartRule>) -> Vec<(String, String, String, String, String)> {
        rekordbox_rules(&definition(true, rules), "UTC")
            .unwrap()
            .expect("supported")
            .conditions
            .into_iter()
            .map(|c| (c.property, c.operator, c.left, c.right, c.unit))
            .collect()
    }

    #[test]
    fn text_number_and_bpm_rules_use_the_reference_properties_operators_and_scale() {
        let got = conditions(vec![
            rule(
                "artist",
                Operator::StringContains,
                RuleValue::Text("Daft".into()),
            ),
            rule(
                "composer",
                Operator::StringEquals,
                RuleValue::Text("X".into()),
            ),
            rule(
                "title",
                Operator::StringStartsWith,
                RuleValue::Text("A".into()),
            ),
            rule(
                "remixer",
                Operator::StringNotContains,
                RuleValue::Text("Z".into()),
            ),
            rule(
                "bpm",
                Operator::NumberBetween,
                RuleValue::NumberRange {
                    start: 120.,
                    end: 128.5,
                },
            ),
            rule("rating", Operator::NumberGreaterThan, RuleValue::Number(3.)),
            rule("year", Operator::NumberLessThan, RuleValue::Number(2000.)),
        ]);
        let t = |p: &str, o: &str, l: &str, r: &str, u: &str| {
            (
                p.to_owned(),
                o.to_owned(),
                l.to_owned(),
                r.to_owned(),
                u.to_owned(),
            )
        };
        assert_eq!(
            got,
            [
                t("artist", "8", "Daft", "", ""),
                t("producer", "1", "X", "", ""),
                t("name", "10", "A", "", ""),
                t("remixedBy", "9", "Z", "", ""),
                t("bpm", "5", "12000", "12850", "0"),
                t("rating", "3", "3", "", ""),
                t("year", "4", "2000", "", ""),
            ]
        );
    }

    #[test]
    fn dates_become_calendar_days_and_relative_dates_use_days_or_months() {
        // 2024-03-10 00:00 in New York is 05:00 UTC.
        let ms = 1_710_046_800_000;
        let rules = vec![
            rule("dateAdded", Operator::DateAfter, RuleValue::Date(ms)),
            rule(
                "dateAdded",
                Operator::DateRecent,
                RuleValue::Relative {
                    count: 2,
                    unit: DateUnit::Weeks,
                },
            ),
            rule(
                "dateAdded",
                Operator::DateNotRecent,
                RuleValue::Relative {
                    count: 3,
                    unit: DateUnit::Months,
                },
            ),
        ];
        let got = rekordbox_rules(&definition(false, rules), "America/New_York")
            .unwrap()
            .unwrap();
        assert!(!got.match_all);
        assert_eq!(got.conditions[0].left, "2024-03-10");
        assert_eq!(
            (
                got.conditions[1].left.as_str(),
                got.conditions[1].unit.as_str()
            ),
            ("14", "day")
        );
        assert_eq!(
            (
                got.conditions[2].left.as_str(),
                got.conditions[2].unit.as_str()
            ),
            ("3", "month")
        );
        assert_eq!(got.conditions[2].operator, "7");
    }

    #[test]
    fn colors_map_to_rekordbox_positions_and_no_color_is_equals_zero() {
        let got = conditions(vec![
            rule(
                "color",
                Operator::ColorEquals,
                RuleValue::Color(Some("Purple".into())),
            ),
            rule(
                "color",
                Operator::ColorNotEquals,
                RuleValue::Color(Some("pink".into())),
            ),
            rule("color", Operator::ExistsNot, RuleValue::None),
        ]);
        assert_eq!(got[0].2, "8");
        assert_eq!(got[1].1, "2");
        assert_eq!(got[1].2, "1");
        assert_eq!((got[2].1.as_str(), got[2].2.as_str()), ("1", "0"));
    }

    #[test]
    fn anything_rekordbox_cannot_express_exactly_means_the_whole_list_is_not_native() {
        let ok = rule(
            "artist",
            Operator::StringContains,
            RuleValue::Text("a".into()),
        );
        for unsupported in [
            rule(
                "artist",
                Operator::StringRegExpMatch,
                RuleValue::Text("a".into()),
            ),
            rule(
                "artist",
                Operator::StringNotStartsWith,
                RuleValue::Text("a".into()),
            ),
            rule(
                "rating",
                Operator::NumberGreaterThanEquals,
                RuleValue::Number(3.),
            ),
            rule(
                "tags",
                Operator::TagsIncludes,
                RuleValue::Ids(vec!["t".into()]),
            ),
            rule(
                "playlist",
                Operator::PlaylistInSpecificPlaylist,
                RuleValue::None,
            ),
            rule("energy", Operator::NumberEquals, RuleValue::Number(5.)),
            rule("releaseDate", Operator::DateAfter, RuleValue::Date(0)),
            rule(
                "color",
                Operator::ColorEquals,
                RuleValue::Color(Some("chartreuse".into())),
            ),
            rule("artist", Operator::StringContains, RuleValue::Number(1.)),
        ] {
            assert!(
                rekordbox_rules(&definition(true, vec![ok.clone(), unsupported]), "UTC")
                    .unwrap()
                    .is_none()
            );
        }
        let mut or = ok.clone();
        or.or = true;
        assert!(
            rekordbox_rules(&definition(true, vec![ok.clone(), or]), "UTC")
                .unwrap()
                .is_none()
        );
        assert!(
            rekordbox_rules(&definition(true, vec![]), "UTC")
                .unwrap()
                .is_none()
        );
        assert!(
            rekordbox_rules(&definition(true, vec![ok]), "UTC")
                .unwrap()
                .is_some()
        );
    }
}
