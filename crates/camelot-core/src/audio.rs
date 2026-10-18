//! Decode samples only. No analysis tags, filenames or metadata labels enter DSP.
use std::{collections::HashSet, fs::File, io::ErrorKind, path::Path};

use anyhow::{Context, Result, bail, ensure};
use rubato::{FftFixedIn, Resampler};
use symphonia::core::{
    audio::SampleBuffer, codecs::DecoderOptions, errors::Error as DecodeError,
    formats::FormatOptions, io::MediaSourceStream, meta::MetadataOptions, probe::Hint,
};

#[derive(Debug)]
pub struct DecodedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub source_sample_rate: u32,
    pub complete: bool,
    pub verified_integrity: Option<bool>,
}

impl DecodedAudio {
    pub fn duration_seconds(&self) -> f64 {
        self.samples.len() as f64 / self.sample_rate as f64
    }
}

struct MonoResampler {
    resampler: Option<FftFixedIn<f32>>,
    pending: Vec<f32>,
    output: Vec<f32>,
    source_rate: u32,
    target_rate: u32,
    frames: usize,
}

impl MonoResampler {
    fn new(source_rate: u32, target_rate: u32) -> Result<Self> {
        let resampler = if source_rate == target_rate {
            None
        } else {
            let (mut a, mut b) = (source_rate, target_rate);
            while b != 0 {
                (a, b) = (b, a % b);
            }
            // A rational FFT resampler needs at least one reduced-ratio input
            // block before producing output. Align chunks so a valid short-
            // audio flush cannot look like a stalled resampler at high/odd rates.
            let block = (source_rate / a) as usize;
            let chunk = 2048_usize.div_ceil(block) * block;
            Some(FftFixedIn::new(
                source_rate as usize,
                target_rate as usize,
                chunk,
                2,
                1,
            )?)
        };
        Ok(Self {
            resampler,
            pending: Vec::new(),
            output: Vec::new(),
            source_rate,
            target_rate,
            frames: 0,
        })
    }

    fn push(&mut self, samples: &[f32]) -> Result<()> {
        self.frames += samples.len();
        let Some(resampler) = &mut self.resampler else {
            self.output.extend_from_slice(samples);
            return Ok(());
        };
        self.pending.extend_from_slice(samples);
        let mut consumed = 0;
        let chunk = resampler.input_frames_next();
        while self.pending.len() - consumed >= chunk {
            let result = resampler.process(&[&self.pending[consumed..consumed + chunk]], None)?;
            self.output.extend_from_slice(&result[0]);
            consumed += chunk;
        }
        self.pending.drain(..consumed);
        Ok(())
    }

    fn finish(mut self) -> Result<Vec<f32>> {
        let Some(mut resampler) = self.resampler else {
            return Ok(self.output);
        };
        let delay = resampler.output_delay();
        let expected = (self.frames as f64 * self.target_rate as f64 / self.source_rate as f64)
            .round() as usize;
        let tail = if self.pending.is_empty() {
            resampler.process_partial::<&[f32]>(None, None)?
        } else {
            resampler.process_partial(Some(&[&self.pending]), None)?
        };
        self.output.extend_from_slice(&tail[0]);
        while self.output.len() < expected + delay {
            let tail = resampler.process_partial::<&[f32]>(None, None)?;
            ensure!(
                !tail[0].is_empty(),
                "Resampler made no progress while flushing"
            );
            self.output.extend_from_slice(&tail[0]);
        }
        self.output.drain(..delay);
        self.output.truncate(expected);
        Ok(self.output)
    }
}

pub fn decode(path: &Path, target_rate: u32, max_seconds: Option<f64>) -> Result<DecodedAudio> {
    Ok(decode_at_rates(path, &[target_rate], max_seconds)?.remove(0))
}

/// Decode/downmix once and resample directly from source PCM at each requested
/// rate. Preserve order; rates must be nonempty and unique. All outputs share
/// the same source-integrity/completion checks and excerpt boundary.
pub fn decode_at_rates(
    path: &Path,
    target_rates: &[u32],
    max_seconds: Option<f64>,
) -> Result<Vec<DecodedAudio>> {
    Ok(decode_inputs(path, target_rates, max_seconds, false)?.0)
}

/// Decode once, retaining native-rate first-channel Float32 PCM for timing in
/// addition to the unchanged downmixed analysis rates. Both use the same packet
/// boundaries, gapless trimming, source identity and completion checks.
pub fn decode_at_rates_with_first_channel(
    path: &Path,
    target_rates: &[u32],
    max_seconds: Option<f64>,
) -> Result<(Vec<DecodedAudio>, DecodedAudio)> {
    let (mixed, first) = decode_inputs(path, target_rates, max_seconds, true)?;
    Ok((mixed, first.context("Missing first-channel timing input")?))
}

