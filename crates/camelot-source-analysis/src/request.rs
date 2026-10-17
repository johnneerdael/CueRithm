use serde::{Deserialize, Serialize};

/// Computed classifier inputs, without authentication or final decisions.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeatureAnalysis {
    pub profile: String,
    pub full_source_frames: usize,
    pub analyzed_frames: usize,
    pub analysis: AnalysisRequest,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct AnalysisRequest {
    pub segments: Vec<KeySegment>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub energy_segment_data: Vec<EnergySegment>,
    pub duration_in_seconds: f64,
    pub tempo: f64,
    pub downbeat_time: f64,
    pub cue_point_start_beat: usize,
    pub cue_point_data: String,
    pub key_algorithm_version: u32,
    pub energy_algorithm_version: String,
    pub cue_point_algorithm_version: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct KeySegment {
    pub start_time: f64,
    pub end_time: f64,
    pub pitch_probabilities: [f64; 12],
    pub features: Vec<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct EnergySegment {
    pub start_time: f64,
    pub end_time: f64,
    pub volume_rms_db: f64,
    pub features: Vec<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_round_trip_preserves_negative_zero_and_adjacent_double_values() {
        let value = f64::from_bits(1.0f64.to_bits() + 1);
        let request = AnalysisRequest {
            segments: vec![KeySegment {
                start_time: 0.0,
                end_time: value,
                pitch_probabilities: [0.0; 12],
                features: vec![value],
            }],
            energy_segment_data: vec![EnergySegment {
                start_time: 0.0,
                end_time: value,
                volume_rms_db: -value,
                features: vec![value],
            }],
            duration_in_seconds: value,
            tempo: value,
            downbeat_time: -0.0,
            cue_point_start_beat: 32,
            cue_point_data: String::new(),
            key_algorithm_version: 94,
            energy_algorithm_version: "2".into(),
            cue_point_algorithm_version: "3".into(),
        };
        let json = serde_json::to_string(&request).unwrap();
        let decoded: AnalysisRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.downbeat_time.to_bits(), (-0.0f64).to_bits());
        assert_eq!(decoded.segments[0].features[0].to_bits(), value.to_bits());
        assert_eq!(
            decoded.energy_segment_data[0].volume_rms_db.to_bits(),
            (-value).to_bits()
        );
        assert!(!json.contains("VIPCode"));
        assert!(!json.contains("EnergyLevel"));
    }
}
