use super::{
    OrganizationMode,
    native::{Authority, Fingerprint, NativePath},
};
use anyhow::{Context, Result};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrganizationPhase {
    Planned,
    Publishing,
    AwaitingReferences,
    Cleanup,
    Complete,
    Unchanged,
    Cancelled,
    Undone,
}
impl OrganizationPhase {
    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Publishing => "publishing",
            Self::AwaitingReferences => "awaiting_references",
            Self::Cleanup => "cleanup",
            Self::Complete => "complete",
            Self::Unchanged => "unchanged",
            Self::Cancelled => "cancelled",
            Self::Undone => "undone",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationFile {
    pub source: NativePath,
    pub destination: NativePath,
    pub source_parent: Authority,
    pub destination_parent: Authority,
    pub original: Fingerprint,
    pub staged: Option<Fingerprint>,
    pub stage_complete: bool,
    pub abandoned_stages: Vec<(String, Fingerprint)>,
    pub stage_name: String,
    pub quarantine_name: String,
    pub unchanged: bool,
    pub case_only: bool,
    pub published: bool,
    pub quarantined: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationRecord {
    pub id: String,
    pub track_id: String,
    pub mode: OrganizationMode,
    pub phase: OrganizationPhase,
    pub files: Vec<OrganizationFile>,
    pub created_directories: Vec<Authority>,
    pub error: Option<String>,
    pub undo_of: Option<String>,
    #[serde(default)]
    pub batch_row_id: Option<String>,
}
impl OrganizationRecord {
    /// Retained originals are recovery assets and are never automatically deleted.
    pub fn retained_bytes(&self) -> u64 {
        self.files
            .iter()
            .filter(|f| f.quarantined)
            .map(|f| f.original.bytes)
            .sum()
    }
}
impl crate::Library {
    pub(crate) fn organization_record(&self, id: &str) -> Result<OrganizationRecord> {
        let value: String = self
            .conn()?
            .query_row(
                "SELECT payload_json FROM organization_operations WHERE id=?1",
                [id],
                |r| r.get(0),
            )
            .optional()?
            .context("Unknown organization operation")?;
        Ok(serde_json::from_str(&value)?)
    }
    pub(crate) fn save_organization(&self, record: &OrganizationRecord) -> Result<()> {
        let state = if record.error.is_some() {
            "recovery_required"
        } else {
            record.phase.key()
        };
        self.conn()?.execute("UPDATE organization_operations SET state=?1,payload_json=?2,error=?3,updated=?4 WHERE id=?5",params![state,serde_json::to_string(record)?,record.error,crate::model::now(),record.id])?;
        Ok(())
    }
    pub fn organization_history(&self) -> Result<Vec<OrganizationRecord>> {
        let conn = self.conn()?;
        let mut query =
            conn.prepare("SELECT payload_json FROM organization_operations ORDER BY created,id")?;
        let json = query
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        json.into_iter()
            .map(|s| Ok(serde_json::from_str(&s)?))
            .collect()
    }
}