fn decode_inputs(
    path: &Path,
    target_rates: &[u32],
    max_seconds: Option<f64>,
    retain_first_channel: bool,
) -> Result<(Vec<DecodedAudio>, Option<DecodedAudio>)> {
    ensure!(
        !target_rates.is_empty(),
        "At least one analysis sample rate is required"
    );
    let mut rates = HashSet::new();
    for &rate in target_rates {
        ensure!(
            (8000..=192000).contains(&rate),
            "Unsupported analysis sample rate"
        );
        ensure!(rates.insert(rate), "Duplicate analysis sample rate");
    }
    if let Some(limit) = max_seconds {
        ensure!(
            limit.is_finite() && limit > 0.0,
            "Analysis duration must be positive and finite"
        );
    }
    let before = path.metadata().context("Read source identity")?;
    let source = MediaSourceStream::new(Box::new(File::open(path)?), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|x| x.to_str()) {
        hint.with_extension(extension);
    }
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            source,
            &FormatOptions {
                enable_gapless: true,
                ..Default::default()
            },
            &MetadataOptions::default(),
        )
        .context("Probe audio")?;
    let mut format = probed.format;
    let track = format.default_track().context("No default audio track")?;
    let track_id = track.id;
    let params = track.codec_params.clone();
    let source_rate = params.sample_rate.context("Audio has no sample rate")?;
    ensure!(
        (8000..=384000).contains(&source_rate),
        "Unsupported source sample rate"
    );
    ensure!(
        !retain_first_channel || source_rate <= 192000,
        "Unsupported native timing sample rate"
    );
    let expected_frames = params.n_frames;
    let trim_allowance = params.delay.unwrap_or(0) as u64 + params.padding.unwrap_or(0) as u64;
    let mut decoder =
        symphonia::default::get_codecs().make(&params, &DecoderOptions { verify: true })?;
    let mut resamplers = target_rates
        .iter()
        .map(|rate| MonoResampler::new(source_rate, *rate))
        .collect::<Result<Vec<_>>>()?;
    let frame_limit = max_seconds.map(|t| (t * source_rate as f64).round() as u64);
    let mut frames = 0_u64;
    let mut reached_limit = false;
    let mut first_channel = retain_first_channel.then(Vec::new);
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(DecodeError::IoError(e)) if e.kind() == ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(error).context("Read audio packet"),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = decoder.decode(&packet).context("Decode audio packet")?;
        ensure!(
            decoded.spec().rate == source_rate,
            "Sample rate changed during decoding"
        );
        let channels = decoded.spec().channels.count();
        ensure!(channels > 0, "Audio packet has no channels");
        let mut interleaved = SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
        interleaved.copy_interleaved_ref(decoded);
        let remaining =
            frame_limit.map_or(usize::MAX, |limit| limit.saturating_sub(frames) as usize);
        let mono: Vec<f32> = interleaved
            .samples()
            .chunks_exact(channels)
            .take(remaining)
            .map(|frame| frame.iter().sum::<f32>() / channels as f32)
            .collect();
        ensure!(
            mono.iter().all(|x| x.is_finite()),
            "Non-finite audio samples"
        );
        if let Some(first) = &mut first_channel {
            first.extend(
                interleaved
                    .samples()
                    .chunks_exact(channels)
                    .take(remaining)
                    .map(|frame| frame[0]),
            );
        }
        frames += mono.len() as u64;
        for resampler in &mut resamplers {
            resampler.push(&mono)?;
        }
        if frame_limit.is_some_and(|limit| frames >= limit) {
            reached_limit = true;
            break;
        }
    }
    ensure!(frames > 0, "No audio samples decoded");
    let complete = !reached_limit
        || expected_frames.is_some_and(|n| frames >= n.saturating_sub(trim_allowance));
    if complete && let Some(expected) = expected_frames {
        ensure!(
            frames.abs_diff(expected) <= trim_allowance,
            "Truncated audio: decoded {frames} frames; expected {expected}"
        );
    }
    let verified_integrity = if complete {
        decoder.finalize().verify_ok
    } else {
        None
    };
    if verified_integrity == Some(false) {
        bail!("Audio integrity verification failed");
    }
    let after = path.metadata()?;
    ensure!(
        before.len() == after.len() && before.modified()? == after.modified()?,
        "Source changed during analysis"
    );
    let downmixed = resamplers
        .into_iter()
        .zip(target_rates)
        .map(|(resampler, &sample_rate)| {
            Ok(DecodedAudio {
                samples: resampler.finish()?,
                sample_rate,
                source_sample_rate: source_rate,
                complete,
                verified_integrity,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let first = first_channel.map(|samples| DecodedAudio {
        samples,
        sample_rate: source_rate,
        source_sample_rate: source_rate,
        complete,
        verified_integrity,
    });
    Ok((downmixed, first))
}
