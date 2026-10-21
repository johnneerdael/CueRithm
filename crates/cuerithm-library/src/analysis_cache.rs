use crate::{JobRun, Library};
use anyhow::{Result, ensure};
use camelot_core::FileAnalysis;
use cuerithm_media::PreparedAudio;
use rusqlite::{OptionalExtension, params};

impl Library {
    /// Transport settings cannot change an already measured or accepted output.
    /// Keep all other settings, optional-config shape and model identities strict.
    pub(crate) fn settings_equal_except_solver(
        before: &crate::SettingsSnapshot,
        after: &crate::SettingsSnapshot,
    ) -> Result<bool> {
        if before.models != after.models
            || before.settings.tagging.is_some() != after.settings.tagging.is_some()
        {
            return Ok(false);
        }
        let mut before = serde_json::to_value(&before.settings)?;
        let mut after = serde_json::to_value(&after.settings)?;
        if let (Some(before), Some(after)) = (
            before
                .get_mut("tagging")
                .and_then(serde_json::Value::as_object_mut),
            after
                .get_mut("tagging")
                .and_then(serde_json::Value::as_object_mut),
        ) {
            before.remove("solver");
            after.remove("solver");
        }
        Ok(before == after)
    }
    /// Seed the new transport revision before its queued job becomes visible.
    /// Copy only the original content hash; cached_analysis still verifies the
    /// actual decoded source, PCM, clock, model and decoder on every read.
    pub(crate) fn alias_solver_retry_analysis(
        &self,
        source: &str,
        before: &str,
        after: &str,
    ) -> Result<()> {
        if before == after {
            return Ok(());
        }
        if !Self::settings_equal_except_solver(
            &self.settings_revision(before)?,
            &self.settings_revision(after)?,
        )? {
            return Ok(());
        }
        self.conn()?.execute("INSERT OR IGNORE INTO preparation_analysis(source_hash,settings_revision,engine_identity,decoder,analysis_json) SELECT source_hash,?1,engine_identity,decoder,analysis_json FROM preparation_analysis WHERE source_hash=?2 AND settings_revision=?3",params![after,source,before])?;
        Ok(())
    }
    pub(crate) fn cached_analysis(
        &self,
        run: &JobRun,
        engine: &str,
        audio: &PreparedAudio,
    ) -> Result<Option<FileAnalysis>> {
        let encoded:Option<String>=self.conn()?.query_row("SELECT analysis_json FROM preparation_analysis WHERE source_hash=?1 AND settings_revision=?2 AND engine_identity=?3 AND decoder=?4",params![run.source_sha256,run.settings_revision,engine,audio.decoder_identity],|r|r.get(0)).optional()?;
        #[cfg(feature = "autotag")]
        let mut encoded = encoded;
        #[cfg(feature = "autotag")]
        let mut reused = false;
        #[cfg(not(feature = "autotag"))]
        let reused = false;
        #[cfg(feature = "autotag")]
        if encoded.is_none() {
            let previous:Option<(String,String)>=self.conn()?.query_row("SELECT analysis_source_hash,analysis_settings_revision FROM preparation_reuse WHERE job_id=?1 AND source_hash=?2",params![run.id,run.source_sha256],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
            if let Some((hash, revision)) = previous {
                let old = self.settings_revision(&revision)?;
                if old.models == run.snapshot.models
                    && serde_json::to_value(&old.settings.engine)?
                        == serde_json::to_value(&run.snapshot.settings.engine)?
                {
                    encoded=self.conn()?.query_row("SELECT analysis_json FROM preparation_analysis WHERE source_hash=?1 AND settings_revision=?2 AND engine_identity=?3 AND decoder=?4",params![hash,revision,engine,audio.decoder_identity],|r|r.get(0)).optional()?;
                    reused = encoded.is_some();
                }
            }
        }
        let Some(mut result) = encoded.and_then(|s| serde_json::from_str::<FileAnalysis>(&s).ok())
        else {
            return Ok(None);
        };
        let Some(media) = &result.media else {
            return Ok(None);
        };
        if Self::validate_analysis(&result).is_err()
            || result.analysis.models != run.snapshot.models
            || (!reused && media.source_sha256 != audio.source_sha256)
            || media.decoder != audio.decoder_identity
            || media.clock.frames != audio.clock.frames
            || media.clock.sample_rate != audio.clock.sample_rate
            || media.pcm_sha256 != audio.pcm_sha256()?
        {
            return Ok(None);
        }
        if reused && let Some(media) = result.media.as_mut() {
            media.source_sha256 = audio.source_sha256.clone();
        }
        // Retain the fully revalidated result under this input revision so a
        // later explicit metadata edit can reuse it without losing the chain.
        if reused {
            self.cache_analysis(run, engine, &result)?;
        }
        Ok(Some(result))
    }
    pub(crate) fn cache_analysis(
        &self,
        run: &JobRun,
        engine: &str,
        result: &FileAnalysis,
    ) -> Result<()> {
        Self::validate_analysis(result)?;
        let media = result
            .media
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Missing analysis cache identity"))?;
        ensure!(
            media.source_sha256 == run.source_sha256
                && result.analysis.models == run.snapshot.models,
            "Analysis cache identity differs from job"
        );
        self.conn()?.execute(
            "INSERT OR REPLACE INTO preparation_analysis VALUES(?1,?2,?3,?4,?5)",
            params![
                run.source_sha256,
                run.settings_revision,
                engine,
                media.decoder,
                serde_json::to_string(result)?
            ],
        )?;
        Ok(())
    }
    pub(crate) fn clear_track_analysis_cache(&self, track: &str) -> Result<()> {
        self.conn()?.execute("DELETE FROM preparation_analysis WHERE source_hash IN (SELECT source_hash FROM jobs WHERE track_id=?1)",[track])?;
        Ok(())
    }
}

#[cfg(all(test, feature = "autotag"))]
mod solver_retry_tests;

#[cfg(test)]
mod equivalence_tests {
    use super::*;
    #[test]
    fn solver_normalization_keeps_optional_shape_all_other_fields_and_models_strict() {
        let settings: crate::PreparationSettings = serde_json::from_value(
            serde_json::json!({"tagging":{"solver":{"endpoint":null,"timeout_seconds":120}}}),
        )
        .unwrap();
        let before = crate::SettingsSnapshot {
            revision: "before".into(),
            settings,
            models: Default::default(),
            preset_capture: None,
        };
        let mut after = before.clone();
        after.revision = "after".into();
        let mut tagging = serde_json::to_value(&after.settings.tagging).unwrap();
        tagging["solver"] =
            serde_json::json!({"endpoint":"http://127.0.0.1:8191","timeout_seconds":41});
        after.settings.tagging = Some(serde_json::from_value(tagging.clone()).unwrap());
        assert!(Library::settings_equal_except_solver(&before, &after).unwrap());
        after.settings.tagging = None;
        assert!(!Library::settings_equal_except_solver(&before, &after).unwrap());
        assert!(!Library::settings_equal_except_solver(&after, &before).unwrap());
        tagging["write_energy"] = serde_json::json!(false);
        after.settings.tagging = Some(serde_json::from_value(tagging).unwrap());
        assert!(!Library::settings_equal_except_solver(&before, &after).unwrap());
        after = before.clone();
        after.models.insert("changed".into(), "model hash".into());
        assert!(!Library::settings_equal_except_solver(&before, &after).unwrap());
    }
}
