#![cfg(feature = "autotag")]
use cuerithm_autotag::{
    CatalogCredentials, CatalogHttpRequest, CatalogHttpResponse, CatalogIdentity, CatalogRuntime,
    CatalogTransport, ProviderId,
};
use cuerithm_credentials::{
    CredentialService, ProviderId as CredentialProviderId, Revocation, Secret, Vault, VaultError,
    Verification, state::FileStateStore,
};
use cuerithm_library::{Library, ProviderAuthMode, ProviderCredentialInput, ProviderSetupKind};
use cuerithm_media::CancellationToken;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, atomic::AtomicBool},
};

#[derive(Clone, Default)]
struct FixtureVault {
    values: Arc<Mutex<BTreeMap<CredentialProviderId, String>>>,
    locked: bool,
}
impl Vault for FixtureVault {
    fn contains(&self, p: CredentialProviderId) -> Result<bool, VaultError> {
        if self.locked {
            return Err(VaultError::Locked);
        }
        Ok(self.values.lock().unwrap().contains_key(&p))
    }
    fn load(&self, p: CredentialProviderId) -> Result<Option<Secret>, VaultError> {
        if self.locked {
            return Err(VaultError::Locked);
        }
        self.values
            .lock()
            .unwrap()
            .get(&p)
            .map(|value| {
                if p == CredentialProviderId::BpmSupreme {
                    Secret::structured(value.clone())
                } else {
                    Secret::new(value.clone())
                }
            })
            .transpose()
            .map_err(|_| VaultError::Corrupt)
    }
    fn store(&self, p: CredentialProviderId, s: &Secret) -> Result<(), VaultError> {
        if self.locked {
            return Err(VaultError::Locked);
        }
        self.values.lock().unwrap().insert(p, s.expose().into());
        Ok(())
    }
    fn remove(&self, p: CredentialProviderId) -> Result<(), VaultError> {
        self.values.lock().unwrap().remove(&p);
        Ok(())
    }
}
struct IdentityTransport;
impl CatalogTransport for IdentityTransport {
    fn execute(
        &self,
        request: CatalogHttpRequest,
        _: &Arc<AtomicBool>,
        _: Option<&Revocation>,
    ) -> anyhow::Result<CatalogHttpResponse> {
        assert_eq!(request.url, "https://api.discogs.com/oauth/identity");
        Ok(CatalogHttpResponse {
            status: 200,
            headers: vec![],
            body: br#"{"id":7,"username":"fixture-account"}"#.to_vec(),
        })
    }
}
fn runtime(library: &Library, vault: FixtureVault) -> Arc<CatalogRuntime> {
    let credentials: Arc<dyn CatalogCredentials> = Arc::new(
        CredentialService::new(
            vault,
            Arc::new(FileStateStore::open(library.data_dir()).unwrap()),
        )
        .unwrap(),
    );
    Arc::new(
        CatalogRuntime::new(
            CatalogIdentity::new("4.2.1-fixture").unwrap(),
            Arc::new(library.clone()),
            credentials,
        )
        .unwrap()
        .with_transport(Arc::new(IdentityTransport)),
    )
}
#[test]
fn native_setup_does_not_enable_providers_change_settings_or_return_secrets() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("library.sqlite");
    let library = Library::open(&path).unwrap();
    let before = library.settings().unwrap();
    let revision = library.revision().unwrap();
    let vault = FixtureVault::default();
    let catalog = runtime(&library, vault.clone());
    let setup = library.provider_runtime_with(catalog.clone()).unwrap();
    let missing = setup.status(ProviderId::Sonovault).unwrap();
    assert!(!missing.can_use);
    assert!(matches!(missing.setup_kind, ProviderSetupKind::ApiKey));
    let previous = missing.credential.unwrap().reference;
    let saved = setup
        .save(
            &previous,
            ProviderCredentialInput::ApiKey {
                secret: "fixture-private-key".into(),
            },
        )
        .unwrap();
    assert_ne!(saved.reference.generation, previous.generation);
    assert!(
        !serde_json::to_string(&saved)
            .unwrap()
            .contains("fixture-private-key")
    );
    assert_eq!(library.settings().unwrap().revision, before.revision);
    assert_eq!(library.revision().unwrap(), revision);
    let volumo = setup.status(ProviderId::Volumo).unwrap();
    assert!(!volumo.can_use);
    assert!(matches!(volumo.setup_kind, ProviderSetupKind::ApiKey));
    let saved_volumo = setup
        .save(
            &volumo.credential.unwrap().reference,
            ProviderCredentialInput::ApiKey {
                secret: "fixture-volumo-private-key".into(),
            },
        )
        .unwrap();
    assert!(setup.status(ProviderId::Volumo).unwrap().can_use);
    assert!(
        !serde_json::to_string(&saved_volumo)
            .unwrap()
            .contains("fixture-volumo-private-key")
    );
    assert_eq!(library.settings().unwrap().revision, before.revision);
    assert_eq!(library.revision().unwrap(), revision);
    let account = setup
        .status(ProviderId::Bpmsupreme)
        .unwrap()
        .credential
        .unwrap();
    setup
        .save(
            &account.reference,
            ProviderCredentialInput::EmailPassword {
                email: "dj@example.invalid".into(),
                password: "fixture password with spaces".into(),
            },
        )
        .unwrap();
    let stored = vault.values.lock().unwrap();
    let value: serde_json::Value =
        serde_json::from_str(&stored[&CredentialProviderId::BpmSupreme]).unwrap();
    assert_eq!(value["password"], "fixture password with spaces");
    drop(stored);
    assert!(
        !serde_json::to_string(&setup.statuses().unwrap())
            .unwrap()
            .contains("fixture password")
    );
    assert!(
        !serde_json::to_string(&setup.statuses().unwrap())
            .unwrap()
            .contains("fixture-volumo-private-key")
    );
    drop(setup);
    drop(catalog);
    drop(library);
    Library::open(&path).unwrap();
}
#[test]
fn discogs_uses_anonymous_budget_until_native_identity_verification_grants_authentication() {
    let temp = tempfile::tempdir().unwrap();
    let library = Library::open(&temp.path().join("library.sqlite")).unwrap();
    let catalog = runtime(&library, FixtureVault::default());
    let setup = library.provider_runtime_with(catalog.clone()).unwrap();
    let initial = setup.status(ProviderId::Discogs).unwrap();
    assert!(initial.can_use);
    assert!(matches!(initial.auth_mode, ProviderAuthMode::Anonymous));
    assert_eq!(initial.rate_limit_per_minute, Some(25));
    let saved = setup
        .save(
            &initial.credential.unwrap().reference,
            ProviderCredentialInput::PersonalToken {
                secret: "fixture-discogs-token".into(),
            },
        )
        .unwrap();
    catalog
        .record_verification(&saved.reference, Verification::Valid)
        .unwrap();
    assert_eq!(
        setup
            .status(ProviderId::Discogs)
            .unwrap()
            .rate_limit_per_minute,
        Some(25),
        "A status enum alone cannot grant the authenticated ceiling"
    );
    let tested = setup
        .test(&saved.reference, &CancellationToken::default())
        .unwrap();
    assert!(matches!(tested.auth_mode, ProviderAuthMode::Authenticated));
    assert_eq!(tested.rate_limit_per_minute, Some(60));
    setup.remove(CredentialProviderId::Discogs).unwrap();
    assert_eq!(
        setup
            .status(ProviderId::Discogs)
            .unwrap()
            .rate_limit_per_minute,
        Some(25)
    );
}
#[test]
fn a_locked_vault_blocks_only_optional_authenticated_sources() {
    let temp = tempfile::tempdir().unwrap();
    let library = Library::open(&temp.path().join("library.sqlite")).unwrap();
    let catalog = runtime(
        &library,
        FixtureVault {
            locked: true,
            ..Default::default()
        },
    );
    let setup = library.provider_runtime_with(catalog).unwrap();
    assert!(setup.status(ProviderId::Musicbrainz).unwrap().can_use);
    assert!(setup.status(ProviderId::Discogs).unwrap().can_use);
    let blocked = setup.status(ProviderId::Sonovault).unwrap();
    assert!(!blocked.can_use);
    let credential = blocked.credential.unwrap();
    assert!(matches!(
        credential.presence,
        cuerithm_credentials::Presence::Unknown
    ));
    assert!(matches!(
        credential.vault,
        cuerithm_credentials::VaultState::Locked
    ));
}
