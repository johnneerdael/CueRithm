//! Bounded, redacted desktop projections. Discovery grants no source authority.
use super::*;
use crate::Library;
use anyhow::{Context, Result, ensure};
use cuerithm_dj_sync::import::*;
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportSourceCandidate {
    pub provider: SourceProvider,
    pub display_location: String,
    pub profile_hints: Vec<String>,
    pub read_state: String,
    pub acceptance: String,
    pub source_id: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ImportOperationQuery {
    pub source_id: Option<String>,
    pub limit: u32,
    pub cursor: Option<String>,
}
impl Default for ImportOperationQuery {
    fn default() -> Self {
        Self {
            source_id: None,
            limit: 100,
            cursor: None,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportOperation {
    pub operation_id: String,
    pub source_id: String,
    pub state: String,
    pub created: i64,
    pub updated: i64,
    pub policy: ImportPolicy,
    pub result: ImportApplyResult,
    pub can_recover: bool,
    pub can_undo: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportOperationPage {
    pub records: Vec<ImportOperation>,
    pub total: u64,
    pub next_cursor: Option<String>,
}
#[derive(Serialize, Deserialize)]
struct HistoryCursor {
    nonce: String,
    revision: u64,
    source: Option<String>,
    after: i64,
}
impl Library {
    pub fn discover_import_sources(&self) -> Result<Vec<ImportSourceCandidate>> {
        let conn = self.conn()?;
        let mut statement=conn.prepare("SELECT id,provider,location_json,profile_json FROM import_sources ORDER BY updated DESC,id LIMIT 1000")?;
        let rows = statement.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?;
        let mut output = Vec::new();
        for row in rows {
            let (id, provider, location, profile) = row?;
            let selection: ImportSourceSelection = serde_json::from_str(&location)?;
            let profile: SourceProfile = serde_json::from_str(&profile)?;
            output.push(ImportSourceCandidate {
                provider: serde_json::from_str(&provider)?,
                display_location: selection.root.to_string_lossy().into_owned(),
                profile_hints: vec![profile.profile_id],
                read_state: "previously_inspected_select_again_after_restart".into(),
                acceptance: "Native acceptance remains profile-specific; see inspection".into(),
                source_id: Some(id),
            });
        }
        // Known locations only. Never recursively search music, browser or account data.
        let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
            .map(std::path::PathBuf::from);
        if let Some(home) = home {
            for candidate in cuerithm_dj_sync::discovery::discover(
                &home,
                &home.join("Music"),
                &home.join("Documents"),
                &[],
            ) {
                use cuerithm_dj_sync::DjSoftware;
                let provider = match candidate.software {
                    DjSoftware::Rekordbox => SourceProvider::Rekordbox,
                    DjSoftware::VirtualDj => SourceProvider::VirtualDj,
                    DjSoftware::EngineDj => SourceProvider::EngineDj,
                    DjSoftware::DjayPro => SourceProvider::DjayPro,
                    DjSoftware::Serato4 => SourceProvider::Serato4,
                    DjSoftware::Mixxx => SourceProvider::Mixxx,
                    DjSoftware::PioneerUsb => SourceProvider::PioneerUsb,
                };
                let display_location = candidate.path.to_string_lossy().into_owned();
                if !output
                    .iter()
                    .any(|c| c.provider == provider && c.display_location == display_location)
                {
                    output.push(ImportSourceCandidate {
                        provider,
                        display_location,
                        profile_hints: Vec::new(),
                        read_state: "not_inspected".into(),
                        acceptance: "Unverified until selected and inspected".into(),
                        source_id: None,
                    });
                }
            }
        }
        Ok(output)
    }
    pub fn import_operation_page(
        &self,
        query: ImportOperationQuery,
    ) -> Result<ImportOperationPage> {
        ensure!(
            (1..=1000).contains(&query.limit),
            "Import history page size must be 1..=1000"
        );
        ensure!(
            query.cursor.as_ref().is_none_or(|v| v.len() <= 4096),
            "Oversized history cursor"
        );
        let nonce = self
            .inner
            .import_runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("Import runtime interrupted"))?
            .nonce
            .clone();
        let conn = self.conn()?;
        let revision = conn.total_changes();
        let after = if let Some(cursor) = &query.cursor {
            let c: HistoryCursor = serde_json::from_str(cursor)?;
            ensure!(
                c.nonce == nonce
                    && c.revision == revision
                    && c.source == query.source_id
                    && c.after > 0,
                "Stale cursor; refresh import history"
            );
            c.after
        } else {
            i64::MAX
        };
        let total = conn.query_row(
            "SELECT COUNT(*) FROM import_operations WHERE (?1 IS NULL OR source_id=?1)",
            [&query.source_id],
            |r| r.get(0),
        )?;
        let mut statement=conn.prepare("SELECT o.rowid,o.id,o.source_id,o.state,o.created,o.updated,p.json,o.receipt_json,EXISTS(SELECT 1 FROM import_processing_receipts r WHERE r.operation_id=o.id AND r.stage='publication.effects' AND r.state IN ('applying','recovery_required')),EXISTS(SELECT 1 FROM import_processing_receipts r WHERE r.operation_id=o.id AND r.stage='publication.effects' AND r.state='applied') FROM import_operations o JOIN import_policies p ON p.id=o.policy_id WHERE (?1 IS NULL OR o.source_id=?1) AND o.rowid<?2 ORDER BY o.rowid DESC LIMIT ?3")?;
        let mut rows = statement.query(params![query.source_id, after, query.limit + 1])?;
        let mut records = Vec::new();
        let mut last = after;
        let mut more = false;
        let mut bytes = 0usize;
        while let Some(row) = rows.next()? {
            let policy: String = row.get(6)?;
            let receipt: String = row.get(7)?;
            ensure!(
                policy.len() + receipt.len() <= 1024 * 1024,
                "Import history record exceeds byte limit"
            );
            if records.len() == query.limit as usize
                || (!records.is_empty() && bytes + policy.len() + receipt.len() > 4 * 1024 * 1024)
            {
                more = true;
                break;
            }
            bytes += policy.len() + receipt.len();
            last = row.get(0)?;
            let mut policy: ImportPolicy = serde_json::from_str(&policy)?;
            // Opaque credential references are private runtime configuration, not history data.
            policy.metadata_enrichment.credential_refs.clear();
            records.push(ImportOperation {
                operation_id: row.get(1)?,
                source_id: row.get(2)?,
                state: row.get(3)?,
                created: row.get(4)?,
                updated: row.get(5)?,
                policy,
                result: serde_json::from_str(&receipt)
                    .context("Invalid import operation receipt")?,
                can_recover: row.get(8)?,
                can_undo: row.get(9)?,
            });
        }
        Ok(ImportOperationPage {
            records,
            total,
            next_cursor: if more {
                Some(serde_json::to_string(&HistoryCursor {
                    nonce,
                    revision,
                    source: query.source_id,
                    after: last,
                })?)
            } else {
                None
            },
        })
    }
}
