use super::*;
use crate::organization::native::{Authority, NativePath, Parent};
use anyhow::{Context, Result, ensure};
use rusqlite::{OptionalExtension, params};
use std::path::PathBuf;
impl Library {
    pub fn register_organization_root(&self, path: PathBuf) -> Result<OrganizationRoot> {
        let path = std::path::absolute(path)?;
        let (_, authority) = Parent::capture(&path)?;
        ensure!(
            !path.starts_with(self.data_dir().canonicalize()?),
            "Choose a music folder outside application data"
        );
        let encoded = NativePath::new(&path).0;
        let json = serde_json::to_string(&authority)?;
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let existing:Option<String>=tx.query_row("SELECT id FROM organization_roots WHERE path=?1 AND authority_json=?2 ORDER BY created,id LIMIT 1",params![encoded,json],|r|r.get(0)).optional()?;
        let id = if let Some(id) = existing {
            id
        } else {
            let id = uuid::Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO organization_roots VALUES(?1,?2,?3,?4)",
                params![id, encoded, json, crate::model::now()],
            )?;
            id
        };
        tx.commit()?;
        Ok(OrganizationRoot {
            id,
            display_path: path.to_string_lossy().into_owned(),
        })
    }
    pub fn organization_roots(&self) -> Result<Vec<OrganizationRoot>> {
        let values = {
            let conn = self.conn()?;
            let mut query =
                conn.prepare("SELECT id,path FROM organization_roots ORDER BY created,id")?;
            query
                .query_map([], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        values
            .into_iter()
            .map(|(id, path)| {
                Ok(OrganizationRoot {
                    id,
                    display_path: crate::model::bytes_path(&path)?
                        .to_string_lossy()
                        .into_owned(),
                })
            })
            .collect()
    }
    pub(crate) fn organization_root_authority(&self, id: &str) -> Result<Authority> {
        let json: String = self
            .conn()?
            .query_row(
                "SELECT authority_json FROM organization_roots WHERE id=?1",
                [id],
                |r| r.get(0),
            )
            .optional()?
            .context("Choose an authorized destination folder")?;
        let authority: Authority = serde_json::from_str(&json)?;
        Parent::reopen(&authority)
            .context("Destination folder changed or is unavailable; choose the folder again")?;
        Ok(authority)
    }
}
