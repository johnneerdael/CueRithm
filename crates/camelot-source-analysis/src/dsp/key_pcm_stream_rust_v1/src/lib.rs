use crate::dsp::key_peak_tracker_portable_v2::{Peak, Tracker};
use sha2::{Digest, Sha256};

#[rustfmt::skip]
#[allow(dead_code)]
#[allow(clippy::chunks_exact_to_as_chunks)] // Preserve the frozen FFT prototype byte for byte.
#[path = "../frozen-fft/fft.rs"]
mod frozen_fft;

pub struct Emission {
    pub start_sample: usize,
    pub mode: &'static str,
    pub peaks: Vec<Peak>,
    pub scores: [f32; 16],
}

pub struct Stream {
    buffer: Vec<f32>,
    cursor: usize,
    discarded: usize,
    input_frames: usize,
    reciprocal: f64,
    finished: bool,
    tracker: Tracker,
    pub emissions: Vec<Emission>,
    pub tuning: Vec<f64>,
    pub cache: Vec<[f32; 88]>,
}

impl Default for Stream {
    fn default() -> Self {
        Self {
            buffer: Vec::new(),
            cursor: 0,
            discarded: 0,
            input_frames: 0,
            reciprocal: 1.0,
            finished: false,
            tracker: Tracker::default(),
            emissions: Vec::new(),
            tuning: Vec::new(),
            cache: Vec::new(),
        }
    }
}

pub fn float32_bits(values: &[f32]) -> Vec<String> {
    values
        .iter()
        .map(|v| format!("{:08x}", v.to_bits()))
        .collect()
}

pub fn peak_bits(peaks: &[Peak]) -> Vec<[String; 2]> {
    peaks
        .iter()
        .map(|p| {
            [
                format!("{:08x}", p.frequency.to_bits()),
                format!("{:08x}", p.amplitude.to_bits()),
            ]
        })
        .collect()
}

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

impl Stream {
    #[allow(clippy::assign_op_pattern, clippy::manual_clamp)] // Preserve source operand/check order.
    pub fn feed(&mut self, interleaved: &[f32], channels: usize) -> Result<(), String> {
        if self.finished {
            return Err("cannot feed after finish".into());
        }
        if !(1..=8).contains(&channels) || !interleaved.len().is_multiple_of(channels) {
            return Err("unsupported channel count or incomplete interleaved frame".into());
        }
        if interleaved.iter().any(|v| !v.is_finite() || v.abs() > 4.0) {
            return Err("bounded finite PCM required".into());
        }
        let frames = interleaved.len() / channels;
        if frames == 0 {
            return Ok(());
        }
        self.reciprocal = 1.0 / channels as f64;
        let mut mono = Vec::with_capacity(frames);
        if channels == 2 {
            let mut total = 0.0_f32;
            for frame in interleaved.as_chunks::<2>().0 {
                mono.push(frame[0] + frame[1]);
                total = (total + frame[0]) + frame[1];
            }
            if !(-2147483648.0..=2147483648.0).contains(&total) {
                mono.fill(0.0);
            }
        } else {
            for frame in interleaved.chunks_exact(channels) {
                let mut value = 0.0_f32;
                for &sample in frame {
                    value = sample + value;
                }
                if value.is_nan() {
                    value = 0.0;
                }
                if value <= -2147483648.0 {
                    value = -2147483648.0;
                }
                if value > 2147483648.0 {
                    value = 2147483648.0;
                }
                mono.push(value);
            }
        }
        self.buffer.extend(mono);
        self.input_frames += frames;
        while self.buffer.len() - self.cursor >= 6144 {
            let (first, second) = frozen_fft::dual(
                &self.buffer[self.cursor..self.cursor + 4096],
                &self.buffer[self.cursor + 2048..self.cursor + 6144],
            );
            self.emit(&first[..513], self.discarded + self.cursor, "dual_first")?;
            self.emit(
                &second[..513],
                self.discarded + self.cursor + 2048,
                "dual_second",
            )?;
            self.cursor += 4096;
        }
        self.compact(32768);
        Ok(())
    }

