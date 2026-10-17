pub struct Stream {
    bands: crate::dsp::energy_band_rust_v1::Bands,
    channels: usize,
    duration: f64,
    skip_start: f64,
    skip_end: f64,
    total: usize,
    cursor: usize,
    buffer: Vec<[f32; 2]>,
    raw_rms: Vec<f32>,
    weighted_rms: Vec<f32>,
    last_spectrum: Option<Vec<f32>>,
    useful_begin: usize,
    useful_end: usize,
}
impl Stream {
    pub fn new(
        channels: usize,
        duration: f64,
        skip_start: f64,
        skip_end: f64,
    ) -> Result<Self, String> {
        if !(1..=2).contains(&channels)
            || !duration.is_finite()
            || !(0.0..=64_000_000f64 / 44100.).contains(&duration)
            || !skip_start.is_finite()
            || !skip_end.is_finite()
            || skip_start < 0.
            || skip_end < 0.
            || skip_start + skip_end >= 1.
        {
            return Err("bounded default energy stream configuration required".into());
        }
        Ok(Self {
            bands: crate::dsp::energy_band_rust_v1::Bands::new()?,
            channels,
            duration,
            skip_start,
            skip_end,
            total: 0,
            cursor: 0,
            buffer: vec![],
            raw_rms: vec![],
            weighted_rms: vec![],
            last_spectrum: None,
            useful_begin: 0,
            useful_end: 0,
        })
    }
    pub fn feed(&mut self, pcm: &[f32]) -> Result<(), String> {
        if !pcm.len().is_multiple_of(self.channels)
            || pcm.len() / self.channels > 32768
            || pcm.iter().any(|v| !v.is_finite() || v.abs() > 1.)
        {
            return Err("bounded finite normalized mono/stereo feed required".into());
        }
        let count = pcm.len() / self.channels;
        if self.total + count > 64_000_000 {
            return Err("stream total frame bound".into());
        }
        if count == 0 {
            return Ok(());
        }
        for i in 0..count {
            let l = pcm[i * self.channels];
            let r = if self.channels == 1 {
                l
            } else {
                pcm[i * self.channels + 1]
            };
            self.buffer.push([l, r]);
        }
        self.total += count;
        while self.buffer.len() - self.cursor >= 2048 {
            let start = self.total - self.buffer.len() + self.cursor;
            let lower = ((self.duration * self.skip_start) * 44100.) as i32;
            let upper = (44100. * (self.duration * (1. - self.skip_end))) as i32;
            let index = self.raw_rms.len();
            if start + 2048 >= lower as usize && start <= upper as usize {
                let pcm: Vec<f32> = self.buffer[self.cursor..self.cursor + 2048]
                    .iter()
                    .flat_map(|v| *v)
                    .collect();
                let frame = crate::dsp::energy_frame_rust_v1::analyze(&pcm)?;
                if self.useful_begin == self.useful_end {
                    self.useful_begin = index;
                }
                self.useful_end = index + 1;
                self.raw_rms.push(frame.raw_rms);
                self.weighted_rms.push(frame.weighted_rms);
                self.bands.push(&frame.spectrum)?;
                self.last_spectrum = Some(frame.spectrum);
            } else {
                self.bands.skip()?;
                self.raw_rms.push(0.);
                self.weighted_rms.push(0.);
            }
            self.cursor += 682;
        }
        if self.cursor > 16384 {
            let drop = self.cursor & !15;
            self.buffer.drain(..drop);
            self.cursor -= drop;
        }
        Ok(())
    }
    fn base_state_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&0u32.to_le_bytes());
        for v in [
            self.total,
            self.buffer.len(),
            self.cursor,
            self.raw_rms.len(),
            self.useful_begin,
            self.useful_end,
            usize::from(self.last_spectrum.is_some()),
        ] {
            out.extend_from_slice(&(v as u32).to_le_bytes());
        }
        for v in [self.duration, self.skip_start, self.skip_end] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for stereo in &self.buffer {
            for v in stereo {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        for v in self.raw_rms.iter().chain(&self.weighted_rms) {
            out.extend_from_slice(&v.to_le_bytes());
        }
        if let Some(spectrum) = &self.last_spectrum {
            for v in spectrum {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        let length = (out.len() - 4) as u32;
        out[..4].copy_from_slice(&length.to_le_bytes());
        out
    }
    pub fn state_bytes(&self) -> Vec<u8> {
        let base = self.base_state_bytes();
        let bands = self.bands.state_bytes();
        let mut out = Vec::new();
        out.extend_from_slice(&((base.len() + bands.len()) as u32).to_le_bytes());
        out.extend_from_slice(&base);
        out.extend_from_slice(&bands);
        out
    }
    pub fn normalize(&mut self) -> Result<f64, String> {
        let mut values = self.weighted_rms[self.useful_begin..self.useful_end].to_vec();
        values.sort_by(f32::total_cmp);
        let quantile = crate::dsp::energy_band_rust_v1::lookup(&values, 0.95)?;
        let floor = if quantile > 0.0001 { quantile } else { 0.0001 };
        let gain = 1.0 / floor;
        let mut scaled = Vec::with_capacity(28);
        for row in self.bands.sustained.iter().chain(&self.bands.transient) {
            let result: Vec<f32> = row.iter().map(|x| (gain * f64::from(*x)) as f32).collect();
            if result.iter().any(|x| !x.is_finite()) {
                return Err("normalization outside finite intermediate profile".into());
            }
            scaled.push(result);
        }
        for (row, value) in self
            .bands
            .sustained
            .iter_mut()
            .chain(&mut self.bands.transient)
            .zip(scaled)
        {
            *row = value;
        }
        Ok(gain)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_frame_threshold_has_no_tail_padding() {
        let mut s = Stream::new(2, 60., 0., 0.).unwrap();
        s.feed(&vec![0.; 2047 * 2]).unwrap();
        assert!(s.raw_rms.is_empty());
        s.feed(&[0., 0.]).unwrap();
        assert_eq!(s.raw_rms.len(), 1);
        assert_eq!(s.cursor, 682);
    }
    #[test]
    fn trim_preserves_absolute_index_and_sixteen_sample_alignment() {
        let mut s = Stream::new(2, 60., 0., 0.).unwrap();
        s.feed(&vec![0.; 20000 * 2]).unwrap();
        assert_eq!(
            (s.total, s.buffer.len(), s.cursor, s.raw_rms.len()),
            (20000, 1600, 14, 27)
        );
    }
    #[test]
    fn skipped_frames_append_zero_and_leave_last_active_spectrum() {
        let mut s = Stream::new(1, 1., 0.25, 0.25).unwrap();
        s.feed(&vec![0.; 25000]).unwrap();
        s.feed(&vec![0.; 25000]).unwrap();
        assert_eq!((s.useful_begin, s.useful_end), (14, 49));
        assert_eq!(s.raw_rms.len(), 71);
        assert!(s.last_spectrum.is_some());
    }
    #[test]
    fn ingestion_retains_negative_zero_in_mono_duplication() {
        let mut s = Stream::new(1, 60., 0., 0.).unwrap();
        s.feed(&[-0.]).unwrap();
        assert_eq!(
            (s.buffer[0][0].to_bits(), s.buffer[0][1].to_bits()),
            (0x80000000, 0x80000000)
        );
    }
    #[test]
    fn normalization_scales_both_signed_families_without_changing_rms_or_history() {
        let mut s = Stream::new(2, 60., 0., 0.).unwrap();
        s.weighted_rms = vec![0., 1.];
        s.raw_rms = vec![0.25, 0.5];
        s.useful_begin = 0;
        s.useful_end = 2;
        s.bands.frames = 2;
        for row in &mut s.bands.sustained {
            *row = vec![0.25, -0.5];
        }
        for row in &mut s.bands.transient {
            *row = vec![0.5, 0.25];
        }
        let gain = s.normalize().unwrap();
        assert_eq!(gain.to_bits(), (1f64 / 0.95).to_bits());
        assert_eq!(
            s.bands.sustained[0],
            vec![(0.25f64 / 0.95) as f32, (-0.5f64 / 0.95) as f32]
        );
        assert_eq!(s.weighted_rms, vec![0., 1.]);
        assert_eq!(s.raw_rms, vec![0.25, 0.5]);
        assert!(s.bands.history.iter().all(Vec::is_empty));
        let before = s.bands.sustained[0][0];
        assert_eq!(s.normalize().unwrap(), gain);
        assert!(s.bands.sustained[0][0] > before);
    }
    #[test]
    fn empty_normalization_uses_source_floor() {
        let mut s = Stream::new(2, 60., 0., 0.).unwrap();
        assert_eq!(s.normalize().unwrap(), 10000.);
    }
    #[test]
    fn rejects_unsupported_config_and_malformed_feed() {
        assert!(Stream::new(0, 60., 0., 0.).is_err());
        assert!(Stream::new(2, 60., 0.5, 0.5).is_err());
        let mut s = Stream::new(2, 60., 0., 0.).unwrap();
        assert!(s.feed(&[0.]).is_err());
    }
}

#[path = "stream_view.rs"]
pub(crate) mod integration_view;
