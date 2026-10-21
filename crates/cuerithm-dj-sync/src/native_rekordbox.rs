//! Native desktop rekordbox assets generated from the shared measured PCM path.
mod artwork;
mod source;
use crate::{DjSoftware, SyncOptions, SyncTrack, pioneer_usb::analysis, timing};
use anyhow::{Context, Result, ensure};
pub use artwork::*;
use cuerithm_media::{CancellationToken, MediaRuntime};
use cuerithm_rekordbox::{
    NativeAnalysisKind, NativeCue,
    anlz::{AnlzFile, AnlzTag, Beat, ParseLimits},
    new_native_analysis,
};
pub use source::*;

pub struct NativeRekordboxAnalysis {
    pub dat: Vec<u8>,
    pub ext: Vec<u8>,
    pub two_ex: Vec<u8>,
}
/// Generate the native desktop DAT/EXT/2EX layouts from complete playable PCM.
/// Existing bundles must instead retain their opaque sections through SDK edits.
/// A 3EX embedding is never synthesized from waveform data.
pub fn native_rekordbox_analysis(
    track: &SyncTrack,
    cues: &[NativeCue],
    runtime: &MediaRuntime,
    cancel: &CancellationToken,
) -> Result<NativeRekordboxAnalysis> {
    cancel.check()?;
    validate_timeline(track)?;
    ensure!(
        track.path.is_absolute(),
        "Native source audio reference must be absolute"
    );
    let path = track
        .path
        .to_str()
        .context("Native source audio reference is not Unicode")?;
    let profile = track
        .export_timing
        .as_ref()
        .context("Native source timing is missing")?;
    ensure!(
        profile.measurement_identity == runtime.export_timing_identity(),
        "Native source timing runtime changed; measure the source again"
    );
    let cache = tempfile::tempdir()?;
    let audio = runtime.decode_cancellable(&track.path, cache.path(), cancel)?;
    ensure!(
        profile.source_sha256 == audio.source_sha256
            && profile.source_sample_rate == audio.info.sample_rate,
        "Native source timing does not belong to the selected audio bytes"
    );
    let mut samples = Vec::with_capacity(usize::try_from(audio.frames)?);
    let mut frame = 0;
    while frame < audio.frames {
        cancel.check()?;
        let stereo = audio.read_frames(frame, 65536)?;
        samples.extend(
            stereo
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| (pair[0] + pair[1]) * 0.5),
        );
        frame += stereo.len() as u64 / 2;
    }
    let mut wave = analysis::waveforms_cancellable(
        &samples,
        audio.clock.sample_rate,
        track,
        &SyncOptions::default(),
        cancel,
    )?;
    let offset = track.native_timeline.as_ref().unwrap().offset_seconds;
    wave.detail = analysis::pad_detail_waveform(&wave.detail, offset)?;
    wave.rgb_detail = analysis::pad_detail_waveform(&wave.rgb_detail, offset)?;
    wave.three_detail = analysis::pad_detail_waveform(&wave.three_detail, offset)?;
    let mut grid_track = track.clone();
    grid_track.duration_seconds = audio.clock.duration_seconds();
    let beats = native_rekordbox_beats(&grid_track)?;
    let authored = new_native_analysis(path, &beats, cues, NativeAnalysisKind::Ext)?;
    let cue_file = AnlzFile::parse(&authored, ParseLimits::default())?;
    let cue_tag = |fourcc, kind| -> Result<Vec<u8>> {
        cue_file
            .tags()
            .iter()
            .find(|tag| tag.fourcc() == fourcc && tag.cues().is_ok_and(|list| list.kind == kind))
            .map(|tag| tag.bytes().to_vec())
            .context("Authored native cue section is missing")
    };
    let samples = profile.decoded_sample_count.unwrap_or(audio.frames);
    let mut vbr = vec![0; 1604];
    vbr[1600..].copy_from_slice(
        &u32::try_from(samples)
            .context("Native decoded sample count exceeds PVBR range")?
            .to_be_bytes(),
    );
    let dat = analysis::file(&[
        AnlzTag::path(path)?.bytes().to_vec(),
        analysis::tag(b"PVBR", 16, &[0; 4], &vbr)?,
        AnlzTag::grid(&beats)?.bytes().to_vec(),
        wave.preview,
        wave.tiny,
        cue_tag(*b"PCOB", 1)?,
        cue_tag(*b"PCOB", 0)?,
    ])?;
    let ext = analysis::file(&[
        AnlzTag::path(path)?.bytes().to_vec(),
        wave.detail,
        wave.rgb_preview,
        wave.rgb_detail,
        cue_tag(*b"PCOB", 1)?,
        cue_tag(*b"PCOB", 0)?,
        cue_tag(*b"PCO2", 1)?,
        cue_tag(*b"PCO2", 0)?,
    ])?;
    let two_ex = analysis::file(&[
        AnlzTag::path(path)?.bytes().to_vec(),
        wave.three_preview,
        wave.three_detail,
        analysis::tag(b"PWVC", 14, &[0, 0], &[0, 134, 0, 177, 0, 156])?,
    ])?;
    cancel.check()?;
    ensure!(
        audio.source_is_unchanged_cancellable(cancel)?,
        "Native source audio changed while generating analysis"
    );
    Ok(NativeRekordboxAnalysis { dat, ext, two_ex })
}
/// Expand the existing sparse tempo anchors using the shared tested grid logic.
pub fn native_rekordbox_beats(track: &SyncTrack) -> Result<Vec<Beat>> {
    Ok(native_rekordbox_grid(track)?.beats)
}
/// Expand once for both PQTZ milliseconds and PQT2 fractional microseconds.
/// The shared grid expansion owns rounding and variable-tempo boundaries.
pub fn native_rekordbox_grid(
    track: &SyncTrack,
) -> Result<cuerithm_rekordbox::anlz::NativeMicrosecondGrid> {
    validate_timeline(track)?;
    ensure!(
        track.duration_seconds.is_finite() && track.duration_seconds > 0.,
        "Native source duration is invalid"
    );
    let mut end = track.clone();
    end.duration_seconds += track.native_timeline.as_ref().unwrap().offset_seconds;
    let (beats, fractions, _) = analysis::grid(&end)?;
    ensure!(!beats.is_empty(), "Native beatgrid has no playable beats");
    Ok(cuerithm_rekordbox::anlz::NativeMicrosecondGrid {
        beats,
        fractions,
        variable: track.beatgrid.len() > 1,
    })
}
fn validate_timeline(track: &SyncTrack) -> Result<()> {
    let applied = track
        .native_timeline
        .as_ref()
        .context("Convert the source to the native rekordbox clock before generating assets")?;
    ensure!(
        applied.target == DjSoftware::Rekordbox,
        "Source export copy belongs to another DJ target"
    );
    timing::export_track(track, DjSoftware::Rekordbox)?;
    ensure!(
        (applied.offset_seconds - timing::offset_seconds(track, DjSoftware::Rekordbox)?).abs()
            < 1e-12,
        "Native timeline offset does not match its measured source profile"
    );
    Ok(())
}
