use super::ClockStatus;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClockIdentity {
    pub source_profile: String,
    pub codec: String,
    pub sample_rate: u32,
    pub media_sha256: String,
    pub decoder: String,
    pub origin_os: String,
    pub renderer: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeTime {
    pub seconds: f64,
    pub identity: ClockIdentity,
}
/// Backend-owned qualification record, never accepted as client preview authority.
#[derive(Debug, Clone)]
pub struct QualifiedClockProfile {
    pub profile_id: String,
    pub identity: ClockIdentity,
    pub offset_seconds: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignedTime {
    pub native_seconds: f64,
    pub canonical_seconds: Option<f64>,
    pub clock_status: ClockStatus,
    pub timing_profile_id: Option<String>,
    pub offset_seconds: Option<f64>,
}
pub fn align_time(
    value: &NativeTime,
    profile: Option<&QualifiedClockProfile>,
) -> Result<AlignedTime> {
    ensure!(
        value.seconds.is_finite() && value.identity.sample_rate > 0,
        "Invalid native time or sample rate"
    );
    let mut result = AlignedTime {
        native_seconds: value.seconds,
        canonical_seconds: None,
        clock_status: ClockStatus::UnresolvedNative,
        timing_profile_id: None,
        offset_seconds: None,
    };
    if let Some(profile) = profile.filter(|p| p.identity == value.identity) {
        ensure!(
            !profile.profile_id.is_empty() && profile.offset_seconds.is_finite(),
            "Invalid qualified timing profile"
        );
        let canonical = value.seconds - profile.offset_seconds;
        ensure!(canonical.is_finite(), "Canonical time overflow");
        result.canonical_seconds = Some(canonical);
        result.clock_status = ClockStatus::ValidatedProfile;
        result.timing_profile_id = Some(profile.profile_id.clone());
        result.offset_seconds = Some(profile.offset_seconds);
    }
    Ok(result)
}
