//! Explicit native stores; never the process-global default/mock builder.
use crate::{ProviderId, Secret, Vault, VaultError};
use std::sync::Arc;
use zeroize::Zeroize;
const SERVICE: &str = "nl.neerdael.cuerithm.provider-credentials.v1";
pub struct NativeVault {
    store: Arc<keyring_core::CredentialStore>,
}
impl NativeVault {
    pub fn new() -> Result<Self, VaultError> {
        #[cfg(target_os = "macos")]
        let store: Arc<keyring_core::CredentialStore> =
            apple_native_keyring_store::keychain::Store::new().map_err(map_error)?;
        #[cfg(target_os = "windows")]
        let store: Arc<keyring_core::CredentialStore> =
            windows_native_keyring_store::Store::new().map_err(map_error)?;
        #[cfg(target_os = "linux")]
        let store: Arc<keyring_core::CredentialStore> =
            dbus_secret_service_keyring_store::Store::new().map_err(map_error)?;
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        return Err(VaultError::Unavailable);
        #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
        Ok(Self { store })
    }
    fn entry(&self, provider: ProviderId) -> Result<keyring_core::Entry, VaultError> {
        let account = match provider {
            ProviderId::SonoVault => "sonovault",
            ProviderId::AudD => "audd",
            ProviderId::RapidApi => "rapidapi",
        };
        #[cfg(target_os = "windows")]
        let modifiers = Some(std::collections::HashMap::from([("persistence", "Local")]));
        #[cfg(not(target_os = "windows"))]
        let modifiers = None;
        self.store
            .build(SERVICE, account, modifiers.as_ref())
            .map_err(map_error)
    }
}
impl Vault for NativeVault {
    fn contains(&self, provider: ProviderId) -> Result<bool, VaultError> {
        match self.entry(provider)?.get_credential() {
            Ok(_) => Ok(true),
            Err(keyring_core::Error::NoEntry) => Ok(false),
            Err(e) => Err(map_error(e)),
        }
    }
    fn load(&self, provider: ProviderId) -> Result<Option<Secret>, VaultError> {
        match self.entry(provider)?.get_password() {
            Ok(value) => Secret::new(value)
                .map(Some)
                .map_err(|_| VaultError::Corrupt),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(e) => Err(map_error(e)),
        }
    }
    fn store(&self, provider: ProviderId, secret: &Secret) -> Result<(), VaultError> {
        self.entry(provider)?
            .set_password(secret.expose())
            .map_err(map_error)
    }
    fn remove(&self, provider: ProviderId) -> Result<(), VaultError> {
        match self.entry(provider)?.delete_credential() {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(e) => Err(map_error(e)),
        }
    }
}
fn map_error(error: keyring_core::Error) -> VaultError {
    match error {
        keyring_core::Error::NoStorageAccess(_) => VaultError::Locked,
        keyring_core::Error::BadEncoding(mut bytes)
        | keyring_core::Error::BadDataFormat(mut bytes, _) => {
            bytes.zeroize();
            VaultError::Corrupt
        }
        keyring_core::Error::Ambiguous(_) => VaultError::Ambiguous,
        keyring_core::Error::Invalid(_, mut detail)
        | keyring_core::Error::BadStoreFormat(mut detail) => {
            detail.zeroize();
            VaultError::Corrupt
        }
        _ => VaultError::Unavailable,
    }
}
