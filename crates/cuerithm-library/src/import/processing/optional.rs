use super::super::guard::MediaGuard;
use super::stages::Stage;
use super::*;
pub(super) struct OptionalContext<'a> {
    pub library: &'a Library,
    pub runtime: &'a MediaRuntime,
    #[cfg(feature = "autotag")]
    pub tagger: Option<&'a cuerithm_autotag::Tagger>,
}
pub(super) fn execute(
    stage: &Stage,
    context: OptionalContext<'_>,
    frozen: &FrozenProcessing,
    policy: &ImportPolicy,
    media: Option<&MediaGuard>,
    fields: &BTreeMap<String, Value>,
    cancel: &CancellationToken,
) -> Result<(Value, Value)> {
    frozen
        .validate_models(camelot_core::AnalysisSelection::default(), cancel)
        .map_err(|e| service::BlockedStage(e.to_string()))?;
    if stage.name.starts_with("file.") {
        return Ok((
            json!({"fields":{},"requested":stage.name}),
            json!({"processing_sha256":frozen.processing_sha256}),
        ));
    }
    let input = metadata(fields)?;
    let (mut changes, metadata_evidence) = if stage.name == "metadata.cleanup" {
        if let Some(error) = &frozen.optional_error {
            return Err(service::BlockedStage(error.clone()).into());
        }
        let report = cuerithm_cleanup::apply(
            frozen
                .cleanup
                .as_ref()
                .ok_or_else(|| service::BlockedStage("Frozen cleanup unavailable".into()))?,
            &input,
        )?;
        let mut changes=report.changes.into_iter().map(|change|{let name=serde_json::to_value(change.field)?.as_str().context("Invalid cleanup field")?.to_owned();Ok((active_name(fields,&name),json!({"presence":if change.after.is_empty(){"empty"}else{"value"},"value":change.after})))}).collect::<Result<BTreeMap<_,_>>>()?;
        if report.metadata.custom_tags != input.custom_tags {
            changes.insert("metadata.custom_tags".into(),json!({"presence":if report.metadata.custom_tags.is_empty(){"empty"}else{"value"},"value":report.metadata.custom_tags}));
        }
        (changes, json!({"kind":"cleanup"}))
    } else {
        #[cfg(feature = "autotag")]
        {
            enrich(context, frozen, policy, media, fields, cancel)?
        }
        #[cfg(not(feature = "autotag"))]
        {
            let _ = (context.library, context.runtime, policy, media);
            return Err(
                service::BlockedStage("Enrichment requires tagging-enabled build".into()).into(),
            );
        }
    };
    changes.retain(|field, _| super::super::projection::field_selected(policy, field));
    let expected: BTreeMap<_, _> = changes
        .keys()
        .map(|field| {
            (
                field.clone(),
                fields.get(field).cloned().unwrap_or(Value::Null),
            )
        })
        .collect();
    Ok((
        json!({"fields":changes}),
        json!({"expected_fields":expected,"processing_sha256":frozen.processing_sha256,"metadata_evidence":metadata_evidence,"provenance":if stage.name=="metadata.cleanup"{"cleanup"}else{"catalog"}}),
    ))
}
fn active_name(fields: &BTreeMap<String, Value>, name: &str) -> String {
    let plural = match name {
        "artist" => "artists",
        "genre" => "genres",
        _ => name,
    };
    if fields.contains_key(&format!("metadata.{plural}")) {
        format!("metadata.{plural}")
    } else {
        format!("metadata.{name}")
    }
}
fn metadata(fields: &BTreeMap<String, Value>) -> Result<cuerithm_cleanup::Metadata> {
    let mut m = cuerithm_cleanup::Metadata::default();
    for f in cuerithm_cleanup::MetadataField::ALL {
        let name = serde_json::to_value(f)?
            .as_str()
            .context("Invalid cleanup field")?
            .to_owned();
        let value = fields
            .get(&active_name(fields, &name))
            .and_then(|v| v.get("value"));
        let values = match value {
            Some(Value::String(v)) => vec![v.clone()],
            Some(Value::Array(a)) => a
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect(),
            _ => vec![],
        };
        m.set(*f, values);
    }
    if let Some(value) = fields
        .get("metadata.custom_tags")
        .and_then(|v| v.get("value"))
    {
        m.custom_tags = serde_json::from_value(value.clone())
            .context("Custom tags cannot be cleaned without a qualified representation")?;
    }
    Ok(m)
}
#[cfg(feature = "autotag")]
fn enrich(
    context: OptionalContext<'_>,
    frozen: &FrozenProcessing,
    policy: &ImportPolicy,
    media: Option<&MediaGuard>,
    fields: &BTreeMap<String, Value>,
    cancel: &CancellationToken,
) -> Result<(BTreeMap<String, Value>, Value)> {
    use cuerithm_autotag::*;

    let mut config: AutotagConfig = frozen
        .tagging
        .clone()
        .map(serde_json::from_value)
        .transpose()?
        .unwrap_or_default();
    config.options.platforms = policy.metadata_enrichment.providers.clone();
    config.recognition.enabled = false;
    config.required_tags.clear();
    config.write_energy = false;
    ensure!(
        !config.options.platforms.is_empty(),
        "Select enrichment providers"
    );
    config
        .validate()
        .map_err(|e| service::BlockedStage(e.to_string()))?;
    // Native credential resolution remains inside the existing provider runtime;
    // opaque references never become passwords in a policy or receipt.
    let media = media.ok_or_else(|| {
        service::BlockedStage("Imported media unresolved for provider query".into())
    })?;
    media
        .validate(cancel)
        .map_err(|e| service::BlockedStage(e.to_string()))?;
    let mut info = context.runtime.probe_cancellable(&media.path, cancel)?;
    info.tags.clear();
    for (field, value) in fields {
        if let Some(name) = field.strip_prefix("metadata.") {
            let values = match value.get("value") {
                Some(Value::String(s)) => s.clone(),
                Some(Value::Array(a)) => a
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join("; "),
                _ => continue,
            };
            let name = match name {
                "artists" => "artist",
                "genres" => "genre",
                name => name,
            };
            info.tags.insert(name.into(), values);
        }
    }
    let owned;
    let tagger = if let Some(tagger) = context.tagger {
        tagger
    } else {
        let catalog = context.library.shared_catalog_runtime()?.ok_or_else(|| {
            service::BlockedStage("Requested provider credential/runtime unavailable".into())
        })?;
        owned = Tagger::with_runtime_and_store(
            CatalogEndpoints::default(),
            Arc::new(context.library.clone()),
            catalog,
        )?;
        &owned
    };
    let input = TagInput {
        path: media.path.clone(),
        source_sha256: media.sha256.clone().context("Missing media")?,
        media: info,
        duration_seconds: 0.0,
    };
    let outcome = tagger
        .match_track_cancellable(&input, &config, &cancel.shared_flag())
        .map_err(|e| {
            service::BlockedStage(format!("Provider unavailable or requires review: {e}"))
        })?;
    if outcome.status != MatchStatus::Matched {
        return Err(service::BlockedStage(
            "Provider identity requires review or has no match".into(),
        )
        .into());
    }
    let chosen = outcome
        .matches
        .first()
        .context("Matched catalog has no candidate")?;
    let raw = serde_json::to_value(&chosen.track)?;
    let mut changes = BTreeMap::new();
    for name in &policy.metadata_enrichment.fields {
        let key = match name.as_str() {
            "artist" => "artists",
            "genre" => "genres",
            n => n,
        };
        if let Some(value) = raw.get(key).filter(|v| !v.is_null()) {
            let field = active_name(fields, name);
            let value = json!({"presence":"value","value":value});
            if fields.get(&field) != Some(&value) {
                changes.insert(field, value);
            }
        }
    }
    media.validate(cancel)?;
    Ok((
        changes,
        json!({"candidate":chosen,"attempted":outcome.attempted}),
    ))
}
