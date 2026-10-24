use super::stages::{self, Stage};
use super::*;
use sha2::{Digest, Sha256};
pub(super) fn proposal_digest(proposal: &Value) -> Result<String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(proposal)?)
    ))
}
pub(super) struct Query<'a> {
    pub stage: &'a Stage,
    pub frozen: &'a FrozenProcessing,
    pub audio: &'a cuerithm_media::PreparedAudio,
    pub fields: &'a BTreeMap<String, Value>,
    pub signature: &'a str,
}
pub(super) fn get(library: &Library, query: Query<'_>) -> Result<Option<(Value, Value)>> {
    let Query {
        stage,
        frozen,
        audio,
        fields,
        signature,
    } = query;
    let cached:Option<(String,String,String,String)>=library.conn()?.query_row("SELECT operation_id,attempt,identity_json,receipt_json FROM import_processing_receipts WHERE stage=?1 AND state='proposed' AND json_extract(identity_json,'$.stage_signature')=?2 AND json_extract(identity_json,'$.cache_reuse') IS NULL ORDER BY rowid LIMIT 1",params![stage.name,signature],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
    let Some((operation_id, attempt, identity, receipt)) = cached else {
        return Ok(None);
    };
    let Some((proposal, identity)) = (|| -> Option<(Value, Value)> {
        let identity: Value = serde_json::from_str(&identity).ok()?;
        let receipt: Value = serde_json::from_str(&receipt).ok()?;
        let proposal = receipt.get("proposal").filter(|v| !v.is_null())?;
        let result: camelot_core::FileAnalysis =
            serde_json::from_value(proposal.get("raw_analysis")?.clone()).ok()?;
        let media = result.media.as_ref()?;
        let pcm = audio.pcm_sha256().ok()?;
        if !result.complete
            || result.selection != Some(stage.selection)
            || proposal.get("field")?.as_str()? != stage.field
            || identity.get("proposal_sha256")?.as_str()? != proposal_digest(proposal).ok()?
        {
            return None;
        }
        let expected = json!({"pcm_sha256":pcm,"source_sha256":audio.source_sha256,"decoder":audio.decoder_identity,"clock":audio.clock,"selection":stage.selection,"engine_source_sha256":camelot_core::analysis::engine_source_sha256(),"processing_sha256":implementation_sha256()});
        for (key, value) in expected.as_object()? {
            if identity.get(key) != Some(value) {
                return None;
            }
        }
        if media.pcm_sha256 != pcm
            || media.source_sha256 != audio.source_sha256
            || media.decoder != audio.decoder_identity
            || serde_json::to_value(&media.clock).ok()?
                != serde_json::to_value(&audio.clock).ok()?
            || media.export_timing != audio.export_timing
        {
            return None;
        }
        if identity.get("engine_source_sha256")?.as_str()? != result.analysis.engine_source_sha256
            || identity.get("config_sha256")?.as_str()? != result.analysis.config_sha256
            || identity.get("models")? != &serde_json::to_value(&result.analysis.models).ok()?
        {
            return None;
        }
        let settings = stages::settings_for_audio(stage, frozen, fields, audio).ok()?;
        if stages::settings_identity(&settings).ok()?
            != stages::settings_identity(
                &serde_json::from_value(identity.get("engine_settings")?.clone()).ok()?,
            )
            .ok()?
        {
            return None;
        }
        for (name, model) in &frozen.models {
            if stages::relevant_model(name, stage.selection)
                && !model
                    .sha256
                    .as_ref()
                    .is_some_and(|sha| result.analysis.models.values().any(|value| value == sha))
            {
                return None;
            }
        }
        // Disabled outputs cannot become a backdoor for adopting prerequisites.
        if !stage.selection.key && result.analysis.key.is_some()
            || !stage.selection.bpm && result.analysis.tempo.is_some()
            || !stage.selection.beatgrid && result.selected_beatgrid.is_some()
            || !stage.selection.energy && result.analysis.energy.is_some()
            || !stage.selection.cues
                && (result.analysis.cues.is_some() || result.analysis.lexicon_cues.is_some())
        {
            return None;
        }
        let value = proposal.get("value")?;
        let valid = match stage.field {
            "key" => result.analysis.key.as_ref().is_some_and(|key| {
                value.get("camelot").and_then(Value::as_str) == Some(&key.camelot)
                    && value.get("presence").and_then(Value::as_str) == Some("value")
            }),
            "bpm" => result.analysis.tempo.as_ref().is_some_and(|tempo| {
                value.get("value").and_then(Value::as_f64) == Some(tempo.bpm)
                    && value.get("presence").and_then(Value::as_str) == Some("value")
            }),
            "energy" => result.analysis.energy.as_ref().is_some_and(|energy| {
                value.get("value").and_then(Value::as_u64) == Some(energy.level as u64)
                    && value.get("method").and_then(Value::as_str) == Some(&energy.method)
            }),
            "beatgrid" => {
                value.get("measured_timing")
                    == Some(&serde_json::to_value(&result.selected_beatgrid).ok()?)
            }
            "cues.hot" => {
                value.get("structural")
                    == Some(&serde_json::to_value(&result.analysis.lexicon_cues).ok()?)
            }
            _ => false,
        };
        valid.then(|| (proposal.clone(), identity))
    })() else {
        return Ok(None);
    };
    let mut reused = serde_json::Map::new();
    for key in [
        "engine_settings",
        "selection",
        "engine_source_sha256",
        "processing_sha256",
        "models",
        "config_sha256",
        "pcm_sha256",
        "decoder",
        "source_sha256",
        "clock",
        "media",
        "stage_signature",
        "proposal_sha256",
    ] {
        if let Some(value) = identity.get(key) {
            reused.insert(key.into(), value.clone());
        }
    }
    reused.insert(
        "cache_reuse".into(),
        json!({"operation_id":operation_id,"attempt":attempt}),
    );
    Ok(Some((proposal, Value::Object(reused))))
}
