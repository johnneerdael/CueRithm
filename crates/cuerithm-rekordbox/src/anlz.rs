//! Native PMAI/ANLZ files, including opaque sections. These APIs only stage bytes;
//! they do not publish files or coordinate a database/asset transaction.
//!
//! Numeric fields stay in native units. Callers must convert playable time once,
//! and update all related DAT/EXT/2EX files together in the sync journal.
use anyhow::{Context, Result, bail, ensure};

#[derive(Clone, Copy, Debug)]
pub struct ParseLimits {
    pub max_bytes: usize,
    pub max_tags: usize,
    pub max_entries: usize,
    pub max_string_bytes: usize,
}
impl Default for ParseLimits {
    fn default() -> Self {
        Self {
            max_bytes: 128 * 1024 * 1024,
            max_tags: 4096,
            max_entries: 1_000_000,
            max_string_bytes: 1024 * 1024,
        }
    }
}
/// PQTZ units: beat within bar (1..4), hundredths of BPM, unsigned milliseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Beat {
    pub number: u16,
    pub tempo: u16,
    pub time_ms: u32,
}
#[derive(Clone, Debug)]
pub struct LegacyCue {
    pub hot_cue: u32,
    pub cue_type: u8,
    pub time_ms: u32,
    pub loop_time_ms: u32,
    pub status: u32,
    pub order_first: u16,
    pub order_last: u16,
}
#[derive(Clone, Debug)]
pub struct ExtendedCue {
    pub hot_cue: u32,
    pub cue_type: u8,
    pub time_ms: u32,
    pub loop_time_ms: u32,
    pub memory_color: u8,
    pub loop_fraction: (u16, u16),
    pub comment: String,
    /// Native color-table code followed by LED red/green/blue. No palette guessing.
    pub color: [u8; 4],
}
#[derive(Clone, Debug)]
pub struct LegacyDetails {
    pub status: u32,
    pub order_first: u16,
    pub order_last: u16,
}
#[derive(Clone, Debug)]
pub struct ExtendedDetails {
    pub memory_color: Option<u8>,
    pub loop_fraction: Option<(u16, u16)>,
    pub comment: Option<String>,
    pub color: Option<[u8; 4]>,
}
#[derive(Clone, Debug)]
pub struct CueEntry {
    pub hot_cue: u32,
    pub cue_type: u8,
    pub time_ms: u32,
    pub loop_time_ms: u32,
    pub legacy: Option<LegacyDetails>,
    pub extended: Option<ExtendedDetails>,
    pub raw: Vec<u8>,
}
#[derive(Clone, Debug)]
pub struct CueList {
    pub kind: u32,
    pub entries: Vec<CueEntry>,
}
/// PQT2 has multiple dialects. Keep the 16-bit body words uninterpreted: some
/// producers pack beat positions here, while Lexicon also writes microseconds.
#[derive(Clone, Debug)]
pub struct ExtendedGrid {
    pub first: Option<Beat>,
    pub last: Option<Beat>,
    pub words: Vec<u16>,
    pub flags: u32,
    pub checksum: u32,
    pub downbeat: u32,
}
/// A microsecond PQT2 validated against its companion PQTZ, including anchors
/// and checksum. Native rekordbox's observed layout reserves the downbeat word.
#[derive(Clone, Debug)]
pub struct NativeMicrosecondGrid {
    pub beats: Vec<Beat>,
    pub fractions: Vec<u16>,
    pub variable: bool,
}
#[derive(Clone, Debug)]
pub struct AnlzTag {
    raw: Vec<u8>,
    limits: ParseLimits,
}
impl AnlzTag {
    fn waveform_layout(&self) -> Result<(usize, usize, Option<usize>)> {
        let (minimum, stride, count_at) = match &self.fourcc() {
            b"PVBR" => (16, 1604, None),
            b"PWAV" | b"PWV2" => (20, 1, Some(12)),
            b"PWV3" => (24, 1, Some(16)),
            b"PWV4" => (24, 6, Some(16)),
            b"PWV5" => (24, 2, Some(16)),
            b"PWV6" => (20, 3, Some(16)),
            b"PWV7" => (24, 3, Some(16)),
            b"PWVC" => (14, 6, None),
            _ => bail!("Unsupported native waveform section"),
        };
        self.expect(&self.fourcc(), minimum)?;
        if count_at == Some(16) {
            ensure!(
                u32be(&self.raw, 12)? as usize == stride,
                "Unsupported native waveform entry layout"
            );
        }
        let count =
            count_at.map_or(Ok(1), |at| u32be(&self.raw, at).map(|count| count as usize))?;
        ensure!(
            count <= self.limits.max_entries,
            "Native waveform exceeds entry budget"
        );
        let length = count
            .checked_mul(stride)
            .context("Native waveform payload length overflow")?;
        slice(&self.raw, self.header_len(), length)?;
        Ok((minimum, length, count_at))
    }
    /// A generated replacement may contain only the known header and payload.
    /// Existing files may carry opaque header extensions and trailing bytes.
    pub fn validate_waveform_template(&self) -> Result<()> {
        let (header, payload, _) = self.waveform_layout()?;
        ensure!(
            self.header_len() == header && self.raw.len() == header + payload,
            "Generated native waveform contains unsupported extensions"
        );
        Ok(())
    }
    /// Replace measured waveform data and its count, retaining the observed
    /// producer fields, header extensions and trailing opaque bytes.
    pub fn with_waveform(&self, generated: &AnlzTag) -> Result<Self> {
        ensure!(
            self.fourcc() == generated.fourcc(),
            "Native waveform replacement type mismatch"
        );
        let (_, old_payload, count_at) = self.waveform_layout()?;
        generated.validate_waveform_template()?;
        let (_, new_payload, _) = generated.waveform_layout()?;
        if let Some(at) = count_at {
            ensure!(
                u32be(&generated.raw, at)? as usize <= self.limits.max_entries,
                "Native waveform exceeds entry budget"
            );
        }
        let mut changed = self.with_payload(
            self.header_len(),
            old_payload,
            &generated.raw[generated.header_len()..generated.header_len() + new_payload],
        )?;
        if let Some(at) = count_at {
            changed.raw[at..at + 4].copy_from_slice(&generated.raw[at..at + 4]);
        }
        Ok(changed)
    }
    /// Edit a known legacy record without discarding opaque record extensions.
    pub fn with_legacy_cue(&self, kind: u32, cue: &LegacyCue) -> Result<Self> {
        self.expect(b"PCPT", 28)?;
        ensure!(self.raw.len() >= 40, "Truncated legacy cue record");
        let built = Self::legacy_cues(kind, std::slice::from_ref(cue))?;
        let replacement = built.cue_records()?;
        let mut tag = self.clone();
        for range in [12..20, 24..40] {
            tag.raw[range.clone()].copy_from_slice(&replacement[0].raw[range]);
        }
        Ok(tag)
    }
    /// Edit known fields/comment while retaining producer flags and trailing
    /// extensions. Short records without the complete known layout fail closed.
    pub fn with_extended_cue(&self, kind: u32, cue: &ExtendedCue) -> Result<Self> {
        self.expect(b"PCP2", 16)?;
        ensure!(
            self.header_len() == 16 && self.raw.len() >= 44,
            "Unsupported extended cue layout"
        );
        let old_size = u32be(&self.raw, 40)? as usize;
        slice(
            &self.raw,
            44,
            old_size.checked_add(4).context("Cue text overflow")?,
        )?;
        let built = Self::extended_cues(kind, std::slice::from_ref(cue))?;
        let replacement = built.cue_records()?;
        let entry = &replacement[0];
        let new_size = u32be(&entry.raw, 40)? as usize;
        ensure!(
            new_size <= self.limits.max_string_bytes,
            "Cue comment exceeds string budget"
        );
        let mut tag = self.with_payload(44, old_size, &entry.raw[44..44 + new_size])?;
        for range in [12..17, 20..29, 36..44] {
            tag.raw[range.clone()].copy_from_slice(&entry.raw[range]);
        }
        tag.raw[44 + new_size..48 + new_size]
            .copy_from_slice(&entry.raw[44 + new_size..48 + new_size]);
        Ok(tag)
    }
    /// Update only the explicitly recognized microsecond dialect; unknown PQT2
    /// dialects retain their opaque words rather than being coerced into it.
    pub fn with_microsecond_grid(
        &self,
        beats: &[Beat],
        fractions: &[u16],
        variable: bool,
        downbeat: usize,
    ) -> Result<Self> {
        let old = self.extended_grid()?;
        ensure!(
            matches!(old.flags, 0x01000002 | 0x02000002) && old.words.iter().all(|v| *v < 1000),
            "Unsupported extended-grid dialect"
        );
        let built = Self::microsecond_grid(beats, fractions, variable, downbeat)?;
        let mut tag =
            self.with_payload(self.header_len(), old.words.len() * 2, &built.raw[56..])?;
        for range in [16..20, 24..52] {
            tag.raw[range.clone()].copy_from_slice(&built.raw[range]);
        }
        Ok(tag)
    }
    /// Recognize the native microsecond layout only with a matching primary
    /// grid. A body of small words alone cannot distinguish PQT2 dialects.
    pub fn native_microsecond_grid(&self, primary: &[Beat]) -> Result<NativeMicrosecondGrid> {
        validate_beats(primary)?;
        let grid = self.extended_grid()?;
        ensure!(
            !primary.is_empty()
                && matches!(grid.flags, 0x01000002 | 0x02000002)
                && grid.downbeat == 0
                && grid.words.len() == primary.len()
                && grid.words.iter().all(|value| *value < 1000)
                && grid.first.as_ref() == primary.first()
                && grid.last.as_ref() == primary.last()
                && grid.checksum == beat_checksum(primary),
            "Unsupported or inconsistent native extended-grid dialect"
        );
        Ok(NativeMicrosecondGrid {
            beats: primary.to_vec(),
            fractions: grid.words,
            variable: grid.flags == 0x02000002,
        })
    }
    /// Update the paired native dialect while retaining its reserved downbeat
    /// word, producer fields, header extensions and trailing opaque bytes.
    pub fn with_native_microsecond_grid(
        &self,
        primary: &[Beat],
        beats: &[Beat],
        fractions: &[u16],
        variable: bool,
    ) -> Result<Self> {
        self.native_microsecond_grid(primary)?;
        ensure!(!beats.is_empty(), "Native beatgrid has no playable beats");
        ensure!(
            beats.len() <= self.limits.max_entries,
            "Grid exceeds entry budget"
        );
        let mut tag = self.with_microsecond_grid(beats, fractions, variable, 0)?;
        put32(&mut tag.raw, 48, 0);
        tag.native_microsecond_grid(beats)?;
        Ok(tag)
    }
    /// Change only the beat count/payload; retain header extensions and trailers.
    pub fn with_beats(&self, beats: &[Beat]) -> Result<Self> {
        self.expect(b"PQTZ", 24)?;
        validate_beats(beats)?;
        ensure!(
            beats.len() <= self.limits.max_entries,
            "Grid exceeds entry budget"
        );
        let old_count = u32be(&self.raw, 20)? as usize;
        let old = entries(&self.raw, self.header_len(), old_count, 8, self.limits)?;
        let mut payload = vec![0; checked_size(0, beats.len(), 8)?];
        for (slot, beat) in payload.as_chunks_mut::<8>().0.iter_mut().zip(beats) {
            write_beat(slot, beat);
        }
        let mut tag = self.with_payload(self.header_len(), old.len(), &payload)?;
        put32(&mut tag.raw, 20, u32::try_from(beats.len())?);
        Ok(tag)
    }
    pub fn with_audio_path(&self, path: &str) -> Result<Self> {
        self.expect(b"PPTH", 16)?;
        let old_size = u32be(&self.raw, 12)? as usize;
        slice(&self.raw, self.header_len(), old_size)?;
        let payload = encode_utf16(path)?;
        ensure!(
            payload.len() <= self.limits.max_string_bytes,
            "Audio path exceeds string budget"
        );
        let mut tag = self.with_payload(self.header_len(), old_size, &payload)?;
        put32(&mut tag.raw, 12, u32::try_from(payload.len())?);
        Ok(tag)
    }
    /// Raw nested records permit retaining unowned/unknown cue extensions while
    /// the typed accessor explicitly reports unsupported record types.
    pub fn cue_records(&self) -> Result<Vec<AnlzTag>> {
        Ok(self.cue_record_range()?.0)
    }
    /// Caller supplies the merged list, retaining records outside its ownership.
    /// Only count/payload/length change; list fields and trailers remain intact.
    pub fn with_cue_records(&self, records: &[AnlzTag]) -> Result<Self> {
        ensure!(
            records.len() <= u16::MAX as usize && records.len() <= self.limits.max_entries,
            "Cue list exceeds entry budget"
        );
        let (_, end) = self.cue_record_range()?;
        let len = records.iter().try_fold(0usize, |n, r| {
            n.checked_add(r.raw.len())
                .context("Cue-list length overflow")
        })?;
        ensure!(len <= self.limits.max_bytes, "Cue list exceeds byte budget");
        let mut payload = Vec::with_capacity(len);
        for record in records {
            payload.extend(&record.raw);
        }
        let mut tag = self.with_payload(self.header_len(), end - self.header_len(), &payload)?;
        let offset = if self.fourcc() == *b"PCOB" { 18 } else { 16 };
        put16(&mut tag.raw, offset, records.len() as u16);
        Ok(tag)
    }
    fn cue_record_range(&self) -> Result<(Vec<AnlzTag>, usize)> {
        let count_at = match &self.fourcc() {
            b"PCOB" => {
                self.expect(b"PCOB", 24)?;
                18
            }
            b"PCO2" => {
                self.expect(b"PCO2", 20)?;
                16
            }
            _ => bail!("Unexpected cue-list type"),
        };
        let count = u16be(&self.raw, count_at)? as usize;
        ensure!(
            count <= self.limits.max_entries,
            "Cue list exceeds entry budget"
        );
        let mut cursor = self.header_len();
        let mut records = Vec::new();
        for _ in 0..count {
            let (_, size) = envelope(&self.raw[cursor..])?;
            records.push(Self::parse(&self.raw[cursor..cursor + size], self.limits)?);
            cursor += size;
        }
        Ok((records, cursor))
    }
    fn with_payload(&self, start: usize, old_len: usize, payload: &[u8]) -> Result<Self> {
        slice(&self.raw, start, old_len)?;
        let len = self
            .raw
            .len()
            .checked_sub(old_len)
            .and_then(|v| v.checked_add(payload.len()))
            .context("ANLZ tag length overflow")?;
        ensure!(len <= self.limits.max_bytes, "ANLZ tag exceeds byte budget");
        let mut raw = Vec::with_capacity(len);
        raw.extend(&self.raw[..start]);
        raw.extend(payload);
        raw.extend(&self.raw[start + old_len..]);
        put32(&mut raw, 8, u32::try_from(len)?);
        Self::parse(&raw, self.limits)
    }
    /// Validate the envelope only; unknown contents remain byte-for-byte intact.
    /// Typed accessors additionally validate their counts/strings/nested records.
    pub fn parse(bytes: &[u8], limits: ParseLimits) -> Result<Self> {
        ensure!(
            bytes.len() <= limits.max_bytes,
            "ANLZ tag exceeds byte budget"
        );
        let (_, len) = envelope(bytes)?;
        ensure!(len == bytes.len(), "ANLZ tag length disagrees with input");
        Ok(Self {
            raw: bytes.to_vec(),
            limits,
        })
    }
    pub fn bytes(&self) -> &[u8] {
        &self.raw
    }
    pub fn fourcc(&self) -> [u8; 4] {
        self.raw[..4].try_into().unwrap()
    }
    pub fn header_len(&self) -> usize {
        u32be(&self.raw, 4).unwrap() as usize
    }
    fn expect(&self, id: &[u8; 4], minimum: usize) -> Result<()> {
        ensure!(&self.raw[..4] == id, "Unexpected ANLZ tag type");
        ensure!(self.header_len() >= minimum, "ANLZ tag header is too short");
        Ok(())
    }
    pub fn beats(&self) -> Result<Vec<Beat>> {
        self.expect(b"PQTZ", 24)?;
        let count = u32be(&self.raw, 20)? as usize;
        let range = entries(&self.raw, self.header_len(), count, 8, self.limits)?;
        range
            .as_chunks::<8>()
            .0
            .iter()
            .map(|bytes| read_beat(bytes))
            .collect()
    }
    pub fn extended_grid(&self) -> Result<ExtendedGrid> {
        self.expect(b"PQT2", 56)?;
        let count = u32be(&self.raw, 40)? as usize;
        let words = entries(&self.raw, self.header_len(), count, 2, self.limits)?;
        Ok(ExtendedGrid {
            first: if count > 0 {
                Some(read_beat(&self.raw[24..32])?)
            } else {
                None
            },
            last: if count > 0 {
                Some(read_beat(&self.raw[32..40])?)
            } else {
                None
            },
            words: words
                .as_chunks::<2>()
                .0
                .iter()
                .map(|b| u16::from_be_bytes([b[0], b[1]]))
                .collect(),
            flags: u32be(&self.raw, 16)?,
            checksum: u32be(&self.raw, 44)?,
            downbeat: u32be(&self.raw, 48)?,
        })
    }
    pub fn cues(&self) -> Result<CueList> {
        let legacy = match &self.fourcc() {
            b"PCOB" => true,
            b"PCO2" => false,
            _ => bail!("Unexpected cue-list type"),
        };
        self.expect(
            if legacy { b"PCOB" } else { b"PCO2" },
            if legacy { 24 } else { 20 },
        )?;
        let count = u16be(&self.raw, if legacy { 18 } else { 16 })? as usize;
        ensure!(
            count <= self.limits.max_entries,
            "Cue list exceeds entry budget"
        );
        let mut cursor = self.header_len();
        let mut decoded = Vec::new();
        for _ in 0..count {
            let remaining = self.raw.get(cursor..).context("Cue entry exceeds tag")?;
            let (header, len) = envelope(remaining)?;
            let b = &remaining[..len];
            let entry = if legacy {
                ensure!(
                    &b[..4] == b"PCPT" && header >= 28 && len >= 40,
                    "Unsupported legacy cue entry"
                );
                CueEntry {
                    hot_cue: u32be(b, 12)?,
                    cue_type: b[28],
                    time_ms: u32be(b, 32)?,
                    loop_time_ms: u32be(b, 36)?,
                    legacy: Some(LegacyDetails {
                        status: u32be(b, 16)?,
                        order_first: u16be(b, 24)?,
                        order_last: u16be(b, 26)?,
                    }),
                    extended: None,
                    raw: b.to_vec(),
                }
            } else {
                ensure!(
                    &b[..4] == b"PCP2" && header == 16 && len >= 28,
                    "Unsupported extended cue entry"
                );
                let comment = if len >= 44 {
                    let size = u32be(b, 40)? as usize;
                    ensure!(
                        size <= self.limits.max_string_bytes,
                        "Cue comment exceeds string budget"
                    );
                    if size == 0 {
                        None
                    } else {
                        Some(utf16(slice(b, 44, size)?)?)
                    }
                } else {
                    None
                };
                let color = if len >= 44 {
                    let at = 44usize
                        .checked_add(u32be(b, 40)? as usize)
                        .context("Cue comment length overflow")?;
                    b.get(at..)
                        .filter(|v| v.len() >= 4)
                        .map(|rgb| rgb[..4].try_into().unwrap())
                } else {
                    None
                };
                CueEntry {
                    hot_cue: u32be(b, 12)?,
                    cue_type: b[16],
                    time_ms: u32be(b, 20)?,
                    loop_time_ms: u32be(b, 24)?,
                    legacy: None,
                    extended: Some(ExtendedDetails {
                        memory_color: b.get(28).copied(),
                        loop_fraction: if len >= 40 {
                            Some((u16be(b, 36)?, u16be(b, 38)?))
                        } else {
                            None
                        },
                        comment,
                        color,
                    }),
                    raw: b.to_vec(),
                }
            };
            decoded.push(entry);
            cursor = cursor
                .checked_add(len)
                .context("Cue-list length overflow")?;
        }
        Ok(CueList {
            kind: u32be(&self.raw, 12)?,
            entries: decoded,
        })
    }
    pub fn audio_path(&self) -> Result<String> {
        self.expect(b"PPTH", 16)?;
        let size = u32be(&self.raw, 12)? as usize;
        ensure!(
            size <= self.limits.max_string_bytes,
            "Audio path exceeds string budget"
        );
        if size == 0 {
            return Ok(String::new());
        }
        utf16(slice(&self.raw, self.header_len(), size)?)
    }
    /// Build a canonical PQTZ block using Lexicon's direct-writer header constant.
    /// Existing unknown bytes are preserved by retaining the original tag instead.
    pub fn grid(beats: &[Beat]) -> Result<Self> {
        validate_beats(beats)?;
        let mut raw = new_tag(b"PQTZ", 24, checked_size(24, beats.len(), 8)?)?;
        put32(&mut raw, 16, 524288);
        put32(&mut raw, 20, u32::try_from(beats.len())?);
        for (slot, beat) in raw[24..].as_chunks_mut::<8>().0.iter_mut().zip(beats) {
            write_beat(slot, beat)
        }
        Self::parse(&raw, ParseLimits::default())
    }
    /// Lexicon's extended-grid dialect, not a converter for arbitrary PQT2 words.
    pub fn microsecond_grid(
        beats: &[Beat],
        fractions: &[u16],
        variable: bool,
        downbeat: usize,
    ) -> Result<Self> {
        validate_beats(beats)?;
        ensure!(
            beats.len() == fractions.len(),
            "Grid fraction count mismatch"
        );
        ensure!(
            fractions.iter().all(|v| *v < 1000),
            "Grid fraction outside microsecond range"
        );
        ensure!(
            beats.is_empty() || downbeat < beats.len(),
            "Downbeat index exceeds beat count"
        );
        let mut raw = new_tag(b"PQT2", 56, checked_size(56, beats.len(), 2)?)?;
        put32(&mut raw, 16, if variable { 0x02000002 } else { 0x01000002 });
        if let (Some(first), Some(last)) = (beats.first(), beats.last()) {
            write_beat(&mut raw[24..32], first);
            write_beat(&mut raw[32..40], last);
        }
        put32(&mut raw, 40, u32::try_from(beats.len())?);
        put32(&mut raw, 44, beat_checksum(beats));
        put32(
            &mut raw,
            48,
            if variable || beats.is_empty() {
                0
            } else {
                u32::try_from(downbeat.checked_add(1).context("Downbeat overflow")?)?
            },
        );
        for (slot, value) in raw[56..].as_chunks_mut::<2>().0.iter_mut().zip(fractions) {
            slot.copy_from_slice(&value.to_be_bytes())
        }
        Self::parse(&raw, ParseLimits::default())
    }
    pub fn path(path: &str) -> Result<Self> {
        let text = encode_utf16(path)?;
        let mut raw = new_tag(b"PPTH", 16, 16 + text.len())?;
        put32(&mut raw, 12, u32::try_from(text.len())?);
        raw[16..].copy_from_slice(&text);
        Self::parse(&raw, ParseLimits::default())
    }
    pub fn legacy_cues(kind: u32, cues: &[LegacyCue]) -> Result<Self> {
        ensure!(cues.len() <= u16::MAX as usize, "Too many native cues");
        let mut raw = new_tag(b"PCOB", 24, checked_size(24, cues.len(), 56)?)?;
        put32(&mut raw, 12, kind);
        put16(&mut raw, 18, cues.len() as u16);
        put32(&mut raw, 20, u32::MAX);
        for (slot, cue) in raw[24..].as_chunks_mut::<56>().0.iter_mut().zip(cues) {
            validate_cue(
                kind,
                cue.hot_cue,
                cue.cue_type,
                cue.time_ms,
                cue.loop_time_ms,
            )?;
            slot[..4].copy_from_slice(b"PCPT");
            put32(slot, 4, 28);
            put32(slot, 8, 56);
            put32(slot, 12, cue.hot_cue);
            put32(slot, 16, cue.status);
            put32(slot, 20, 65536);
            put16(slot, 24, cue.order_first);
            put16(slot, 26, cue.order_last);
            slot[28..32].copy_from_slice(&[cue.cue_type, 0, 3, 232]);
            put32(slot, 32, cue.time_ms);
            put32(slot, 36, cue.loop_time_ms);
        }
        ensure!(kind <= 1, "Unsupported cue-list kind");
        Self::parse(&raw, ParseLimits::default())
    }
    pub fn extended_cues(kind: u32, cues: &[ExtendedCue]) -> Result<Self> {
        ensure!(
            kind <= 1 && cues.len() <= u16::MAX as usize,
            "Unsupported cue-list kind/count"
        );
        let mut raw = new_tag(b"PCO2", 20, 20)?;
        put32(&mut raw, 12, kind);
        put16(&mut raw, 16, cues.len() as u16);
        for cue in cues {
            validate_cue(
                kind,
                cue.hot_cue,
                cue.cue_type,
                cue.time_ms,
                cue.loop_time_ms,
            )?;
            let (n, d) = cue.loop_fraction;
            ensure!((n == 0) == (d == 0), "Invalid native loop fraction");
            let text = if cue.comment.is_empty() {
                Vec::new()
            } else {
                encode_utf16(&cue.comment)?
            };
            let mut entry = new_tag(b"PCP2", 16, 88 + text.len())?;
            put32(&mut entry, 12, cue.hot_cue);
            entry[16..20].copy_from_slice(&[cue.cue_type, 0, 3, 232]);
            put32(&mut entry, 20, cue.time_ms);
            put32(&mut entry, 24, cue.loop_time_ms);
            entry[28] = cue.memory_color;
            entry[29] = 1;
            put16(&mut entry, 36, n);
            put16(&mut entry, 38, d);
            put32(&mut entry, 40, u32::try_from(text.len())?);
            entry[44..44 + text.len()].copy_from_slice(&text);
            entry[44 + text.len()..48 + text.len()].copy_from_slice(&cue.color);
            ensure!(
                raw.len()
                    .checked_add(entry.len())
                    .is_some_and(|v| v <= ParseLimits::default().max_bytes),
                "Cue list exceeds byte budget"
            );
            raw.extend(entry);
        }
        let len = u32::try_from(raw.len())?;
        put32(&mut raw, 8, len);
        Self::parse(&raw, ParseLimits::default())
    }
}
#[derive(Clone, Debug)]
pub struct AnlzFile {
    header: Vec<u8>,
    tags: Vec<AnlzTag>,
    limits: ParseLimits,
}
impl AnlzFile {
    /// Construct a minimal analysis container from authored known tags. Waveform
    /// or spectral sections must come from measured PCM, never placeholders.
    pub fn new(tags: Vec<AnlzTag>) -> Result<Self> {
        let mut header = new_tag(b"PMAI", 28, 28)?;
        put32(&mut header, 12, 1);
        put32(&mut header, 16, 65536);
        put32(&mut header, 20, 65536);
        let file = Self {
            header,
            tags,
            limits: ParseLimits::default(),
        };
        Self::parse(&file.to_bytes()?, ParseLimits::default())
    }
    pub fn parse(bytes: &[u8], limits: ParseLimits) -> Result<Self> {
        ensure!(
            bytes.len() <= limits.max_bytes,
            "ANLZ file exceeds byte budget"
        );
        let (header, len) = envelope(bytes)?;
        ensure!(&bytes[..4] == b"PMAI", "Invalid ANLZ file signature");
        ensure!(len == bytes.len(), "ANLZ file length disagrees with input");
        let mut tags = Vec::new();
        let mut at = header;
        while at < len {
            ensure!(tags.len() < limits.max_tags, "ANLZ file exceeds tag budget");
            let (_, size) = envelope(&bytes[at..])?;
            tags.push(AnlzTag::parse(&bytes[at..at + size], limits)?);
            at += size;
        }
        Ok(Self {
            header: bytes[..header].to_vec(),
            tags,
            limits,
        })
    }
    pub fn tags(&self) -> &[AnlzTag] {
        &self.tags
    }
    /// Append an explicitly authored section without moving or removing any
    /// existing section. Preserve the file's original header and parse budgets.
    pub fn append(&mut self, tag: AnlzTag) -> Result<()> {
        ensure!(
            self.tags.len() < self.limits.max_tags,
            "ANLZ file exceeds tag budget"
        );
        let length = self
            .encoded_len()?
            .checked_add(tag.raw.len())
            .context("ANLZ file length overflow")?;
        ensure!(
            length <= self.limits.max_bytes,
            "ANLZ appended section exceeds byte budget"
        );
        self.tags.push(AnlzTag::parse(&tag.raw, self.limits)?);
        Ok(())
    }
    /// Replace the selected tag, preserving ordering and every other byte. A
    /// format or memory/hot-list mismatch is rejected rather than changing scope.
    pub fn replace(&mut self, index: usize, replacement: AnlzTag) -> Result<()> {
        let current = self
            .tags
            .get(index)
            .context("ANLZ tag index is unavailable")?;
        ensure!(
            current.fourcc() == replacement.fourcc(),
            "ANLZ replacement type mismatch"
        );
        if matches!(&current.fourcc(), b"PCOB" | b"PCO2") {
            ensure!(
                u32be(current.bytes(), 12)? == u32be(replacement.bytes(), 12)?,
                "Cue-list replacement scope mismatch"
            );
        }
        let total = self
            .encoded_len()?
            .checked_sub(current.raw.len())
            .and_then(|v| v.checked_add(replacement.raw.len()))
            .context("ANLZ file length overflow")?;
        ensure!(
            total <= self.limits.max_bytes,
            "ANLZ replacement exceeds byte budget"
        );
        self.tags[index] = AnlzTag::parse(&replacement.raw, self.limits)?;
        Ok(())
    }
    fn encoded_len(&self) -> Result<usize> {
        self.tags.iter().try_fold(self.header.len(), |n, t| {
            n.checked_add(t.raw.len())
                .context("ANLZ file length overflow")
        })
    }
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let len = self.encoded_len()?;
        ensure!(
            len <= self.limits.max_bytes,
            "ANLZ output exceeds byte budget"
        );
        let mut raw = Vec::with_capacity(len);
        raw.extend(&self.header);
        put32(&mut raw, 8, u32::try_from(len)?);
        for tag in &self.tags {
            raw.extend(&tag.raw)
        }
        Ok(raw)
    }
}
fn envelope(bytes: &[u8]) -> Result<(usize, usize)> {
    ensure!(bytes.len() >= 12, "Truncated ANLZ envelope");
    let header = u32be(bytes, 4)? as usize;
    let len = u32be(bytes, 8)? as usize;
    ensure!(
        header >= 12 && header <= len && len <= bytes.len(),
        "Invalid ANLZ envelope lengths"
    );
    Ok((header, len))
}
fn slice(bytes: &[u8], at: usize, len: usize) -> Result<&[u8]> {
    let end = at.checked_add(len).context("ANLZ field length overflow")?;
    bytes.get(at..end).context("Truncated ANLZ field")
}
fn u32be(b: &[u8], at: usize) -> Result<u32> {
    Ok(u32::from_be_bytes(slice(b, at, 4)?.try_into()?))
}
fn u16be(b: &[u8], at: usize) -> Result<u16> {
    Ok(u16::from_be_bytes(slice(b, at, 2)?.try_into()?))
}
fn put32(b: &mut [u8], at: usize, value: u32) {
    b[at..at + 4].copy_from_slice(&value.to_be_bytes())
}
fn put16(b: &mut [u8], at: usize, value: u16) {
    b[at..at + 2].copy_from_slice(&value.to_be_bytes())
}
fn entries(b: &[u8], at: usize, count: usize, size: usize, limits: ParseLimits) -> Result<&[u8]> {
    ensure!(
        count <= limits.max_entries,
        "ANLZ entry count exceeds budget"
    );
    slice(
        b,
        at,
        count
            .checked_mul(size)
            .context("ANLZ entry size overflow")?,
    )
}
fn read_beat(b: &[u8]) -> Result<Beat> {
    Ok(Beat {
        number: u16be(b, 0)?,
        tempo: u16be(b, 2)?,
        time_ms: u32be(b, 4)?,
    })
}
fn write_beat(b: &mut [u8], beat: &Beat) {
    put16(b, 0, beat.number);
    put16(b, 2, beat.tempo);
    put32(b, 4, beat.time_ms)
}
fn checked_size(base: usize, count: usize, size: usize) -> Result<usize> {
    base.checked_add(
        count
            .checked_mul(size)
            .context("ANLZ payload size overflow")?,
    )
    .context("ANLZ total size overflow")
}
fn new_tag(id: &[u8; 4], header: usize, len: usize) -> Result<Vec<u8>> {
    ensure!(
        len >= header && header >= 12 && len <= ParseLimits::default().max_bytes,
        "Invalid ANLZ output size"
    );
    let mut b = vec![0; len];
    b[..4].copy_from_slice(id);
    put32(&mut b, 4, u32::try_from(header)?);
    put32(&mut b, 8, u32::try_from(len)?);
    Ok(b)
}
fn validate_beats(beats: &[Beat]) -> Result<()> {
    ensure!(
        beats.len() <= ParseLimits::default().max_entries,
        "Grid exceeds entry budget"
    );
    ensure!(
        beats
            .iter()
            .all(|b| (1..=4).contains(&b.number) && b.tempo > 0),
        "Invalid native beat/tempo"
    );
    ensure!(
        beats.windows(2).all(|b| b[0].time_ms < b[1].time_ms),
        "Native beats must increase in time"
    );
    Ok(())
}
fn validate_cue(kind: u32, hot: u32, ty: u8, start: u32, end: u32) -> Result<()> {
    ensure!(
        kind <= 1 && ((kind == 0 && hot == 0) || (kind == 1 && hot > 0)),
        "Cue does not match list scope"
    );
    ensure!(ty == 1 || ty == 2, "Unsupported native cue type");
    ensure!(ty != 2 || end > start, "Loop must end after cue start");
    Ok(())
}
fn utf16(bytes: &[u8]) -> Result<String> {
    ensure!(
        bytes.len() >= 2 && bytes.len().is_multiple_of(2),
        "Invalid UTF-16 field length"
    );
    let words: Vec<_> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| u16::from_be_bytes([b[0], b[1]]))
        .collect();
    ensure!(words.last() == Some(&0), "UTF-16 field lacks terminator");
    let end = words.iter().rposition(|v| *v != 0).map_or(0, |i| i + 1);
    ensure!(
        !words[..end].contains(&0),
        "UTF-16 field contains interior NUL"
    );
    String::from_utf16(&words[..end]).context("Invalid UTF-16 field")
}
fn encode_utf16(value: &str) -> Result<Vec<u8>> {
    ensure!(!value.contains('\0'), "Text contains interior NUL");
    let count = value.encode_utf16().count();
    let len = checked_size(2, count, 2)?;
    ensure!(
        len <= ParseLimits::default().max_string_bytes,
        "Text exceeds string budget"
    );
    Ok(value
        .encode_utf16()
        .chain([0])
        .flat_map(u16::to_be_bytes)
        .collect())
}

fn beat_checksum(beats: &[Beat]) -> u32 {
    beats.iter().fold(0u32, |sum, beat| {
        sum.wrapping_add(u32::from(beat.number))
            .wrapping_add(u32::from(beat.tempo))
            .wrapping_add(beat.time_ms)
    })
}
