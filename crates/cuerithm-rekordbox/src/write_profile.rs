//! Schema recognition is not native write acceptance. Only the exact macOS
//! application/schema pairs recorded in the native acceptance contract can write.
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

/// A schema-only check cannot authorize publication. A production write
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
    user_version: i64,
    application_id: i64,
}

impl AcceptedNativeProfile {
    fn matches(
        &self,
        platform: &str,
        installation: &TargetInstallation,
        executable_sha256: &str,
        schema: &DatabaseSchema,
    ) -> bool {
        self.platform == platform
            && self.version == installation.version
            && self.major == installation.major
            && self.executable_sha256 == executable_sha256
            && self.schema_sha256 == schema.fingerprint
            && self.user_version == schema.user_version
            && self.application_id == schema.application_id
    }
}

// Populate each entry only after the isolated native application acceptance
// recorded in the direct-sync contract passes for that exact build/protocol.
const ACCEPTED_NATIVE_PROFILES: &[AcceptedNativeProfile] = &[
    AcceptedNativeProfile {
        id: "macos-rekordbox-6.8.7.0303-native-v1",
        platform: "macos",
        version: "6.8.7.0303",
        major: 6,
        executable_sha256: "aab4838b9768860da941f232db3f31cfa397a7a79f6052cde1aa6584f2d5dc3c",
        schema_sha256: "ddc512c75d38fb50ca9688b8bca2e179130b482f001b9df9272c0bd21ff9baaf",
        user_version: 0,
        application_id: 0,
    },
    AcceptedNativeProfile {
        id: "macos-rekordbox-7.2.19.0342-native-v1",
        platform: "macos",
        version: "7.2.19.0342",
        major: 7,
        executable_sha256: "07dcecbaf304f777e435539c83c7dcc02e41de1eaf2e0a75e8a9852e4411a244",
        schema_sha256: "6bd02ac0a3f9cadfd3ae9b4c95e1ca0663fd3e7717a74e89a4c153834611d2eb",
        user_version: 0,
        application_id: 0,
    },
];

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
    if let Some(profile) = ACCEPTED_NATIVE_PROFILES
        .iter()
        .find(|profile| profile.matches(std::env::consts::OS, installation, &hash, schema))
    {
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

#[cfg(test)]
mod tests {
    use super::*;

    const SYNTHETIC_PLATFORM: &str = "synthetic-platform";
    const SYNTHETIC_EXECUTABLE_SHA256: &str =
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn synthetic_inputs() -> (AcceptedNativeProfile, TargetInstallation, DatabaseSchema) {
        let profile = AcceptedNativeProfile {
            id: "unit-test-only",
            platform: SYNTHETIC_PLATFORM,
            version: "synthetic-1.0",
            major: 7,
            executable_sha256: SYNTHETIC_EXECUTABLE_SHA256,
            schema_sha256: "4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945",
            user_version: 17,
            application_id: 23,
        };
        let installation = TargetInstallation {
            root: "synthetic-installation".into(),
            executable: "synthetic-executable".into(),
            version: profile.version.into(),
            major: profile.major,
            version_source: "unit test".into(),
        };
        let schema = DatabaseSchema {
            user_version: profile.user_version,
            application_id: profile.application_id,
            fingerprint: profile.schema_sha256.into(),
            objects: vec![],
        };
        (profile, installation, schema)
    }

    #[test]
    fn matching_synthetic_profile_requires_all_bindings() {
        let (profile, installation, schema) = synthetic_inputs();
        // Use a valid objects digest without referring to a native schema.
        assert!(validate_write_profile(&schema).is_ok());
        assert!(profile.matches(
            SYNTHETIC_PLATFORM,
            &installation,
            SYNTHETIC_EXECUTABLE_SHA256,
            &schema,
        ));
    }

    #[test]
    fn synthetic_profile_rejects_platform_mismatch() {
        let (profile, installation, schema) = synthetic_inputs();
        assert!(!profile.matches(
            "other-platform",
            &installation,
            SYNTHETIC_EXECUTABLE_SHA256,
            &schema,
        ));
    }

    #[test]
    fn synthetic_profile_rejects_version_mismatch() {
        let (profile, mut installation, schema) = synthetic_inputs();
        installation.version.push_str(".1");
        assert!(!profile.matches(
            SYNTHETIC_PLATFORM,
            &installation,
            SYNTHETIC_EXECUTABLE_SHA256,
            &schema,
        ));
    }

    #[test]
    fn synthetic_profile_rejects_application_major_mismatch() {
        let (profile, mut installation, schema) = synthetic_inputs();
        installation.major += 1;
        assert!(!profile.matches(
            SYNTHETIC_PLATFORM,
            &installation,
            SYNTHETIC_EXECUTABLE_SHA256,
            &schema,
        ));
    }

    #[test]
    fn synthetic_profile_rejects_executable_digest_mismatch() {
        let (profile, installation, schema) = synthetic_inputs();
        assert!(!profile.matches(
            SYNTHETIC_PLATFORM,
            &installation,
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            &schema,
        ));
    }

    #[test]
    fn synthetic_profile_rejects_schema_digest_mismatch() {
        let (profile, installation, mut schema) = synthetic_inputs();
        schema.fingerprint = "different-schema".into();
        assert!(!profile.matches(
            SYNTHETIC_PLATFORM,
            &installation,
            SYNTHETIC_EXECUTABLE_SHA256,
            &schema,
        ));
    }

    #[test]
    fn synthetic_profile_rejects_user_version_mismatch() {
        let (profile, installation, mut schema) = synthetic_inputs();
        schema.user_version += 1;
        assert!(!profile.matches(
            SYNTHETIC_PLATFORM,
            &installation,
            SYNTHETIC_EXECUTABLE_SHA256,
            &schema,
        ));
    }

    #[test]
    fn synthetic_profile_rejects_application_id_mismatch() {
        let (profile, installation, mut schema) = synthetic_inputs();
        schema.application_id += 1;
        assert!(!profile.matches(
            SYNTHETIC_PLATFORM,
            &installation,
            SYNTHETIC_EXECUTABLE_SHA256,
            &schema,
        ));
    }
}
