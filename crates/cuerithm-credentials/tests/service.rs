use cuerithm_credentials::*;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
#[derive(Default, Clone)]
struct Fake(Arc<Mutex<BTreeMap<ProviderId, String>>>);
impl Vault for Fake {
    fn contains(&self, p: ProviderId) -> Result<bool, VaultError> {
        Ok(self.0.lock().unwrap().contains_key(&p))
    }
    fn load(&self, p: ProviderId) -> Result<Option<Secret>, VaultError> {
        Ok(self.0.lock().unwrap().get(&p).map(|s| {
            if matches!(p, ProviderId::BpmSupreme | ProviderId::Spotify) {
                Secret::structured(s.clone()).unwrap()
            } else {
                Secret::new(s.clone()).unwrap()
            }
        }))
    }
    fn store(&self, p: ProviderId, s: &Secret) -> Result<(), VaultError> {
        self.0.lock().unwrap().insert(p, s.expose().into());
        Ok(())
    }
    fn remove(&self, p: ProviderId) -> Result<(), VaultError> {
        self.0.lock().unwrap().remove(&p);
        Ok(())
    }
}
#[test]
fn saved_keys_have_presence_only_dtos_and_redacted_debug() {
    let service = new_service(Fake::default());
    let initial = service.status(ProviderId::SonoVault).unwrap();
    let secret = Secret::new("fixture-secret".into()).unwrap();
    assert!(!format!("{secret:?}").contains("fixture-secret"));
    let saved = service.save(&initial.reference, secret).unwrap();
    assert_eq!(saved.presence, Presence::Present);
    assert_ne!(initial.reference, saved.reference);
    let json = serde_json::to_string(&saved).unwrap();
    assert!(!json.contains("fixture-secret"));
    assert!(!json.contains("api_key"));
    service
        .run(&saved.reference, |key, _| assert_eq!(key, "fixture-secret"))
        .unwrap();
}
#[test]
fn removal_rejects_queued_old_save_and_work() {
    let service = new_service(Fake::default());
    let old = service.status(ProviderId::AudD).unwrap().reference;
    service.remove(ProviderId::AudD).unwrap();
    assert_eq!(
        service.save(&old, Secret::new("fixture-old".into()).unwrap()),
        Err(CredentialError::Stale)
    );
    assert_eq!(service.run(&old, |_, _| ()), Err(CredentialError::Stale));
    assert_eq!(
        service.status(ProviderId::AudD).unwrap().presence,
        Presence::Missing
    );
}
#[test]
fn blank_control_and_oversized_keys_are_rejected() {
    for value in [
        "".into(),
        " ".into(),
        "one\ntwo".into(),
        "one two".into(),
        "x".repeat(4097),
    ] {
        assert!(matches!(
            Secret::new(value),
            Err(CredentialError::InvalidKey)
        ));
    }
}
#[test]
fn verification_is_bound_to_current_generation() {
    let service = new_service(Fake::default());
    let first = service.status(ProviderId::RapidApi).unwrap();
    let first = service
        .save(&first.reference, Secret::new("fixture-one".into()).unwrap())
        .unwrap();
    let verified = service
        .verify(&first.reference, |_, _| Verification::Valid)
        .unwrap();
    assert_eq!(verified.verification, Verification::Valid);
    let replacement = service
        .save(&first.reference, Secret::new("fixture-two".into()).unwrap())
        .unwrap();
    assert_eq!(replacement.verification, Verification::Untested);
    assert_eq!(
        service.run(&first.reference, |_, _| ()),
        Err(CredentialError::Stale)
    );
}

