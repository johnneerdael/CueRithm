use crate::{
    EnergySegment,
    dsp::{
        energy_band_rust_v1, energy_feature_rust_v1, energy_log10_rust_v1, energy_temporal_rust_v1,
    },
};
use anyhow::{Result, ensure};

pub(crate) fn analyze(pcm: &[f32], full_frames: usize) -> Result<Vec<EnergySegment>> {
    let duration = full_frames as f64 / 44100.0;
    let mut stream = energy_band_rust_v1::stream::Stream::new(2, duration, 0.0, 0.0)
        .map_err(anyhow::Error::msg)?;
    for feed in pcm.chunks(1024) {
        stream.feed(feed).map_err(anyhow::Error::msg)?;
    }
    let input = stream.timelines();
    if input.raw_rms.is_empty() {
        return Ok(Vec::new());
    }
    ensure!(
        input.useful_begin == 0 && input.useful_end == input.raw_rms.len(),
        "energy useful interval differs from the default full slice"
    );
    let output = energy_temporal_rust_v1::analyze(
        input.sustained,
        input.transient,
        input.weighted_rms,
        input.raw_rms,
        duration,
        0.0,
        0.0,
    )
    .map_err(anyhow::Error::msg)?;
    drop(stream);
    let descriptions = output.descriptions;
    drop(output.smoothed);
    let Some(first) = descriptions.first() else {
        return Ok(Vec::new());
    };
    let interval = (60.0 / first.tempo) * 32.0;
    let mut segments = Vec::with_capacity(descriptions.len());
    for (index, description) in descriptions.iter().enumerate() {
        if description.rejected {
            continue;
        }
        let record = energy_feature_rust_v1::reduce(description).map_err(anyhow::Error::msg)?;
        let start = interval * index as f64;
        let end = interval * (index + 1) as f64;
        let volume = energy_log10_rust_v1::log10(description.rms.max(1e-50))
            .map_err(anyhow::Error::msg)?
            * 20.0;
        segments.push(EnergySegment {
            start_time: start,
            end_time: start + (end - start),
            volume_rms_db: volume,
            features: record.features,
        });
    }
    Ok(segments)
}
