use super::format::*;
use anyhow::{Context, Result, bail, ensure};
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};
struct Strict<'a> {
    depth: usize,
    remaining: &'a std::cell::Cell<usize>,
}
impl Strict<'_> {
    fn charge<E: de::Error>(&self, bytes: usize) -> std::result::Result<(), E> {
        let available = self.remaining.get();
        if bytes > available {
            return Err(E::custom("JSON allocation budget exceeded"));
        }
        self.remaining.set(available - bytes);
        Ok(())
    }
}
impl<'de> DeserializeSeed<'de> for Strict<'_> {
    type Value = Value;
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> std::result::Result<Value, D::Error> {
        if self.depth == 0 {
            return Err(de::Error::custom("JSON nesting limit"));
        }
        // Account for the Value and amortized vector capacity before parsing a
        // child. Count strings/map nodes separately in their visitors.
        self.charge::<D::Error>(64)?;
        d.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for Strict<'_> {
    type Value = Value;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("bounded JSON without duplicate keys")
    }
    fn visit_bool<E: de::Error>(self, v: bool) -> std::result::Result<Value, E> {
        Ok(Value::Bool(v))
    }
    fn visit_i64<E: de::Error>(self, v: i64) -> std::result::Result<Value, E> {
        Ok(v.into())
    }
    fn visit_u64<E: de::Error>(self, v: u64) -> std::result::Result<Value, E> {
        Ok(v.into())
    }
    fn visit_f64<E: de::Error>(self, v: f64) -> std::result::Result<Value, E> {
        Number::from_f64(v)
            .map(Value::Number)
            .ok_or_else(|| E::custom("nonfinite JSON"))
    }
    fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<Value, E> {
        self.charge::<E>(v.len().saturating_mul(2))?;
        Ok(v.into())
    }
    fn visit_string<E: de::Error>(self, v: String) -> std::result::Result<Value, E> {
        self.charge::<E>(v.capacity().saturating_mul(2))?;
        Ok(v.into())
    }
    fn visit_none<E: de::Error>(self) -> std::result::Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_unit<E: de::Error>(self) -> std::result::Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> std::result::Result<Value, A::Error> {
        let mut v = vec![];
        while let Some(x) = a.next_element_seed(Strict {
            depth: self.depth - 1,
            remaining: self.remaining,
        })? {
            v.push(x)
        }
        Ok(Value::Array(v))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> std::result::Result<Value, A::Error> {
        let mut m = Map::new();
        while let Some(k) = a.next_key::<String>()? {
            self.charge::<A::Error>(128usize.saturating_add(k.capacity().saturating_mul(2)))?;
            if m.contains_key(&k) {
                return Err(de::Error::custom(format!("duplicate JSON key {k}")));
            }
            m.insert(
                k,
                a.next_value_seed(Strict {
                    depth: self.depth - 1,
                    remaining: self.remaining,
                })?,
            );
        }
        Ok(Value::Object(m))
    }
}
pub fn parse_strict_json(bytes: &[u8], depth: usize) -> Result<Value> {
    parse_strict_json_with_budget(bytes, depth, 64 << 20)
}
/// Conservative allocation accounting bounds each resident JSON record, not only
/// its serialized bytes. In particular small integer arrays cannot amplify a
/// compressed input into an unbounded Value vector.
pub fn parse_strict_json_with_budget(bytes: &[u8], depth: usize, budget: usize) -> Result<Value> {
    let remaining = std::cell::Cell::new(budget);
    let mut d = serde_json::Deserializer::from_slice(bytes);
    let v = Strict {
        depth,
        remaining: &remaining,
    }
    .deserialize(&mut d)?;
    d.end()?;
    Ok(v)
}
fn fields<'a>(v: &'a Value, names: &str) -> Result<&'a Map<String, Value>> {
    let m = v.as_object().context("record value must be object")?;
    let set: BTreeSet<_> = names.split_whitespace().collect();
    for k in &set {
        ensure!(m.contains_key(*k), "Missing required field {k}");
    }
    for k in m.keys() {
        ensure!(
            set.contains(k.as_str()),
            "Unknown field {k}; use namespaced extensions"
        );
    }
    Ok(m)
}
fn text(v: &Value) -> Result<&str> {
    v.as_str().context("expected text")
}
type ReferenceIndex<'a> = BTreeMap<&'a str, BTreeMap<&'a str, &'a LogicalRecord>>;
fn reference<'a>(index: &ReferenceIndex<'a>, kind: &str, id: &Value) -> Result<&'a LogicalRecord> {
    let id = text(id)?;
    index
        .get(kind)
        .and_then(|m| m.get(id))
        .copied()
        .with_context(|| format!("Dangling {kind} reference {id}"))
}
fn extensions(m: &Map<String, Value>) -> Result<()> {
    for k in m.keys() {
        ensure!(
            k.contains('.') && !k.contains('/'),
            "Extension key must be reverse-domain namespaced"
        );
    }
    Ok(())
}
pub fn validate_logical_archive(a: &LogicalArchive, limits: &ArchiveLimits) -> Result<()> {
    let index: ReferenceIndex = a
        .records
        .iter()
        .map(|(kind, rows)| {
            (
                kind.as_str(),
                rows.iter().map(|r| (r.id.as_str(), r)).collect(),
            )
        })
        .collect();
    let m = &a.manifest;
    ensure!(
        m.format == "cuerithm-library" && !m.contains_music,
        "Not a metadata-only CueRithm archive"
    );
    ensure!(
        m.version.major == 1 && m.min_reader.major == 1 && m.min_reader.minor == 0,
        "Unsupported archive version"
    );
    uuid::Uuid::parse_str(&m.archive_id)?;
    uuid::Uuid::parse_str(&m.library_id)?;
    let date = chrono::DateTime::parse_from_rfc3339(&m.created_at)?;
    ensure!(
        date.offset().local_minus_utc() == 0,
        "Archive time must be UTC"
    );
    ensure!(
        ["macos", "windows", "linux"].contains(&m.producer.platform.as_str()),
        "Invalid producer platform"
    );
    for f in &m.required_features {
        ensure!(
            FEATURES.contains(&f.as_str()),
            "Unsupported required feature {f}"
        );
    }
    for f in FEATURES {
        ensure!(
            m.required_features.iter().any(|x| x == f),
            "Missing required feature"
        );
    }
    extensions(&m.extensions)?;
    let mut memory = 0u64;
    for (kind, _) in KINDS {
        let rows = a
            .records
            .get(*kind)
            .with_context(|| format!("Missing required payload {kind}"))?;
        ensure!(
            rows.len()
                <= if *kind == "tracks" {
                    limits.tracks
                } else {
                    limits.relations
                },
            "Record count limit"
        );
        let mut ids = BTreeSet::new();
        for r in rows {
            memory = memory
                .checked_add((serde_json::to_vec(r)?.len() as u64).saturating_mul(3))
                .context("Memory budget overflow")?;
            ensure!(
                memory <= limits.memory_bytes,
                "Archive staging memory budget exceeded"
            );
            ensure!(
                !r.id.is_empty() && r.id.len() <= 256 && r.revision > 0 && ids.insert(&r.id),
                "Invalid or duplicate record ID"
            );
            ensure!(r.value.is_object(), "Expected record object");
            extensions(&r.extensions)?;
            validate_record(&index, kind, r, limits)?;
            validate_types(&index, kind, &r.value)?;
        }
    }
    ensure!(
        a.records["settings"]
            .iter()
            .filter(|r| r.value["selected"] == true)
            .count()
            == 1,
        "Exactly one selected settings revision required"
    );
    ensure!(
        a.records["preferences"].len() == 1 && a.records["preferences"][0].id == "library",
        "Exactly one library preferences required"
    );
    let playlists: &Vec<LogicalRecord> = &a.records["playlists"];
    let mut checked = BTreeSet::new();
    for p in playlists {
        let mut seen = BTreeSet::new();
        let mut current = p;
        while !current.value["parent_id"].is_null() {
            if checked.contains(&current.id) {
                break;
            }
            ensure!(seen.insert(&current.id), "Playlist cycle");
            current = reference(&index, "playlists", &current.value["parent_id"])?;
            ensure!(
                current.value["kind"] == "folder",
                "Playlist parent must be a folder"
            );
        }
        checked.extend(seen);
    }
    let mut pairs = BTreeSet::new();
    let mut positions = BTreeSet::new();
    for r in &a.records["playlist_members"] {
        ensure!(
            pairs.insert((
                r.value["playlist_id"].to_string(),
                r.value["track_id"].to_string()
            )) && positions.insert((
                r.value["playlist_id"].to_string(),
                r.value["position"].to_string()
            )),
            "Duplicate membership or position"
        );
    }
    for (kind, field) in [
        ("musical_revisions", "selected_musical_revision_id"),
        ("analysis_revisions", "selected_analysis_revision_id"),
    ] {
        let mut selected = BTreeMap::new();
        for r in &a.records[kind] {
            if r.value["selected"] == true {
                let track = text(&r.value["track_id"])?;
                ensure!(
                    selected.insert(track, &r.id).is_none(),
                    "Multiple selected revisions"
                );
            }
        }
        for t in &a.records["tracks"] {
            if let Some(id) = t.value[field].as_str() {
                let r = reference(&index, kind, &t.value[field])?;
                ensure!(
                    r.value["track_id"] == t.id
                        && r.value["selected"] == true
                        && selected
                            .get(t.id.as_str())
                            .is_some_and(|selected| selected.as_str() == id),
                    "Selected revision mismatch"
                );
            } else {
                ensure!(
                    !selected.contains_key(t.id.as_str()),
                    "Selected revision not referenced by track"
                );
            }
        }
    }
    Ok(())
}
fn validate_record(
    index: &ReferenceIndex<'_>,
    kind: &str,
    r: &LogicalRecord,
    l: &ArchiveLimits,
) -> Result<()> {
    let v = &r.value;
    match kind {
        "roots" => {
            fields(v, "label source_flavor source_root case_rule")?;
            ensure!(
                ["sensitive", "insensitive", "unknown"].contains(&text(&v["case_rule"])?),
                "Invalid case rule"
            );
        }
        "locators" => match text(&v["kind"])? {
            "local" => {
                fields(v, "kind source_flavor original root_id components")?;
                let original: crate::EncodedPath = serde_json::from_value(v["original"].clone())?;
                let flavor: crate::PathFlavor = serde_json::from_value(v["source_flavor"].clone())?;
                let root = if v["root_id"].is_null() {
                    None
                } else {
                    let root = reference(index, "roots", &v["root_id"])?;
                    ensure!(
                        root.value["source_flavor"] == v["source_flavor"],
                        "Locator/root flavor mismatch"
                    );
                    Some(serde_json::from_value(root.value["source_root"].clone())?)
                };
                let components = if v["components"].is_null() {
                    None
                } else {
                    Some(
                        v["components"]
                            .as_array()
                            .context("Invalid components")?
                            .iter()
                            .map(|c| {
                                if let Some(s) = c.as_str() {
                                    Ok(crate::EncodedPath::utf8(s))
                                } else {
                                    Ok(serde_json::from_value(c.clone())?)
                                }
                            })
                            .collect::<Result<Vec<_>>>()?,
                    )
                };
                crate::PortableLocator {
                    id: r.id.clone(),
                    flavor,
                    original,
                    source_root: root,
                    components,
                }
                .validate()?;
            }
            "streaming" => {
                fields(v, "kind service remote_id")?;
                text(&v["service"])?;
                text(&v["remote_id"])?;
            }
            "unsupported" => {
                fields(v, "kind source_kind source_value")?;
                ensure!(v["source_value"].is_object(), "Invalid unsupported locator");
            }
            _ => bail!("Unknown locator kind"),
        },
        "settings" => {
            fields(
                v,
                "portable_options historical_revision model_identity created_at_ms selected",
            )?;
            let p = fields(
                &v["portable_options"],
                "mik_style_energy stability_millis analysis cleanup tagging",
            )?;
            ensure!(
                p["mik_style_energy"].is_boolean()
                    && p["stability_millis"]
                        .as_u64()
                        .is_some_and(|n| (10..=60000).contains(&n)),
                "Invalid portable settings"
            );
            validate_options(&v["portable_options"])?;
            ensure!(
                v["selected"].is_boolean() && v["created_at_ms"].is_i64(),
                "Invalid settings types"
            );
        }
        "tracks" => {
            fields(
                v,
                "locator_id origin source_id source_record_id state archived archived_at_ms created_at_ms updated_at_ms content metadata attributes media analysis_status selected_musical_revision_id selected_analysis_revision_id historical_preparation",
            )?;
            reference(index, "locators", &v["locator_id"])?;
            if !v["source_id"].is_null() {
                reference(index, "import_sources", &v["source_id"])?;
            }
            ensure!(
                ["cuerithm", "lexicon"].contains(&text(&v["origin"])?),
                "Invalid origin"
            );
            ensure!(
                ["ready", "incoming"].contains(&text(&v["state"])?),
                "Invalid state"
            );
            ensure!(
                ["complete", "imported", "not_analyzed", "stale"]
                    .contains(&text(&v["analysis_status"])?),
                "Invalid analysis status"
            );
            {
                let k = "archived";
                ensure!(v[k].is_boolean(), "Invalid boolean");
            }
            for k in ["created_at_ms", "updated_at_ms"] {
                ensure!(v[k].is_i64(), "Invalid timestamp");
            }
            fields(&v["content"], "sha256 size_bytes observed_mtime_ns")?;
            if !v["content"]["sha256"].is_null() {
                digest(text(&v["content"]["sha256"])?)?;
            }
            let meta = fields(
                &v["metadata"],
                "title artist album album_artist genre comment grouping label remixer mix producer composer lyricist",
            )?;
            for v in meta.values() {
                ensure!(
                    v.as_array().is_some_and(|a| a.iter().all(Value::is_string)),
                    "Metadata must be string lists"
                );
            }
            fields(
                &v["attributes"],
                "rating release_year release_date track_number disc_number color play_count last_played_at_ms extra1 extra2",
            )?;
            fields(
                &v["media"],
                "codec sample_rate channels bits_per_sample bitrate_kbps tags properties",
            )?;
            fields(
                &v["historical_preparation"],
                "settings_revision tagging_summary cleanup_summary",
            )?;
        }
        "musical_revisions" => {
            fields(
                v,
                "track_id origin created_at_ms selected key bpm energy duration_seconds cues tempo_markers source_revision",
            )?;
            reference(index, "tracks", &v["track_id"])?;
            ensure!(v["selected"].is_boolean(), "Invalid selected flag");
            for (k, zero) in [("bpm", false), ("duration_seconds", true)] {
                if !v[k].is_null() {
                    let n = v[k].as_f64().context("Invalid musical number")?;
                    ensure!(
                        n.is_finite() && (n > 0.0 || zero && n == 0.0),
                        "Invalid musical range"
                    );
                }
            }
            if !v["energy"].is_null() {
                ensure!(
                    v["energy"].as_u64().is_some_and(|n| (1..=10).contains(&n)),
                    "Invalid energy"
                );
            }
            for k in ["cues", "tempo_markers"] {
                let items = v[k].as_array().context("Expected timing array")?;
                ensure!(items.len() <= l.per_track, "Timing count limit");
                let mut ids = BTreeSet::new();
                for item in items {
                    ensure!(ids.insert(text(&item["id"])?), "Duplicate timing ID");
                    if k == "cues" {
                        fields(
                            item,
                            "id kind start_seconds end_seconds slot memory name color active_loop source_data",
                        )?;
                        ensure!(
                            item["memory"].is_boolean() && item["active_loop"].is_boolean(),
                            "Invalid cue flag"
                        );
                    } else {
                        fields(
                            item,
                            "id start_seconds bpm beat_number beats_per_bar source_data",
                        )?;
                        ensure!(
                            item["bpm"]
                                .as_f64()
                                .is_some_and(|n| n > 0.0 && n.is_finite()),
                            "Invalid tempo"
                        );
                    }
                    ensure!(
                        item["start_seconds"].as_f64().is_some_and(f64::is_finite),
                        "Invalid timing"
                    );
                }
            }
        }
        "analysis_revisions" => {
            fields(
                v,
                "track_id created_at_ms schema engine_identity settings_revision model_identity result selected",
            )?;
            reference(index, "tracks", &v["track_id"])?;
        }
        "tag_categories" => {
            fields(v, "name position color")?;
        }
        "tags" => {
            fields(v, "category_id name position shortcut")?;
            reference(index, "tag_categories", &v["category_id"])?;
        }
        "track_tags" => {
            fields(v, "track_id tag_id")?;
            reference(index, "tracks", &v["track_id"])?;
            reference(index, "tags", &v["tag_id"])?;
        }
        "playlists" => {
            fields(
                v,
                "parent_id kind name position favorite_position deleted deleted_by definition source_definition definition_status",
            )?;
            ensure!(
                ["folder", "playlist", "smartlist"].contains(&text(&v["kind"])?),
                "Invalid playlist kind"
            );
            ensure!(
                v["position"].is_u64() && v["deleted"].is_boolean(),
                "Invalid playlist values"
            );
            if !v["definition"].is_null() && v["definition_status"] == "active" {
                validate_smart_shape(&v["definition"])?;
                let d: cuerithm_smartlists::SmartDefinition =
                    serde_json::from_value(v["definition"].clone())?;
                d.validate_shape()?;
                ensure!(d.rules.len() <= 128, "Too many active smart rules");
            }
        }
        "playlist_members" => {
            fields(v, "playlist_id track_id position")?;
            let p = reference(index, "playlists", &v["playlist_id"])?;
            ensure!(
                p.value["kind"] == "playlist",
                "Membership requires ordinary playlist"
            );
            reference(index, "tracks", &v["track_id"])?;
            ensure!(v["position"].is_u64(), "Invalid membership position");
        }
        "preferences" => {
            fields(
                v,
                "key_mixing_mode time_zone display_options source_smart_options",
            )?;
            let _: chrono_tz::Tz = text(&v["time_zone"])?.parse()?;
            ensure!(
                ["harmonic", "fuzzy"].contains(&text(&v["key_mixing_mode"])?),
                "Invalid key mixing mode"
            );
            fields(&v["display_options"], "key_notation")?;
            fields(&v["source_smart_options"], "lexicon_key_mixing_mode")?;
        }
        "import_sources" => {
            fields(
                v,
                "vendor source_library_id profile archive_sha256 imported_at_ms source_schema",
            )?;
        }
        "external_links" => {
            fields(
                v,
                "track_id source_id record_kind record_id observation baseline source_locator_id binding_status",
            )?;
            reference(index, "import_sources", &v["source_id"])?;
            if !v["track_id"].is_null() {
                reference(index, "tracks", &v["track_id"])?;
            }
            ensure!(
                v["binding_status"] == "inactive",
                "External bindings must be inactive"
            );
        }
        _ => bail!("Unknown required domain"),
    };
    Ok(())
}
pub(crate) fn digest(s: &str) -> Result<()> {
    ensure!(
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "Invalid SHA-256"
    );
    Ok(())
}
fn validate_options(v: &Value) -> Result<()> {
    if !v["analysis"].is_null() {
        let a = &v["analysis"];
        fields(
            a,
            "sample_rate config structure_features source_profile_timing lexicon_timing lexicon_energy lexicon_tempo lexicon_config cue_template",
        )?;
        ensure!(
            a["sample_rate"].as_u64().is_some_and(|n| n > 0),
            "Invalid sample rate"
        );
        for k in [
            "structure_features",
            "source_profile_timing",
            "lexicon_timing",
            "lexicon_energy",
        ] {
            ensure!(a[k].is_boolean(), "Invalid analysis flag");
        }
        let c = &a["config"];
        fields(
            c,
            "key_frame_size rhythm_frame_size key_hop_size rhythm_hop_size min_bpm max_bpm",
        )?;
        for (frame, hop) in [
            ("key_frame_size", "key_hop_size"),
            ("rhythm_frame_size", "rhythm_hop_size"),
        ] {
            let n = c[frame].as_u64().context("Invalid FFT frame")?;
            ensure!(
                (256..=32768).contains(&n)
                    && n.is_power_of_two()
                    && c[hop].as_u64().is_some_and(|h| h >= 1 && h <= n),
                "Invalid FFT dimensions"
            );
        }
        let min = c["min_bpm"].as_f64().context("Invalid BPM range")?;
        let max = c["max_bpm"].as_f64().context("Invalid BPM range")?;
        ensure!(
            min >= 30.0 && min < max && max <= 300.0,
            "Invalid BPM range"
        );
        if !a["lexicon_tempo"].is_null() {
            let t = &a["lexicon_tempo"];
            fields(t, "min_tempo max_tempo")?;
            let min = t["min_tempo"].as_f64().context("Invalid tempo range")?;
            let max = t["max_tempo"].as_f64().context("Invalid tempo range")?;
            ensure!(
                min >= 50.0 && max <= 300.0 && max - min >= 20.0,
                "Invalid tempo range"
            );
        }
        if !a["lexicon_config"].is_null() {
            let c = &a["lexicon_config"];
            fields(
                c,
                "genre drop_at_start min_cue_length_beats add_emergency_loop",
            )?;
            ensure!(
                c["genre"].is_null() || c["genre"].is_string(),
                "Invalid cue genre"
            );
            ensure!(
                ["Auto", "Never"].contains(&text(&c["drop_at_start"])?),
                "Invalid drop preference"
            );
            ensure!(
                c["min_cue_length_beats"].as_u64().is_some_and(|n| n > 0)
                    && c["add_emergency_loop"].is_boolean(),
                "Invalid cue settings"
            );
        }
        if !a["cue_template"].is_null() {
            let c = &a["cue_template"];
            fields(
                c,
                "overwrite startCueBehavior useFirstCueAsStart breakdownMinBeats keepCuePositions enableCustomAnchors dropAtStart customAnchors rows",
            )?;
            let rows = c["rows"].as_array().context("Invalid cue rows")?;
            ensure!(rows.len() <= 64, "Cue template row limit");
            for r in rows {
                fields(
                    r,
                    "checked markerType name cueType color label anchor position beatsOffset loopDuration",
                )?;
                ensure!(
                    r["checked"].is_boolean() && r["name"].is_string(),
                    "Invalid template row"
                );
            }
            ensure!(c["customAnchors"].is_object(), "Invalid custom anchor data");
        }
    }
    if !v["tagging"].is_null() {
        let t = &v["tagging"];
        fields(
            t,
            "providers no_match analysis_tag_source write_energy strictness match_duration max_duration_difference match_by_id short_title merge_genres album_tagging album_tagging_ratio",
        )?;
        let providers = t["providers"].as_array().context("Invalid provider list")?;
        let mut seen = BTreeSet::new();
        for p in providers {
            let p = text(p)?;
            ensure!(
                ["beatport", "traxsource"].contains(&p) && seen.insert(p),
                "Unsupported/duplicate provider"
            );
        }
        ensure!(
            ["keep_existing", "attention"].contains(&text(&t["no_match"])?),
            "Invalid no-match policy"
        );
        ensure!(
            ["measured", "catalog"].contains(&text(&t["analysis_tag_source"])?),
            "Invalid tag source"
        );
        for k in [
            "write_energy",
            "match_duration",
            "match_by_id",
            "short_title",
            "merge_genres",
            "album_tagging",
        ] {
            ensure!(t[k].is_boolean(), "Invalid tag preference");
        }
        for k in ["strictness", "album_tagging_ratio"] {
            ensure!(
                t[k].as_f64().is_some_and(|n| (0.0..=1.0).contains(&n)),
                "Invalid matching ratio"
            );
        }
        ensure!(
            t["max_duration_difference"].is_u64(),
            "Invalid duration difference"
        );
    }
    if !v["cleanup"].is_null() {
        let c = &v["cleanup"];
        fields(c, "fixes genres artists")?;
        let fixes = c["fixes"].as_array().context("Invalid cleanup fixes")?;
        ensure!(fixes.len() <= 64, "Cleanup fix limit");
        let mut seen = BTreeSet::new();
        for f in fixes {
            fields(f, "id enabled ignored_values rule")?;
            let id = text(&f["id"])?;
            ensure!(
                !id.is_empty() && id.len() <= 128 && seen.insert(id) && f["enabled"].is_boolean(),
                "Invalid cleanup fix"
            );
            ensure!(
                f["ignored_values"]
                    .as_array()
                    .is_some_and(|a| a.iter().all(Value::is_string)),
                "Invalid ignored values"
            );
            let rule = &f["rule"];
            let names = match text(&rule["kind"])? {
                "title_case" => "kind field uppercase lowercase",
                "replace_characters" => "kind characters",
                "decode_entities" | "remove_number_prefix" | "mix_parentheses" => "kind",
                "extract_artist" => "kind separator part",
                "extract_remixer" => "kind word remove_title",
                "remove_garbage" | "remove_promotional_text" => "kind field",
                "remove_urls" => "kind field clear_field",
                "remove_common_text" => "kind pattern",
                _ => bail!("Unknown cleanup rule"),
            };
            fields(rule, names)?;
        }
        fields(&c["genres"], "enabled mappings protected_values conversion")?;
        fields(&c["artists"], "enabled roles mappings protected_values")?;
    }
    Ok(())
}
fn nullable(v: &Value, predicate: fn(&Value) -> bool) -> bool {
    v.is_null() || predicate(v)
}
fn nonnegative(v: &Value) -> bool {
    v.as_f64().is_some_and(|n| n >= 0.0 && n.is_finite())
}
fn validate_types(index: &ReferenceIndex<'_>, kind: &str, v: &Value) -> Result<()> {
    match kind {
        "roots" => {
            text(&v["label"])?;
            let _: crate::PathFlavor = serde_json::from_value(v["source_flavor"].clone())?;
            let path: crate::EncodedPath = serde_json::from_value(v["source_root"].clone())?;
            path.bytes()?;
        }
        "tracks" => {
            for k in [
                "source_id",
                "source_record_id",
                "selected_musical_revision_id",
                "selected_analysis_revision_id",
            ] {
                ensure!(
                    nullable(&v[k], Value::is_string),
                    "Invalid nullable track ID"
                );
            }
            ensure!(
                nullable(&v["archived_at_ms"], Value::is_i64),
                "Invalid archived date"
            );
            let c = &v["content"];
            ensure!(
                nullable(&c["size_bytes"], Value::is_u64),
                "Invalid encoded file size"
            );
            if !c["observed_mtime_ns"].is_null() {
                let text = text(&c["observed_mtime_ns"])?;
                ensure!(
                    !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()),
                    "Invalid decimal mtime"
                );
            }
            let a = &v["attributes"];
            for k in [
                "rating",
                "release_year",
                "track_number",
                "disc_number",
                "play_count",
            ] {
                ensure!(nullable(&a[k], Value::is_u64), "Invalid unsigned attribute");
            }
            ensure!(
                a["rating"].as_u64().is_none_or(|n| n <= 5)
                    && a["release_year"].as_u64() != Some(0),
                "Invalid rating or year"
            );
            for k in ["release_date", "color", "extra1", "extra2"] {
                ensure!(nullable(&a[k], Value::is_string), "Invalid text attribute");
            }
            ensure!(
                nullable(&a["last_played_at_ms"], Value::is_i64),
                "Invalid played time"
            );
            let m = &v["media"];
            for k in ["sample_rate", "channels", "bits_per_sample"] {
                ensure!(nullable(&m[k], Value::is_u64), "Invalid media integer");
            }
            ensure!(
                nullable(&m["codec"], Value::is_string)
                    && nullable(&m["bitrate_kbps"], nonnegative),
                "Invalid media field"
            );
            ensure!(
                m["tags"]
                    .as_object()
                    .is_some_and(|m| m.values().all(Value::is_string))
                    && m["properties"].is_object(),
                "Invalid media tags/properties"
            );
            for k in ["settings_revision", "tagging_summary", "cleanup_summary"] {
                ensure!(
                    nullable(&v["historical_preparation"][k], Value::is_string),
                    "Invalid historical preparation"
                );
            }
        }
        "musical_revisions" => {
            ensure!(
                ["cuerithm", "lexicon", "manual"].contains(&text(&v["origin"])?),
                "Invalid musical origin"
            );
            ensure!(
                v["created_at_ms"].is_i64() && nullable(&v["source_revision"], Value::is_string),
                "Invalid musical provenance"
            );
            if !v["key"].is_null() {
                let k = &v["key"];
                fields(k, "original pitch_class mode")?;
                text(&k["original"])?;
                ensure!(
                    k["pitch_class"].is_null()
                        || k["pitch_class"].as_u64().is_some_and(|n| n <= 11),
                    "Invalid key pitch"
                );
                ensure!(
                    k["mode"].is_null() || ["major", "minor"].contains(&text(&k["mode"])?),
                    "Invalid key mode"
                );
            }
            let mut anchors = BTreeMap::new();
            for marker in v["tempo_markers"].as_array().unwrap() {
                let start = marker["start_seconds"].as_f64().unwrap();
                let bpm = marker["bpm"].as_f64().unwrap();
                let anchor = if start == 0.0 { 0 } else { start.to_bits() };
                if let Some(previous) = anchors.insert(anchor, bpm) {
                    ensure!(previous == bpm, "Conflicting tempo anchors require review");
                }
                for k in ["beat_number", "beats_per_bar"] {
                    ensure!(
                        marker[k].is_null() || marker[k].as_u64().is_some_and(|n| n > 0),
                        "Invalid beat phase"
                    );
                }
                ensure!(
                    marker["source_data"].is_object(),
                    "Invalid marker source data"
                );
            }
            for c in v["cues"].as_array().unwrap() {
                ensure!(
                    ["cue", "fade_in", "fade_out", "load", "loop", "unknown"]
                        .contains(&text(&c["kind"])?),
                    "Invalid cue kind"
                );
                ensure!(
                    nullable(&c["end_seconds"], Value::is_number)
                        && nullable(&c["slot"], Value::is_u64)
                        && nullable(&c["name"], Value::is_string)
                        && nullable(&c["color"], Value::is_string)
                        && c["source_data"].is_object(),
                    "Invalid cue fields"
                );
                if c["active_loop"] == true {
                    ensure!(
                        c["kind"] == "loop"
                            && c["end_seconds"]
                                .as_f64()
                                .is_some_and(|end| end > c["start_seconds"].as_f64().unwrap()),
                        "Invalid executable loop"
                    );
                }
            }
        }
        "analysis_revisions" => {
            ensure!(
                v["created_at_ms"].is_i64() && v["selected"].is_boolean(),
                "Invalid analysis revision provenance"
            );
            text(&v["schema"])?;
            fields(&v["engine_identity"], "name version implementation_sha256")?;
            text(&v["engine_identity"]["name"])?;
            text(&v["engine_identity"]["version"])?;
            if !v["engine_identity"]["implementation_sha256"].is_null() {
                digest(text(&v["engine_identity"]["implementation_sha256"])?)?;
            }
            for model in v["model_identity"]
                .as_array()
                .context("Invalid analysis model identity")?
            {
                fields(model, "name sha256")?;
                text(&model["name"])?;
                if !model["sha256"].is_null() {
                    digest(text(&model["sha256"])?)?;
                }
            }
            ensure!(v["result"].is_object(), "Analysis result must be an object");
            if v["schema"] == "cuerithm-file-analysis-v1" {
                allowed_fields(
                    &v["result"],
                    "publication analysis complete verified_integrity source_sample_rate beat_sample_rate neural_key_sample_rate openkeyscan_sample_rate source_timing elapsed_seconds media",
                )?;
                allowed_fields(
                    &v["result"]["analysis"],
                    "engine engine_source_sha256 models config_sha256 duration_seconds sample_rate key tempo structure cues lexicon_cues energy",
                )?;
                let _: camelot_core::FileAnalysis = serde_json::from_value(v["result"].clone())
                    .context("Invalid known-schema FileAnalysis revision")?;
            }
        }
        "tag_categories" | "tags" => {
            text(&v["name"])?;
            ensure!(v["position"].is_i64(), "Invalid taxonomy position");
            ensure!(
                nullable(
                    &v[if kind == "tags" { "shortcut" } else { "color" }],
                    Value::is_string
                ),
                "Invalid taxonomy text"
            );
        }
        "playlists" => {
            text(&v["name"])?;
            ensure!(
                nullable(&v["parent_id"], Value::is_string)
                    && nullable(&v["favorite_position"], Value::is_u64)
                    && nullable(&v["deleted_by"], Value::is_string),
                "Invalid playlist types"
            );
            ensure!(
                ["active", "unsupported"].contains(&text(&v["definition_status"])?),
                "Invalid definition status"
            );
        }
        "preferences" => {
            ensure!(
                ["original", "camelot", "open_key", "musical"]
                    .contains(&text(&v["display_options"]["key_notation"])?),
                "Invalid key notation"
            );
            let mode = &v["source_smart_options"]["lexicon_key_mixing_mode"];
            ensure!(
                mode.is_null() || ["harmonic", "fuzzy"].contains(&text(mode)?),
                "Invalid source key mode"
            );
        }
        "import_sources" => {
            for k in ["vendor", "source_library_id", "profile"] {
                text(&v[k])?;
            }
            if !v["archive_sha256"].is_null() {
                digest(text(&v["archive_sha256"])?)?;
            }
            ensure!(
                v["imported_at_ms"].is_i64() && v["source_schema"].is_object(),
                "Invalid import source provenance"
            );
        }
        "external_links" => {
            for k in ["record_kind", "record_id"] {
                text(&v[k])?;
            }
            if !v["source_locator_id"].is_null() {
                reference(index, "locators", &v["source_locator_id"])?;
            }
            let baseline = v["baseline"].as_object().context("Invalid baseline map")?;
            for (name, b) in baseline {
                ensure!("title artist album album_artist genre comment grouping label remixer mix producer composer lyricist key bpm energy duration rating release_year release_date track_number disc_number source_path cues tags".split_whitespace().any(|s|s==name),"Unknown external baseline field");
                fields(b, "local remote owner acknowledged")?;
                ensure!(
                    ["local", "remote", "shared", "unowned"].contains(&text(&b["owner"])?)
                        && b["acknowledged"].is_boolean(),
                    "Invalid external ownership"
                );
                validate_canonical_value(name, &b["local"])?;
                validate_canonical_value(name, &b["remote"])?;
                if name == "source_path" {
                    for side in ["local", "remote"] {
                        if !b[side].is_null() {
                            reference(index, "locators", &b[side])?;
                        }
                    }
                }
            }
            if !v["observation"].is_null() {
                let o = &v["observation"];
                fields(o, "metadata musical source_locator_id")?;
                for section in ["metadata", "musical"] {
                    let values = o[section].as_object().context("Invalid observation")?;
                    for (field, field_value) in values {
                        ensure!("title artist album album_artist genre comment grouping label remixer mix producer composer lyricist key bpm energy duration rating release_year release_date track_number disc_number cues tags".split_whitespace().any(|s|s==field),"Unknown observation field");
                        validate_canonical_value(field, field_value)?;
                    }
                }
                if !o["source_locator_id"].is_null() {
                    reference(index, "locators", &o["source_locator_id"])?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}
pub(crate) fn validate_record_from_lookup(
    kind: &str,
    r: &LogicalRecord,
    l: &ArchiveLimits,
    lookup: &dyn Fn(&str, &str) -> Result<Option<LogicalRecord>>,
) -> Result<()> {
    ensure!(
        !r.id.is_empty() && r.id.len() <= 256 && r.revision > 0,
        "Invalid logical record ID/revision"
    );
    ensure!(
        serde_json::to_vec(r)?.len() <= l.record_bytes,
        "Record size limit"
    );
    extensions(&r.extensions)?;
    let mut referenced: BTreeMap<String, BTreeMap<String, LogicalRecord>> = BTreeMap::new();
    let mut load = |kind: &str, value: &Value| -> Result<()> {
        if value.is_null() {
            return Ok(());
        }
        let id = text(value)?;
        let record = lookup(kind, id)?.with_context(|| format!("Dangling {kind} reference"))?;
        referenced
            .entry(kind.into())
            .or_default()
            .insert(id.into(), record);
        Ok(())
    };
    let v = &r.value;
    match kind {
        "locators" => {
            if v["kind"] == "local" {
                load("roots", &v["root_id"])?;
            }
        }
        "tracks" => {
            load("locators", &v["locator_id"])?;
            load("import_sources", &v["source_id"])?;
        }
        "musical_revisions" | "analysis_revisions" => {
            load("tracks", &v["track_id"])?;
            if kind == "analysis_revisions" {
                load("settings", &v["settings_revision"])?;
            }
        }
        "tags" => load("tag_categories", &v["category_id"])?,
        "track_tags" => {
            load("tracks", &v["track_id"])?;
            load("tags", &v["tag_id"])?;
        }
        "playlists" => load("playlists", &v["parent_id"])?,
        "playlist_members" => {
            load("playlists", &v["playlist_id"])?;
            load("tracks", &v["track_id"])?;
        }
        "external_links" => {
            load("import_sources", &v["source_id"])?;
            load("tracks", &v["track_id"])?;
            load("locators", &v["source_locator_id"])?;
            load("locators", &v["observation"]["source_locator_id"])?;
            if let Some(b) = v["baseline"].as_object().and_then(|m| m.get("source_path")) {
                load("locators", &b["local"])?;
                load("locators", &b["remote"])?;
            }
        }
        _ => {}
    }
    let index: ReferenceIndex = referenced
        .iter()
        .map(|(k, rows)| {
            (
                k.as_str(),
                rows.iter().map(|(id, r)| (id.as_str(), r)).collect(),
            )
        })
        .collect();
    validate_record(&index, kind, r, l)?;
    validate_types(&index, kind, v)?;
    if let Some(curation) = r.extensions.get("com.cuerithm.curation") {
        fields(curation, "categories user_rating notes")?;
        ensure!(
            curation["user_rating"].is_null()
                || curation["user_rating"].as_u64().is_some_and(|n| n <= 5),
            "Invalid saved curation rating"
        );
        text(&curation["notes"])?;
        let categories = curation["categories"]
            .as_object()
            .context("Invalid saved curation categories")?;
        for (category, values) in categories {
            ensure!(
                lookup("tag_categories", category)?.is_some(),
                "Missing saved curation category"
            );
            let values = values.as_array().context("Invalid saved curation values")?;
            ensure!(values.len() <= l.per_track, "Curation assignment limit");
            for value in values {
                let tag = lookup("tags", text(value)?)?.context("Missing saved curation tag")?;
                ensure!(
                    tag.value["category_id"] == *category,
                    "Curation tag belongs to different category"
                );
            }
        }
    }
    if kind == "playlists" && v["definition_status"] == "active" && !v["definition"].is_null() {
        let d: cuerithm_smartlists::SmartDefinition =
            serde_json::from_value(v["definition"].clone())?;
        for rule in d.rules {
            match rule.value {
                cuerithm_smartlists::RuleValue::Playlist(p) => {
                    let record =
                        lookup("playlists", &p.id)?.context("Missing smart playlist reference")?;
                    ensure!(
                        record.value["kind"] == serde_json::to_value(p.kind)?,
                        "Smart playlist reference kind mismatch"
                    );
                }
                cuerithm_smartlists::RuleValue::Ids(ids) => {
                    for id in ids {
                        ensure!(
                            lookup("tags", &id)?.is_some()
                                || lookup("tag_categories", &id)?.is_some(),
                            "Missing smart taxonomy reference"
                        );
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn validate_smart_shape(v: &Value) -> Result<()> {
    fields(v, "match_all rules")?;
    ensure!(v["match_all"].is_boolean(), "Invalid smart match mode");
    let rules = v["rules"].as_array().context("Invalid smart rules")?;
    ensure!(rules.len() <= 128, "Smart rule count limit");
    for rule in rules {
        fields(rule, "id field operator value or")?;
        ensure!(rule["or"].is_boolean(), "Invalid smart OR flag");
        let value = &rule["value"];
        let kind = text(&value["kind"])?;
        fields(value, if kind == "none" { "kind" } else { "kind value" })?;
        match kind {
            "none" => {}
            "text" => {
                text(&value["value"])?;
            }
            "number" => ensure!(value["value"].is_number(), "Invalid smart number"),
            "number_range" | "date_range" => {
                fields(&value["value"], "start end")?;
            }
            "date" => ensure!(value["value"].is_i64(), "Invalid smart date"),
            "relative" => {
                fields(&value["value"], "count unit")?;
            }
            "color" => ensure!(
                nullable(&value["value"], Value::is_string),
                "Invalid smart color"
            ),
            "ids" => ensure!(
                value["value"]
                    .as_array()
                    .is_some_and(|a| a.iter().all(Value::is_string)),
                "Invalid smart IDs"
            ),
            "playlist" => {
                fields(&value["value"], "id kind path")?;
                ensure!(
                    value["value"]["path"]
                        .as_array()
                        .is_some_and(|a| a.iter().all(Value::is_string)),
                    "Invalid playlist provenance path"
                );
            }
            _ => bail!("Unknown smart value kind"),
        }
    }
    Ok(())
}
fn allowed_fields(value: &Value, names: &str) -> Result<()> {
    let object = value.as_object().context("Expected schema object")?;
    for name in object.keys() {
        ensure!(
            names.split_whitespace().any(|known| known == name),
            "Unknown frozen schema field {name}"
        );
    }
    Ok(())
}
fn validate_canonical_value(field: &str, value: &Value) -> Result<()> {
    if value.is_null() {
        return Ok(());
    }
    match field {
        "title" | "artist" | "album" | "album_artist" | "genre" | "comment" | "grouping"
        | "label" | "remixer" | "mix" | "producer" | "composer" | "lyricist" => ensure!(
            value
                .as_array()
                .is_some_and(|values| values.iter().all(Value::is_string)),
            "Canonical metadata must be ordered string lists"
        ),
        "key" => {
            fields(value, "original pitch_class mode")?;
            text(&value["original"])?;
            ensure!(
                value["pitch_class"].is_null()
                    || value["pitch_class"].as_u64().is_some_and(|n| n <= 11),
                "Invalid canonical key pitch"
            );
            ensure!(
                value["mode"].is_null() || ["major", "minor"].contains(&text(&value["mode"])?),
                "Invalid canonical key mode"
            );
        }
        "bpm" => ensure!(
            value.as_f64().is_some_and(|n| n > 0.0 && n.is_finite()),
            "Invalid canonical BPM"
        ),
        "energy" => ensure!(
            value.as_u64().is_some_and(|n| (1..=10).contains(&n)),
            "Invalid canonical energy"
        ),
        "duration" => ensure!(
            value.as_f64().is_some_and(|n| n >= 0.0 && n.is_finite()),
            "Invalid canonical duration"
        ),
        "rating" => ensure!(
            value.as_u64().is_some_and(|n| n <= 5),
            "Invalid canonical rating"
        ),
        "release_year" => ensure!(
            value.as_u64().is_some_and(|n| n > 0),
            "Invalid canonical year"
        ),
        "track_number" | "disc_number" => {
            ensure!(value.is_u64(), "Invalid canonical track/disc number")
        }
        "release_date" | "source_path" => {
            text(value)?;
        }
        "tags" => {
            let tags = value.as_array().context("Invalid canonical tag IDs")?;
            ensure!(
                tags.len() <= 4096
                    && tags
                        .iter()
                        .all(|id| id.as_str().is_some_and(|s| !s.is_empty() && s.len() <= 256)),
                "Invalid canonical source tag IDs"
            );
        }
        "cues" => {
            let cues = value.as_array().context("Invalid canonical cue array")?;
            ensure!(cues.len() <= 4096, "Canonical cue count limit");
            let mut ids = BTreeSet::new();
            for cue in cues {
                fields(
                    cue,
                    "id kind start_seconds end_seconds slot memory name color active_loop source_data",
                )?;
                let id = text(&cue["id"])?;
                ensure!(
                    !id.is_empty() && id.len() <= 256 && ids.insert(id),
                    "Invalid or duplicate canonical cue ID"
                );
                ensure!(
                    ["cue", "fade_in", "fade_out", "load", "loop", "unknown"]
                        .contains(&text(&cue["kind"])?),
                    "Invalid canonical cue kind"
                );
                ensure!(
                    cue["start_seconds"].as_f64().is_some_and(f64::is_finite)
                        && nullable(&cue["end_seconds"], Value::is_number)
                        && nullable(&cue["slot"], Value::is_u64)
                        && cue["memory"].is_boolean()
                        && cue["active_loop"].is_boolean()
                        && nullable(&cue["name"], Value::is_string)
                        && nullable(&cue["color"], Value::is_string)
                        && cue["source_data"].is_object(),
                    "Invalid canonical cue fields"
                );
                if cue["active_loop"] == true {
                    ensure!(
                        cue["kind"] == "loop"
                            && cue["end_seconds"]
                                .as_f64()
                                .is_some_and(|end| end > cue["start_seconds"].as_f64().unwrap()),
                        "Invalid executable canonical loop"
                    );
                }
            }
        }
        _ => bail!("Unknown canonical field {field}"),
    };
    Ok(())
}