#[test]
fn removal_cancels_inflight_work_and_suppresses_stale_verification() {
    let service = new_service(Fake::default());
    let initial = service.status(ProviderId::SonoVault).unwrap();
    let saved = service
        .save(
            &initial.reference,
            Secret::new("fixture-token".into()).unwrap(),
        )
        .unwrap();
    let result = service.verify(&saved.reference, |_, revocation| {
        assert!(!revocation.is_revoked());
        service.remove(ProviderId::SonoVault).unwrap();
        assert!(revocation.is_revoked());
        Verification::Valid
    });
    assert_eq!(result, Err(CredentialError::Stale));
    assert_eq!(
        service.status(ProviderId::SonoVault).unwrap().presence,
        Presence::Missing
    );
}
#[test]
fn restart_does_not_accept_old_job_references() {
    let vault = Fake::default();
    let service = new_service(vault.clone());
    let reference = service.status(ProviderId::AudD).unwrap().reference;
    let saved = service
        .save(&reference, Secret::new("fixture-token".into()).unwrap())
        .unwrap();
    let restarted = new_service(vault);
    assert_eq!(
        restarted.status(ProviderId::AudD).unwrap().presence,
        Presence::Present
    );
    assert_eq!(
        restarted.run(&saved.reference, |_, _| ()),
        Err(CredentialError::Stale)
    );
}
struct Failing {
    inner: Fake,
    fail: Arc<std::sync::atomic::AtomicBool>,
}
impl Vault for Failing {
    fn contains(&self, p: ProviderId) -> Result<bool, VaultError> {
        if self.fail.load(std::sync::atomic::Ordering::SeqCst) {
            Err(VaultError::Locked)
        } else {
            self.inner.contains(p)
        }
    }
    fn load(&self, p: ProviderId) -> Result<Option<Secret>, VaultError> {
        if self.fail.load(std::sync::atomic::Ordering::SeqCst) {
            Err(VaultError::Locked)
        } else {
            self.inner.load(p)
        }
    }
    fn store(&self, p: ProviderId, s: &Secret) -> Result<(), VaultError> {
        self.inner.store(p, s)?;
        if self.fail.load(std::sync::atomic::Ordering::SeqCst) {
            Err(VaultError::Unavailable)
        } else {
            Ok(())
        }
    }
    fn remove(&self, p: ProviderId) -> Result<(), VaultError> {
        if self.fail.load(std::sync::atomic::Ordering::SeqCst) {
            Err(VaultError::Locked)
        } else {
            self.inner.remove(p)
        }
    }
}
#[test]
fn partial_vault_mutation_stays_fail_closed_until_explicit_replacement() {
    let fail = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let service = new_service(Failing {
        inner: Fake::default(),
        fail: fail.clone(),
    });
    let original = service.status(ProviderId::SonoVault).unwrap();
    fail.store(true, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        service.save(
            &original.reference,
            Secret::new("fixture-uncertain".into()).unwrap()
        ),
        Err(CredentialError::Vault(VaultError::Unavailable))
    );
    let uncertain = service.status(ProviderId::SonoVault).unwrap();
    assert_eq!(uncertain.vault, VaultState::Uncertain);
    assert_eq!(
        service.run(&uncertain.reference, |_, _| ()),
        Err(CredentialError::MutationUncertain)
    );
    fail.store(false, std::sync::atomic::Ordering::SeqCst);
    let saved = service
        .save(
            &uncertain.reference,
            Secret::new("fixture-known".into()).unwrap(),
        )
        .unwrap();
    assert_eq!(saved.vault, VaultState::Ready);
}
#[test]
fn unavailable_vault_status_is_not_reported_as_missing() {
    let fail = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let service = new_service(Failing {
        inner: Fake::default(),
        fail,
    });
    let status = service.status(ProviderId::RapidApi).unwrap();
    assert_eq!(status.presence, Presence::Unknown);
    assert_eq!(status.vault, VaultState::Locked);
}
#[test]
fn concurrent_removal_revokes_started_transport_and_old_queued_save() {
    let service = new_service(Fake::default());
    let status = service.status(ProviderId::AudD).unwrap();
    let saved = service
        .save(
            &status.reference,
            Secret::new("fixture-token".into()).unwrap(),
        )
        .unwrap();
    let worker = service.clone();
    let reference = saved.reference.clone();
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (resume_tx, resume_rx) = std::sync::mpsc::channel();
    let task = std::thread::spawn(move || {
        worker.run(&reference, |_, revocation| {
            started_tx.send(()).unwrap();
            resume_rx.recv().unwrap();
            assert!(revocation.is_revoked());
        })
    });
    started_rx.recv().unwrap();
    service.remove(ProviderId::AudD).unwrap();
    resume_tx.send(()).unwrap();
    assert_eq!(task.join().unwrap(), Err(CredentialError::Stale));
    assert_eq!(
        service.save(&saved.reference, Secret::new("fixture-old".into()).unwrap()),
        Err(CredentialError::Stale)
    );
}

#[test]
fn provider_ids_match_catalog_identifiers() {
    for (provider, name) in [
        (ProviderId::SonoVault, "sonovault"),
        (ProviderId::AudD, "audd"),
        (ProviderId::RapidApi, "rapidapi"),
    ] {
        assert_eq!(serde_json::to_value(provider).unwrap(), name);
    }
}

