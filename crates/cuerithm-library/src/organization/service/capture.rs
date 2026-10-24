use super::*;
use crate::{
    model::{bytes_path, path_bytes},
    organization::native::{Authority, Fingerprint, NativePath, Parent},
};
use anyhow::{Context, Result, ensure};
use cuerithm_cleanup::{Metadata, MetadataField as F};
use cuerithm_media::CancellationToken;
use cuerithm_organize::{
    paths::{NativePath as PurePath, PathRules},
    plan::{self, FileSnapshot, PlanConfig, PlanInput, RootSnapshot, Sidecar, TrackInput},
    template::NamingContext,
};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::Path};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct FrozenTrack {
    pub path: NativePath,
    pub fingerprint: Fingerprint,
    pub path_revision: u64,
    pub metadata_revision: u64,
    pub current_job: Option<String>,
    pub attempt: Option<u32>,
    pub state: String,
    pub source_parent: Authority,
    pub metadata: NamingContext,
    #[serde(default)]
    pub sidecar_parents: Vec<Authority>,
    pub files: Vec<plan::PlannedFile>,
    pub reference_digest: String,
    #[serde(default)]
    pub import_bindings_digest: Option<String>,
    pub reference_bytes: u64,
    pub reference_storage: Vec<NativePath>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct FrozenRow {
    pub public: OrganizationRow,
    pub captured: Option<FrozenTrack>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct FrozenPlan {
    pub public: OrganizationPreview,
    pub settings: OrganizationSettings,
    pub root: Option<Authority>,
    pub directories: Vec<super::inventory::DirectorySnapshot>,
    #[serde(default)]
    pub preparation_job_id: Option<String>,
}
struct RowData {
    id: String,
    path: Option<Vec<u8>>,
    hash: Option<String>,
    path_revision: u64,
    metadata_revision: u64,
    current_job: Option<String>,
    attempt: Option<u32>,
    state: String,
    archived: bool,
    title: String,
    native: Option<String>,
    info: String,
    key: Option<String>,
    bpm: Option<f64>,
    energy: Option<u8>,
}
impl Library {
    pub(super) fn organization_scope_ids(&self, scope: &OrganizationScope) -> Result<Vec<String>> {
        let conn = self.conn()?;
        let ids = match scope {
            OrganizationScope::Tracks { track_ids } => {
                ensure!(
                    track_ids.len() <= 100_000,
                    "Too many tracks in organization scope"
                );
                let mut seen = BTreeSet::new();
                track_ids
                    .iter()
                    .filter(|id| seen.insert((*id).clone()))
                    .cloned()
                    .collect()
            }
            OrganizationScope::AllReady => {
                let mut q=conn.prepare("SELECT id FROM tracks WHERE state='ready' AND archived=0 ORDER BY sort_title,id")?;
                q.query_map([], |r| r.get(0))?
                    .collect::<rusqlite::Result<Vec<String>>>()?
            }
            OrganizationScope::Playlist { playlist_id } => {
                drop(conn);
                self.playlist_members(playlist_id)?
            }
            OrganizationScope::Folder { root_id, recursive } => {
                let json: String = conn.query_row(
                    "SELECT authority_json FROM organization_roots WHERE id=?1",
                    [root_id],
                    |r| r.get(0),
                )?;
                let authority: Authority = serde_json::from_str(&json)?;
                drop(conn);
                Parent::reopen(&authority)?;
                let root = authority.path.path()?;
                let conn = self.conn()?;
                let mut q=conn.prepare("SELECT id,path FROM tracks WHERE state='ready' AND archived=0 AND path IS NOT NULL ORDER BY sort_title,id")?;
                q.query_map([], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?
                .into_iter()
                .filter_map(|(id, path)| match bytes_path(&path) {
                    Ok(path)
                        if cuerithm_organize::paths::is_within(&root, &path)
                            && (*recursive
                                || cuerithm_organize::paths::is_direct_child(&root, &path)) =>
                    {
                        Some(Ok(id))
                    }
                    Ok(_) => None,
                    Err(error) => Some(Err(error)),
                })
                .collect::<Result<Vec<_>>>()?
            }
        };
        ensure!(
            ids.len() <= 100_000,
            "Organization scope exceeds 100,000 tracks"
        );
        Ok(ids)
    }
    pub(super) fn capture_organization_row(
        &self,
        id: &str,
        allow_incoming: bool,
        needs_naming: bool,
        cancel: &CancellationToken,
    ) -> Result<(FrozenRow, TrackInput)> {
        cancel.check()?;
        let row = {
            let conn = self.conn()?;
            conn.query_row("SELECT t.id,t.path,t.source_hash,t.path_revision,t.metadata_revision,t.current_job,j.attempts,t.state,t.archived,t.title,t.native_metadata_json,t.metadata_json,t.musical_key,t.bpm,t.energy FROM tracks t LEFT JOIN jobs j ON j.id=t.current_job WHERE t.id=?1",[id],|r|Ok(RowData{id:r.get(0)?,path:r.get(1)?,hash:r.get(2)?,path_revision:r.get(3)?,metadata_revision:r.get(4)?,current_job:r.get(5)?,attempt:r.get(6)?,state:r.get(7)?,archived:r.get(8)?,title:r.get(9)?,native:r.get(10)?,info:r.get(11)?,key:r.get(12)?,bpm:r.get(13)?,energy:r.get(14)?}))?
        };
        ensure!(
            !row.archived
                && (matches!(row.state.as_str(), "ready" | "imported")
                    || (allow_incoming && row.state == "incoming")),
            "Only eligible, non-archived tracks can be organized"
        );
        let path = bytes_path(
            row.path
                .as_deref()
                .context("Local file is not linked; relocate the track before organization")?,
        )?;
        ensure!(
            !cuerithm_organize::paths::is_within(&self.data_dir().canonicalize()?, &path),
            "Application cache files cannot be organized"
        );
        let (parent, source_parent) =
            Parent::capture(path.parent().context("Missing source folder")?)?;
        let fingerprint =
            parent.fingerprint(Path::new(path.file_name().context("Missing source name")?))?;
        ensure!(
            Some(fingerprint.sha256.as_str()) == row.hash.as_deref(),
            "Source changed; prepare the track before organization"
        );
        let metadata: Metadata = if !needs_naming {
            Metadata::default()
        } else if self.conn()?.query_row(
            "SELECT EXISTS(SELECT 1 FROM import_items WHERE id=?1)",
            [id],
            |r| r.get::<_, bool>(0),
        )? {
            crate::import::publication_metadata(&*self.conn()?, id)?
        } else if let Some(native) = &row.native {
            serde_json::from_str(native)?
        } else {
            #[cfg(feature = "autotag")]
            {
                let info: cuerithm_media::MediaInfo = serde_json::from_str(&row.info)
                    .context("Prepare the track to obtain native metadata")?;
                cuerithm_autotag::read_metadata(&path, &info)?
            }
            #[cfg(not(feature = "autotag"))]
            {
                anyhow::bail!(
                    "Native metadata is unavailable; prepare the track in a tagging-enabled build"
                )
            }
        };
        let get = |field| metadata.fields.get(&field).cloned().unwrap_or_default();
        let first = |field| {
            metadata
                .first(field)
                .map(str::to_owned)
                .filter(|v| !v.trim().is_empty())
        };
        let info: Option<cuerithm_media::MediaInfo> = serde_json::from_str(&row.info).ok();
        let tag = |keys: &[&str]| {
            info.as_ref()
                .and_then(|info| {
                    keys.iter().find_map(|key| {
                        info.tags
                            .iter()
                            .find(|(actual, _)| actual.eq_ignore_ascii_case(key))
                            .map(|(_, value)| value)
                    })
                })
                .cloned()
                .filter(|v| !v.trim().is_empty())
        };
        let context = NamingContext {
            artists: get(F::Artist),
            title: first(F::Title),
            album: first(F::Album),
            album_artists: get(F::AlbumArtist),
            genres: get(F::Genre),
            label: first(F::Label),
            version: first(F::Mix),
            release_date: tag(&[
                "releasedate",
                "release_date",
                "release date",
                "release_time",
                "releasetime",
                "tdrl",
                "date",
                "year",
            ]),
            key: row.key,
            bpm_hundredths: row
                .bpm
                .filter(|value| value.is_finite() && *value > 0.0)
                .map(|v| (v * 100.0).round() as u32),
            energy: row.energy,
            track: tag(&["tracknumber", "track_number", "track"])
                .and_then(|v| v.split('/').next()?.parse().ok()),
            disc: tag(&["discnumber", "disc_number", "disc"])
                .and_then(|v| v.split('/').next()?.parse().ok()),
            original_name: path.file_stem().and_then(|v| v.to_str()).map(str::to_owned),
            parent: path
                .parent()
                .and_then(Path::file_name)
                .and_then(|v| v.to_str())
                .map(str::to_owned),
        };
        let source = snapshot(&path, &fingerprint, row.path_revision)?;
        let sidecars = Vec::new();
        let public = OrganizationRow {
            id: uuid::Uuid::new_v4().to_string(),
            track_id: row.id,
            title: context.title.clone().unwrap_or(row.title),
            before_path: path.to_string_lossy().into_owned(),
            after_path: None,
            outcome: "ready".into(),
            reason: None,
            operation_id: None,
            references: Vec::new(),
        };
        let frozen = FrozenTrack {
            path: NativePath::new(&path),
            fingerprint,
            path_revision: row.path_revision,
            metadata_revision: row.metadata_revision,
            current_job: row.current_job,
            attempt: row.attempt,
            state: row.state,
            source_parent,
            metadata: context.clone(),
            sidecar_parents: Vec::new(),
            files: Vec::new(),
            reference_digest: String::new(),
            import_bindings_digest: crate::organization_references::import_binding_digest(
                &*self.conn()?,
                id,
            )?,
            reference_bytes: 0,
            reference_storage: Vec::new(),
        };
        let input = TrackInput {
            id: id.into(),
            source,
            metadata_revision: row.metadata_revision,
            metadata: context,
            sidecars,
            references: Vec::new(),
        };
        Ok((
            FrozenRow {
                public,
                captured: Some(frozen),
            },
            input,
        ))
    }
    pub(super) fn organization_owned_sidecars(
        &self,
        track: &str,
        source: &Path,
        cancel: &CancellationToken,
    ) -> Result<Vec<(Sidecar, Authority)>> {
        let rows = {
            let conn = self.conn()?;
            let mut q=conn.prepare("SELECT a.path,a.source_hash,EXISTS(SELECT 1 FROM organization_owned_sidecars other WHERE other.path=a.path AND other.track_id!=a.track_id) FROM organization_owned_sidecars a WHERE a.track_id=?1 ORDER BY a.ordinal,a.path")?;
            q.query_map([track], |r| {
                Ok((
                    r.get::<_, Vec<u8>>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, bool>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
        };
        let mut seen = BTreeSet::new();
        let mut sidecars = Vec::new();
        for (bytes, expected, shared) in rows {
            ensure!(
                !shared,
                "Known folder artwork is shared by other tracks; a per-track move cannot claim it"
            );
            cancel.check()?;
            if !seen.insert(bytes.clone()) {
                continue;
            }
            let path = bytes_path(&bytes)?;
            let relative = path
                .strip_prefix(source.parent().context("Missing source folder")?)
                .context(
                    "Known owned artwork is outside its audio folder; cannot move it safely",
                )?;
            let (parent, authority) =
                Parent::capture(path.parent().context("Missing sidecar folder")?)?;
            let name = Path::new(path.file_name().context("Missing sidecar name")?);
            let fingerprint = parent.fingerprint(name)?;
            ensure!(
                fingerprint.sha256 == expected,
                "Owned artwork changed; refresh preparation before organization"
            );
            sidecars.push((
                Sidecar {
                    source: snapshot(&path, &fingerprint, 0)?,
                    destination_name: relative
                        .to_str()
                        .context("Owned sidecar path cannot be represented safely")?
                        .replace(std::path::MAIN_SEPARATOR, "/"),
                },
                authority,
            ));
        }
        Ok(sidecars)
    }
    pub(super) fn validate_frozen_track(
        conn: &Connection,
        track: &str,
        frozen: &FrozenTrack,
        allow_destination: Option<&Path>,
    ) -> Result<()> {
        struct Revision {
            path: Vec<u8>,
            hash: String,
            path_revision: u64,
            metadata_revision: u64,
            job: Option<String>,
            attempt: Option<u32>,
            state: String,
            archived: bool,
        }
        let current=conn.query_row("SELECT t.path,t.source_hash,t.path_revision,t.metadata_revision,t.current_job,j.attempts,t.state,t.archived FROM tracks t LEFT JOIN jobs j ON j.id=t.current_job WHERE t.id=?1",[track],|r|Ok(Revision{path:r.get(0)?,hash:r.get(1)?,path_revision:r.get(2)?,metadata_revision:r.get(3)?,job:r.get(4)?,attempt:r.get(5)?,state:r.get(6)?,archived:r.get(7)?}))?;
        ensure!(
            crate::organization_references::import_binding_digest(conn, track)?
                == frozen.import_bindings_digest,
            "Original import references changed; preview organization again"
        );
        let original =
            current.path == frozen.path.0 && current.path_revision == frozen.path_revision;
        let published = allow_destination.is_some_and(|path| {
            current.path == path_bytes(path)
                && current.path_revision == frozen.path_revision.saturating_add(1)
        });
        ensure!(
            (original || published)
                && current.hash == frozen.fingerprint.sha256
                && current.metadata_revision == frozen.metadata_revision
                && current.job == frozen.current_job
                && current.attempt == frozen.attempt
                && current.state == frozen.state
                && !current.archived,
            "Track path, metadata or preparation changed; create a new organization preview"
        );
        Ok(())
    }
}
pub(super) fn snapshot(
    path: &Path,
    fingerprint: &Fingerprint,
    path_revision: u64,
) -> Result<FileSnapshot> {
    Ok(FileSnapshot {
        path: PurePath(path.into()),
        identity: format!(
            "{}:{}",
            fingerprint.identity.volume, fingerprint.identity.object
        ),
        sha256: fingerprint.sha256.clone(),
        byte_len: fingerprint.bytes,
        path_revision,
    })
}
pub(super) fn config(
    settings: &OrganizationSettings,
    root: Option<&Authority>,
) -> Result<PlanConfig> {
    let managed_root = match root {
        Some(root) => Some(RootSnapshot {
            path: PurePath(root.path.path()?),
            identity: serde_json::to_string(root)?,
        }),
        None => None,
    };
    Ok(PlanConfig {
        mode: match settings.mode {
            OrganizationSettingMode::LeaveInPlace => plan::OrganizationMode::LeaveInPlace,
            OrganizationSettingMode::RenameInPlace => plan::OrganizationMode::RenameInPlace,
            OrganizationSettingMode::Move => plan::OrganizationMode::Move,
            OrganizationSettingMode::Copy => plan::OrganizationMode::Copy,
        },
        pattern: settings.pattern.clone(),
        missing: settings.missing_policy(),
        organization_optional: settings.organization_optional,
        rules: PathRules::Portable,
        managed_root,
    })
}

pub(super) fn build(
    config: PlanConfig,
    tracks: Vec<TrackInput>,
    non_files: Vec<plan::NonFileEntry>,
    cancel: &CancellationToken,
) -> Result<plan::CompletedPlan> {
    plan::build_plan(
        PlanInput {
            config,
            tracks,
            existing: Vec::new(),
            non_files,
        },
        || cancel.is_cancelled(),
    )
    .map_err(|error| anyhow::anyhow!("Organization plan: {error:?}"))
}

impl Library {
    pub(crate) fn organization_service_owned(&self, id: &str) -> Result<bool> {
        Ok(self.conn()?.query_row(
            "SELECT EXISTS(SELECT 1 FROM organization_batch_rows WHERE operation_id=?1)",
            [id],
            |r| r.get(0),
        )?)
    }
    pub(crate) fn organization_validate_operation_tx(
        &self,
        tx: &rusqlite::Transaction<'_>,
        record: &crate::organization::OrganizationRecord,
    ) -> Result<()> {
        let json: Option<String> = tx
            .query_row(
                "SELECT payload_json FROM organization_batch_rows WHERE operation_id=?1",
                [&record.id],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(json) = json {
            let row: FrozenRow = serde_json::from_str(&json)?;
            let frozen = row
                .captured
                .as_ref()
                .context("Missing frozen batch source")?;
            let main = record
                .files
                .first()
                .context("Missing bound audio operation")?;
            ensure!(
                main.source == frozen.path
                    && crate::organization::native::matches(&main.original, &frozen.fingerprint)
                    && main.source_parent.path == frozen.source_parent.path
                    && main.source_parent.ancestors == frozen.source_parent.ancestors,
                "Native source identity or parent changed after preview"
            );
            if record.undo_of.is_none() {
                ensure!(
                    record.files.len() == frozen.files.len(),
                    "Organization file scope changed after preview"
                );
                for (index, (actual, expected)) in
                    record.files.iter().zip(&frozen.files).enumerate()
                {
                    ensure!(
                        actual.source.path()? == expected.source.path.0
                            && actual.destination.path()? == expected.destination.0
                            && actual.original.sha256 == expected.source.sha256
                            && actual.original.bytes == expected.source.byte_len
                            && format!(
                                "{}:{}",
                                actual.original.identity.volume, actual.original.identity.object
                            ) == expected.source.identity,
                        "Organization asset changed after preview"
                    );
                    if index > 0 {
                        let parent = frozen
                            .sidecar_parents
                            .get(index - 1)
                            .context("Missing captured sidecar authority")?;
                        ensure!(
                            actual.source_parent.path == parent.path
                                && actual.source_parent.ancestors == parent.ancestors,
                            "Owned artwork parent changed after preview"
                        );
                    }
                }
            }
            Self::validate_frozen_track(
                tx,
                &row.public.track_id,
                frozen,
                Some(&main.destination.path()?),
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod portable_scan_tests {
    use super::*;
    #[test]
    fn folder_scope_keeps_mapped_tracks_with_unbound_portable_entries() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let library = Library::open(&root.join("data/library.sqlite")).unwrap();
        let music = root.join("music");
        std::fs::create_dir(&music).unwrap();
        let source = music.join("mapped.flac");
        std::fs::write(&source, b"opaque authored source").unwrap();
        library
            .enqueue(
                std::slice::from_ref(&source),
                library.settings().unwrap().revision,
            )
            .unwrap();
        library
            .conn()
            .unwrap()
            .execute("UPDATE tracks SET state='ready'", [])
            .unwrap();
        let mut native = Metadata::default();
        native.fields.insert(F::Title, vec!["Mapped title".into()]);
        native
            .fields
            .insert(F::Artist, vec!["Authored artist".into()]);
        library
            .conn()
            .unwrap()
            .execute(
                "UPDATE tracks SET native_metadata_json=?1",
                [serde_json::to_string(&native).unwrap()],
            )
            .unwrap();
        let mut unresolved = library.portable_tracks().unwrap().remove(0);
        let bound_id = unresolved.id.clone();
        unresolved.id = "unresolved".into();
        unresolved.locator.id = "locator:unresolved".into();
        unresolved.source_sha256 = None;
        unresolved.metadata.insert(
            "comment".into(),
            serde_json::json!("Portable metadata survives"),
        );
        library
            .import_portable_tracks(&[unresolved.clone()])
            .unwrap();
        let jobs = library.jobs().unwrap().len();
        let registered = library.register_organization_root(music).unwrap();
        let preview = library
            .preview_organization(
                OrganizationPreviewRequest {
                    scope: OrganizationScope::Folder {
                        root_id: registered.id,
                        recursive: true,
                    },
                    settings: OrganizationSettings {
                        mode: OrganizationSettingMode::RenameInPlace,
                        ..Default::default()
                    },
                },
                &CancellationToken::default(),
            )
            .unwrap();
        let rows = library
            .organization_preview_page(&preview.id, 0, 100)
            .unwrap();
        assert_eq!(rows.total, 1);
        assert_eq!(rows.rows[0].track_id, bound_id);
        assert_ne!(rows.rows[0].outcome, "blocked", "{:?}", rows.rows[0].reason);
        let selected = library
            .preview_organization(
                OrganizationPreviewRequest {
                    scope: OrganizationScope::Tracks {
                        track_ids: vec!["unresolved".into()],
                    },
                    settings: OrganizationSettings {
                        mode: OrganizationSettingMode::RenameInPlace,
                        ..Default::default()
                    },
                },
                &CancellationToken::default(),
            )
            .unwrap();
        let selected_rows = library
            .organization_preview_page(&selected.id, 0, 100)
            .unwrap();
        assert_eq!(selected_rows.rows[0].outcome, "blocked");
        assert!(
            selected_rows.rows[0]
                .reason
                .as_deref()
                .unwrap()
                .contains("Local file is not linked")
        );
        assert_eq!(library.local_track_binding("unresolved").unwrap(), None);
        let after = library
            .portable_tracks()
            .unwrap()
            .into_iter()
            .find(|t| t.id == "unresolved")
            .unwrap();
        assert_eq!(after.metadata, unresolved.metadata);
        assert_eq!(after.locator, unresolved.locator);
        assert_eq!(library.jobs().unwrap().len(), jobs);
        assert_eq!(std::fs::read(source).unwrap(), b"opaque authored source");
    }
}
