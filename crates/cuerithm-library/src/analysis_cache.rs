use crate::{JobRun, Library};
use anyhow::{Result, ensure};
use camelot_core::FileAnalysis;
use cuerithm_media::PreparedAudio;
use rusqlite::{OptionalExtension, params};

impl Library {
    pub(crate) fn cached_analysis(
        &self,
        run: &JobRun,
        engine: &str,
        audio: &PreparedAudio,
    ) -> Result<Option<FileAnalysis>> {
        let encoded:Option<String>=self.conn()?.query_row("SELECT analysis_json FROM preparation_analysis WHERE source_hash=?1 AND settings_revision=?2 AND engine_identity=?3 AND decoder=?4",params![run.source_sha256,run.settings_revision,engine,audio.decoder_identity],|r|r.get(0)).optional()?;
        let Some(result) = encoded.and_then(|s| serde_json::from_str::<FileAnalysis>(&s).ok())
        else {
            return Ok(None);
        };
        let Some(media) = &result.media else {
            return Ok(None);
        };
        if Self::validate_analysis(&result).is_err()
            || result.analysis.models != run.snapshot.models
            || media.source_sha256 != audio.source_sha256
            || media.decoder != audio.decoder_identity
            || media.clock.frames != audio.clock.frames
            || media.clock.sample_rate != audio.clock.sample_rate
            || media.pcm_sha256 != audio.pcm_sha256()?
        {
            return Ok(None);
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
