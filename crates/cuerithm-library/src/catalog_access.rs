use crate::Library;
use anyhow::Result;
use cuerithm_autotag::{ChallengeStore, SourceAccess, TaggingCatalog};

impl ChallengeStore for Library {
    fn requires_solver(&self, source: &str) -> Result<bool> {
        Ok(self.conn()?.query_row(
            "SELECT EXISTS(SELECT 1 FROM catalog_access WHERE source=?1 AND requires_solver=1)",
            [source],
            |r| r.get(0),
        )?)
    }
    fn mark_requires_solver(&self, source: &str) -> Result<()> {
        let conn = self.conn()?;
        let changed = conn.execute(
            "INSERT INTO catalog_access(source,requires_solver) VALUES (?1,1) ON CONFLICT(source) DO UPDATE SET requires_solver=1 WHERE requires_solver=0",
            [source],
        )?;
        if changed > 0 {
            Self::bump(&conn)?;
        }
        Ok(())
    }
}
impl Library {
    pub fn tagging_catalog(&self) -> Result<TaggingCatalog> {
        let mut catalog = cuerithm_autotag::settings_catalog();
        for provider in &catalog.providers {
            catalog.access.push(SourceAccess {
                id: provider.id.clone(),
                requires_solver: self.requires_solver(&provider.id)?,
            });
        }
        Ok(catalog)
    }
    pub fn reset_catalog_access(&self) -> Result<()> {
        let conn = self.conn()?;
        if conn.execute("DELETE FROM catalog_access", [])? > 0 {
            Self::bump(&conn)?;
        }
        Ok(())
    }
}
