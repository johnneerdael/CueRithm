//! Source-derived bounded classic mark storage and lookup primitives.
//! This is not a native ABI, allocator reproduction, or complete beat detector.
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct Mark {
    pub sample: i32,
    pub transition_energy: f32,
    pub local_bpm: f32,
    pub confidence: f32,
}
pub const MAX_MARKS: usize = 1_000_000;
pub const MAX_COORDINATE: i32 = 1_000_000_000;
const ZERO: Mark = Mark {
    sample: 0,
    transition_energy: 0.0,
    local_bpm: 0.0,
    confidence: 0.0,
};
#[derive(Debug, PartialEq)]
pub enum Error {
    Bounds,
    NonFinite,
    Index,
    Initialized,
    Capacity,
    Allocation,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
fn coordinate(sample: i32) -> Result<(), Error> {
    if !(-MAX_COORDINATE..=MAX_COORDINATE).contains(&sample) {
        return Err(Error::Bounds);
    }
    Ok(())
}
fn finite(x: f32) -> Result<(), Error> {
    if !x.is_finite() {
        return Err(Error::NonFinite);
    }
    Ok(())
}
impl Mark {
    /// Literal source storage codec. Arithmetic/container admission validates separately.
    pub fn to_le_bytes(self) -> [u8; 16] {
        let mut out = [0; 16];
        out[..4].copy_from_slice(&self.sample.to_le_bytes());
        out[4..8].copy_from_slice(&self.transition_energy.to_le_bytes());
        out[8..12].copy_from_slice(&self.local_bpm.to_le_bytes());
        out[12..].copy_from_slice(&self.confidence.to_le_bytes());
        out
    }
    pub fn from_le_bytes(bytes: [u8; 16]) -> Self {
        Self {
            sample: i32::from_le_bytes(bytes[..4].try_into().unwrap()),
            transition_energy: f32::from_le_bytes(bytes[4..8].try_into().unwrap()),
            local_bpm: f32::from_le_bytes(bytes[8..12].try_into().unwrap()),
            confidence: f32::from_le_bytes(bytes[12..].try_into().unwrap()),
        }
    }
    /// Classic GetBeatMark projection: source sample@0 plus fields@8/@12, omitting@4.
    pub fn export_le_bytes(self) -> [u8; 12] {
        let bytes = self.to_le_bytes();
        let mut out = [0; 12];
        out[..4].copy_from_slice(&bytes[..4]);
        out[4..].copy_from_slice(&bytes[8..]);
        out
    }
    fn validate(self) -> Result<(), Error> {
        coordinate(self.sample)?;
        finite(self.transition_energy)?;
        finite(self.local_bpm)?;
        finite(self.confidence)
    }
}
/// `slots.len()` is the source's semantic capacity, distinct from count and Rust heap capacity.
/// Newly allocated unused slots are safe zeros, not native uninitialized-memory witnesses.
#[derive(Default)]
pub struct MarkBuffer {
    slots: Vec<Mark>,
    count: usize,
    initialized: bool,
}
impl MarkBuffer {
    pub fn new() -> Self {
        Self::default()
    }
    fn grow_to(&mut self, capacity: usize) -> Result<(), Error> {
        if capacity > MAX_MARKS {
            return Err(Error::Bounds);
        }
        if capacity > self.slots.len() {
            self.slots
                .try_reserve_exact(capacity - self.slots.len())
                .map_err(|_| Error::Allocation)?;
            self.slots.resize(capacity, ZERO);
        }
        Ok(())
    }
    pub fn generate_initial_buffers(&mut self, capacity: usize) -> Result<(), Error> {
        if self.initialized {
            return Err(Error::Initialized);
        }
        // Source permits a hazardous overwrite after uninitialized AddMark; excluded explicitly.
        if self.count != 0 || !self.slots.is_empty() {
            return Err(Error::Capacity);
        }
        self.grow_to(capacity)?;
        self.initialized = true;
        Ok(())
    }
    pub fn capacity(&self) -> usize {
        self.slots.len()
    }
    pub fn count(&self) -> usize {
        self.count
    }
    pub fn initialized(&self) -> bool {
        self.initialized
    }
    pub fn records(&self) -> &[Mark] {
        &self.slots[..self.count]
    }
    pub fn add(&mut self, mark: Mark) -> Result<(), Error> {
        mark.validate()?;
        let next = self.count.checked_add(1).ok_or(Error::Bounds)?;
        self.grow_to(next)?;
        self.slots[self.count] = mark;
        self.count = next;
        Ok(())
    }
    pub fn add_transition(&mut self, sample: i32, energy: f32) -> Result<(), Error> {
        self.add(Mark {
            sample,
            transition_energy: energy,
            ..ZERO
        })
    }
    pub fn insert(&mut self, index: i32, mark: Mark) -> Result<(), Error> {
        if index < 0 || index as usize > self.count {
            return Err(Error::Index);
        }
        mark.validate()?;
        let index = index as usize;
        let next = self.count.checked_add(1).ok_or(Error::Bounds)?;
        self.grow_to(next)?;
        self.slots.copy_within(index..self.count, index + 1);
        self.slots[index] = mark;
        self.count = next;
        Ok(())
    }
    pub fn remove_last(&mut self) -> Option<Mark> {
        if self.count == 0 {
            return None;
        }
        self.count -= 1;
        Some(self.slots[self.count])
    }
    pub fn reset(&mut self) {
        self.slots.fill(ZERO);
        self.count = 0;
    }
    /// FlushMarks retains the LAST n physical records at the front without shrinking capacity.
    pub fn flush_last(&mut self, count: usize) -> Result<(), Error> {
        if count > self.count {
            return Err(Error::Index);
        }
        self.slots.copy_within(self.count - count..self.count, 0);
        self.count = count;
        Ok(())
    }
    pub fn put_buffers(&mut self, records: &[Mark]) -> Result<(), Error> {
        if records.len() > MAX_MARKS {
            return Err(Error::Bounds);
        }
        for &mark in records {
            mark.validate()?;
        }
        // 32944..32958: an uninitialized existing capacity must be zero or equal to n.
        if !self.initialized && self.capacity() != 0 && self.capacity() != records.len() {
            return Err(Error::Capacity);
        }
        self.grow_to(records.len())?;
        self.slots[..records.len()].copy_from_slice(records);
        self.count = records.len();
        self.initialized = true;
        Ok(())
    }
    pub fn get_mark(&self, index: i32) -> i32 {
        if index < 0 || index as usize >= self.count {
            -1
        } else {
            self.slots[index as usize].sample
        }
    }
    pub fn last_mark(&self) -> i32 {
        self.records().last().map_or(-1, |m| m.sample)
    }
    pub fn get_dist(&self, index: i32) -> i32 {
        if index < 0 || index as usize >= self.count.saturating_sub(1) {
            return -1;
        }
        let i = index as usize;
        self.slots[i + 1].sample - self.slots[i].sample
    }
    /// Inherited physical API, never translated through an onset threshold map.
    pub fn transition_energy(&self, index: i32) -> f32 {
        if index < 0 || index as usize >= self.count {
            -1.0
        } else {
            self.slots[index as usize].transition_energy
        }
    }
    pub fn set_transition_energy(&mut self, index: i32, value: f32) -> Result<(), Error> {
        if index < 0 || index as usize >= self.count {
            return Err(Error::Index);
        }
        finite(value)?;
        self.slots[index as usize].transition_energy = value;
        Ok(())
    }
    /// Base CBeatMarks method is a no-op; OnsetMarks overrides it.
    pub fn set_transition_threshold(&mut self, threshold: f32) -> Result<(), Error> {
        finite(threshold)
    }
    /// Exact source scan32848: later distance ties, first increase stop, special final sentinel.
    pub fn get_closest(&self, start: i32, target: i32) -> Result<i32, Error> {
        coordinate(target)?;
        if start < 0 {
            return Err(Error::Index);
        }
        let start = start as usize;
        if self.count < 2 || start >= self.count - 1 {
            return Ok(-1);
        }
        // Explicit coordinate bounds guarantee every signed subtraction fits i32.
        let mut distance = (self.slots[start].sample - target).unsigned_abs();
        for i in start + 1..self.count {
            let next = (self.slots[i].sample - target).unsigned_abs();
            if next > distance {
                return Ok((i - 1) as i32);
            }
            distance = next;
        }
        Ok(if self.slots[self.count - 1].sample > target {
            (self.count - 1) as i32
        } else {
            -1
        })
    }
}
/// Only virtual count/GetMark use the snapshot threshold map. Inherited APIs stay physical.
#[derive(Default)]
pub struct OnsetMarks {
    buffer: MarkBuffer,
    map: Vec<usize>,
    active: bool,
}
impl OnsetMarks {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn base(&self) -> &MarkBuffer {
        &self.buffer
    }
    pub fn generate_initial_buffers(&mut self, n: usize) -> Result<(), Error> {
        self.buffer.generate_initial_buffers(n)
    }
    pub fn add(&mut self, m: Mark) -> Result<(), Error> {
        self.buffer.add(m)
    }
    pub fn insert(&mut self, i: i32, m: Mark) -> Result<(), Error> {
        self.buffer.insert(i, m)
    }
    pub fn remove_last(&mut self) -> Option<Mark> {
        self.buffer.remove_last()
    }
    pub fn reset(&mut self) {
        self.buffer.reset();
    }
    pub fn flush_last(&mut self, n: usize) -> Result<(), Error> {
        self.buffer.flush_last(n)
    }
    pub fn put_buffers(&mut self, m: &[Mark]) -> Result<(), Error> {
        self.buffer.put_buffers(m)
    }
    pub fn set_transition_energy(&mut self, i: i32, v: f32) -> Result<(), Error> {
        self.buffer.set_transition_energy(i, v)
    }
    pub fn set_transition_threshold(&mut self, threshold: f32) -> Result<(), Error> {
        finite(threshold)?;
        let mut map = Vec::new();
        map.try_reserve_exact(self.buffer.count())
            .map_err(|_| Error::Allocation)?;
        for (i, m) in self.buffer.records().iter().enumerate() {
            if m.transition_energy > threshold {
                map.push(i);
            }
        }
        self.map = map;
        self.active = true;
        Ok(())
    }
    pub fn count(&self) -> usize {
        if self.active {
            self.map.len()
        } else {
            self.buffer.count()
        }
    }
    pub fn threshold_active(&self) -> bool {
        self.active
    }
    pub fn get_mark(&self, index: i32) -> i32 {
        if !self.active {
            return self.buffer.get_mark(index);
        }
        if index < 0 {
            return -1;
        }
        self.map
            .get(index as usize)
            .and_then(|&i| self.buffer.slots.get(i))
            .map_or(-1, |m| m.sample)
    }
}
