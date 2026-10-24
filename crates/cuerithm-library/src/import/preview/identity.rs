use super::*;
use std::path::Path;
/// Reuse an exact existing native identity without changing native ownership.
pub(super) fn legacy_rekordbox(
    lib: &Library,
    header: &ObservationHeader,
    track: &TrackObservation,
) -> Result<Option<String>> {
    if header.source.provider != SourceProvider::Rekordbox
        || track
            .extensions
            .get("native_deleted")
            .and_then(Value::as_i64)
            != Some(0)
    {
        return Ok(None);
    }
    let Some(database) = header.extensions.get("native_database_identity") else {
        return Ok(None);
    };
    let Some(database_id) = database["DBID"].as_str() else {
        return Ok(None);
    };
    let device = database["DeviceID"].as_str().unwrap_or("");
    let Some(source) = track.extensions.get("native_identity") else {
        return Ok(None);
    };
    let Some(uuid) = source["uuid"].as_str().filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let Some(master) = source["master_song_id"].as_str().filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let conn = lib.conn()?;
    let row: Option<(String, String)> = conn.query_row(
        "SELECT l.local_track_id,l.identity_json FROM native_track_links l JOIN native_targets t ON t.id=l.target_id JOIN tracks s ON s.id=l.local_track_id WHERE t.provider='rekordbox' AND t.database_id=?1 AND t.device_id=?2 AND l.content_id=?3",
        params![database_id,device,track.external_id],
        |r| Ok((r.get(0)?,r.get(1)?)),
    ).optional()?;
    let Some((local, identity)) = row else {
        return Ok(None);
    };
    let identity: cuerithm_rekordbox::TrackIdentity = serde_json::from_str(&identity)?;
    Ok((identity.database_id == database_id
        && identity.content_id == track.external_id
        && identity.uuid.as_deref() == Some(uuid)
        && identity.master_song_id.as_deref() == Some(master))
    .then_some(local))
}
/// Exact-path linking is restricted to a qualified native volume. Overlay/tmpfs
/// cannot supply that proof and therefore yields no automatic identity match.
pub(super) fn exact_path(
    lib: &Library,
    path: &Path,
    track: &TrackObservation,
    mapped: bool,
) -> Result<Option<String>> {
    let Ok(canonical) = path.canonicalize() else {
        return Ok(None);
    };
    let Ok(volume) = crate::dj_sync_service::import_media_volume_id(&canonical) else {
        return Ok(None);
    };
    if !mapped
        && track
            .locations
            .volume_id
            .as_ref()
            .is_some_and(|hint| hint != &volume)
    {
        return Ok(None);
    }
    let conn = lib.conn()?;
    let mut query=conn.prepare("SELECT id FROM tracks WHERE path=?1 UNION ALL SELECT id FROM import_items WHERE local_track_id IS NULL AND state='imported' AND path=?1")?;
    let ids = query
        .query_map([crate::model::path_bytes(&canonical)], |r| {
            r.get::<_, String>(0)
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(if ids.len() == 1 {
        Some(ids[0].clone())
    } else {
        None
    })
}
pub(super) fn content_candidates(
    lib: &Library,
    media: &MediaGuard,
    cancel: &CancellationToken,
) -> Result<Vec<String>> {
    let Some(hash) = &media.sha256 else {
        return Ok(vec![]);
    };
    let candidates = {
        let conn = lib.conn()?;
        let mut query=conn.prepare("SELECT id,path FROM tracks WHERE source_hash=?1 UNION ALL SELECT id,path FROM import_items WHERE local_track_id IS NULL AND state='imported' AND source_hash=?1 AND path IS NOT NULL LIMIT 1001")?;
        query
            .query_map([hash], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    ensure!(
        candidates.len() <= 1000,
        "Content identity has more than 1000 candidates; choose explicit existing items"
    );
    let mut ids = Vec::new();
    for (id, path) in candidates {
        cancel.check()?;
        let guard = MediaGuard::capture(&bytes_path(&path)?, cancel)?;
        if guard.sha256 == media.sha256 {
            ids.push(id);
        }
    }
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn content_candidates_are_explicit_review_inputs_and_never_auto_links() {
        let dir = tempfile::tempdir().unwrap();
        let lib = Library::open(&dir.path().join("library.db")).unwrap();
        let path = dir.path().join("a.wav");
        std::fs::write(&path, b"synthetic media bytes").unwrap();
        let guard = MediaGuard::capture(&path, &CancellationToken::default()).unwrap();
        lib.conn().unwrap().execute("INSERT INTO tracks(id,path,title,source_hash,state,created,updated) VALUES('existing',?1,'Existing',?2,'ready',1,1)",params![crate::model::path_bytes(&path),guard.sha256]).unwrap();
        assert_eq!(
            content_candidates(&lib, &guard, &CancellationToken::default()).unwrap(),
            vec!["existing"]
        );
        std::fs::write(&path, b"modified media bytes").unwrap();
        assert!(
            content_candidates(&lib, &guard, &CancellationToken::default())
                .unwrap()
                .is_empty()
        );
    }
}