use cuerithm_credentials::state::*;
struct FailAck {
    file: Arc<FileStateStore>,
    fail: std::sync::atomic::AtomicBool,
}
impl StateStore for FailAck {
    fn claim_authority(&self) -> Result<Box<dyn Send + Sync>, StateError> {
        self.file.claim_authority()
    }
    fn load_or_create(&self, p: ProviderId) -> Result<MutationState, StateError> {
        self.file.load_or_create(p)
    }
    fn compare_exchange(
        &self,
        p: ProviderId,
        old: &MutationState,
        new: &MutationState,
    ) -> Result<(), StateError> {
        if new.pending.is_none() && self.fail.swap(false, std::sync::atomic::Ordering::SeqCst) {
            Err(StateError::Unavailable)
        } else {
            self.file.compare_exchange(p, old, new)
        }
    }
}
#[test]
fn crash_before_ack_keeps_saved_key_unusable_after_restart() {
    let dir = tempfile::tempdir().unwrap();
    let file = Arc::new(FileStateStore::open(dir.path()).unwrap());
    let vault = Fake::default();
    let fault = Arc::new(FailAck {
        file: file.clone(),
        fail: std::sync::atomic::AtomicBool::new(true),
    });
    let service = CredentialService::new(vault.clone(), fault).unwrap();
    let initial = service.status(ProviderId::SonoVault).unwrap();
    assert!(
        service
            .save(
                &initial.reference,
                Secret::new("fixture-no-disk-key".into()).unwrap()
            )
            .is_err()
    );
    drop(service);
    let restarted = CredentialService::new(vault, file.clone()).unwrap();
    let status = restarted.status(ProviderId::SonoVault).unwrap();
    let marker = std::fs::read_to_string(dir.path().join("credentials-state/state.json")).unwrap();
    assert!(!marker.contains("fixture-no-disk-key"));
    assert!(!marker.contains("api_key"));
    assert_eq!(status.vault, VaultState::Uncertain);
    assert_eq!(
        restarted.run(&status.reference, |_, _| ()),
        Err(CredentialError::MutationUncertain)
    );
    assert_eq!(
        restarted.save(
            &initial.reference,
            Secret::new("fixture-stale".into()).unwrap()
        ),
        Err(CredentialError::Stale)
    );
    let repaired = restarted
        .save(
            &status.reference,
            Secret::new("fixture-repaired".into()).unwrap(),
        )
        .unwrap();
    assert_eq!(repaired.vault, VaultState::Ready);
    assert_eq!(
        file.load_or_create(ProviderId::SonoVault).unwrap().pending,
        None
    );
}
#[test]
fn failed_delete_intent_survives_restart_and_cannot_reuse_old_key() {
    let dir = tempfile::tempdir().unwrap();
    let file = Arc::new(FileStateStore::open(dir.path()).unwrap());
    let fail = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let vault = Fake::default();
    let service = CredentialService::new(
        Failing {
            inner: vault.clone(),
            fail: fail.clone(),
        },
        file.clone(),
    )
    .unwrap();
    let initial = service.status(ProviderId::AudD).unwrap();
    let saved = service
        .save(
            &initial.reference,
            Secret::new("fixture-old-key".into()).unwrap(),
        )
        .unwrap();
    fail.store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(service.remove(ProviderId::AudD).is_err());
    drop(service);
    let restarted = CredentialService::new(vault.clone(), file).unwrap();
    let status = restarted.status(ProviderId::AudD).unwrap();
    assert_eq!(status.vault, VaultState::Uncertain);
    assert_eq!(
        restarted.run(&status.reference, |_, _| ()),
        Err(CredentialError::MutationUncertain)
    );
    assert_eq!(
        restarted.run(&saved.reference, |_, _| ()),
        Err(CredentialError::Stale)
    );
    assert_eq!(
        restarted.remove(ProviderId::AudD).unwrap().presence,
        Presence::Missing
    );
    assert!(!vault.contains(ProviderId::AudD).unwrap());
}
#[test]
fn file_state_is_nonsecret_and_survives_a_new_store_instance() {
    let dir = tempfile::tempdir().unwrap();
    let file = FileStateStore::open(dir.path()).unwrap();
    let initial = file.load_or_create(ProviderId::RapidApi).unwrap();
    let pending = MutationState {
        generation: uuid::Uuid::new_v4(),
        pending: Some(MutationKind::Remove),
    };
    file.compare_exchange(ProviderId::RapidApi, &initial, &pending)
        .unwrap();
    drop(file);
    let reopened = FileStateStore::open(dir.path()).unwrap();
    assert_eq!(
        reopened.load_or_create(ProviderId::RapidApi).unwrap(),
        pending
    );
    let bytes = std::fs::read(dir.path().join("credentials-state/state.json")).unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert!(!text.contains("fixture-"));
    assert!(!text.contains("api_key"));
    assert!(text.contains("generation"));
}