    pub fn finish(&mut self) -> Result<(), String> {
        if self.finished {
            return Ok(());
        }
        while self.buffer.len() - self.cursor >= 4096 {
            let mut spectrum: Vec<_> = self.buffer[self.cursor..self.cursor + 4096]
                .iter()
                .map(|&re| frozen_fft::Complex { re, im: 0.0 })
                .collect();
            frozen_fft::fft(&mut spectrum);
            self.emit(&spectrum[..513], self.discarded + self.cursor, "single")?;
            self.cursor += 2048;
        }
        self.compact(16384);
        let scores: Vec<_> = self.emissions.iter().map(|e| e.scores).collect();
        self.tuning = crate::dsp::key_tuning_portable_v2::finish(&scores);
        let kernel = crate::dsp::key_cache_finalizer_portable_v4::Kernel::default();
        self.cache = self
            .emissions
            .iter()
            .zip(&self.tuning)
            .map(|(emission, &tuning)| {
                let peaks: Vec<_> = emission
                    .peaks
                    .iter()
                    .map(|p| [p.frequency, p.amplitude])
                    .collect();
                kernel.frame(&peaks, tuning)
            })
            .collect();
        self.finished = true;
        Ok(())
    }

    fn compact(&mut self, threshold: usize) {
        if self.cursor > threshold {
            let removed = self.cursor & !15;
            self.buffer.drain(..removed);
            self.cursor -= removed;
            self.discarded += removed;
        }
    }

    fn emit(
        &mut self,
        values: &[frozen_fft::Complex],
        start_sample: usize,
        mode: &'static str,
    ) -> Result<(), String> {
        let spectrum: Vec<_> = values
            .iter()
            .map(|z| crate::dsp::key_peak_tracker_portable_v2::Complex { re: z.re, im: z.im })
            .collect();
        let peaks = self.tracker.push(&spectrum)?;
        let pairs: Vec<_> = peaks.iter().map(|p| [p.frequency, p.amplitude]).collect();
        let scores = crate::dsp::key_tuning_portable_v2::score_frame(&pairs);
        self.emissions.push(Emission {
            start_sample,
            mode,
            peaks,
            scores,
        });
        Ok(())
    }

    pub fn rows(&self, start: usize) -> Vec<serde_json::Value> {
        self.emissions.iter().enumerate().skip(start).map(|(frame,e)| serde_json::json!({
            "frame":frame,"retained_peaks":peak_bits(&e.peaks),"score_bits":float32_bits(&e.scores)
        })).collect()
    }

