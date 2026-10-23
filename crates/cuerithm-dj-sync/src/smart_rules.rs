//! Whole-definition native smart-list compilation. Unsupported expressions retain
//! their captured ordinary membership; no supported fragment is exported alone.
use crate::{DjSoftware, KeyNotation, SyncLibrary, SyncOptions, SyncPlaylist, SyncTextField};
use cuerithm_smartlists::{Operator as Op, RuleValue, SmartRule};
use serde_json::json;

type CompileResult<T> = std::result::Result<T, String>;
#[derive(Debug, Clone)]
pub(crate) enum NativeSmart {
    VirtualDj(String),
    EngineDj(String),
    Djay(Vec<DjayFilter>),
    Serato4(String),
}
#[derive(Debug, Clone)]
pub(crate) struct DjayFilter {
    pub field: &'static str,
    pub name: &'static str,
    pub multiple: bool,
    pub operator: u16,
    pub value: DjayValue,
}
#[derive(Debug, Clone)]
pub(crate) enum DjayValue {
    Text(String),
    Number {
        start: f64,
        end: Option<f64>,
    },
    Set(Vec<String>),
    Date {
        start: Option<f64>,
        end: Option<f64>,
        relative: Option<(i64, u8)>,
    },
}
#[derive(Debug, Clone)]
enum Predicate {
    Text {
        field: String,
        op: Op,
        value: String,
    },
    Number {
        field: String,
        op: Op,
        value: f64,
    },
    Range {
        field: String,
        start: f64,
        end: f64,
    },
    Missing(bool),
    CompatibleKeys(Vec<u8>),
    Date {
        op: Op,
        value: RuleValue,
        zone: String,
    },
}

