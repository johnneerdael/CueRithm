use super::*;
use cuerithm_credentials::{
    CredentialError, CredentialService, ProviderId, Secret, Vault, VaultError,
    state::{FileStateStore, StateError},
};
#[derive(Clone, Default)]
struct FakeVault(Arc<Mutex<BTreeMap<ProviderId, String>>>);
impl Vault for FakeVault {
    fn contains(&self, p: ProviderId) -> std::result::Result<bool, VaultError> {
        Ok(self.0.lock().unwrap().contains_key(&p))
    }
    fn load(&self, p: ProviderId) -> std::result::Result<Option<Secret>, VaultError> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .get(&p)
            .map(|v| Secret::new(v.clone()).unwrap()))
    }
    fn store(&self, p: ProviderId, s: &Secret) -> std::result::Result<(), VaultError> {
        self.0.lock().unwrap().insert(p, s.expose().into());
        Ok(())
    }
    fn remove(&self, p: ProviderId) -> std::result::Result<(), VaultError> {
        self.0.lock().unwrap().remove(&p);
        Ok(())
    }
}
fn fixture() -> (tempfile::TempDir, PathBuf) {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().canonicalize().unwrap().join("provider-state");
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(&root).unwrap();
    (t, root)
}
fn authority(root: &Path, vault: FakeVault) -> (Arc<CatalogRuntime>, Arc<ApplicationCatalogState>) {
    let path = root.to_path_buf();
    runtime_at(Some(path.clone()), "test", move || {
        Ok(Arc::new(CredentialService::new(
            vault,
            Arc::new(
                FileStateStore::open(&path)
                    .map_err(|_| anyhow::anyhow!("Fixture marker unavailable"))?,
            ),
        )?) as Arc<dyn CatalogCredentials>)
    })
    .unwrap()
}
fn claim() -> RateClaim {
    RateClaim {
        service: cuerithm_autotag::CatalogService::Discogs,
        now_ms: 1000,
        min_interval_ms: 0,
        window_ms: 60000,
        max_starts: 25,
    }
}
#[test]
fn separate_libraries_share_one_budget_and_restart_does_not_reset_it() {
    let (_t, root) = fixture();
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let vault = FakeVault::default();
    let liba = crate::Library::open(&a.path().join("library.db")).unwrap();
    let libb = crate::Library::open(&b.path().join("library.db")).unwrap();
    let (ra, sa) = authority(&root, vault.clone());
    let pa = liba.provider_runtime_with(ra.clone()).unwrap();
    let (rb, sb) = authority(&root, vault.clone());
    let pb = libb.provider_runtime_with(rb.clone()).unwrap();
    assert!(Arc::ptr_eq(&ra, &rb));
    for _ in 0..15 {
        assert_eq!(sa.claim_start(&claim()).unwrap(), StartDecision::Granted)
    }
    for _ in 0..10 {
        assert_eq!(sb.claim_start(&claim()).unwrap(), StartDecision::Granted)
    }
    assert_eq!(
        sb.claim_start(&claim()).unwrap(),
        StartDecision::WaitUntil(61000)
    );
    drop((pa, pb, ra, rb, sa, sb, liba, libb));
    let c = tempfile::tempdir().unwrap();
    let libc = crate::Library::open(&c.path().join("library.db")).unwrap();
    let (rc, sc) = authority(&root, vault);
    let _pc = libc.provider_runtime_with(rc).unwrap();
    assert_eq!(
        sc.claim_start(&claim()).unwrap(),
        StartDecision::WaitUntil(61000),
        "Closing one library must not reset the application budget for another"
    );
}
#[test]
fn global_cache_and_manual_hold_survive_a_new_library() {
    let (_t, root) = fixture();
    let vault = FakeVault::default();
    let (r, s) = authority(&root, vault.clone());
    let key = "a".repeat(64);
    s.cache_put(
        &key,
        &CachedResponse {
            body: b"{}".to_vec(),
            expires_ms: 99999,
        },
    )
    .unwrap();
    s.observe(&RateObservation {
        service: claim().service,
        manual_hold: Some(true),
        not_before_ms: None,
        limit: None,
        remaining: None,
        observed_ms: 1000,
    })
    .unwrap();
    drop((r, s));
    let (_r, s) = authority(&root, vault);
    assert_eq!(s.cache_get(&key, 1001).unwrap().unwrap().body, b"{}");
    assert_eq!(
        s.claim_start(&claim()).unwrap(),
        StartDecision::WaitUntil(i64::MAX)
    );
}
#[test]
fn separate_library_mutation_invalidates_old_reference_and_competing_private_owner_is_busy() {
    let (_t, root) = fixture();
    let vault = FakeVault::default();
    let (ra, _sa) = authority(&root, vault.clone());
    let old = ra.credential_status(ProviderId::Discogs).unwrap();
    let saved = ra
        .credential_save(&old.reference, Secret::new("fixture-first".into()).unwrap())
        .unwrap();
    let (rb, sb) = authority(&root, vault.clone());
    let next = rb
        .credential_save(
            &saved.reference,
            Secret::new("fixture-second".into()).unwrap(),
        )
        .unwrap();
    assert_ne!(saved.reference, next.reference);
    assert!(ra.with_credential(&saved.reference, |_, _| Ok(())).is_err());
    let other = CredentialService::new(vault, Arc::new(FileStateStore::open(&root).unwrap()));
    assert!(matches!(
        other,
        Err(CredentialError::State(StateError::Busy))
    ));
    assert_eq!(
        sb.claim_start(&claim()).unwrap(),
        StartDecision::Granted,
        "Private ownership contention must not block public catalog pacing"
    );
}

