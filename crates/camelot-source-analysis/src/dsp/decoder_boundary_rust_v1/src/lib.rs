/// Input packet lengths are required. No track-specific data or reference labels.
#[derive(Default, Debug)]
pub struct BatchBoundary {
    pending: u64,
    emitted: u64,
}
impl BatchBoundary {
    pub fn push(&mut self, frames: u64) {
        if frames == 0 {
            return;
        }
        let available = self.pending + frames;
        if available < 4096 {
            self.emitted += available;
            self.pending = 0;
        } else {
            self.emitted += available / 4096 * 4096;
            self.pending = available % 4096;
        }
    }
    pub fn analyzed_frames(&self) -> u64 {
        self.emitted
    }
}
#[cfg(test)]
mod tests {
    use super::BatchBoundary;
    fn analyze(blocks: &[u64]) -> u64 {
        let mut state = BatchBoundary::default();
        for &n in blocks {
            state.push(n);
        }
        state.analyzed_frames()
    }
    #[test]
    fn preserves_exact_batches() {
        assert_eq!(analyze(&[4096, 8192]), 12288);
    }
    #[test]
    fn short_input_flushes_all_available() {
        assert_eq!(analyze(&[4608, 1024]), 5632);
    }
    #[test]
    fn completed_final_batch_keeps_residual_pending() {
        assert_eq!(analyze(&[4608, 4095]), 8192);
    }
    #[test]
    fn carries_pending_audio_between_input_packets() {
        assert_eq!(analyze(&[4608; 8]), 36864);
    }
    #[test]
    fn no_new_input_does_not_flush_pending_audio() {
        assert_eq!(analyze(&[4608, 0]), 4096);
    }
    #[test]
    fn same_total_with_different_packets_can_have_different_boundary() {
        assert_eq!(analyze(&[4608]), 4096);
        assert_eq!(analyze(&[4096, 512]), 4608);
    }
}
