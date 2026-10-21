use super::*;
use anyhow::Result;
impl Library {
    pub fn organization_alternates(&self) -> Result<Vec<OrganizationAlternate>> {
        self.organization_alternate_records()?
            .into_iter()
            .map(|record| {
                Ok(OrganizationAlternate {
                    operation_id: record.operation_id,
                    track_id: record.track_id,
                    display_path: record.path.path()?.to_string_lossy().into_owned(),
                    captured_sha256: record.captured_sha256,
                    state: record.state,
                    observed_sha256: record.observed_sha256,
                    ignored_sha256: record.ignored_sha256,
                })
            })
            .collect()
    }
}
