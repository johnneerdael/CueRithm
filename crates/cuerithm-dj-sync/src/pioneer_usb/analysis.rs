use super::TrackRecord;
use crate::{SyncOptions, SyncTrack, UsbCueDestination};
use anyhow::{Context, Result, ensure};
use cuerithm_rekordbox::anlz::{AnlzTag, Beat, ExtendedCue, LegacyCue};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) fn write(
    root: &Path,
    record: &mut TrackRecord,
    options: &SyncOptions,
    files: &mut Vec<PathBuf>,
    runtime: Option<&cuerithm_media::MediaRuntime>,
) -> Result<()> {
    let track = &record.track;
    let mut wave = if let Some(runtime) = runtime {
        let cache = tempfile::tempdir()?;
        let audio = runtime.decode(&track.path, cache.path())?;
        waveforms(
            &audio.mono_samples()?,
            audio.clock.sample_rate,
            track,
            options,
        )?
    } else {
        ensure!(
            !track
                .path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("mp3")),
            "MP3 device waveforms require the bundled playable-PCM decoder"
        );
        let audio = camelot_core::audio::decode(&track.path, 22050, None)
            .context("Decode complete audio for Pioneer USB waveform")?;
        ensure!(
            audio.complete && !audio.samples.is_empty(),
            "Incomplete USB waveform decode"
        );
        waveforms(&audio.samples, audio.sample_rate, track, options)?
    };
    let offset = track
        .native_timeline
        .as_ref()
        .map_or(0., |timing| timing.offset_seconds);
    wave.detail = pad_detail_waveform(&wave.detail, offset)?;
    wave.rgb_detail = pad_detail_waveform(&wave.rgb_detail, offset)?;
    wave.three_detail = pad_detail_waveform(&wave.three_detail, offset)?;
    let (beats, fractions, downbeat) = grid(track)?;
    ensure!(!beats.is_empty(), "USB beatgrid has no playable beats");
    let (hot, memory) = cues(track, options)?;
    let mut vbr = vec![0; 1604];
    let samples = track
        .export_timing
        .as_ref()
        .and_then(|profile| profile.decoded_sample_count)
        .and_then(|count| u32::try_from(count).ok())
        .filter(|count| *count > 0)
        .unwrap_or(0);
    vbr[1600..].copy_from_slice(&samples.to_be_bytes());
    let mut dat = vec![
        AnlzTag::path(&record.path)?.bytes().to_vec(),
        tag(b"PVBR", 16, &[0; 4], &vbr)?,
        AnlzTag::grid(&beats)?.bytes().to_vec(),
        wave.preview,
        wave.tiny,
    ];
    let legacy_hot: Vec<_> = hot.iter().filter(|c| c.hot_cue <= 3).map(legacy).collect();
    let legacy_extra: Vec<_> = hot.iter().filter(|c| c.hot_cue > 3).map(legacy).collect();
    dat.push(AnlzTag::legacy_cues(1, &legacy_hot)?.bytes().to_vec());
    let memory_legacy: Vec<_> = memory
        .iter()
        .enumerate()
        .map(|(n, c)| LegacyCue {
            hot_cue: 0,
            cue_type: c.cue_type,
            time_ms: c.time_ms,
            loop_time_ms: c.loop_time_ms,
            // Activation belongs to memory PCPT only. Match the serialized
            // timeline because memory entries are sorted and deduplicated.
            status: if c.cue_type == 2
                && track.cues.iter().any(|cue| {
                    cue.active_loop
                        && cue.end_seconds.is_some_and(|end| {
                            (cue.start_seconds * 1000. + 1e-6).floor() as u32 == c.time_ms
                                && (end * 1000. + 1e-6).floor() as u32 == c.loop_time_ms
                        })
                }) {
                4
            } else {
                0
            },
            order_first: if n == 0 { u16::MAX } else { n as u16 - 1 },
            order_last: if n + 1 == memory.len() {
                u16::MAX
            } else {
                n as u16 + 1
            },
        })
        .collect();
    let mut memory_tag = AnlzTag::legacy_cues(0, &memory_legacy)?.bytes().to_vec();
    if !memory.is_empty() {
        memory_tag[20..24].copy_from_slice(&((memory.len() - 1) as u32).to_be_bytes());
    }
    dat.push(memory_tag);
    let ext = vec![
        AnlzTag::path(&record.path)?.bytes().to_vec(),
        wave.detail,
        AnlzTag::legacy_cues(1, &legacy_extra)?.bytes().to_vec(),
        AnlzTag::legacy_cues(0, &[])?.bytes().to_vec(),
        AnlzTag::extended_cues(1, &hot)?.bytes().to_vec(),
        AnlzTag::extended_cues(0, &memory)?.bytes().to_vec(),
        AnlzTag::microsecond_grid(&beats, &fractions, track.beatgrid.len() > 1, downbeat)?
            .bytes()
            .to_vec(),
        wave.rgb_detail,
        wave.rgb_preview,
    ];
    let modern = vec![
        AnlzTag::path(&record.path)?.bytes().to_vec(),
        wave.three_detail,
        wave.three_preview,
        tag(b"PWVC", 14, &[0, 0], &[0, 134, 0, 177, 0, 156])?,
    ];
    let base = record
        .analysis_path
        .strip_suffix(".DAT")
        .context("USB analysis path lacks .DAT")?;
    for (suffix, tags) in [("DAT", dat), ("EXT", ext), ("2EX", modern)] {
        let path = PathBuf::from(format!("{base}.{suffix}").trim_start_matches('/'));
        fs::create_dir_all(root.join(&path).parent().unwrap())?;
        fs::write(root.join(&path), file(&tags)?)?;
        files.push(path);
    }
    Ok(())
}
fn legacy(c: &ExtendedCue) -> LegacyCue {
    LegacyCue {
        hot_cue: c.hot_cue,
        cue_type: c.cue_type,
        time_ms: c.time_ms,
        loop_time_ms: c.loop_time_ms,
        status: 0,
        order_first: u16::MAX,
        order_last: u16::MAX,
    }
}
fn cues(track: &SyncTrack, options: &SyncOptions) -> Result<(Vec<ExtendedCue>, Vec<ExtendedCue>)> {
    let mut hot = Vec::new();
    let mut memory = Vec::new();
    let mut slots = std::collections::HashSet::new();
    for cue in &track.cues {
        ensure!(
            cue.start_seconds.is_finite()
                && cue.start_seconds >= 0.0
                && cue.start_seconds * 1000.0 <= u32::MAX as f64,
            "Invalid USB cue time"
        );
        if let Some(end) = cue.end_seconds {
            ensure!(
                end.is_finite() && end > cue.start_seconds && end * 1000.0 <= u32::MAX as f64,
                "Invalid USB loop end"
            );
        }
        let color = cue.color.unwrap_or(0);
        let rgb = [(color >> 16) as u8, (color >> 8) as u8, color as u8];
        let entry = ExtendedCue {
            hot_cue: cue.index as u32 + 1,
            cue_type: if cue.end_seconds.is_some() { 2 } else { 1 },
            time_ms: (cue.start_seconds * 1000.0 + 1e-6).floor() as u32,
            loop_time_ms: cue
                .end_seconds
                .map(|s| (s * 1000.0 + 1e-6).floor() as u32)
                .unwrap_or(u32::MAX),
            memory_color: super::color_id(cue.color, options.nearest_color),
            loop_fraction: if cue.end_seconds.is_some() {
                (4, 1)
            } else {
                (0, 0)
            },
            comment: cue.name.clone(),
            color: [0, rgb[0], rgb[1], rgb[2]],
        };
        if matches!(
            options.usb_cue_destination,
            UsbCueDestination::Hot | UsbCueDestination::Both
        ) {
            ensure!(
                cue.index < 8 && slots.insert(cue.index),
                "USB supports eight distinct hotcue slots"
            );
            hot.push(entry.clone());
        }
        if matches!(
            options.usb_cue_destination,
            UsbCueDestination::Memory | UsbCueDestination::Both
        ) {
            memory.push(ExtendedCue {
                hot_cue: 0,
                ..entry
            });
        }
    }
    hot.sort_by_key(|c| std::cmp::Reverse(c.hot_cue));
    memory.sort_by_key(|c| std::cmp::Reverse(c.time_ms));
    memory.dedup_by_key(|c| (c.time_ms, c.loop_time_ms, c.cue_type));
    Ok((hot, memory))
}
pub(crate) fn grid(track: &SyncTrack) -> Result<(Vec<Beat>, Vec<u16>, usize)> {
    let mut beats = Vec::new();
    let mut fractions = Vec::new();
    let mut downbeat = None;
    let markers = &track.beatgrid;
    ensure!(!markers.is_empty(), "Beatgrid has no tempo markers");
    for (n, marker) in markers.iter().enumerate() {
        ensure!(
            marker.seconds.is_finite()
                && marker.seconds >= 0.0
                && marker.bpm.is_finite()
                && marker.bpm > 0.0,
            "Invalid USB tempo marker"
        );
        ensure!(
            n == 0 || marker.seconds > markers[n - 1].seconds,
            "USB tempo markers must be ordered"
        );
    }
    let first = &markers[0];
    let period = 60.0 / first.bpm;
    let before = (first.seconds / period).floor() as usize;
    ensure!(before <= 200_000, "USB grid preroll exceeds entry limit");
    let first_number = if (1..=4).contains(&first.beat) {
        first.beat
    } else {
        1
    };
    for n in (1..=before).rev() {
        let seconds = first.seconds - n as f64 * period;
        let number = (first_number as i64 - 1 - n as i64).rem_euclid(4) as u16 + 1;
        push_beat(&mut beats, &mut fractions, seconds, first.bpm, number)?;
    }
    for (n, marker) in markers.iter().enumerate() {
        let period = 60.0 / marker.bpm;
        let end = markers.get(n + 1).map_or(track.duration_seconds, |m| {
            (m.seconds - period * 0.1).min(track.duration_seconds)
        });
        let mut seconds = marker.seconds;
        let mut number = if (1..=4).contains(&marker.beat) {
            marker.beat as u16
        } else {
            1
        };
        while seconds < end {
            ensure!(beats.len() < 200_000, "USB grid exceeds entry limit");
            if downbeat.is_none() && number == 1 {
                downbeat = Some(beats.len());
            }
            push_beat(&mut beats, &mut fractions, seconds, marker.bpm, number)?;
            seconds += period;
            number = number % 4 + 1;
        }
    }
    Ok((beats, fractions, downbeat.unwrap_or(0)))
}
fn push_beat(
    beats: &mut Vec<Beat>,
    fractions: &mut Vec<u16>,
    seconds: f64,
    bpm: f64,
    number: u16,
) -> Result<()> {
    let ms = seconds * 1000.0;
    ensure!(ms <= u32::MAX as f64, "USB beat time overflow");
    let floor = (ms + 1e-6).floor();
    beats.push(Beat {
        number,
        tempo: (bpm * 100.0).round().clamp(1.0, u16::MAX as f64) as u16,
        time_ms: floor as u32,
    });
    fractions.push(((ms - floor) * 1000.0).round().clamp(0.0, 999.0) as u16);
    Ok(())
}
pub(crate) fn tag(kind: &[u8; 4], header: usize, extra: &[u8], payload: &[u8]) -> Result<Vec<u8>> {
    ensure!(
        header >= 12 && extra.len() == header - 12,
        "Invalid generated ANLZ header"
    );
    let size = header
        .checked_add(payload.len())
        .context("ANLZ size overflow")?;
    let mut bytes = Vec::with_capacity(size);
    bytes.extend(kind);
    bytes.extend((header as u32).to_be_bytes());
    bytes.extend(u32::try_from(size)?.to_be_bytes());
    bytes.extend(extra);
    bytes.extend(payload);
    Ok(bytes)
}
pub(crate) fn file(tags: &[Vec<u8>]) -> Result<Vec<u8>> {
    let size = 28 + tags.iter().map(Vec::len).sum::<usize>();
    let mut bytes = b"PMAI".to_vec();
    bytes.extend(28_u32.to_be_bytes());
    bytes.extend(u32::try_from(size)?.to_be_bytes());
    bytes.extend([0, 0, 0, 1, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0]);
    for tag in tags {
        bytes.extend(tag);
    }
    Ok(bytes)
}
pub(crate) struct Waves {
    pub(crate) preview: Vec<u8>,
    pub(crate) tiny: Vec<u8>,
    pub(crate) detail: Vec<u8>,
    pub(crate) rgb_preview: Vec<u8>,
    pub(crate) rgb_detail: Vec<u8>,
    pub(crate) three_preview: Vec<u8>,
    pub(crate) three_detail: Vec<u8>,
}
struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
    x: [f64; 2],
    y: [f64; 2],
}
impl Biquad {
    fn new(rate: u32, hz: f64, high: bool) -> Self {
        let omega = std::f64::consts::TAU * hz / f64::from(rate);
        let c = omega.cos();
        let alpha = omega.sin() / std::f64::consts::SQRT_2;
        let a0 = 1.0 + alpha;
        let b = if high {
            [(1.0 + c) / 2.0, -(1.0 + c), (1.0 + c) / 2.0]
        } else {
            [(1.0 - c) / 2.0, 1.0 - c, (1.0 - c) / 2.0]
        };
        Self {
            b: b.map(|b| b / a0),
            a: [-2.0 * c / a0, (1.0 - alpha) / a0],
            x: [0.0; 2],
            y: [0.0; 2],
        }
    }
    fn sample(&mut self, x: f64) -> f64 {
        let y = self.b[0] * x + self.b[1] * self.x[0] + self.b[2] * self.x[1]
            - self.a[0] * self.y[0]
            - self.a[1] * self.y[1];
        self.x = [x, self.x[0]];
        self.y = [y, self.y[0]];
        y
    }
}
fn reduced(data: &[f64], size: usize, peak: bool) -> Vec<f64> {
    (0..size)
        .map(|n| {
            let start = n * data.len() / size;
            let end = ((n + 1) * data.len() / size).max(start + 1).min(data.len());
            if start >= data.len() {
                return 0.0;
            }
            let slice = &data[start..end];
            if peak {
                slice.iter().copied().fold(0.0, f64::max)
            } else {
                slice.iter().sum::<f64>() / slice.len() as f64
            }
        })
        .collect()
}
fn waveforms(
    samples: &[f32],
    rate: u32,
    track: &SyncTrack,
    options: &SyncOptions,
) -> Result<Waves> {
    waveforms_cancellable(samples, rate, track, options, &Default::default())
}
pub(crate) fn waveforms_cancellable(
    samples: &[f32],
    rate: u32,
    track: &SyncTrack,
    options: &SyncOptions,
    cancel: &cuerithm_media::CancellationToken,
) -> Result<Waves> {
    cancel.check()?;
    ensure!(rate >= 150, "Waveform sample rate is too small");
    let frame = (f64::from(rate) / 150.0).round() as usize;
    let count = samples.len() / frame;
    ensure!(count > 0, "Audio too short to generate USB waveform");
    let mut rms = Vec::with_capacity(count);
    let mut peaks = Vec::with_capacity(count);
    let mut low = Vec::with_capacity(count);
    let mut mid = Vec::with_capacity(count);
    let mut high = Vec::with_capacity(count);
    let mut lp = Biquad::new(rate, 100.0, false);
    let mut hp = Biquad::new(rate, 300.0, true);
    let mut mp = Biquad::new(rate, 3000.0, false);
    let mut hi = Biquad::new(rate, 1500.0, true);
    for chunk in samples.chunks_exact(frame) {
        cancel.check()?;
        let mut energy = 0.0;
        let mut p = 0_f64;
        let mut l = 0_f64;
        let mut m = 0_f64;
        let mut h = 0_f64;
        for sample in chunk {
            let s = f64::from(*sample);
            ensure!(s.is_finite(), "Non-finite PCM in USB waveform");
            energy += s * s;
            p = p.max(s.abs());
            l = l.max(lp.sample(s).abs());
            m = m.max(mp.sample(hp.sample(s)).abs());
            h = h.max(hi.sample(s).abs());
        }
        rms.push((energy / frame as f64).sqrt());
        peaks.push(p);
        low.push(l);
        mid.push(m);
        high.push(h);
    }
    let scale = rms.iter().copied().fold(1e-12, f64::max);
    let heights = |data: &[f64], minimum: f64, maximum: f64| {
        data.iter()
            .map(|v| (v / scale * 33.0 - 2.0).round().clamp(minimum, maximum) as u8)
            .collect::<Vec<_>>()
    };
    let white = |peak: f64| (-13.0 * peak + 10.0).round().clamp(0.0, 7.0) as u8;
    let preview_rms = reduced(&rms, 400, false);
    let preview_peaks = reduced(&peaks, 400, true);
    let preview_bytes: Vec<_> = heights(&preview_rms, 2.0, 31.0)
        .into_iter()
        .zip(preview_peaks)
        .map(|(h, p)| h | (white(p) << 5))
        .collect();
    let preview = tag(
        b"PWAV",
        20,
        &[400_u32.to_be_bytes(), 65536_u32.to_be_bytes()].concat(),
        &preview_bytes,
    )?;
    let tiny_bytes: Vec<_> = reduced(&rms, 100, false)
        .iter()
        .map(|v| (v / scale * 15.0 - 2.0).round().clamp(2.0, 15.0) as u8)
        .collect();
    let tiny = tag(
        b"PWV2",
        20,
        &[100_u32.to_be_bytes(), 65536_u32.to_be_bytes()].concat(),
        &tiny_bytes,
    )?;
    let detail_bytes: Vec<_> = heights(&rms, 0.0, 31.0)
        .into_iter()
        .zip(&peaks)
        .map(|(h, p)| h | (white(*p) << 5))
        .collect();
    let detail = wave_tag(b"PWV3", 1, count, 9830400, &detail_bytes)?;
    let mut rgb_detail = Vec::with_capacity(count * 2);
    for n in 0..count {
        let max = low[n].max(mid[n]).max(high[n]).max(1e-12);
        let rgb = [low[n], mid[n], high[n]].map(|b| (b / max * 7.0).round().clamp(0.0, 7.0) as u16);
        let h = (rms[n] / scale * 33.0 - 2.0).round().clamp(0.0, 31.0) as u16;
        let word = (rgb[0] << 13) | (rgb[1] << 10) | (rgb[2] << 7) | (h << 2);
        rgb_detail.extend(word.to_be_bytes());
    }
    let rgb_detail = wave_tag(b"PWV5", 2, count, 9831173, &rgb_detail)?;
    let pr = reduced(&rms, 1200, false);
    let pl = reduced(&low, 1200, true);
    let pm = reduced(&mid, 1200, true);
    let ph = reduced(&high, 1200, true);
    let mut rgb = vec![0; 7200];
    for n in 0..1200 {
        let level = pr[n] / scale;
        if level < 0.01 {
            continue;
        }
        let base = n * 6;
        let a = (121.0 * level).round().min(121.0) as u8;
        rgb[base] = a;
        rgb[base + 1] = (256_u16 - u16::from(a)) as u8 | 128;
        rgb[base + 2] = (106.0 * level).round().min(106.0) as u8;
        let total = (pl[n] + pm[n] + ph[n]).max(1e-12);
        rgb[base + 3] = (pl[n] / total * level * 101.0).round().min(101.0) as u8;
        rgb[base + 4] = (pm[n] / total * level * 71.0).round().min(71.0) as u8;
        rgb[base + 5] = (ph[n] / total * level * 62.0).round().min(62.0) as u8;
    }
    if options.extra_cue_markers {
        for cue in &track.cues {
            let canonical_time = cue.start_seconds
                - track
                    .native_timeline
                    .as_ref()
                    .map_or(0., |timing| timing.offset_seconds);
            let center = ((canonical_time / track.duration_seconds * 1199.0).round() as usize)
                .clamp(6, 1193);
            let c = cue.color.unwrap_or(0xffffff);
            let rgb_c = [(c >> 16) as u8, (c >> 8) as u8, c as u8];
            let max = f64::from(*rgb_c.iter().max().unwrap()).max(1.0);
            let colors = rgb_c.map(|v| {
                let n = f64::from(v) / max;
                if n < 0.35 {
                    0
                } else {
                    (n * 127.0).round() as u8
                }
            });
            for n in center - 6..=center + 6 {
                rgb[n * 6..n * 6 + 6]
                    .copy_from_slice(&[121, 135, 106, colors[0], colors[1], colors[2]]);
            }
        }
    }
    let rgb_preview = wave_tag(b"PWV4", 6, 1200, 0, &rgb)?;
    let bands = |l: &[f64], m: &[f64], h: &[f64], maxima: [f64; 3]| {
        let scales = [l, m, h].map(|v| v.iter().copied().fold(1e-12, f64::max));
        (0..l.len())
            .flat_map(|n| {
                [
                    (m[n] / scales[1] * maxima[1]).round().min(255.0) as u8,
                    (h[n] / scales[2] * maxima[2]).round().min(255.0) as u8,
                    ((l[n] / scales[0]).powi(2) * maxima[0]).round().min(255.0) as u8,
                ]
            })
            .collect::<Vec<_>>()
    };
    let three_preview = three_tag(
        b"PWV6",
        1200,
        None,
        &bands(&pl, &pm, &ph, [51.0, 47.0, 40.0]),
    )?;
    let three_detail = three_tag(
        b"PWV7",
        count,
        Some(9830400),
        &bands(&low, &mid, &high, [180.0, 90.0, 108.0]),
    )?;
    Ok(Waves {
        preview,
        tiny,
        detail,
        rgb_detail,
        rgb_preview,
        three_detail,
        three_preview,
    })
}
/// Fresh canonical detail arrays use 150 Hz bins. Existing assets copied from a
/// device bypass generation entirely and therefore never receive padding twice.
pub(crate) fn pad_detail_waveform(bytes: &[u8], offset: f64) -> Result<Vec<u8>> {
    ensure!(offset.is_finite(), "Invalid waveform timing offset");
    if offset <= 0. || !matches!(bytes.get(..4), Some(b"PWV3" | b"PWV5" | b"PWV7")) {
        return Ok(bytes.to_vec());
    }
    let frames = (150. * offset).round() as usize;
    if frames == 0 {
        return Ok(bytes.to_vec());
    }
    ensure!(
        frames <= 9000 && bytes.len() >= 20,
        "Waveform timing padding exceeds limits"
    );
    let header = u32::from_be_bytes(bytes[4..8].try_into()?) as usize;
    let length = u32::from_be_bytes(bytes[8..12].try_into()?) as usize;
    let stride = u32::from_be_bytes(bytes[12..16].try_into()?) as usize;
    let count = u32::from_be_bytes(bytes[16..20].try_into()?) as usize;
    ensure!(
        length == bytes.len()
            && header >= 20
            && header <= length
            && (1..=16).contains(&stride)
            && count > 0
            && count.checked_mul(stride) == Some(length - header),
        "Invalid detailed waveform payload"
    );
    let added = frames
        .checked_mul(stride)
        .context("Waveform padding overflow")?;
    let mut output = Vec::with_capacity(length + added);
    output.extend(&bytes[..header]);
    for _ in 0..frames {
        output.extend(&bytes[header..header + stride]);
    }
    output.extend(&bytes[header..]);
    let size = u32::try_from(output.len())?;
    output[8..12].copy_from_slice(&size.to_be_bytes());
    output[16..20].copy_from_slice(&u32::try_from(count + frames)?.to_be_bytes());
    Ok(output)
}

