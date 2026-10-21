//! Schema recognition is not native write acceptance. There are currently no
//! production-accepted database/asset write profiles for either rekordbox 6 or 7.
use crate::{DatabaseSchema, TargetInstallation};
use anyhow::{Result, ensure};
use serde::Serialize;
use sha2::{Digest, Sha256};

const OBSERVED_SEVEN_SCHEMA: &str =
    "6bd02ac0a3f9cadfd3ae9b4c95e1ca0663fd3e7717a74e89a4c153834611d2eb";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WriteRestriction {
    UnrecognizedSchema,
    NativeAcceptancePending,
    UnrecognizedApplication,
}

/// A schema-only check cannot authorize publication. A future production write
/// capability must also bind accepted app/build and database/asset protocols.
/// Do not deserialize a capability supplied by a caller or promote a read-only
/// schema observation into such a capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum WriteCapability {
    ReadOnly {
        restriction: WriteRestriction,
    },
    Writable {
        profile_id: &'static str,
        application_major: u32,
    },
}

impl WriteCapability {
    pub fn can_write(&self) -> bool {
        matches!(self, Self::Writable { .. })
    }
}

struct AcceptedNativeProfile {
    id: &'static str,
    platform: &'static str,
    version: &'static str,
    major: u32,
    executable_sha256: &'static str,
    schema_sha256: &'static str,
}
// Populate each entry only after the isolated native application acceptance
// recorded in the direct-sync contract passes for that exact build/protocol.
const ACCEPTED_NATIVE_PROFILES: &[AcceptedNativeProfile] = &[];

/// Application/build and schema acceptance is a separate, stronger check than
/// read support. A capability is never deserialized from a caller's request.
pub fn accepted_write_capability(
    installation: &TargetInstallation,
    schema: &DatabaseSchema,
) -> Result<WriteCapability> {
    accepted_write_capability_cancellable(
        installation,
        schema,
        &cuerithm_media::CancellationToken::default(),
    )
}
pub fn accepted_write_capability_cancellable(
    installation: &TargetInstallation,
    schema: &DatabaseSchema,
    cancel: &cuerithm_media::CancellationToken,
) -> Result<WriteCapability> {
    let observed = validate_write_profile(schema)?;
    cancel.check()?;
    let hash = cuerithm_media::file_sha256_cancellable(&installation.executable, cancel)?;
    if let Some(profile) = ACCEPTED_NATIVE_PROFILES.iter().find(|profile| {
        profile.platform == std::env::consts::OS
            && profile.version == installation.version
            && profile.major == installation.major
            && profile.executable_sha256 == hash
            && profile.schema_sha256 == schema.fingerprint
    }) {
        return Ok(WriteCapability::Writable {
            profile_id: profile.id,
            application_major: profile.major,
        });
    }
    Ok(if ACCEPTED_NATIVE_PROFILES.is_empty() {
        observed
    } else {
        WriteCapability::ReadOnly {
            restriction: WriteRestriction::UnrecognizedApplication,
        }
    })
}

pub fn validate_write_profile(schema: &DatabaseSchema) -> Result<WriteCapability> {
    let actual = format!("{:x}", Sha256::digest(serde_json::to_vec(&schema.objects)?));
    ensure!(
        schema.fingerprint == actual,
        "Database schema fingerprint does not match its objects"
    );
    Ok(WriteCapability::ReadOnly {
        restriction: if actual == OBSERVED_SEVEN_SCHEMA {
            WriteRestriction::NativeAcceptancePending
        } else {
            WriteRestriction::UnrecognizedSchema
        },
    })
}
