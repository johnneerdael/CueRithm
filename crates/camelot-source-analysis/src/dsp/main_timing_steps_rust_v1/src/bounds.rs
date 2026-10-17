//! Resource policy for this private prototype, separate from target DSP semantics.
pub const MAX_NORMALIZED_BEATS: usize = 1_000_000;
pub const MAX_INPUT_BEATS: usize = 100_000;
pub const MAX_FEATURE_FRAMES: usize = 100_000;
pub const MAX_ABS_TIME: f64 = 86_400.0;
pub fn projected_normalized_count(times: &[f64], tempo: f64) -> Result<usize, String> {
    if times.len() > MAX_NORMALIZED_BEATS
        || !tempo.is_finite()
        || times
            .iter()
            .any(|t| !t.is_finite() || t.abs() > MAX_ABS_TIME)
        || times.windows(2).any(|p| p[1] <= p[0])
    {
        return Err("beat normalization violates bounded numeric/count contract".into());
    }
    if times.len() < 2 || tempo <= 0.0 {
        return Ok(times.len());
    }
    let span = times[times.len() - 1] - times[0];
    let ratio = ((60.0 / span) * (times.len() - 1) as f64) / tempo;
    if !span.is_finite() || span <= 0.0 || !ratio.is_finite() || ratio <= 0.0 {
        return Err("beat normalization ratio is not finite positive".into());
    }
    let exponent = (ratio.ln() / std::f64::consts::LN_2 + 0.5).floor();
    let mut factor = exponent.exp2();
    if !factor.is_finite() || factor <= 0.0 {
        return Err("beat normalization factor is not finite positive".into());
    }
    let mut count = times.len();
    let mut passes = 0;
    while factor > 1.75 {
        passes += 1;
        if passes > 64 {
            return Err("beat octave normalization exceeds bounded pass count".into());
        }
        count = count.div_ceil(2);
        factor *= 0.5;
    }
    while factor < 0.6 {
        passes += 1;
        if passes > 64 {
            return Err("beat octave normalization exceeds bounded pass count".into());
        }
        count = count
            .checked_mul(2)
            .and_then(|n| n.checked_sub(1))
            .filter(|&n| n <= MAX_NORMALIZED_BEATS)
            .ok_or("projected normalized beat count exceeds bounded maximum")?;
        factor += factor;
    }
    Ok(count)
}
pub fn validate_waveform(
    fs: f64,
    hop: i32,
    duration: f64,
    samples: &[f32],
    flags: &[u32],
) -> Result<(), String> {
    if !(1000.0..=384000.0).contains(&fs)
        || !(2..=65536).contains(&hop)
        || !duration.is_finite()
        || duration.abs() > MAX_ABS_TIME
        || samples.len() > MAX_FEATURE_FRAMES
        || samples.len() != flags.len()
        || samples.iter().any(|v| !v.is_finite() || *v < 0.0)
        || flags.iter().any(|v| *v > 255)
    {
        return Err("waveform violates bounded geometry/size/finite feature contract".into());
    }
    Ok(())
}
pub fn validate_legacy_entry(times: &[f64], tempo: f64) -> Result<(), String> {
    if times.len() > MAX_INPUT_BEATS {
        return Err("legacy input count exceeds bounded maximum".into());
    }
    projected_normalized_count(times, tempo)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn enormous_densification_is_rejected_without_allocating_it() {
        // Safe: this invokes only the projected-count checker, never v1 DSP.
        assert!(projected_normalized_count(&[0., 1.], 1e12).is_err());
    }
    #[test]
    fn extreme_finite_tempo_is_rejected_without_normalization_loop() {
        assert!(projected_normalized_count(&[0., 1.], f64::MAX).is_err());
    }
    #[test]
    fn nonfinite_span_is_rejected_before_logarithm() {
        assert!(projected_normalized_count(&[-f64::MAX, f64::MAX], 120.).is_err());
    }
}