#[derive(Default)]
struct MemoryState(Mutex<BTreeMap<ProviderId, MutationState>>);
impl StateStore for MemoryState {
    fn claim_authority(&self) -> Result<Box<dyn Send + Sync>, StateError> {
        Ok(Box::new(()))
    }
    fn load_or_create(&self, p: ProviderId) -> Result<MutationState, StateError> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .entry(p)
            .or_insert_with(|| MutationState {
                generation: uuid::Uuid::new_v4(),
                pending: None,
            })
            .clone())
    }
    fn compare_exchange(
        &self,
        p: ProviderId,
        old: &MutationState,
        new: &MutationState,
    ) -> Result<(), StateError> {
        let mut state = self.0.lock().unwrap();
        if state.get(&p) != Some(old) {
            return Err(StateError::Conflict);
        }
        state.insert(p, new.clone());
        Ok(())
    }
}
fn new_service<V: Vault>(vault: V) -> CredentialService<V> {
    CredentialService::new(vault, Arc::new(MemoryState::default())).unwrap()
}

#[test]
fn live_file_authority_cannot_be_opened_twice() {
    let dir = tempfile::tempdir().unwrap();
    let state = Arc::new(FileStateStore::open(dir.path()).unwrap());
    let first = CredentialService::new(Fake::default(), state.clone()).unwrap();
    assert!(CredentialService::new(Fake::default(), state.clone()).is_err());
    drop(first);
    assert!(CredentialService::new(Fake::default(), state).is_ok());
}
struct FailIntent(Arc<FileStateStore>);
impl StateStore for FailIntent {
    fn claim_authority(&self) -> Result<Box<dyn Send + Sync>, StateError> {
        self.0.claim_authority()
    }
    fn load_or_create(&self, p: ProviderId) -> Result<MutationState, StateError> {
        self.0.load_or_create(p)
    }
    fn compare_exchange(
        &self,
        _: ProviderId,
        _: &MutationState,
        _: &MutationState,
    ) -> Result<(), StateError> {
        Err(StateError::Unavailable)
    }
}
#[test]
fn intent_persistence_failure_does_not_touch_vault() {
    let dir = tempfile::tempdir().unwrap();
    let file = Arc::new(FileStateStore::open(dir.path()).unwrap());
    let vault = Fake::default();
    let service = CredentialService::new(vault.clone(), Arc::new(FailIntent(file))).unwrap();
    let reference = service.status(ProviderId::SonoVault).unwrap().reference;
    assert!(
        service
            .save(
                &reference,
                Secret::new("fixture-must-not-save".into()).unwrap()
            )
            .is_err()
    );
    assert!(!vault.contains(ProviderId::SonoVault).unwrap());
}
#[test]
fn corrupt_marker_state_is_never_silently_reinitialized() {
    let dir = tempfile::tempdir().unwrap();
    let state = FileStateStore::open(dir.path()).unwrap();
    state.load_or_create(ProviderId::AudD).unwrap();
    let path = dir.path().join("credentials-state/state.json");
    std::fs::write(&path, b"{broken").unwrap();
    assert_eq!(
        state.load_or_create(ProviderId::AudD),
        Err(StateError::Corrupt)
    );
    assert!(CredentialService::new(Fake::default(), Arc::new(state)).is_err());
}

