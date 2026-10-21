//! Same-filesystem quarantine for native publication artifacts.
//! Lock order: organization -> tagging -> cleanup -> short SQLite operations.
//! Never acquire organization/tagging gates while holding the cleanup gate.
use crate::{
    Library,
    model::{now, path_bytes},
    organization::{Authority, Fingerprint, NativeParent, NativePath},
};
use anyhow::{Context, Result, ensure};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::path::Path;
#[derive(Clone, Serialize, Deserialize)]
struct Payload {
    source: NativePath,
    parent: Authority,
    expected: Option<Fingerprint>,
    directory: String,
    private: Option<Authority>,
}
#[derive(Clone)]
struct Record {
    id: String,
    owner_kind: String,
    owner_id: String,
    payload: Payload,
    json: String,
    state: String,
    error: Option<String>,
}
#[derive(Debug, Default, Serialize)]
pub(crate) struct ArtifactCleanupStorage {
    pub pending: u64,
    pub retained_files: u64,
    pub retained_bytes: u64,
    pub unknown_sizes: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CleanupBoundary {
    Intent,
    DirectoryReady,
    SourceMoved,
    BeforeDelete,
    Deleted,
}
fn same(a: &Fingerprint, b: &Fingerprint) -> bool {
    a.identity == b.identity && a.sha256 == b.sha256 && a.bytes == b.bytes
}
fn valid_native_path(path: &Path) -> bool {
    if !path.is_absolute()
        || path.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
    {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        if path.as_os_str().as_bytes().contains(&0) {
            return false;
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        if path.as_os_str().encode_wide().any(|c| c == 0) {
            return false;
        }
    }
    true
}
fn validate_payload(payload: &Payload, source_bytes: &[u8], quarantine_bytes: &[u8]) -> Result<()> {
    ensure!(
        payload.source.0 == source_bytes,
        "Cleanup source receipt disagrees with its saved key"
    );
    let source = payload.source.path()?;
    let parent = payload.parent.path.path()?;
    ensure!(
        valid_native_path(&source)
            && valid_native_path(&parent)
            && source.parent() == Some(parent.as_path())
            && !payload.parent.ancestors.is_empty(),
        "Invalid cleanup source parent"
    );
    let suffix = payload
        .directory
        .strip_prefix(".cuerithm-cleanup-")
        .context("Invalid private cleanup directory")?;
    uuid::Uuid::parse_str(suffix)?;
    ensure!(
        NativePath::new(&parent.join(&payload.directory).join("artifact")).0 == quarantine_bytes,
        "Cleanup quarantine receipt disagrees with its saved key"
    );
    let expected = payload
        .expected
        .as_ref()
        .context("Unknown cleanup objects cannot become owned")?;
    ensure!(
        expected.sha256.len() == 64
            && expected.sha256.bytes().all(|c| c.is_ascii_hexdigit())
            && expected.bytes <= i64::MAX as u64,
        "Invalid cleanup fingerprint"
    );
    if let Some(private) = &payload.private {
        ensure!(
            private.path == NativePath::new(&parent.join(&payload.directory))
                && private.ancestors.len() == payload.parent.ancestors.len() + 1
                && private.ancestors.starts_with(&payload.parent.ancestors),
            "Invalid private cleanup authority"
        );
    }
    Ok(())
}
impl Library {
    pub(crate) fn quarantine_artifact(
        &self,
        kind: &str,
        owner: &str,
        source: &Path,
        parent: &Authority,
        expected: &Fingerprint,
    ) -> Result<()> {
        self.quarantine_artifact_with(kind, owner, source, parent, expected, |_| Ok(()))
    }
    pub(crate) fn quarantine_artifact_with(
        &self,
        kind: &str,
        owner: &str,
        source: &Path,
        parent: &Authority,
        expected: &Fingerprint,
        boundary: impl FnMut(CleanupBoundary) -> Result<()>,
    ) -> Result<()> {
        self.quarantine_artifact_impl(kind, owner, source, parent, expected, boundary)
    }
    pub(crate) fn retain_unknown_artifact(
        &self,
        kind: &str,
        owner: &str,
        source: &Path,
        parent: &Authority,
        observed: Option<&Fingerprint>,
        reason: &str,
    ) -> Result<()> {
        let _gate = self
            .inner
            .artifact_cleanup_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Artifact cleanup gate interrupted"))?;
        self.retain_unknown_artifact_locked(kind, owner, source, parent, observed, reason)
    }
    pub(crate) fn recover_artifact_cleanup(&self, limit: usize) -> Result<ArtifactCleanupStorage> {
        self.recover_artifact_cleanup_impl(limit)
    }
    pub(crate) fn artifact_cleanup_storage(&self) -> Result<ArtifactCleanupStorage> {
        let conn = self.conn()?;
        Ok(ArtifactCleanupStorage{pending:conn.query_row("SELECT COUNT(*) FROM artifact_cleanup_moves WHERE state!='removed'",[],|r|r.get(0))?,retained_files:conn.query_row("SELECT COUNT(*) FROM artifact_cleanup_moves WHERE state!='removed' AND observed_bytes>0",[],|r|r.get(0))?,retained_bytes:conn.query_row("SELECT COALESCE(SUM(observed_bytes),0) FROM artifact_cleanup_moves WHERE state!='removed'",[],|r|r.get(0))?,unknown_sizes:conn.query_row("SELECT COUNT(*) FROM artifact_cleanup_moves WHERE state!='removed' AND state!='file_removed' AND observed_bytes=0",[],|r|r.get(0))?})
    }
}

impl Library {
    fn cleanup_record(&self, kind: &str, owner: &str, source: &Path) -> Result<Option<Record>> {
        type StoredReceipt = (
            String,
            Option<Vec<u8>>,
            String,
            Option<String>,
            Vec<u8>,
            Vec<u8>,
        );
        let row:Option<StoredReceipt>=self.conn()?.query_row("SELECT id,CASE WHEN typeof(payload_json)='text' AND length(CAST(payload_json AS BLOB))<=131072 THEN CAST(payload_json AS BLOB) ELSE NULL END,state,error,source_path,quarantine_path FROM artifact_cleanup_moves WHERE owner_kind=?1 AND owner_id=?2 AND source_path=?3",params![kind,owner,path_bytes(source)],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).optional()?;
        let Some((id, bytes, state, error, source_key, quarantine_key)) = row else {
            return Ok(None);
        };
        ensure!(
            state != "retained",
            "{}",
            error
                .as_deref()
                .unwrap_or("Unknown cleanup object retained; manual inspection required")
        );
        let decoded: Result<(String, Payload)> = (|| {
            ensure!(
                matches!(
                    state.as_str(),
                    "planned" | "moved" | "file_removed" | "removed"
                ),
                "Unsupported cleanup receipt state"
            );
            let json = String::from_utf8(
                bytes.context("Cleanup receipt exceeds bounded storage or is not text")?,
            )?;
            let payload: Payload = serde_json::from_str(&json)?;
            validate_payload(&payload, &source_key, &quarantine_key)?;
            ensure!(
                source_key == path_bytes(source),
                "Cleanup source lookup differs from receipt key"
            );
            Ok((json, payload))
        })();
        let (json, payload) = match decoded {
            Ok(decoded) => decoded,
            Err(_) => {
                self.conn()?.execute("UPDATE artifact_cleanup_moves SET state='retained',observed_bytes=0,error='Corrupt auxiliary cleanup receipt retained without filesystem access',updated=?1 WHERE id=?2 AND state=?3",params![now(),id,state])?;
                anyhow::bail!(
                    "Corrupt auxiliary cleanup receipt retained without filesystem access"
                );
            }
        };
        Ok(Some(Record {
            id,
            owner_kind: kind.into(),
            owner_id: owner.into(),
            payload,
            json,
            state,
            error,
        }))
    }
    fn save_cleanup_record(
        &self,
        record: &mut Record,
        state: &str,
        error: Option<String>,
        bytes: u64,
    ) -> Result<()> {
        let json = serde_json::to_string(&record.payload)?;
        ensure!(
            json.len() <= 128 * 1024,
            "Cleanup receipt exceeds bounded storage"
        );
        ensure!(self.conn()?.execute("UPDATE artifact_cleanup_moves SET payload_json=?1,quarantine_path=?2,state=?3,observed_bytes=?4,error=?5,updated=?6 WHERE id=?7 AND state=?8 AND payload_json=?9",params![json,path_bytes(&record.payload.parent.path.path()?.join(&record.payload.directory).join("artifact")),state,bytes,error,now(),record.id,record.state,record.json])?==1,"Cleanup receipt changed; all files retained");
        record.json = json;
        record.state = state.into();
        record.error = error;
        Ok(())
    }
    fn retain_unknown_artifact_locked(
        &self,
        kind: &str,
        owner: &str,
        source: &Path,
        parent: &Authority,
        observed: Option<&Fingerprint>,
        reason: &str,
    ) -> Result<()> {
        let payload = Payload {
            source: NativePath::new(source),
            parent: parent.clone(),
            expected: None,
            directory: String::new(),
            private: None,
        };
        let json = serde_json::to_string(&payload)?;
        self.conn()?.execute("INSERT INTO artifact_cleanup_moves(id,owner_kind,owner_id,source_path,quarantine_path,payload_json,state,observed_bytes,error,created,updated) VALUES(?1,?2,?3,?4,?4,?5,'retained',?6,?7,?8,?8) ON CONFLICT(owner_kind,owner_id,source_path) DO UPDATE SET state='retained',observed_bytes=excluded.observed_bytes,error=excluded.error,updated=excluded.updated",params![uuid::Uuid::new_v4().to_string(),kind,owner,path_bytes(source),json,observed.map_or(0,|p|p.bytes),reason.chars().take(2048).collect::<String>(),now()])?;
        Ok(())
    }
    fn quarantine_artifact_impl(
        &self,
        kind: &str,
        owner: &str,
        source: &Path,
        parent: &Authority,
        expected: &Fingerprint,
        mut boundary: impl FnMut(CleanupBoundary) -> Result<()>,
    ) -> Result<()> {
        ensure!(
            !kind.is_empty()
                && kind.len() <= 64
                && kind.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
                && owner.len() <= 128,
            "Invalid artifact owner"
        );
        ensure!(
            source.parent() == Some(parent.path.path()?.as_path()),
            "Cleanup source leaves its captured parent"
        );
        let _gate = self
            .inner
            .artifact_cleanup_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Artifact cleanup gate interrupted"))?;
        let mut record = if let Some(record) = self.cleanup_record(kind, owner, source)? {
            let saved=record.payload.expected.as_ref().context("Unknown cleanup objects are retained; a later equal-byte file cannot establish ownership")?;
            ensure!(
                same(saved, expected)
                    && record.payload.parent.path == parent.path
                    && record.payload.parent.ancestors == parent.ancestors,
                "Cleanup source proof changed; captured receipt is immutable"
            );
            record
        } else {
            let payload = Payload {
                source: NativePath::new(source),
                parent: parent.clone(),
                expected: Some(expected.clone()),
                directory: format!(".cuerithm-cleanup-{}", uuid::Uuid::new_v4()),
                private: None,
            };
            let json = serde_json::to_string(&payload)?;
            let record = Record {
                id: uuid::Uuid::new_v4().to_string(),
                owner_kind: kind.into(),
                owner_id: owner.into(),
                payload,
                json,
                state: "planned".into(),
                error: None,
            };
            self.conn()?.execute("INSERT INTO artifact_cleanup_moves(id,owner_kind,owner_id,source_path,quarantine_path,payload_json,state,observed_bytes,created,updated) VALUES(?1,?2,?3,?4,?5,?6,'planned',?7,?8,?8)",params![record.id,kind,owner,path_bytes(source),path_bytes(&parent.path.path()?.join(&record.payload.directory).join("artifact")),record.json,expected.bytes,now()])?;
            record
        };
        boundary(CleanupBoundary::Intent)?;
        self.execute_artifact_cleanup(&mut record, &mut boundary)
    }
    fn execute_artifact_cleanup(
        &self,
        record: &mut Record,
        boundary: &mut impl FnMut(CleanupBoundary) -> Result<()>,
    ) -> Result<()> {
        if record.state == "removed" {
            return Ok(());
        }
        ensure!(
            record.state != "retained",
            "{}",
            record
                .error
                .as_deref()
                .unwrap_or("Unknown artifact retained; manual inspection required")
        );
        let expected = record
            .payload
            .expected
            .clone()
            .context("Unknown artifacts cannot be adopted")?;
        let source = record.payload.source.path()?;
        let leaf = Path::new(source.file_name().context("Missing cleanup source leaf")?);
        let parent = NativeParent::reopen(&record.payload.parent)?;
        if record.state == "file_removed" {
            return self.finish_cleanup_directory(record, &parent);
        }
        let private = if let Some(authority) = &record.payload.private {
            NativeParent::reopen_private(authority)?
        } else {
            let mut created = None;
            for _ in 0..3 {
                match parent.create_private_directory(Path::new(&record.payload.directory)) {
                    Ok((private, authority)) => {
                        record.payload.private = Some(authority);
                        self.save_cleanup_record(record, "planned", None, expected.bytes)?;
                        created = Some(private);
                        break;
                    }
                    Err(error)
                        if error.downcast_ref::<std::io::Error>().is_some_and(|error| {
                            error.kind() == std::io::ErrorKind::AlreadyExists
                        }) =>
                    {
                        let abandoned = record
                            .payload
                            .parent
                            .path
                            .path()?
                            .join(&record.payload.directory);
                        self.retain_unknown_artifact_locked(
                            &record.owner_kind,
                            &record.owner_id,
                            &abandoned,
                            &record.payload.parent,
                            None,
                            "Unproven cleanup directory retained after interruption",
                        )?;
                        record.payload.directory =
                            format!(".cuerithm-cleanup-{}", uuid::Uuid::new_v4());
                        self.save_cleanup_record(record, "planned", None, expected.bytes)?;
                    }
                    Err(error) => {
                        self.save_cleanup_record(record,"planned",Some(format!("Private cleanup directory unavailable; artifact retained: {error:#}")),expected.bytes)?;
                        return Err(error);
                    }
                }
            }
            created
                .context("Could not reserve a fresh private cleanup directory; artifact retained")?
        };
        boundary(CleanupBoundary::DirectoryReady)?;
        let target = Path::new("artifact");
        let source_present = parent.optional(leaf)?.is_some();
        let captured = private.optional(target)?;
        if let Some(captured) = captured.as_ref().filter(|_| source_present) {
            self.save_cleanup_record(record,"retained",Some("Source leaf reappeared while its quarantine is occupied; both versions retained".into()),captured.bytes)?;
            anyhow::bail!("Both source and quarantine exist; unknown replacement retained")
        }
        if source_present {
            parent.rename(leaf, &private, target)?;
            boundary(CleanupBoundary::SourceMoved)?;
        }
        let Some(actual) = private.optional(target)? else {
            self.save_cleanup_record(record, "file_removed", None, 0)?;
            drop(private);
            return self.finish_cleanup_directory(record, &parent);
        };
        if !same(&actual, &expected) {
            self.save_cleanup_record(
                record,
                "retained",
                Some(
                    "Quarantined object differs from saved native identity or bytes; preserved"
                        .into(),
                ),
                actual.bytes,
            )?;
            anyhow::bail!("Unknown quarantined object retained; cleanup remains pending")
        }
        NativeParent::reopen(&record.payload.parent)?;
        NativeParent::reopen_private(
            record
                .payload
                .private
                .as_ref()
                .context("Missing private cleanup authority")?,
        )?;
        self.save_cleanup_record(record, "moved", None, actual.bytes)?;
        boundary(CleanupBoundary::BeforeDelete)?;
        if private
            .optional(target)?
            .is_some_and(|observed| !same(&observed, &expected))
        {
            self.save_cleanup_record(
                record,
                "retained",
                Some(
                    "Private quarantine leaf changed before retirement; all versions retained"
                        .into(),
                ),
                actual.bytes,
            )?;
            anyhow::bail!("Changed quarantine object retained")
        }
        private.remove_private_leaf(target, &expected)?;
        boundary(CleanupBoundary::Deleted)?;
        self.save_cleanup_record(record, "file_removed", None, 0)?;
        drop(private);
        self.finish_cleanup_directory(record, &parent)
    }
    fn finish_cleanup_directory(&self, record: &mut Record, parent: &NativeParent) -> Result<()> {
        let authority = record
            .payload
            .private
            .as_ref()
            .context("Missing private directory receipt")?;
        parent.remove_private_directory(Path::new(&record.payload.directory), authority)?;
        self.save_cleanup_record(record, "removed", None, 0)
    }
    fn recover_artifact_cleanup_impl(&self, limit: usize) -> Result<ArtifactCleanupStorage> {
        ensure!(
            (1..=200).contains(&limit),
            "Artifact cleanup recovery requires limit1..200"
        );
        let _gate = self
            .inner
            .artifact_cleanup_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Artifact cleanup gate interrupted"))?;
        let records = {
            let conn = self.conn()?;
            let mut q=conn.prepare("SELECT id,owner_kind,owner_id,CASE WHEN typeof(payload_json)='text' AND length(CAST(payload_json AS BLOB))<=131072 THEN CAST(payload_json AS BLOB) ELSE NULL END,state,error,source_path,quarantine_path FROM artifact_cleanup_moves WHERE state NOT IN ('removed','retained') ORDER BY created,id LIMIT ?1")?;
            q.query_map([limit], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<Vec<u8>>>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, Vec<u8>>(6)?,
                    r.get::<_, Vec<u8>>(7)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
        };
        for (id, owner_kind, owner_id, bytes, state, error, source_path, quarantine_path) in records
        {
            let parsed = (|| -> Result<(String, Payload)> {
                ensure!(
                    matches!(state.as_str(), "planned" | "moved" | "file_removed"),
                    "Unsupported cleanup phase"
                );
                let json =
                    String::from_utf8(bytes.context("Invalid or oversized cleanup receipt")?)?;
                let payload: Payload = serde_json::from_str(&json)?;
                validate_payload(&payload, &source_path, &quarantine_path)?;
                Ok((json, payload))
            })();
            let (json, payload) = match parsed {
                Ok(value) => value,
                Err(_) => {
                    self.conn()?.execute("UPDATE artifact_cleanup_moves SET state='retained',observed_bytes=0,error='Corrupt auxiliary cleanup receipt retained without filesystem access',updated=?1 WHERE id=?2 AND state=?3",params![now(),id,state])?;
                    continue;
                }
            };
            let mut record = Record {
                id,
                owner_kind,
                owner_id,
                payload,
                json,
                state,
                error,
            };
            if let Err(error) = self.execute_artifact_cleanup(&mut record, &mut |_| Ok(())) {
                self.conn()?.execute("UPDATE artifact_cleanup_moves SET error=?1,updated=?2 WHERE id=?3 AND state!='removed'",params![format!("{error:#}").chars().take(2048).collect::<String>(),now(),record.id])?;
            }
        }
        self.artifact_cleanup_storage()
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (
        tempfile::TempDir,
        Library,
        std::path::PathBuf,
        Authority,
        Fingerprint,
    ) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let lib = Library::open(&root.join("data/library.sqlite")).unwrap();
        let source = root.join(".cuerithm-stage.tmp");
        std::fs::write(&source, b"known artifact").unwrap();
        let (parent, expected) = crate::organization::capture_regular_source(&source).unwrap();
        (temp, lib, source, parent, expected)
    }
    fn quarantined(lib: &Library) -> std::path::PathBuf {
        let encoded:Vec<u8>=lib.conn().unwrap().query_row("SELECT quarantine_path FROM artifact_cleanup_moves WHERE owner_id='fixture' ORDER BY created LIMIT 1",[],|r|r.get(0)).unwrap();
        crate::model::bytes_path(&encoded).unwrap()
    }
    #[test]
    fn corrupt_direct_receipt_source_key_never_moves_an_unrelated_file() {
        let (_temp, lib, source, parent, expected) = fixture();
        let unrelated = source.with_file_name("unrelated-user-file");
        std::fs::write(&unrelated, b"unrelated user bytes").unwrap();
        assert!(
            lib.quarantine_artifact_with(
                "tag_write",
                "fixture",
                &source,
                &parent,
                &expected,
                |boundary| {
                    if boundary == CleanupBoundary::Intent {
                        anyhow::bail!("fault before any filesystem operation")
                    }
                    Ok(())
                }
            )
            .is_err()
        );
        let json: String = lib
            .conn()
            .unwrap()
            .query_row(
                "SELECT payload_json FROM artifact_cleanup_moves WHERE owner_id='fixture'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let mut payload: Payload = serde_json::from_str(&json).unwrap();
        payload.source = NativePath::new(&unrelated);
        let corrupt = serde_json::to_string(&payload).unwrap();
        lib.conn()
            .unwrap()
            .execute(
                "UPDATE artifact_cleanup_moves SET payload_json=?1 WHERE owner_id='fixture'",
                [&corrupt],
            )
            .unwrap();
        assert!(
            lib.quarantine_artifact("tag_write", "fixture", &source, &parent, &expected)
                .is_err()
        );
        assert!(
            unrelated.exists(),
            "direct caller must validate source SQL key before even moving unknown files"
        );
        assert!(source.exists());
        let saved: String = lib
            .conn()
            .unwrap()
            .query_row(
                "SELECT payload_json FROM artifact_cleanup_moves WHERE owner_id='fixture'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            saved, corrupt,
            "corrupt auxiliary receipt remains available for diagnosis"
        );
        lib.recover_artifact_cleanup(100).unwrap();
        assert_eq!(std::fs::read(&unrelated).unwrap(), b"unrelated user bytes");
    }
    #[test]
    fn corrupt_auxiliary_receipt_does_not_touch_paths_or_block_other_cleanup() {
        let (_temp, lib, source, parent, expected) = fixture();
        assert!(
            lib.quarantine_artifact_with(
                "tag_write",
                "fixture",
                &source,
                &parent,
                &expected,
                |boundary| {
                    if boundary == CleanupBoundary::Intent {
                        anyhow::bail!("stop after intent")
                    }
                    Ok(())
                }
            )
            .is_err()
        );
        let sentinel = source.with_extension("unrelated");
        std::fs::write(&sentinel, b"unrelated user data").unwrap();
        let bad = "{not valid JSON";
        lib.conn().unwrap().execute("INSERT INTO artifact_cleanup_moves(id,owner_kind,owner_id,source_path,quarantine_path,payload_json,state,observed_bytes,created,updated) VALUES('corrupt','tag_write','corrupt',?1,?1,?2,'planned',0,0,0)",params![path_bytes(&sentinel),bad]).unwrap();
        let storage = lib.recover_artifact_cleanup(32).expect(
            "one corrupt auxiliary cleanup receipt must not prevent valid cleanup or offline open",
        );
        assert!(!source.exists());
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"unrelated user data");
        let (json, state, error): (String, String, String) = lib
            .conn()
            .unwrap()
            .query_row(
                "SELECT payload_json,state,error FROM artifact_cleanup_moves WHERE id='corrupt'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(json, bad);
        assert_eq!(state, "retained");
        assert!(error.contains("receipt"));
        assert_eq!(storage.pending, 1);
        assert_eq!(storage.unknown_sizes, 1);
    }
    #[test]
    fn each_interruption_around_move_and_delete_recovers_without_adopting_another_object() {
        for stop in [
            CleanupBoundary::Intent,
            CleanupBoundary::DirectoryReady,
            CleanupBoundary::SourceMoved,
            CleanupBoundary::BeforeDelete,
            CleanupBoundary::Deleted,
        ] {
            let (_temp, lib, source, parent, expected) = fixture();
            let result = lib.quarantine_artifact_with(
                "tag_write",
                "fixture",
                &source,
                &parent,
                &expected,
                |boundary| {
                    if boundary == stop {
                        anyhow::bail!("injected boundary fault")
                    }
                    Ok(())
                },
            );
            assert!(result.is_err());
            let target = quarantined(&lib);
            let database = lib.data_dir().join("library.sqlite");
            drop(lib);
            let lib = Library::open(&database).unwrap();
            lib.recover_artifact_cleanup(32).unwrap();
            assert!(!source.exists(), "{stop:?}");
            assert!(!target.exists(), "{stop:?}");
            assert!(
                !target.parent().unwrap().exists(),
                "owned empty cleanup directory must retire: {stop:?}"
            );
            assert_eq!(
                lib.artifact_cleanup_storage().unwrap().pending,
                0,
                "{stop:?}"
            );
        }
    }
    #[test]
    fn unknown_equal_byte_source_is_captured_and_retained_instead_of_deleted() {
        let (_temp, lib, source, parent, expected) = fixture();
        let original = source.with_extension("original");
        std::fs::rename(&source, &original).unwrap();
        std::fs::write(&source, b"known artifact").unwrap();
        let (_, unknown) = crate::organization::capture_regular_source(&source).unwrap();
        assert!(
            lib.quarantine_artifact("tag_write", "fixture", &source, &parent, &expected)
                .is_err()
        );
        let target = quarantined(&lib);
        let (_, captured) = crate::organization::capture_regular_source(&target).unwrap();
        assert_eq!(captured.identity, unknown.identity);
        assert!(original.exists());
        lib.recover_artifact_cleanup(32).unwrap();
        assert!(target.exists());
        assert_eq!(lib.artifact_cleanup_storage().unwrap().retained_files, 1);
    }
    #[test]
    fn replacement_inside_quarantine_before_delete_remains_with_original() {
        let (_temp, lib, source, parent, expected) = fixture();
        let result = lib.quarantine_artifact_with(
            "tag_write",
            "fixture",
            &source,
            &parent,
            &expected,
            |boundary| {
                if boundary == CleanupBoundary::BeforeDelete {
                    let target = quarantined(&lib);
                    std::fs::rename(&target, target.with_extension("known-original"))?;
                    std::fs::write(&target, b"known artifact")?;
                }
                Ok(())
            },
        );
        assert!(result.is_err());
        let target = quarantined(&lib);
        assert_eq!(std::fs::read(&target).unwrap(), b"known artifact");
        assert!(target.with_extension("known-original").exists());
        assert_eq!(lib.artifact_cleanup_storage().unwrap().pending, 1);
    }
    #[cfg(unix)]
    #[test]
    fn ancestor_replacement_never_mutates_its_equal_byte_replacement_leaf() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let lib = Library::open(&root.join("data/library.sqlite")).unwrap();
        let music = root.join("music");
        let moved = root.join("moved");
        std::fs::create_dir(&music).unwrap();
        let source = music.join(".cuerithm-stage.tmp");
        std::fs::write(&source, b"known artifact").unwrap();
        let (parent, expected) = crate::organization::capture_regular_source(&source).unwrap();
        let result = lib.quarantine_artifact_with(
            "tag_write",
            "fixture",
            &source,
            &parent,
            &expected,
            |boundary| {
                if boundary == CleanupBoundary::DirectoryReady {
                    std::fs::rename(&music, &moved)?;
                    std::fs::create_dir(&music)?;
                    std::fs::write(&source, b"known artifact")?;
                }
                Ok(())
            },
        );
        assert!(result.is_err());
        assert_eq!(std::fs::read(&source).unwrap(), b"known artifact");
        let target = quarantined(&lib);
        let retained = moved.join(target.strip_prefix(&music).unwrap());
        assert_eq!(std::fs::read(retained).unwrap(), b"known artifact");
        assert_eq!(lib.artifact_cleanup_storage().unwrap().pending, 1);
    }
    #[test]
    fn owned_artifact_moves_through_durable_private_quarantine() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let lib = Library::open(&root.join("data/library.sqlite")).unwrap();
        let source = root.join(".cuerithm-stage.tmp");
        std::fs::write(&source, b"known publication artifact").unwrap();
        let (parent, expected) = crate::organization::capture_regular_source(&source).unwrap();
        lib.quarantine_artifact("tag_write", "fixture", &source, &parent, &expected)
            .expect("owned artifacts must be safely quarantined and retired");
        assert!(!source.exists());
        let state: String = lib
            .conn()
            .unwrap()
            .query_row("SELECT state FROM artifact_cleanup_moves", [], |r| r.get(0))
            .unwrap();
        assert_eq!(state, "removed");
        assert_eq!(lib.artifact_cleanup_storage().unwrap().pending, 0);
    }
}
