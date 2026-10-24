use super::*;
/// Resolve only immutable adapter-authored candidates. The client supplies an ID,
/// never the value/normalization that may become mutation authority.
pub(super) fn resolve(request: &ImportPreviewRequest, track: &mut TrackObservation) -> Result<()> {
    let mut seen = std::collections::BTreeSet::new();
    for choice in request
        .candidate_choices
        .iter()
        .filter(|choice| choice.external_id == track.external_id)
    {
        ensure!(
            seen.insert(&choice.field),
            "Duplicate source candidate choice"
        );
        let entries = track
            .extensions
            .get("field_candidates")
            .and_then(|v| v.get(&choice.field))
            .and_then(Value::as_array)
            .context("Unknown observed candidate field")?;
        let matches = entries
            .iter()
            .filter(|v| v.get("id").and_then(Value::as_str) == Some(&choice.candidate_id))
            .collect::<Vec<_>>();
        ensure!(
            matches.len() == 1,
            "Unknown or ambiguous observed candidate ID"
        );
        let candidate = matches[0];
        let field = candidate
            .get("normalized_field")
            .and_then(Value::as_str)
            .context("Source candidate requires a new inspection with qualified normalization")?;
        let value = candidate
            .get("normalized_value")
            .context("Candidate has no qualified normalization")?
            .clone();
        validate_custom_field(field, &value)?;
        match field {
            "key" => track.key = Some(serde_json::from_value(value)?),
            "bpm" => track.bpm = Some(serde_json::from_value(value)?),
            _ if field.starts_with("metadata.") => {
                track.metadata.insert(
                    field.trim_start_matches("metadata.").into(),
                    serde_json::from_value(value)?,
                );
            }
            _ => anyhow::bail!("Unsupported normalized candidate field"),
        }
        track.issues.retain(|issue| {
            !(issue.code == "djstudio_surface_disagreement"
                && issue.field.as_deref() == Some(&choice.field))
        });
    }
    Ok(())
}
