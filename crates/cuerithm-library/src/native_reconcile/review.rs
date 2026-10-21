use super::*;
use crate::{Library, native_sync};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, params};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeReconciliationField {
    pub baseline: NativeFieldBaseline,
    pub local: NativeFieldObservation,
    pub remote: NativeFieldObservation,
    pub result: NativeReconciliation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeReconciliationSnapshot {
    pub target_id: String,
    pub content_id: String,
    pub local_track_id: String,
    pub target_revision: u64,
    pub library_revision: u64,
    pub track_epoch: u64,
    pub identity_json: String,
    pub fields: BTreeMap<String, NativeReconciliationField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeReconciliationPreview {
    pub id: String,
    pub snapshot: NativeReconciliationSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeReconciliationReview {
    pub preview: NativeReconciliationPreview,
    pub decisions: BTreeMap<String, NativeFieldDecision>,
    pub revision: u64,
    pub stale_reason: Option<String>,
}

fn snapshot(
    conn: &Connection,
    target: &str,
    content: &str,
) -> Result<NativeReconciliationSnapshot> {
    let (local, status, identity, observation, baseline):
        (Option<String>, String, String, Option<String>, String) = conn.query_row(
        "SELECT local_track_id,status,identity_json,observation_json,baseline_json FROM native_track_links WHERE target_id=?1 AND content_id=?2",
        params![target, content],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
    )?;
    ensure!(
        status == "linked",
        "Native identity or observation needs attention"
    );
    let local = local.context("Native track is not linked to a local track")?;
    let observation: cuerithm_rekordbox::ImportedTrack =
        serde_json::from_str(&observation.context("Native observation is unavailable")?)?;
    let baseline_identity: cuerithm_rekordbox::TrackIdentity = serde_json::from_str(&identity)?;
    ensure!(
        !native_sync::changed_identity(&baseline_identity, &observation.identity),
        "Native identity changed"
    );
    let baselines: BTreeMap<String, NativeFieldBaseline> = serde_json::from_str(&baseline)?;
    let local_values = native_sync::local_fields(conn, &local)?;
    let remote_values = native_sync::remote_fields(&observation)?;
    let fields = baselines
        .into_iter()
        .map(|(name, baseline)| {
            let local = local_values.get(&name).cloned().map_or(
                NativeFieldObservation::Unavailable,
                NativeFieldObservation::Present,
            );
            let remote = remote_values.get(&name).cloned().map_or(
                NativeFieldObservation::Unavailable,
                NativeFieldObservation::Present,
            );
            let result = reconcile_field(&baseline, &local, &remote);
            (
                name,
                NativeReconciliationField {
                    baseline,
                    local,
                    remote,
                    result,
                },
            )
        })
        .collect();
    Ok(NativeReconciliationSnapshot {
        target_id: target.into(),
        content_id: content.into(),
        local_track_id: local.clone(),
        target_revision: conn.query_row(
            "SELECT revision FROM native_targets WHERE id=?1",
            [target],
            |r| r.get(0),
        )?,
        library_revision: conn.query_row(
            "SELECT value FROM meta WHERE key='revision'",
            [],
            |r| r.get(0),
        )?,
        track_epoch: conn.query_row(
            "SELECT epoch FROM maintenance_track_epochs WHERE id=?1",
            [&local],
            |r| r.get(0),
        )?,
        identity_json: identity,
        fields,
    })
}

fn load(conn: &Connection, id: &str) -> Result<NativeReconciliationReview> {
    let (saved, decisions, revision): (String, String, u64) = conn.query_row(
        "SELECT snapshot_json,decisions_json,revision FROM native_reconciliation_plans WHERE id=?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let saved: NativeReconciliationSnapshot = serde_json::from_str(&saved)?;
    let stale_reason = match snapshot(conn, &saved.target_id, &saved.content_id) {
        Ok(current) if current == saved => None,
        Ok(_) => Some("Local or recorded native inputs changed; create a new preview".into()),
        Err(error) => Some(format!(
            "Native reconciliation inputs are unavailable: {error}"
        )),
    };
    Ok(NativeReconciliationReview {
        preview: NativeReconciliationPreview {
            id: id.into(),
            snapshot: saved,
        },
        decisions: serde_json::from_str(&decisions)?,
        revision,
        stale_reason,
    })
}

impl Library {
    /// Capture recorded observations for review. This does not read a live target
    /// or promise target-file freshness; the publication coordinator must do so.
    pub fn preview_native_reconciliation(
        &self,
        target: &str,
        content: &str,
    ) -> Result<NativeReconciliationPreview> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let current = snapshot(&tx, target, content)?;
        let id = uuid::Uuid::new_v4().to_string();
        let now = crate::model::now();
        tx.execute(
            "INSERT INTO native_reconciliation_plans(id,target_id,content_id,snapshot_json,created,updated) VALUES(?1,?2,?3,?4,?5,?5)",
            params![id, target, content, serde_json::to_string(&current)?, now],
        )?;
        tx.commit()?;
        Ok(NativeReconciliationPreview {
            id,
            snapshot: current,
        })
    }

    /// Save reviewed intent only. In particular, neither AcceptEqual nor a
    /// source choice acknowledges a baseline or writes local metadata/music.
    pub fn review_native_reconciliation(
        &self,
        id: &str,
        expected_revision: u64,
        decisions: BTreeMap<String, NativeFieldDecision>,
    ) -> Result<NativeReconciliationReview> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let mut review = load(&tx, id)?;
        ensure!(
            review.revision == expected_revision,
            "Native review revision changed; reload the review"
        );
        ensure!(
            review.stale_reason.is_none(),
            "Native reconciliation preview is stale; create a new preview"
        );
        for (name, decision) in &decisions {
            let field = review
                .preview
                .snapshot
                .fields
                .get(name)
                .context("Unknown native reconciliation field")?;
            if *decision == NativeFieldDecision::Skip {
                continue;
            }
            let (NativeFieldObservation::Present(local), NativeFieldObservation::Present(remote)) =
                (&field.local, &field.remote)
            else {
                anyhow::bail!("Unavailable native field cannot be resolved");
            };
            if *decision == NativeFieldDecision::AcceptEqual {
                ensure!(
                    local == remote,
                    "Unequal native field cannot be accepted as equal"
                );
            }
        }
        review.revision = review
            .revision
            .checked_add(1)
            .context("Native review revision exhausted")?;
        review.decisions = decisions;
        tx.execute("UPDATE native_reconciliation_plans SET decisions_json=?1,revision=?2,updated=?3 WHERE id=?4",
            params![serde_json::to_string(&review.decisions)?, review.revision, crate::model::now(), id])?;
        tx.commit()?;
        Ok(review)
    }

    pub fn native_reconciliation_review(&self, id: &str) -> Result<NativeReconciliationReview> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let result = load(&tx, id)?;
        tx.commit()?;
        Ok(result)
    }
}
