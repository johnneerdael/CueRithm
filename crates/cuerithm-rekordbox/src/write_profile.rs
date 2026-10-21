//! Schema recognition is not native write acceptance. There are currently no
//! production-accepted database/asset write profiles for either rekordbox 6 or 7.
use crate::DatabaseSchema;
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
}

/// A schema-only check cannot authorize publication. A future production write
/// capability must also bind accepted app/build and database/asset protocols.
/// Do not deserialize a capability supplied by a caller or promote a read-only
/// schema observation into such a capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum WriteCapability {
    ReadOnly { restriction: WriteRestriction },
}

impl WriteCapability {
    pub fn can_write(&self) -> bool {
        false
    }
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