    pub fn state(&self) -> serde_json::Value {
        let buffer_bytes: Vec<_> = self.buffer.iter().flat_map(|f| f.to_le_bytes()).collect();
        let mut retained_bytes = (self.emissions.len() as u32).to_le_bytes().to_vec();
        let mut score_bytes = Vec::new();
        for e in &self.emissions {
            retained_bytes.extend_from_slice(&(e.peaks.len() as u32).to_le_bytes());
            for p in &e.peaks {
                retained_bytes.extend_from_slice(&p.frequency.to_le_bytes());
                retained_bytes.extend_from_slice(&p.amplitude.to_le_bytes());
            }
            for score in e.scores {
                score_bytes.extend_from_slice(&score.to_le_bytes());
            }
        }
        let queue: Vec<_> = self.tracker.queue().iter().map(|p| peak_bits(p)).collect();
        serde_json::json!({
            "buffer_frames":self.buffer.len(),"buffer_sha256":digest(&buffer_bytes),
            "buffer_first8_bits":float32_bits(&self.buffer[..self.buffer.len().min(8)]),
            "buffer_last8_bits":float32_bits(&self.buffer[self.buffer.len().saturating_sub(8)..]),
            "cursor":self.cursor,"input_frames":self.input_frames,"channel_reciprocal_bits":format!("{:016x}",self.reciprocal.to_bits()),
            "finished":self.finished,"retained_frames":self.emissions.len(),"score_frames":self.emissions.len(),
            "tuning_frames":self.tuning.len(),"cache_frames":self.cache.len(),
            "harmonicity_queue":queue,"retained_sha256":digest(&retained_bytes),"scores_sha256":digest(&score_bytes)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_sum_is_not_normalized() {
        let mut stream = Stream::default();
        stream.feed(&[0.25, 0.5], 2).unwrap();
        assert_eq!(stream.buffer, [0.75]);
        assert_eq!(stream.reciprocal.to_bits(), 0.5_f64.to_bits());
    }

    #[test]
    fn generic_channel_fold_retains_order_and_signed_zero() {
        let mut stream = Stream::default();
        stream.feed(&[1.0, 2.0_f32.powi(-24), -1.0], 3).unwrap();
        assert_eq!(stream.buffer[0].to_bits(), 0);
        for (channels, expected) in [(1, 0), (2, 0x80000000), (3, 0)] {
            let mut stream = Stream::default();
            stream.feed(&vec![-0.0; channels], channels).unwrap();
            assert_eq!(stream.buffer[0].to_bits(), expected);
        }
    }

    #[test]
    fn incomplete_input_is_not_padded_at_finish() {
        let mut stream = Stream::default();
        stream.feed(&vec![0.0; 4095], 1).unwrap();
        stream.finish().unwrap();
        assert!(stream.emissions.is_empty());
        assert_eq!(stream.buffer.len(), 4095);
    }

    #[test]
    fn one_complete_window_waits_until_finish() {
        let mut stream = Stream::default();
        stream.feed(&vec![0.0; 4096], 1).unwrap();
        assert!(stream.emissions.is_empty());
        stream.finish().unwrap();
        assert_eq!(stream.emissions.len(), 1);
        assert_eq!(stream.emissions[0].mode, "single");
        assert_eq!(stream.emissions[0].start_sample, 0);
        assert_eq!(stream.cursor, 2048);
        assert_eq!(stream.tuning.len(), 1);
        assert_eq!(stream.cache.len(), 1);
    }

    #[test]
    fn paired_windows_require_6144_samples() {
        let mut stream = Stream::default();
        stream.feed(&vec![0.0; 6143], 1).unwrap();
        assert!(stream.emissions.is_empty());
        stream.feed(&[0.0], 1).unwrap();
        assert_eq!(stream.emissions.len(), 2);
        assert_eq!(
            (stream.emissions[0].mode, stream.emissions[1].mode),
            ("dual_first", "dual_second")
        );
        assert_eq!(
            (
                stream.emissions[0].start_sample,
                stream.emissions[1].start_sample
            ),
            (0, 2048)
        );
        stream.finish().unwrap();
        assert_eq!(stream.emissions.len(), 2);
    }

    #[test]
    fn original_512_frame_feeds_emit_pair_at_twelfth_call() {
        let mut stream = Stream::default();
        for _ in 0..11 {
            stream.feed(&vec![0.0; 512], 1).unwrap();
        }
        assert!(stream.emissions.is_empty());
        stream.feed(&vec![0.0; 512], 1).unwrap();
        assert_eq!(stream.emissions.len(), 2);
    }

    #[test]
    fn final_complete_single_window_follows_dual_pair() {
        let mut stream = Stream::default();
        stream.feed(&vec![0.0; 8192], 1).unwrap();
        assert_eq!(stream.emissions.len(), 2);
        stream.finish().unwrap();
        assert_eq!(stream.emissions.len(), 3);
        assert_eq!(stream.emissions[2].mode, "single");
        assert_eq!(stream.emissions[2].start_sample, 4096);
    }

    #[test]
    fn compaction_thresholds_differ_between_feeding_and_finish() {
        let mut stream = Stream::default();
        for _ in 0..68 {
            stream.feed(&vec![0.0; 512], 1).unwrap();
        }
        assert_eq!((stream.buffer.len(), stream.cursor), (34816, 32768));
        assert_eq!(stream.discarded, 0);
        stream.finish().unwrap();
        assert_eq!((stream.buffer.len(), stream.cursor), (2048, 0));
        assert_eq!(stream.discarded, 32768);
    }

    #[test]
    fn invalid_input_is_rejected_before_state_changes() {
        let mut stream = Stream::default();
        assert!(stream.feed(&[0.0], 0).is_err());
        assert!(stream.feed(&[0.0], 2).is_err());
        assert!(stream.feed(&[f32::NAN], 1).is_err());
        assert!(stream.buffer.is_empty());
        assert_eq!(stream.input_frames, 0);
    }

    #[test]
    fn finish_is_idempotent_and_never_flushes_tracking_tail() {
        let mut stream = Stream::default();
        stream.feed(&vec![0.0; 8192], 1).unwrap();
        stream.finish().unwrap();
        let count = stream.emissions.len();
        stream.finish().unwrap();
        assert_eq!(count, 3);
        assert_eq!(stream.emissions.len(), count);
        assert_eq!(stream.cache.len(), count);
    }
}