#[test]
fn deleted_key_without_ack_stays_uncertain_until_explicit_retry() {
    let dir = tempfile::tempdir().unwrap();
    let file = Arc::new(FileStateStore::open(dir.path()).unwrap());
    let fault = Arc::new(FailAck {
        file: file.clone(),
        fail: std::sync::atomic::AtomicBool::new(false),
    });
    let vault = Fake::default();
    let service = CredentialService::new(vault.clone(), fault.clone()).unwrap();
    let initial = service.status(ProviderId::SonoVault).unwrap();
    let saved = service
        .save(
            &initial.reference,
            Secret::new("fixture-deleted".into()).unwrap(),
        )
        .unwrap();
    fault.fail.store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(service.remove(ProviderId::SonoVault).is_err());
    assert!(!vault.contains(ProviderId::SonoVault).unwrap());
    drop(service);
    let restarted = CredentialService::new(vault.clone(), file).unwrap();
    let status = restarted.status(ProviderId::SonoVault).unwrap();
    assert_eq!(status.vault, VaultState::Uncertain);
    assert_eq!(
        restarted.save(
            &saved.reference,
            Secret::new("fixture-resurrect".into()).unwrap()
        ),
        Err(CredentialError::Stale)
    );
    assert_eq!(
        restarted.remove(ProviderId::SonoVault).unwrap().presence,
        Presence::Missing
    );
    assert!(!vault.contains(ProviderId::SonoVault).unwrap());
}
struct PanicVault(Fake);
impl Vault for PanicVault {
    fn contains(&self, p: ProviderId) -> Result<bool, VaultError> {
        self.0.contains(p)
    }
    fn load(&self, p: ProviderId) -> Result<Option<Secret>, VaultError> {
        self.0.load(p)
    }
    fn store(&self, _: ProviderId, _: &Secret) -> Result<(), VaultError> {
        panic!("injected crash before OS write")
    }
    fn remove(&self, p: ProviderId) -> Result<(), VaultError> {
        self.0.remove(p)
    }
}
#[test]
fn interruption_after_intent_before_native_write_fails_closed_on_restart() {
    let dir = tempfile::tempdir().unwrap();
    let file = Arc::new(FileStateStore::open(dir.path()).unwrap());
    let vault = Fake::default();
    let service = CredentialService::new(PanicVault(vault.clone()), file.clone()).unwrap();
    let reference = service.status(ProviderId::AudD).unwrap().reference;
    let crash = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        service.save(
            &reference,
            Secret::new("fixture-never-written".into()).unwrap(),
        )
    }));
    assert!(crash.is_err());
    drop(service);
    let restarted = CredentialService::new(vault, file).unwrap();
    let status = restarted.status(ProviderId::AudD).unwrap();
    assert_eq!(status.vault, VaultState::Uncertain);
    assert_eq!(
        restarted.run(&status.reference, |_, _| ()),
        Err(CredentialError::MutationUncertain)
    );
}
#[cfg(unix)]
#[test]
fn marker_symlink_is_rejected_without_following_or_replacing_target() {
    let dir = tempfile::tempdir().unwrap();
    let store = FileStateStore::open(dir.path()).unwrap();
    let outside = dir.path().join("outside");
    std::fs::write(&outside, b"sentinel").unwrap();
    std::os::unix::fs::symlink(&outside, dir.path().join("credentials-state/state.json")).unwrap();
    assert_eq!(
        store.load_or_create(ProviderId::SonoVault),
        Err(StateError::UnsafePath)
    );
    assert_eq!(std::fs::read(outside).unwrap(), b"sentinel");
}

#[test]
fn structured_account_password_spaces_preserve_token_validation() {
    let secret =
        Secret::structured(r#"{"email":"fixture@example.invalid","password":"two words"}"#.into())
            .unwrap();
    let value: serde_json::Value = serde_json::from_str(secret.expose()).unwrap();
    assert_eq!(value["password"], "two words");
    assert!(Secret::new("two words".into()).is_err());
    assert!(Secret::structured("[]".into()).is_err());
    assert!(Secret::structured("{}".into()).is_err());
    assert!(!format!("{secret:?}").contains("two words"));
    let service = new_service(Fake::default());
    let old = service.status(ProviderId::BpmSupreme).unwrap();
    let saved = service.save(&old.reference, secret).unwrap();
    service
        .run(&saved.reference, |value, _| {
            let parsed: serde_json::Value = serde_json::from_str(value).unwrap();
            assert_eq!(parsed["password"], "two words");
        })
        .unwrap();
}
#[test]
fn discogs_account_verification_cas_is_revoked_by_remove() {
    let service = new_service(Fake::default());
    let first = service.status(ProviderId::Discogs).unwrap();
    let saved = service
        .save(
            &first.reference,
            Secret::new("fixture-personal".into()).unwrap(),
        )
        .unwrap();
    service
        .record_verification(&saved.reference, Verification::Valid)
        .unwrap();
    service.remove(ProviderId::Discogs).unwrap();
    assert_eq!(
        service.record_verification(&saved.reference, Verification::Valid),
        Err(CredentialError::Stale)
    );
    assert_eq!(
        service.status(ProviderId::Discogs).unwrap().verification,
        Verification::Untested
    );
}

#[test]
fn volumo_uses_opaque_generation_bound_credentials_without_secret_dtos() {
    let service = new_service(Fake::default());
    let first = service.status(ProviderId::Volumo).unwrap();
    assert_eq!(first.presence, Presence::Missing);
    let saved = service
        .save(
            &first.reference,
            Secret::new("fixture-volumo-token".into()).unwrap(),
        )
        .unwrap();
    assert_eq!(saved.presence, Presence::Present);
    let dto = serde_json::to_string(&saved).unwrap();
    assert!(dto.contains("volumo"));
    assert!(!dto.contains("fixture-volumo-token"));
    service.remove(ProviderId::Volumo).unwrap();
    assert_eq!(
        service.run(&saved.reference, |_, _| ()),
        Err(CredentialError::Stale)
    );
}
