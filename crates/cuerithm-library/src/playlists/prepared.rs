use crate::{Library, PlaylistChange, PlaylistEdit, PlaylistKind};
use anyhow::Result;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedPlaylistSaveRequest {
    pub name: String,
    pub track_ids: Vec<String>,
    pub expected_library_revision: u64,
    pub expected_playlist_revision: u64,
}
impl Library {
    pub fn save_prepared_playlist(
        &self,
        request: PreparedPlaylistSaveRequest,
    ) -> Result<PlaylistChange> {
        anyhow::ensure!(
            !request.name.trim().is_empty() && request.name.len() <= 255,
            "Choose a playlist name of 1 to 255 bytes"
        );
        anyhow::ensure!(
            !request.track_ids.is_empty(),
            "Select at least one prepared track"
        );
        anyhow::ensure!(
            request
                .track_ids
                .iter()
                .all(|id| !id.is_empty() && id.len() <= 256),
            "Invalid track selection"
        );
        let mut conn = self.conn()?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let revision: u64 =
            tx.query_row("SELECT value FROM meta WHERE key='revision'", [], |r| {
                r.get(0)
            })?;
        anyhow::ensure!(
            revision == request.expected_library_revision,
            "Library changed; refresh the preview before saving. Your selections are retained."
        );
        let mut seen = std::collections::BTreeSet::new();
        let ids: Vec<_> = request
            .track_ids
            .into_iter()
            .filter(|id| seen.insert(id.clone()))
            .collect();
        for id in &ids {
            let ready: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM tracks WHERE id=?1 AND state='ready' AND archived=0)",
                [id],
                |r| r.get(0),
            )?;
            anyhow::ensure!(
                ready,
                "A selected track is no longer prepared or available; refresh matches before saving"
            );
        }
        let id = uuid::Uuid::new_v4().to_string();
        let change = crate::playlists::apply_edit_tx(
            &tx,
            PlaylistEdit::Batch {
                edits: vec![
                    PlaylistEdit::Create {
                        id: Some(id.clone()),
                        parent_id: None,
                        kind: PlaylistKind::Playlist,
                        name: request.name.trim().into(),
                        definition: None,
                    },
                    PlaylistEdit::AddMembers { id, track_ids: ids },
                ],
            },
            request.expected_playlist_revision,
        )?;
        tx.commit()?;
        Ok(change)
    }
}