fn wave_tag(
    kind: &[u8; 4],
    size: u32,
    count: usize,
    unknown: u32,
    payload: &[u8],
) -> Result<Vec<u8>> {
    tag(
        kind,
        24,
        &[
            size.to_be_bytes(),
            u32::try_from(count)?.to_be_bytes(),
            unknown.to_be_bytes(),
        ]
        .concat(),
        payload,
    )
}
fn three_tag(
    kind: &[u8; 4],
    count: usize,
    unknown: Option<u32>,
    payload: &[u8],
) -> Result<Vec<u8>> {
    let mut extra = [3_u32.to_be_bytes(), u32::try_from(count)?.to_be_bytes()].concat();
    if let Some(u) = unknown {
        extra.extend(u.to_be_bytes());
    }
    tag(kind, 12 + extra.len(), &extra, payload)
}

#[cfg(test)]
mod timing_tests {
    use super::*;
    #[test]
    fn detail_padding_matches_source_frame_quantization_strides_and_first_frame_contents() {
        for (kind, stride) in [(b"PWV3", 1_u32), (b"PWV5", 2), (b"PWV7", 3)] {
            let payload: Vec<u8> = (1..=stride * 3).map(|v| v as u8).collect();
            let original = wave_tag(kind, stride, 3, 0, &payload).unwrap();
            for (offset, frames) in [(0., 0), (-0.005, 0), (0.001, 0), (0.025, 4), (0.051, 8)] {
                let shifted = pad_detail_waveform(&original, offset).unwrap();
                assert_eq!(
                    u32::from_be_bytes(shifted[16..20].try_into().unwrap()),
                    3 + frames
                );
                assert_eq!(
                    u32::from_be_bytes(shifted[8..12].try_into().unwrap()) as usize,
                    shifted.len()
                );
                assert_eq!(
                    &shifted[24 + frames as usize * stride as usize..],
                    payload.as_slice()
                );
                for frame in shifted[24..24 + frames as usize * stride as usize]
                    .chunks_exact(stride as usize)
                {
                    assert_eq!(frame, &payload[..stride as usize]);
                }
            }
        }
        for kind in [b"PWAV", b"PWV2", b"PWV4", b"PWV6"] {
            let original = wave_tag(kind, 1, 3, 0, &[7, 8, 9]).unwrap();
            assert_eq!(pad_detail_waveform(&original, 0.051).unwrap(), original);
        }
    }
}