/// Report the same whole-list fallback decisions used by each native writer.
pub fn smart_fallbacks(
    library: &SyncLibrary,
    options: &SyncOptions,
    target: DjSoftware,
) -> Vec<String> {
    library
        .playlists
        .iter()
        .filter_map(|playlist| {
            compile(playlist, library, options, target)
                .err()
                .map(|reason| format!("Smartlist '{}' was materialized: {reason}", playlist.name))
        })
        .collect()
}
pub(crate) fn compile(
    playlist: &SyncPlaylist,
    library: &SyncLibrary,
    options: &SyncOptions,
    target: DjSoftware,
) -> CompileResult<Option<NativeSmart>> {
    compile_platform(playlist, library, options, target, cfg!(windows))
}
fn compile_platform(
    playlist: &SyncPlaylist,
    library: &SyncLibrary,
    options: &SyncOptions,
    target: DjSoftware,
    windows: bool,
) -> CompileResult<Option<NativeSmart>> {
    let Some(definition) = &playlist.smart else {
        return Ok(None);
    };
    if options.materialize_smartlists {
        return Ok(None);
    }
    if target == DjSoftware::PioneerUsb {
        return Err("Pioneer USB stores ordinary playlist membership".into());
    }
    if target == DjSoftware::Mixxx {
        return Err("Mixxx has no smart playlists or crates".into());
    }
    if target == DjSoftware::DjayPro && windows {
        return Err("current djay on Windows requires ordinary playlists".into());
    }
    if playlist.folder {
        return Err("a folder cannot contain a native smart definition".into());
    }
    definition
        .validate_shape()
        .map_err(|error| format!("invalid definition: {error}"))?;
    if definition.rules.is_empty() {
        return Err("an empty definition must stay an empty playlist".into());
    }
    if definition.match_all && definition.rules.iter().filter(|rule| rule.or).count() > 1 {
        return Err("mixed AND/OR groups have no verified native representation".into());
    }
    if target == DjSoftware::EngineDj {
        let mut current = Some(playlist);
        let mut seen = std::collections::HashSet::new();
        while let Some(node) = current {
            if !seen.insert(&node.id) || node.name.contains(';') {
                return Err(
                    "the native Engine playlist path cannot represent this hierarchy".into(),
                );
            }
            current = node
                .parent_id
                .as_ref()
                .and_then(|id| library.playlists.iter().find(|node| &node.id == id));
        }
    }
    let all = definition.match_all || definition.rules.len() == 1;
    if target == DjSoftware::DjayPro && !all {
        return Err("djay's current filter set cannot preserve a general OR".into());
    }
    let mut predicates = Vec::new();
    for rule in &definition.rules {
        predicates.extend(predicate(rule, options, target).map_err(|reason| {
            format!(
                "rule '{}' ({} / {:?}): {reason}",
                rule.id, rule.field, rule.operator
            )
        })?);
    }
    let result = match target {
        DjSoftware::VirtualDj => NativeSmart::VirtualDj(virtual_dj(&predicates, all, options)?),
        DjSoftware::EngineDj => NativeSmart::EngineDj(engine(&predicates, all, options)?),
        DjSoftware::DjayPro => NativeSmart::Djay(djay(&predicates, options)?),
        DjSoftware::Serato4 => NativeSmart::Serato4(serato(&predicates, all, library, options)?),
        DjSoftware::PioneerUsb | DjSoftware::Mixxx => unreachable!(),
        DjSoftware::Rekordbox => {
            return Err("Rekordbox smart playlists require the native collection workflow".into());
        }
    };
    Ok(Some(result))
}
fn text_field(field: &str) -> Option<SyncTextField> {
    Some(match field {
        "title" => SyncTextField::Title,
        "artist" => SyncTextField::Artist,
        "album" => SyncTextField::Album,
        "genre" => SyncTextField::Genre,
        "label" => SyncTextField::Label,
        "comment" => SyncTextField::Comment,
        "key" => SyncTextField::Key,
        _ => return None,
    })
}
fn field_name(field: SyncTextField) -> &'static str {
    match field {
        SyncTextField::Title => "title",
        SyncTextField::Artist => "artist",
        SyncTextField::Album => "album",
        SyncTextField::Genre => "genre",
        SyncTextField::Label => "label",
        SyncTextField::Comment => "comment",
        SyncTextField::Key => "key",
    }
}
fn mapped_field(field: &str, options: &SyncOptions) -> CompileResult<String> {
    let Some(source) = text_field(field) else {
        if matches!(
            field,
            "location"
                | "composer"
                | "remixer"
                | "mix"
                | "grouping"
                | "extra1"
                | "extra2"
                | "fileType"
        ) {
            return Ok(field.into());
        }
        return Err("this field is not exported by the portable metadata model".into());
    };
    if options
        .field_mappings
        .get(&source)
        .copied()
        .unwrap_or(source)
        == source
    {
        return Ok(field.into());
    }
    if source == SyncTextField::Key {
        return Err("field mapping overwrites the native musical-key field".into());
    }
    options
        .field_mappings
        .iter()
        .find(|(destination, mapped)| **mapped == source && **destination != SyncTextField::Key)
        .map(|(destination, _)| field_name(*destination).to_owned())
        .ok_or_else(|| {
            "field mapping does not retain this source value in a comparable native field".into()
        })
}
fn predicate(
    rule: &SmartRule,
    options: &SyncOptions,
    target: DjSoftware,
) -> CompileResult<Vec<Predicate>> {
    use Op::*;
    if matches!(
        rule.operator,
        DateEquals
            | DateNotEquals
            | DateAfter
            | DateBefore
            | DateRecent
            | DateNotRecent
            | DateBetween
    ) {
        if rule.field != "dateAdded" {
            return Err("this native target does not export the source date field".into());
        }
        let context = options
            .smart_context
            .as_ref()
            .ok_or("saved source date and IANA timezone context are not exported")?;
        if context.time_zone != context.native_time_zone {
            return Err(
                "the library calendar timezone differs from the native application's timezone"
                    .into(),
            );
        }
        let _: chrono_tz::Tz = context
            .time_zone
            .parse()
            .map_err(|_| "invalid smart calendar timezone")?;
        return Ok(vec![Predicate::Date {
            op: rule.operator,
            value: rule.value.clone(),
            zone: context.time_zone.clone(),
        }]);
    }
    if rule.operator == KeySimilar {
        if target != DjSoftware::EngineDj || rule.field != "key" {
            return Err("this target has no verified compatible-key predicate".into());
        }
        if options
            .field_mappings
            .get(&SyncTextField::Key)
            .is_some_and(|field| *field != SyncTextField::Key)
        {
            return Err("field mapping overwrites the musical-key field".into());
        }
        let context = options
            .smart_context
            .as_ref()
            .ok_or("the harmonic/fuzzy key preference is not exported")?;
        let RuleValue::Text(query) = &rule.value else {
            return Err("compatible key requires a text key".into());
        };
        let key = crate::formatting::parse_key(query).map_err(|_| "invalid musical key")?;
        let mut numbers = Vec::new();
        for n in 1..=12 {
            for mode in ['A', 'B'] {
                let candidate = format!("{n}{mode}");
                if cuerithm_smartlists::compatible_keys(
                    &candidate,
                    &key.camelot(),
                    context.key_mixing_mode,
                ) {
                    let open = key_query(&candidate, KeyNotation::OpenKey)?;
                    let index: u8 = open[..open.len() - 1]
                        .parse()
                        .map_err(|_| "invalid Open Key")?;
                    numbers.push((index - 1) * 2 + u8::from(open.ends_with('m')));
                }
            }
        }
        numbers.sort();
        numbers.dedup();
        return Ok(vec![Predicate::CompatibleKeys(numbers)]);
    }
    let predicate = match (&rule.operator, &rule.value) {
        (
            StringContains | StringNotContains | StringEquals | StringNotEquals | StringStartsWith
            | StringEndsWith,
            RuleValue::Text(value),
        ) => {
            if value.is_empty() {
                return Err(
                    "empty text predicates have different native missing-value semantics".into(),
                );
            }
            Predicate::Text {
                field: mapped_field(&rule.field, options)?,
                op: rule.operator,
                value: value.clone(),
            }
        }
        (
            NumberEquals
            | NumberNotEquals
            | NumberGreaterThan
            | NumberGreaterThanEquals
            | NumberLessThan
            | NumberLessThanEquals,
            RuleValue::Number(value),
        ) => {
            numeric_field(&rule.field, options, target)?;
            Predicate::Number {
                field: rule.field.clone(),
                op: rule.operator,
                value: *value,
            }
        }
        (NumberBetween, RuleValue::NumberRange { start, end }) => {
            numeric_field(&rule.field, options, target)?;
            Predicate::Range {
                field: rule.field.clone(),
                start: start.min(*end),
                end: start.max(*end),
            }
        }
        (BooleanTrue | BooleanFalse, RuleValue::None)
            if rule.field == "CustomFileMissing" && target == DjSoftware::VirtualDj =>
        {
            Predicate::Missing(rule.operator == BooleanTrue)
        }
        (ColorEquals | ColorNotEquals, _) => {
            return Err(
                "the portable RGB value does not retain the smart rule's original color identity"
                    .into(),
            );
        }
        _ => return Err("this operator has no verified whole-rule native translation".into()),
    };
    Ok(vec![predicate])
}
fn numeric_field(field: &str, options: &SyncOptions, target: DjSoftware) -> CompileResult<()> {
    if !matches!(
        field,
        "bpm" | "rating" | "year" | "duration" | "bitrate" | "sizeBytes" | "trackNumber"
    ) {
        return Err("this numeric field is not exported".into());
    }
    if matches!(field, "bitrate" | "sizeBytes" | "trackNumber") && target != DjSoftware::VirtualDj {
        return Err("this target does not preserve this numeric source field".into());
    }
    if field == "bpm"
        && options.skip_beatgrids
        && matches!(
            target,
            DjSoftware::Serato4 | DjSoftware::DjayPro | DjSoftware::VirtualDj
        )
    {
        return Err("preserved target tempo may differ from the local BPM".into());
    }
    if field == "bpm" && target == DjSoftware::DjayPro {
        return Err(
            "djay's native BPM comparator has no established two-decimal source quantization"
                .into(),
        );
    }
    if field == "duration"
        && matches!(
            target,
            DjSoftware::EngineDj | DjSoftware::DjayPro | DjSoftware::Serato4
        )
    {
        return Err("this target cannot preserve the source duration predicate exactly".into());
    }
    Ok(())
}
fn key_query(value: &str, notation: KeyNotation) -> CompileResult<String> {
    let key = crate::formatting::parse_key(value).map_err(|_| "invalid musical key".to_owned())?;
    if !key.camelot().eq_ignore_ascii_case(value) {
        return Err("key text predicates require canonical Camelot equality".into());
    }
    crate::formatting::format_key(&key.camelot(), notation).map_err(|error| error.to_string())
}
fn scalar_operator(op: Op) -> CompileResult<&'static str> {
    Ok(match op {
        Op::NumberEquals => "=",
        Op::NumberNotEquals => "!=",
        Op::NumberGreaterThan => ">",
        Op::NumberGreaterThanEquals => ">=",
        Op::NumberLessThan => "<",
        Op::NumberLessThanEquals => "<=",
        _ => return Err("unsupported numeric operator".into()),
    })
}
fn virtual_dj(
    predicates: &[Predicate],
    all: bool,
    _options: &SyncOptions,
) -> CompileResult<String> {
    let mut parts = Vec::new();
    for predicate in predicates {
        match predicate {
            Predicate::Text { field, op, value } => {
                let name = match field.as_str() {
                    "title" => "Title",
                    "artist" => "Artist",
                    "album" => "Album",
                    "genre" => "Genre",
                    "label" => "Label",
                    "comment" => "Comment",
                    "key" => "Key",
                    "location" => "File Path",
                    "composer" => "Composer",
                    "remixer" => "Remixer",
                    "mix" => "Remix",
                    "grouping" => "Grouping",
                    "extra1" => "User 1",
                    "extra2" => "User 2",
                    _ => return Err("VirtualDJ cannot compare the mapped field".into()),
                };
                let value = if field == "key" {
                    if !matches!(op, Op::StringEquals | Op::StringNotEquals) {
                        return Err("key patterns cannot be translated to musical notation".into());
                    }
                    key_query(value, KeyNotation::Musical)?
                } else {
                    value.clone()
                };
                if value.contains('"') && value.contains('\'') {
                    return Err("text contains both native quote delimiters".into());
                }
                let quote = if value.contains('"') { '\'' } else { '"' };
                let operator = match op {
                    Op::StringContains => "contains",
                    Op::StringNotContains => "doesn't contain",
                    Op::StringEquals => "is",
                    Op::StringNotEquals => "is not",
                    Op::StringStartsWith => "starts with",
                    Op::StringEndsWith => "ends with",
                    _ => unreachable!(),
                };
                parts.push(format!("{name} {operator} {quote}{value}{quote}"));
            }
            Predicate::Number { field, op, value } => parts.push(format!(
                "{} {} {value}",
                vdj_number(field)?,
                scalar_operator(*op)?
            )),
            Predicate::Range { field, start, end } => {
                if !all {
                    return Err("a range inside OR requires an unverified nested group".into());
                }
                let field = vdj_number(field)?;
                parts.push(format!("{field} >= {start}"));
                parts.push(format!("{field} <= {end}"));
                if *start <= 0.0 && *end >= 0.0 {
                    parts.push(format!("{field} != 0"));
                }
            }
            Predicate::Missing(missing) => parts.push(format!("Exists = {}", u8::from(!missing))),
            Predicate::CompatibleKeys(_) => {
                return Err("VirtualDJ has no verified compatible-key predicate".into());
            }
            Predicate::Date { op, value, zone } => {
                parts.extend(dates::virtual_dj(*op, value, zone)?)
            }
        }
    }
    Ok(parts.join(if all { " and " } else { " or " }))
}
fn vdj_number(field: &str) -> CompileResult<&'static str> {
    Ok(match field {
        "bpm" => "BPM",
        "rating" => "Rating",
        "year" => "Year",
        "duration" => "Length",
        "bitrate" => "Bitrate",
        "sizeBytes" => "File Size",
        "trackNumber" => "Track",
        _ => return Err("unsupported VirtualDJ numeric field".into()),
    })
}
fn engine(predicates: &[Predicate], all: bool, _options: &SyncOptions) -> CompileResult<String> {
    let mut rules = Vec::new();
    for predicate in predicates {
        match predicate {
            Predicate::Text {field,op,value}=>{
                if field!="key" {return Err("Engine text comparison does not preserve the source trim and Unicode case semantics".into());}
                if !matches!(op,Op::StringEquals|Op::StringNotEquals){return Err("Engine key patterns are not native key comparisons".into());}
                let open=key_query(value,KeyNotation::OpenKey)?;let number:u8=open[..open.len()-1].parse().map_err(|_|"invalid Open Key")?;
                let param=((number-1)*2+u8::from(open.ends_with('m'))).to_string();
                rules.push(json!({"col":"IFNULL(key,-1)","con":if *op==Op::StringEquals{"="}else{"!="},"param":param,"v":"2.20.0"}));
            }
            Predicate::Number { field, op, value } => {
                let (col, scale) = engine_number(field)?;
                if !(value * scale).is_finite() {
                    return Err("scaled numeric threshold exceeds the native range".into());
                }
                rules.push(json!({"col":col,"con":scalar_operator(*op)?,"param":(value*scale).to_string(),"v":"2.20.0"}));
            }
            Predicate::Range { field, start, end } => {
                let (col, scale) = engine_number(field)?;
                if !(start * scale).is_finite() || !(end * scale).is_finite() {
                    return Err("scaled numeric range exceeds the native range".into());
                }
                rules.push(json!({"col":col,"con":"BETWEEN","param":format!("{} AND {}",start*scale,end*scale),"v":"2.20.0"}));
                if *start <= 0.0 && *end >= 0.0 {
                    if !all {
                        return Err("zero-excluding range inside OR requires a nested group".into());
                    }
                    rules.push(json!({"col":col,"con":"!=","param":"0","v":"2.20.0"}));
                }
            }
            Predicate::Missing(_) => return Err("unsupported Engine boolean predicate".into()),
            Predicate::CompatibleKeys(keys)=>rules.push(json!({"col":"key","con":"IN","param":format!("({})",keys.iter().map(u8::to_string).collect::<Vec<_>>().join(",")),"v":"2.20.0"})),
            Predicate::Date{op,value,zone}=>rules.extend(dates::engine(*op,value,zone)?),
        }
    }
    Ok(json!({"match":if all {"all"}else{"one"},"rules":rules,"rv":1}).to_string())
}
fn engine_number(field: &str) -> CompileResult<(&'static str, f64)> {
    // The native slot contains SQL expressions. Two-decimal rounding matches the
    // source smart evaluator; integer ROUND(bpmAnalyzed) would change its meaning.
    Ok(match field {
        "bpm" => (
            "CAST(CAST(printf('%.14e',IFNULL(bpmAnalyzed,0)*100) AS REAL)+0.5 AS INTEGER)/100.0",
            1.0,
        ),
        "rating" => ("IFNULL(rating,0)", 20.0),
        "year" => ("IFNULL(year,0)", 1.0),
        _ => return Err("Engine cannot preserve this numeric field".into()),
    })
}
fn djay(predicates: &[Predicate], _options: &SyncOptions) -> CompileResult<Vec<DjayFilter>> {
    let mut filters = Vec::new();
    for predicate in predicates {
        let filter = match predicate {
            Predicate::Text { field, op, value } => {
                let (path, name, multiple) = match field.as_str() {
                    "title" => ("title", "Title", false),
                    "artist" => ("artist", "Artist", true),
                    "album" => ("album", "Album", true),
                    "genre" => ("genre", "Genre", true),
                    "label" => ("label", "Label", true),
                    "comment" => ("comments", "Comments", false),
                    "key" => ("key", "Key", true),
                    "composer" => ("composer", "Composer", true),
                    "grouping" => ("grouping", "Grouping", false),
                    _ => return Err("djay cannot compare the mapped field".into()),
                };
                let value = if matches!(field.as_str(), "key" | "genre") {
                    if *op != Op::StringEquals {
                        return Err("djay key/genre filters support equality sets only".into());
                    }
                    DjayValue::Set(vec![if field == "key" {
                        key_query(value, KeyNotation::OpenKey)?.to_lowercase()
                    } else {
                        value.clone()
                    }])
                } else {
                    DjayValue::Text(value.clone())
                };
                DjayFilter {
                    field: path,
                    name,
                    multiple,
                    operator: match op {
                        Op::StringContains => 0,
                        Op::StringNotContains => 1,
                        Op::StringEquals => 2,
                        Op::StringNotEquals => 3,
                        Op::StringStartsWith => 4,
                        Op::StringEndsWith => 5,
                        _ => unreachable!(),
                    },
                    value,
                }
            }
            Predicate::Number { field, op, value } => {
                let (name, multiple) = djay_number(field)?;
                let (operator, start, end) = match op {
                    Op::NumberEquals => (0, *value, None),
                    Op::NumberNotEquals => (1, *value, None),
                    Op::NumberGreaterThan => (2, *value, None),
                    Op::NumberLessThan => (3, *value, None),
                    Op::NumberGreaterThanEquals => (4, *value, Some(f64::MAX)),
                    Op::NumberLessThanEquals => (4, -f64::MAX, Some(*value)),
                    _ => unreachable!(),
                };
                DjayFilter {
                    field: djay_field(field)?,
                    name,
                    multiple,
                    operator,
                    value: DjayValue::Number { start, end },
                }
            }
            Predicate::Range { field, start, end } => {
                let (name, multiple) = djay_number(field)?;
                if *start <= 0.0 && *end >= 0.0 {
                    filters.push(DjayFilter {
                        field: djay_field(field)?,
                        name,
                        multiple,
                        operator: 1,
                        value: DjayValue::Number {
                            start: 0.0,
                            end: None,
                        },
                    });
                }
                DjayFilter {
                    field: djay_field(field)?,
                    name,
                    multiple,
                    operator: 4,
                    value: DjayValue::Number {
                        start: *start,
                        end: Some(*end),
                    },
                }
            }
            Predicate::Missing(_) => return Err("unsupported djay boolean predicate".into()),
            Predicate::CompatibleKeys(_) => {
                return Err("djay has no verified compatible-key predicate".into());
            }
            Predicate::Date { op, value, zone } => dates::djay(*op, value, zone)?,
        };
        filters.push(filter);
    }
    Ok(filters)
}
fn djay_field(field: &str) -> CompileResult<&'static str> {
    Ok(match field {
        "bpm" => "bpm",
        "rating" => "rating",
        "year" => "year",
        _ => return Err("unsupported djay numeric field".into()),
    })
}
fn djay_number(field: &str) -> CompileResult<(&'static str, bool)> {
    Ok(match field {
        "bpm" => ("BPM", false),
        "rating" => ("Rating", false),
        "year" => ("Year", false),
        _ => return Err("unsupported djay numeric field".into()),
    })
}
fn serato(
    predicates: &[Predicate],
    all: bool,
    library: &SyncLibrary,
    options: &SyncOptions,
) -> CompileResult<String> {
    let mut rules = Vec::new();
    for predicate in predicates {
        match predicate {
            Predicate::Text { field, op, value } => {
                let attribute = match field.as_str() {
                    "title" => "name",
                    "artist" => "artist",
                    "album" => "album",
                    "genre" => "genre",
                    "label" => "label",
                    "comment" => "comments",
                    "key" => "key",
                    "composer" => "composer",
                    "remixer" => "remixer",
                    "grouping" => "grouping",
                    _ => return Err("Serato cannot compare the mapped field".into()),
                };
                let operation = match op {
                    Op::StringContains => "CONTAINS",
                    Op::StringNotContains => "DOES_NOT_CONTAIN",
                    Op::StringEquals => "IS",
                    Op::StringNotEquals => "IS_NOT",
                    _ => return Err("Serato has no native prefix/suffix rule".into()),
                };
                let value = if field == "key" {
                    if !matches!(op, Op::StringEquals | Op::StringNotEquals) {
                        return Err("key patterns cannot follow key notation conversion".into());
                    }
                    let notation = if options.key_notation == KeyNotation::Original {
                        if !library.tracks.iter().all(|track| {
                            track.key.is_empty()
                                || crate::formatting::parse_key(&track.key)
                                    .is_ok_and(|key| key.camelot().eq_ignore_ascii_case(&track.key))
                        }) {
                            return Err(
                                "Original key spelling is not uniformly canonical Camelot".into()
                            );
                        }
                        KeyNotation::Camelot
                    } else {
                        options.key_notation
                    };
                    key_query(value, notation)?
                } else {
                    value.clone()
                };
                rules.push(json!({"attribute":attribute,"operation":operation,"type":"STRING","version":1,"value":value}));
            }
            Predicate::Number { field, op, value } => {
                rules.push(serato_number(field, *op, *value)?)
            }
            Predicate::Range { field, start, end } => {
                if !all {
                    return Err(
                        "a numeric range inside OR requires an unsupported nested group".into(),
                    );
                }
                let start = if matches!(field.as_str(), "rating" | "year") {
                    start.max(1.0)
                } else {
                    *start
                };
                if start > *end {
                    return Err("the range matches no present values".into());
                }
                if field == "bpm" && start <= 0.0 && *end >= 0.0 {
                    return Err("Serato cannot exclude missing BPM from this range".into());
                }
                rules.push(serato_number(field, Op::NumberGreaterThanEquals, start)?);
                rules.push(serato_number(field, Op::NumberLessThanEquals, *end)?);
            }
            Predicate::Missing(_) => return Err("unsupported Serato boolean predicate".into()),
            Predicate::CompatibleKeys(_) => {
                return Err("Serato has no verified compatible-key predicate".into());
            }
            Predicate::Date { op, value, zone } => rules.extend(dates::serato(*op, value, zone)?),
        }
    }
    Ok(json!({"spaces":["Serato Library","iTunes"],"conjunction":if all {"AND"}else{"OR"},"live_updates":true,"rules":rules}).to_string())
}
mod dates;
fn serato_number(field: &str, op: Op, value: f64) -> CompileResult<serde_json::Value> {
    if field == "bpm" {
        let operation = match op {Op::NumberEquals=>"IS",Op::NumberGreaterThanEquals=>"GREATER_THAN_OR_EQUAL_TO",Op::NumberLessThanEquals=>"LESS_THAN_OR_EQUAL_TO",
            _=>return Err("strict/negative BPM comparisons cannot be preserved by Serato's inclusive operations".into())};
        return Ok(
            json!({"attribute":"bpm","operation":operation,"type":"UINT32","version":1,"value":value}),
        );
    }
    if !matches!(field, "rating" | "year") {
        return Err("unsupported Serato numeric field".into());
    }
    let (operation, number) = match op {
        Op::NumberEquals if value.fract() == 0.0 => ("IS", value),
        Op::NumberGreaterThan => ("GREATER_THAN_OR_EQUAL_TO", value.floor() + 1.0),
        Op::NumberGreaterThanEquals => ("GREATER_THAN_OR_EQUAL_TO", value.ceil()),
        Op::NumberLessThan => ("LESS_THAN_OR_EQUAL_TO", value.ceil() - 1.0),
        Op::NumberLessThanEquals => ("LESS_THAN_OR_EQUAL_TO", value.floor()),
        _ => return Err("Serato cannot preserve this integer comparison".into()),
    };
    if number < 0.0 || number > if field == "rating" { 5.0 } else { 65535.0 } {
        return Err("integer threshold is outside the native field domain".into());
    }
    if field == "year" {
        if number == 0.0 {
            return Err("a missing Serato year is an empty string, not zero".into());
        }
        let (operation, number) = match operation {
            "IS" => ("IS", number),
            "GREATER_THAN_OR_EQUAL_TO" => ("AFTER", number - 1.0),
            _ => ("BEFORE", number + 1.0),
        };
        Ok(
            json!({"attribute":"year","operation":operation,"type":"STRING","version":1,"value":format!("{number:.0}")}),
        )
    } else {
        Ok(
            json!({"attribute":"rating","operation":operation,"type":"UINT32","version":1,"value":(number*20.0) as u32}),
        )
    }
}

#[cfg(test)]
#[path = "smart_rules/tests.rs"]
mod tests;