#[test]
fn global_hold_clear_preserves_original_server_exhaustion_and_ignores_stale_clear() {
    let (_t, root) = fixture();
    let vault = FakeVault::default();
    let (r, s) = authority(&root, vault.clone());
    s.observe(&RateObservation {
        service: claim().service,
        manual_hold: Some(true),
        not_before_ms: None,
        limit: Some(25),
        remaining: Some(0),
        observed_ms: 1000,
    })
    .unwrap();
    drop((r, s));
    let (_r, s) = authority(&root, vault);
    s.observe(&RateObservation {
        service: claim().service,
        manual_hold: Some(false),
        not_before_ms: None,
        limit: None,
        remaining: None,
        observed_ms: 500,
    })
    .unwrap();
    assert_eq!(
        s.claim_start(&RateClaim {
            now_ms: 2000,
            ..claim()
        })
        .unwrap(),
        StartDecision::WaitUntil(i64::MAX)
    );
    s.observe(&RateObservation {
        service: claim().service,
        manual_hold: Some(false),
        not_before_ms: None,
        limit: None,
        remaining: None,
        observed_ms: 2000,
    })
    .unwrap();
    assert_eq!(
        s.claim_start(&RateClaim {
            now_ms: 2000,
            ..claim()
        })
        .unwrap(),
        StartDecision::WaitUntil(61000)
    );
    assert_eq!(
        s.claim_start(&RateClaim {
            now_ms: 61000,
            ..claim()
        })
        .unwrap(),
        StartDecision::Granted
    );
}
#[test]
fn unknown_global_schema_blocks_network_without_blocking_offline_library() {
    let (_t, root) = fixture();
    let conn = open_connection(&root).unwrap();
    conn.pragma_update(None, "user_version", 99).unwrap();
    drop(conn);
    let (r, _s) = authority(&root, FakeVault::default());
    let library_dir = tempfile::tempdir().unwrap();
    let library = crate::Library::open(&library_dir.path().join("library.db")).unwrap();
    let setup = library.provider_runtime_with(r).unwrap();
    let status = setup
        .status(cuerithm_autotag::ProviderId::Musicbrainz)
        .unwrap();
    assert!(!status.can_use);
    assert_eq!(status.reason.unwrap().code, "catalog_state_unavailable");
    assert_eq!(library.counts().unwrap().tracks, 0);
    let conn = Connection::open(root.join("catalog.sqlite3")).unwrap();
    assert_eq!(
        conn.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        99
    );
}
#[cfg(unix)]
#[test]
fn global_state_never_follows_a_database_symlink() {
    let (t, root) = fixture();
    let target = t.path().join("untouched");
    std::fs::write(&target, b"unknown bytes").unwrap();
    std::os::unix::fs::symlink(&target, root.join("catalog.sqlite3")).unwrap();
    let state = ApplicationCatalogState::at(Some(root));
    assert!(state.claim_start(&claim()).is_err());
    assert_eq!(std::fs::read(target).unwrap(), b"unknown bytes");
}
