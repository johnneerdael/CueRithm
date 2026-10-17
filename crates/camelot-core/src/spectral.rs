use rustfft::{Fft, FftPlanner, num_complex::Complex};
use std::sync::Arc;

pub(crate) struct Spectrum {
    fft: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
    buffer: Vec<Complex<f32>>,
    scratch: Vec<Complex<f32>>,
}

impl Spectrum {
    pub fn new(size: usize) -> Self {
        let fft = FftPlanner::new().plan_fft_forward(size);
        let scratch = vec![Complex::default(); fft.get_inplace_scratch_len()];
        Self {
            fft,
            window: (0..size)
                .map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / size as f32).cos())
                .collect(),
            buffer: vec![Complex::default(); size],
            scratch,
        }
    }

    pub fn magnitudes(&mut self, input: &[f32], output: &mut [f64]) {
        for ((value, sample), window) in self.buffer.iter_mut().zip(input).zip(&self.window) {
            *value = Complex::new(sample * window, 0.0);
        }
        self.fft
            .process_with_scratch(&mut self.buffer, &mut self.scratch);
        for (magnitude, value) in output.iter_mut().zip(&self.buffer) {
            *magnitude = value.norm() as f64;
        }
    }
}
