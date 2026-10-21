use cuerithm_credentials::CredentialStatus;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize)]
pub struct ProviderSetupReason {
    pub code: String,
    pub message: String,
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderSetupKind {
    None,
    ApiKey,
    PersonalToken,
    EmailPassword,
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderAuthMode {
    Public,
    Anonymous,
    Authenticated,
    Unavailable,
}
#[derive(Clone, Serialize)]
pub struct ProviderQuota {
    pub remaining: Option<u64>,
    pub limit: Option<u64>,
    pub resets_at_ms: Option<i64>,
}
#[derive(Clone, Serialize)]
pub struct ProviderSetupStatus {
    pub id: String,
    pub revision: String,
    pub setup_kind: ProviderSetupKind,
    pub credential: Option<CredentialStatus>,
    pub auth_mode: ProviderAuthMode,
    pub can_use: bool,
    pub can_test: bool,
    pub reason: Option<ProviderSetupReason>,
    pub rate_limit_per_minute: Option<u32>,
    pub min_request_interval_ms: Option<u64>,
    pub retry_at_ms: Option<i64>,
    pub quota: Option<ProviderQuota>,
}
// Deliberately neither Debug nor Serialize: these values may only enter the vault.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProviderCredentialInput {
    ApiKey { secret: String },
    PersonalToken { secret: String },
    EmailPassword { email: String, password: String },
}
pub use cuerithm_credentials::{
    CredentialRef as ProviderCredentialRef, CredentialStatus as ProviderCredentialStatus,
    ProviderId as ProviderCredentialId,
};
pub(super) fn reason(code: &str, message: &str) -> Option<ProviderSetupReason> {
    Some(ProviderSetupReason {
        code: code.into(),
        message: message.into(),
    })
}
