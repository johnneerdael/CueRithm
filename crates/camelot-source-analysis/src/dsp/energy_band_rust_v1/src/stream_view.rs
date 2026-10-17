use super::Stream;

/// Read the unnormalized timelines without serializing the retained test format.
pub(crate) struct Timelines<'a> {
    pub sustained: &'a [Vec<f32>; 14],
    pub transient: &'a [Vec<f32>; 14],
    pub weighted_rms: &'a [f32],
    pub raw_rms: &'a [f32],
    pub useful_begin: usize,
    pub useful_end: usize,
}

impl Stream {
    pub(crate) fn timelines(&self) -> Timelines<'_> {
        Timelines {
            sustained: &self.bands.sustained,
            transient: &self.bands.transient,
            weighted_rms: &self.weighted_rms,
            raw_rms: &self.raw_rms,
            useful_begin: self.useful_begin,
            useful_end: self.useful_end,
        }
    }
}
