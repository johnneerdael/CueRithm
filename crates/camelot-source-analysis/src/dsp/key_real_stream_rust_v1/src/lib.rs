use crate::dsp::decoder_boundary_rust_v1::BatchBoundary;
use anyhow::{Context, ensure};
use std::{fs::File, io::ErrorKind, path::Path};
use symphonia::core::{
    audio::SampleBuffer,
    codecs::{CODEC_TYPE_FLAC, DecoderOptions},
    errors::Error as DecodeError,
    formats::FormatOptions,
    io::MediaSourceStream,
    meta::MetadataOptions,
    probe::Hint,
};

pub struct DecodedProfile {
    pub interleaved: Vec<f32>,
    pub full_frames: usize,
    pub analyzed_frames: usize,
    pub decoded_packets: usize,
}

pub fn decode_profile(path: &Path) -> anyhow::Result<DecodedProfile> {
    ensure!(
        path.to_string_lossy()
            .to_ascii_lowercase()
            .ends_with("flac"),
        "FLAC reader path required"
    );
    let before = path.metadata()?;
    let source = MediaSourceStream::new(Box::new(File::open(path)?), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("flac");
    let mut format = symphonia::default::get_probe()
        .format(
            &hint,
            source,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )?
        .format;
    let track = format.default_track().context("No audio track")?;
    let track_id = track.id;
    let params = track.codec_params.clone();
    ensure!(
        params.codec == CODEC_TYPE_FLAC && params.sample_rate == Some(44100),
        "Only source-rate44100Hz FLAC validated"
    );
    ensure!(
        params.bits_per_sample == Some(16),
        "Only16-bit source validated"
    );
    let mut decoder =
        symphonia::default::get_codecs().make(&params, &DecoderOptions { verify: true })?;
    let mut interleaved = Vec::new();
    let mut full_frames = 0;
    let mut decoded_packets = 0;
    let mut boundary = BatchBoundary::default();
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(DecodeError::IoError(error)) if error.kind() == ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(error.into()),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = decoder.decode(&packet)?;
        ensure!(
            decoded.spec().rate == 44100 && decoded.spec().channels.count() == 2,
            "Only44100Hz stereo validated"
        );
        let frames = decoded.frames();
        full_frames += frames;
        ensure!(
            full_frames <= 64_000_000,
            "input exceeds prototype frame limit"
        );
        decoded_packets += 1;
        boundary.push(frames as u64);
        let mut converted = SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
        converted.copy_interleaved_ref(decoded);
        ensure!(
            converted.samples().len() == frames * 2,
            "invalid converted frame count"
        );
        interleaved.extend_from_slice(converted.samples());
    }
    ensure!(
        Some(full_frames as u64) == params.n_frames,
        "Incomplete source decode"
    );
    ensure!(
        decoder.finalize().verify_ok == Some(true),
        "FLAC integrity verification failed"
    );
    let after = path.metadata()?;
    ensure!(
        before.len() == after.len() && before.modified()? == after.modified()?,
        "Source changed during decoding"
    );
    let analyzed_frames = boundary.analyzed_frames() as usize;
    ensure!(
        analyzed_frames <= full_frames && interleaved.len() == full_frames * 2,
        "invalid boundary shape"
    );
    interleaved.truncate(analyzed_frames * 2);
    Ok(DecodedProfile {
        interleaved,
        full_frames,
        analyzed_frames,
        decoded_packets,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interleaved_signed16_samples_are_exactly_normalized() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/stereo16-44100.flac");
        let decoded = decode_profile(&path).unwrap();
        let expected: Vec<f32> = [
            (-32768_i16, 32767_i16),
            (-1, 1),
            (0, 16384),
            (12345, -23456),
        ]
        .into_iter()
        .cycle()
        .take(64)
        .flat_map(|(l, r)| [f32::from(l) / 32768.0, f32::from(r) / 32768.0])
        .collect();
        assert_eq!(
            decoded
                .interleaved
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>(),
            expected.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
        );
        assert_eq!(decoded.full_frames, 64);
        assert_eq!(decoded.analyzed_frames, 64);
        assert_eq!(decoded.decoded_packets, 1);
    }

    #[test]
    fn resampling_is_rejected_instead_of_inventing_a_reference_profile() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/stereo16-48000.flac");
        let result = decode_profile(&path);
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("44100"));
    }
}
