//! Lazy native authority. Failure never substitutes a plaintext or in-memory key store.
use anyhow::{Result, anyhow};
use cuerithm_autotag::CatalogCredentials;
use cuerithm_credentials::{
    CredentialRef, CredentialService, CredentialStatus, ProviderId, Revocation, Secret, Vault,
    VaultError, Verification, native::NativeVault, state::FileStateStore,
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

struct LazyVault(Mutex<Option<NativeVault>>);
impl LazyVault {
    fn with<T>(
        &self,
        action: impl FnOnce(&NativeVault) -> std::result::Result<T, VaultError>,
    ) -> std::result::Result<T, VaultError> {
        let mut native = self.0.lock().map_err(|_| VaultError::Unavailable)?;
        if native.is_none() {
            *native = Some(NativeVault::new()?);
        }
        action(native.as_ref().ok_or(VaultError::Unavailable)?)
    }
}
impl Vault for LazyVault {
    fn contains(&self, p: ProviderId) -> std::result::Result<bool, VaultError> {
        self.with(|native| native.contains(p))
    }
    fn load(&self, p: ProviderId) -> std::result::Result<Option<Secret>, VaultError> {
        self.with(|native| native.load(p))
    }
    fn store(&self, p: ProviderId, s: &Secret) -> std::result::Result<(), VaultError> {
        self.with(|native| native.store(p, s))
    }
    fn remove(&self, p: ProviderId) -> std::result::Result<(), VaultError> {
        self.with(|native| native.remove(p))
    }
}
pub(super) struct NativeCredentialAuthority {
    root: PathBuf,
    authority: Mutex<Option<Arc<dyn CatalogCredentials>>>,
}
impl NativeCredentialAuthority {
    pub(super) fn new(root: PathBuf) -> Self {
        Self {
            root,
            authority: Mutex::new(None),
        }
    }
    fn current(&self) -> Result<Arc<dyn CatalogCredentials>> {
        let mut authority = self
            .authority
            .lock()
            .map_err(|_| anyhow!("Native credential authority interrupted"))?;
        if let Some(authority) = authority.as_ref() {
            return Ok(authority.clone());
        }
        super::global_state::prepare_root(&self.root)
            .map_err(|_| anyhow!("Application credential directory is unavailable or unsafe"))?;
        let store = FileStateStore::open(&self.root)
            .map_err(|_| anyhow!("Native credential marker is unavailable"))?;
        let service = CredentialService::new(LazyVault(Mutex::new(None)), Arc::new(store))
            .map_err(|error| {
                if matches!(
                    error,
                    cuerithm_credentials::CredentialError::State(
                        cuerithm_credentials::state::StateError::Busy
                    )
                ) {
                    anyhow!("Native credential authority is busy in another process")
                } else {
                    anyhow!("Native credential authority is unavailable")
                }
            })?;
        let service: Arc<dyn CatalogCredentials> = Arc::new(service);
        *authority = Some(service.clone());
        Ok(service)
    }
}
impl CatalogCredentials for NativeCredentialAuthority {
    fn status(&self, p: ProviderId) -> Result<CredentialStatus> {
        self.current()?.status(p)
    }
    fn save(&self, r: &CredentialRef, s: Secret) -> Result<CredentialStatus> {
        self.current()?.save(r, s)
    }
    fn remove(&self, p: ProviderId) -> Result<CredentialStatus> {
        self.current()?.remove(p)
    }
    fn with_secret(
        &self,
        r: &CredentialRef,
        operation: &mut dyn FnMut(&str, Revocation) -> Result<()>,
    ) -> Result<()> {
        self.current()?.with_secret(r, operation)
    }
    fn record_verification(&self, r: &CredentialRef, v: Verification) -> Result<CredentialStatus> {
        self.current()?.record_verification(r, v)
    }
}
