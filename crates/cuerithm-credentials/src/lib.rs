use serde::{Deserialize, Serialize};
use std::{
    fmt,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use uuid::Uuid;
use zeroize::Zeroizing;
pub mod native;
pub mod state;
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderId {
    #[serde(rename = "sonovault")]
    SonoVault,
    #[serde(rename = "audd")]
    AudD,
    #[serde(rename = "rapidapi")]
    RapidApi,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VaultError {
    Locked,
    Unavailable,
    Denied,
    Corrupt,
    Ambiguous,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CredentialError {
    Vault(VaultError),
    InvalidKey,
    Stale,
    Missing,
    Poisoned,
    MutationUncertain,
    State(state::StateError),
}
impl fmt::Display for CredentialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for CredentialError {}
pub struct Secret(Zeroizing<String>);
impl Secret {
    pub fn new(value: String) -> Result<Self, CredentialError> {
        let value = Zeroizing::new(value);
        if value.is_empty()
            || value.len() > 4096
            || value.chars().any(|c| c.is_control() || c.is_whitespace())
        {
            return Err(CredentialError::InvalidKey);
        }
        Ok(Self(value))
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret([REDACTED])")
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialRef {
    pub provider: ProviderId,
    pub generation: Uuid,
    pub session: Uuid,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Presence {
    Present,
    Missing,
    Unknown,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VaultState {
    Ready,
    Locked,
    Unavailable,
    Denied,
    Corrupt,
    Ambiguous,
    Uncertain,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verification {
    Untested,
    Valid,
    Invalid,
    EmailUnverified,
    Restricted,
    RateLimited,
    QuotaExhausted,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialStatus {
    pub reference: CredentialRef,
    pub presence: Presence,
    pub vault: VaultState,
    pub verification: Verification,
}
pub trait Vault: Send + Sync + 'static {
    fn contains(&self, provider: ProviderId) -> Result<bool, VaultError>;
    fn load(&self, provider: ProviderId) -> Result<Option<Secret>, VaultError>;
    fn store(&self, provider: ProviderId, secret: &Secret) -> Result<(), VaultError>;
    fn remove(&self, provider: ProviderId) -> Result<(), VaultError>;
}
#[derive(Clone)]
pub struct Revocation(Arc<AtomicBool>);
impl Revocation {
    pub fn is_revoked(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

struct EntryState {
    reference: CredentialRef,
    revocation: Revocation,
    verification: Verification,
    persisted: state::MutationState,
}
impl EntryState {
    fn new(provider: ProviderId, session: Uuid, persisted: state::MutationState) -> Self {
        Self {
            reference: CredentialRef {
                provider,
                generation: persisted.generation,
                session,
            },
            revocation: Revocation(Arc::new(AtomicBool::new(false))),
            verification: Verification::Untested,
            persisted,
        }
    }
    fn replace(&mut self, persisted: state::MutationState) {
        self.revocation.0.store(true, Ordering::SeqCst);
        *self = Self::new(self.reference.provider, self.reference.session, persisted);
    }
}
struct Inner<V> {
    vault: V,
    store: Arc<dyn state::StateStore>,
    _authority: Box<dyn Send + Sync>,
    entries: std::collections::BTreeMap<ProviderId, EntryState>,
}
/// One native authority per application. Share clones; do not construct competing instances.
pub struct CredentialService<V: Vault> {
    inner: Arc<std::sync::Mutex<Inner<V>>>,
}
impl<V: Vault> Clone for CredentialService<V> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}
impl<V: Vault> CredentialService<V> {
    pub fn new(vault: V, store: Arc<dyn state::StateStore>) -> Result<Self, CredentialError> {
        let authority = store.claim_authority().map_err(CredentialError::State)?;
        let session = Uuid::new_v4();
        let mut entries = std::collections::BTreeMap::new();
        for provider in [
            ProviderId::SonoVault,
            ProviderId::AudD,
            ProviderId::RapidApi,
        ] {
            let persisted = store
                .load_or_create(provider)
                .map_err(CredentialError::State)?;
            entries.insert(provider, EntryState::new(provider, session, persisted));
        }
        Ok(Self {
            inner: Arc::new(std::sync::Mutex::new(Inner {
                vault,
                store,
                entries,
                _authority: authority,
            })),
        })
    }
    pub fn status(&self, provider: ProviderId) -> Result<CredentialStatus, CredentialError> {
        let mut state = self.inner.lock().map_err(|_| CredentialError::Poisoned)?;
        refresh(&mut state, provider)?;
        Ok(status(&state, provider))
    }
    /// Persist intent before any native vault mutation. Compare-and-set rejects queued old saves.
    pub fn save(
        &self,
        expected: &CredentialRef,
        secret: Secret,
    ) -> Result<CredentialStatus, CredentialError> {
        let mut state = self.inner.lock().map_err(|_| CredentialError::Poisoned)?;
        refresh(&mut state, expected.provider)?;
        check(&state, expected)?;
        begin(&mut state, expected.provider, state::MutationKind::Save)?;
        state
            .vault
            .store(expected.provider, &secret)
            .map_err(CredentialError::Vault)?;
        acknowledge(&mut state, expected.provider)?;
        Ok(status(&state, expected.provider))
    }
    /// Removal always rotates/persists the generation before deleting; retry is explicit.
    pub fn remove(&self, provider: ProviderId) -> Result<CredentialStatus, CredentialError> {
        let mut state = self.inner.lock().map_err(|_| CredentialError::Poisoned)?;
        refresh(&mut state, provider)?;
        begin(&mut state, provider, state::MutationKind::Remove)?;
        state
            .vault
            .remove(provider)
            .map_err(CredentialError::Vault)?;
        acknowledge(&mut state, provider)?;
        Ok(status(&state, provider))
    }
    /// Trusted native provider callback only. Never expose as a bridge/query command.
    pub fn run<R>(
        &self,
        expected: &CredentialRef,
        operation: impl FnOnce(&str, Revocation) -> R,
    ) -> Result<R, CredentialError> {
        let (secret, revocation) = {
            let mut state = self.inner.lock().map_err(|_| CredentialError::Poisoned)?;
            refresh(&mut state, expected.provider)?;
            check(&state, expected)?;
            let entry = &state.entries[&expected.provider];
            if entry.persisted.pending.is_some() {
                return Err(CredentialError::MutationUncertain);
            }
            (
                state
                    .vault
                    .load(expected.provider)
                    .map_err(CredentialError::Vault)?
                    .ok_or(CredentialError::Missing)?,
                entry.revocation.clone(),
            )
        };
        if revocation.is_revoked() {
            return Err(CredentialError::Stale);
        }
        let result = operation(secret.expose(), revocation.clone());
        if revocation.is_revoked() {
            return Err(CredentialError::Stale);
        }
        let mut state = self.inner.lock().map_err(|_| CredentialError::Poisoned)?;
        refresh(&mut state, expected.provider)?;
        check(&state, expected)?;
        Ok(result)
    }
    pub fn verify(
        &self,
        expected: &CredentialRef,
        operation: impl FnOnce(&str, Revocation) -> Verification,
    ) -> Result<CredentialStatus, CredentialError> {
        let verification = self.run(expected, operation)?;
        let mut state = self.inner.lock().map_err(|_| CredentialError::Poisoned)?;
        refresh(&mut state, expected.provider)?;
        check(&state, expected)?;
        state
            .entries
            .get_mut(&expected.provider)
            .unwrap()
            .verification = verification;
        Ok(status(&state, expected.provider))
    }
}
fn check<V>(state: &Inner<V>, expected: &CredentialRef) -> Result<(), CredentialError> {
    if state.entries[&expected.provider].reference != *expected {
        Err(CredentialError::Stale)
    } else {
        Ok(())
    }
}
fn refresh<V>(state: &mut Inner<V>, provider: ProviderId) -> Result<(), CredentialError> {
    let persisted = state
        .store
        .load_or_create(provider)
        .map_err(CredentialError::State)?;
    let entry = state.entries.get_mut(&provider).unwrap();
    if entry.persisted != persisted {
        entry.replace(persisted);
    }
    Ok(())
}
fn begin<V>(
    state: &mut Inner<V>,
    provider: ProviderId,
    kind: state::MutationKind,
) -> Result<(), CredentialError> {
    let old = &state.entries[&provider].persisted;
    let next = state::MutationState {
        generation: Uuid::new_v4(),
        pending: Some(kind),
    };
    state
        .store
        .compare_exchange(provider, old, &next)
        .map_err(CredentialError::State)?;
    state.entries.get_mut(&provider).unwrap().replace(next);
    Ok(())
}
fn acknowledge<V>(state: &mut Inner<V>, provider: ProviderId) -> Result<(), CredentialError> {
    let old = &state.entries[&provider].persisted;
    let next = state::MutationState {
        generation: old.generation,
        pending: None,
    };
    state
        .store
        .compare_exchange(provider, old, &next)
        .map_err(CredentialError::State)?;
    state.entries.get_mut(&provider).unwrap().persisted = next;
    Ok(())
}
fn status<V: Vault>(state: &Inner<V>, provider: ProviderId) -> CredentialStatus {
    let entry = &state.entries[&provider];
    let (presence, vault) = if entry.persisted.pending.is_some() {
        (Presence::Unknown, VaultState::Uncertain)
    } else {
        match state.vault.contains(provider) {
            Ok(true) => (Presence::Present, VaultState::Ready),
            Ok(false) => (Presence::Missing, VaultState::Ready),
            Err(error) => (
                Presence::Unknown,
                match error {
                    VaultError::Locked => VaultState::Locked,
                    VaultError::Unavailable => VaultState::Unavailable,
                    VaultError::Denied => VaultState::Denied,
                    VaultError::Corrupt => VaultState::Corrupt,
                    VaultError::Ambiguous => VaultState::Ambiguous,
                },
            ),
        }
    };
    CredentialStatus {
        reference: entry.reference.clone(),
        presence,
        vault,
        verification: if presence == Presence::Present {
            entry.verification
        } else {
            Verification::Untested
        },
    }
}
