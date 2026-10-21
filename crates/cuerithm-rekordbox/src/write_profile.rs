//! Recognized database/header protocols and supported application families.
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
/// capability also binds the supported application family and database/header
/// protocol. Publication separately freezes installed executable bytes.
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

// Protocol recognition authorizes the data formats; installation identity is a
// separate preview precondition, never a list of accepted patch releases.
fn protocol(schema: &DatabaseSchema) -> Option<(&'static str, u32)> {
    if schema.user_version != 0 || schema.application_id != 0 {
        return None;
    }
    match schema.fingerprint.as_str() {
        "ddc512c75d38fb50ca9688b8bca2e179130b482f001b9df9272c0bd21ff9baaf" => {
            Some(("rekordbox-6-schema-v1", 6))
        }
        OBSERVED_SEVEN_SCHEMA => Some(("rekordbox-7-schema-v1", 7)),
        _ => None,
    }
}

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
    validate_write_profile(schema)?;
    cancel.check()?;
    // Availability and byte stability are checked here and captured separately
    // by the publication service. No vendor program is executed.
    cuerithm_media::file_sha256_cancellable(&installation.executable, cancel)?;
    Ok(application_capability(installation, schema))
}
fn application_capability(
    installation: &TargetInstallation,
    schema: &DatabaseSchema,
) -> WriteCapability {
    let numeric = installation.version.len() <= 64
        && installation
            .version
            .split('.')
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()));
    let parts = installation
        .version
        .split('.')
        .map(str::parse::<u32>)
        .collect::<std::result::Result<Vec<_>, _>>();
    let version_major = parts
        .ok()
        .filter(|_| numeric)
        .filter(|v| !v.is_empty() && v.len() <= 4)
        .map(|v| v[0]);
    match protocol(schema) {
        Some((profile_id, major))
            if installation.major == major && version_major == Some(major) =>
        {
            WriteCapability::Writable {
                profile_id,
                application_major: major,
            }
        }
        Some(_) => WriteCapability::ReadOnly {
            restriction: WriteRestriction::UnrecognizedApplication,
        },
        None => WriteCapability::ReadOnly {
            restriction: WriteRestriction::UnrecognizedSchema,
        },
    }
}

pub fn validate_write_profile(schema: &DatabaseSchema) -> Result<WriteCapability> {
    let actual = format!("{:x}", Sha256::digest(serde_json::to_vec(&schema.objects)?));
    ensure!(
        schema.fingerprint == actual,
        "Database schema fingerprint does not match its objects"
    );
    // A schema observation without a selected installation is read-only.
    Ok(WriteCapability::ReadOnly {
        restriction: if protocol(schema).is_some() {
            WriteRestriction::NativeAcceptancePending
        } else {
            WriteRestriction::UnrecognizedSchema
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn schema(fingerprint: &str) -> DatabaseSchema {
        DatabaseSchema {
            fingerprint: fingerprint.into(),
            user_version: 0,
            application_id: 0,
            objects: vec![],
        }
    }
    #[test]
    fn protocols_require_recognized_schema_and_headers() {
        for (fingerprint, major) in [
            (
                "ddc512c75d38fb50ca9688b8bca2e179130b482f001b9df9272c0bd21ff9baaf",
                6,
            ),
            (OBSERVED_SEVEN_SCHEMA, 7),
        ] {
            let mut observed = schema(fingerprint);
            assert_eq!(protocol(&observed).unwrap().1, major);
            observed.user_version = 1;
            assert!(protocol(&observed).is_none());
            observed.user_version = 0;
            observed.application_id = 1;
            assert!(protocol(&observed).is_none());
        }
        assert!(protocol(&schema("unknown")).is_none());
    }
    #[test]
    fn compatible_patch_builds_and_windows_executables_use_family_protocols() {
        let mut installation = TargetInstallation {
            root: "C:/rekordbox".into(),
            executable: "C:/rekordbox/rekordbox.exe".into(),
            version: "7.9.42.1".into(),
            major: 7,
            version_source: "explicit_version_hint".into(),
        };
        let observed = schema(OBSERVED_SEVEN_SCHEMA);
        assert!(application_capability(&installation, &observed).can_write());
        installation.version = "7.0.1".into();
        assert!(application_capability(&installation, &observed).can_write());
        for version in ["8.0", "7.garbage", "7.1.2.3.4", "6.8.7"] {
            installation.version = version.into();
            assert!(!application_capability(&installation, &observed).can_write());
        }
        installation.major = 8;
        installation.version = "8.0".into();
        assert!(!application_capability(&installation, &observed).can_write());
    }
    #[test]
    fn caller_fingerprint_cannot_authorize_different_objects() {
        assert!(validate_write_profile(&schema(OBSERVED_SEVEN_SCHEMA)).is_err());
        let empty = schema("4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945");
        assert!(!validate_write_profile(&empty).unwrap().can_write());
    }
}
