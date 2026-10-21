use super::storage::*;
use crate::Library;
use anyhow::{Result, ensure};
use rusqlite::{Connection, OptionalExtension, params};
impl Library {
    pub(crate) fn link_curation_child_tx(
        conn: &Connection,
        publication: &str,
        preview: Option<&str>,
        batch: &str,
        track: &str,
    ) -> Result<()> {
        ensure!(conn.execute("UPDATE curation_publications SET child_batch_id=?1 WHERE id=?2 AND track_id=?3 AND child_batch_id IS NULL AND state IN ('saving','recovery_required') AND (?4 IS NULL OR child_preview_id=?4) AND EXISTS(SELECT 1 FROM curation_tracks c WHERE c.track_id=?3 AND c.active_publication_id=?2)",params![batch,publication,track,preview])?==1,"Curate child lost its exact durable publication binding");
        Ok(())
    }
    pub(crate) fn ack_curation_child_tx(conn: &Connection, batch: &str, track: &str) -> Result<()> {
        let publication:Option<String>=conn.query_row("SELECT id FROM curation_publications WHERE child_batch_id=?1 AND track_id=?2 AND state IN ('saving','recovery_required','waiting_incoming','error')",params![batch,track],|row|row.get(0)).optional()?;
        if let Some(id) = publication {
            super::publication::commit_tx(conn, &id)?;
            Self::advance_curation_input_tx(conn, track)?;
            let ready:Option<String>=conn.query_row("SELECT json_object('path',json_extract(payload_json,'$.row.capture.path'),'parent',json_extract(payload_json,'$.row.capture.parent'),'fingerprint',json_extract(payload_json,'$.publication')) FROM metadata_edit_writes WHERE batch_id=?1 AND track_id=?2 AND state IN ('committed','undone') AND json_type(payload_json,'$.publication')='object' ORDER BY created DESC LIMIT 1",params![batch,track],|row|row.get(0)).optional()?;
            let proof = if ready.is_some() {
                ready
            } else {
                conn.query_row("SELECT json_object('path',json_extract(w.artifact_authorities_json,'$[0].path'),'parent',json_extract(w.artifact_authorities_json,'$[0].parent'),'fingerprint',COALESCE(json_extract(w.artifact_authorities_json,'$[0].published'),json_extract(w.artifact_authorities_json,'$[0].before'))) FROM metadata_edit_overrides o JOIN tag_writes w ON w.job_id=o.job_id AND w.attempt=o.claimed_attempt JOIN tracks t ON t.id=o.track_id WHERE o.batch_id=?1 AND o.track_id=?2 AND w.state='committed' AND w.after_hash=t.source_hash ORDER BY w.created DESC LIMIT 1",params![batch,track],|row|row.get::<_,String>(0)).optional()?
            };
            conn.execute(
                "UPDATE curation_tracks SET input_proof_json=?1 WHERE track_id=?2",
                params![proof, track],
            )?;
        }
        Ok(())
    }
    pub(crate) fn advance_curation_input_tx(conn: &Connection, track: &str) -> Result<()> {
        let input:Input=conn.query_row("SELECT path,source_hash,path_revision,metadata_revision,current_job,state FROM tracks WHERE id=?1",[track],|row|Ok(Input{path:row.get(0)?,source_hash:row.get(1)?,path_revision:row.get(2)?,metadata_revision:row.get(3)?,current_job:row.get(4)?,state:row.get(5)?}))?;
        conn.execute(
            "UPDATE curation_tracks SET input_json=?1 WHERE track_id=?2",
            params![json(&input)?, track],
        )?;
        Ok(())
    }
    pub(crate) fn finalize_curation_preparation_tx(
        conn: &Connection,
        run: &crate::JobRun,
        path: &crate::organization::NativePath,
        parent: &crate::organization::Authority,
        fingerprint: &crate::organization::Fingerprint,
    ) -> Result<()> {
        let input:Option<String>=conn.query_row("SELECT input_json FROM curation_tracks WHERE track_id=?1 AND revision!=acknowledged_revision",[&run.track_id],|row|row.get(0)).optional()?;
        let Some(input) = input else { return Ok(()) };
        let input: Input = serde_json::from_str(&input)?;
        if input.current_job.as_deref() != Some(run.id.as_str()) || input.state != "incoming" {
            return Ok(());
        }
        let own:bool=input.source_hash==run.source_sha256||conn.query_row("SELECT EXISTS(SELECT 1 FROM tag_writes WHERE job_id=?1 AND attempt=?2 AND state='committed' AND after_hash=?3)",params![run.id,run.attempt,input.source_hash],|row|row.get(0))?;
        ensure!(own, "Curate source changed outside its preparation owner");
        Self::advance_curation_input_tx(conn, &run.track_id)?;
        let proof = serde_json::to_string(
            &serde_json::json!({"path":path,"parent":parent,"fingerprint":fingerprint}),
        )?;
        conn.execute(
            "UPDATE curation_tracks SET input_proof_json=?1 WHERE track_id=?2",
            params![proof, run.track_id],
        )?;
        conn.execute("UPDATE curation_publications SET input_json=(SELECT input_json FROM curation_tracks WHERE track_id=?2),input_proof_json=?1 WHERE track_id=?2 AND child_batch_id IS NULL AND state IN ('saving','waiting_incoming','recovery_required')",params![proof,run.track_id])?;
        Ok(())
    }
}
