//! The protobuf (proto2) messages Mixxx keeps in `library.beats` and `library.keys`.
//! Field numbers follow Mixxx's `beats.proto` and `keys.proto`; the messages are
//! small enough to encode directly instead of adding a protobuf dependency.

fn varint(mut value: u64, out: &mut Vec<u8>) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

/// `int32` fields sign-extend, so a negative value takes ten bytes.
fn int32(value: i32, out: &mut Vec<u8>) {
    varint(i64::from(value) as u64, out);
}

fn length_delimited(field: u32, body: &[u8], out: &mut Vec<u8>) {
    varint(u64::from(field << 3 | 2), out);
    varint(body.len() as u64, out);
    out.extend_from_slice(body);
}

/// `message Beat { optional int32 frame_position = 1; }`
fn beat(frame: i32) -> Vec<u8> {
    let mut body = vec![0x08];
    int32(frame, &mut body);
    body
}

/// `BeatGrid { Bpm bpm = 1; Beat first_beat = 2; }`, the constant-tempo grid.
pub(super) fn beat_grid(bpm: f64, first_beat_frame: i32) -> Vec<u8> {
    // `message Bpm { optional double bpm = 1; }`: field 1, 64-bit.
    let mut tempo = vec![0x09];
    tempo.extend_from_slice(&bpm.to_le_bytes());
    let mut out = Vec::new();
    length_delimited(1, &tempo, &mut out);
    length_delimited(2, &beat(first_beat_frame), &mut out);
    out
}

/// `BeatMap { repeated Beat beat = 1; }`, a variable-tempo list of beat frames.
pub(super) fn beat_map(frames: &[i32]) -> Vec<u8> {
    let mut out = Vec::new();
    for frame in frames {
        length_delimited(1, &beat(*frame), &mut out);
    }
    out
}

/// `KeyMap { global_key = 1; global_key_text = 2; source = 4; }`. `source` is
/// 0 analyzer, 1 file metadata, 2 user.
pub(super) fn key_map(key_id: u8, id3_text: &str, source: u8) -> Vec<u8> {
    let mut out = vec![0x08];
    varint(u64::from(key_id), &mut out);
    length_delimited(2, id3_text.as_bytes(), &mut out);
    out.push(0x20);
    varint(u64::from(source), &mut out);
    out
}

pub(super) mod decode {
    //! Minimal reader: proves the encoders round-trip in tests and lets the staged
    //! database be checked before it is published.
    use anyhow::{Result, bail, ensure};

    #[derive(Debug, PartialEq)]
    pub enum Field {
        Varint(u64),
        Fixed64([u8; 8]),
        Bytes(Vec<u8>),
    }

    fn read_varint(bytes: &[u8], at: &mut usize) -> Result<u64> {
        let mut value = 0u64;
        for shift in (0..70).step_by(7) {
            let byte = *bytes
                .get(*at)
                .ok_or_else(|| anyhow::anyhow!("truncated varint"))?;
            *at += 1;
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        bail!("varint too long")
    }

    pub fn fields(bytes: &[u8]) -> Result<Vec<(u32, Field)>> {
        let mut at = 0;
        let mut out = Vec::new();
        while at < bytes.len() {
            let tag = read_varint(bytes, &mut at)?;
            let field = (tag >> 3) as u32;
            let value = match tag & 7 {
                0 => Field::Varint(read_varint(bytes, &mut at)?),
                1 => {
                    ensure!(at + 8 <= bytes.len(), "truncated fixed64");
                    let mut raw = [0u8; 8];
                    raw.copy_from_slice(&bytes[at..at + 8]);
                    at += 8;
                    Field::Fixed64(raw)
                }
                2 => {
                    let length = read_varint(bytes, &mut at)? as usize;
                    ensure!(at + length <= bytes.len(), "truncated bytes");
                    let value = bytes[at..at + length].to_vec();
                    at += length;
                    Field::Bytes(value)
                }
                other => bail!("unexpected wire type {other}"),
            };
            out.push((field, value));
        }
        Ok(out)
    }

    fn only(parsed: &[(u32, Field)], field: u32) -> Result<&Field> {
        let mut matching = parsed.iter().filter(|(number, _)| *number == field);
        let found = matching
            .next()
            .ok_or_else(|| anyhow::anyhow!("missing field {field}"))?;
        ensure!(matching.next().is_none(), "repeated field {field}");
        Ok(&found.1)
    }

    fn frame(message: &[u8]) -> Result<i32> {
        match only(&fields(message)?, 1)? {
            Field::Varint(value) => Ok(*value as i64 as i32),
            other => bail!("frame_position is not a varint: {other:?}"),
        }
    }

    pub fn beat_grid(bytes: &[u8]) -> Result<(f64, i32)> {
        let parsed = fields(bytes)?;
        let Field::Bytes(tempo) = only(&parsed, 1)? else {
            bail!("bpm is not a message")
        };
        let tempo_fields = fields(tempo)?;
        let Field::Fixed64(raw) = only(&tempo_fields, 1)? else {
            bail!("bpm is not a double")
        };
        let Field::Bytes(first) = only(&parsed, 2)? else {
            bail!("first_beat is not a message")
        };
        Ok((f64::from_le_bytes(*raw), frame(first)?))
    }

    pub fn beat_map(bytes: &[u8]) -> Result<Vec<i32>> {
        fields(bytes)?
            .into_iter()
            .map(|(number, value)| {
                ensure!(number == 1, "unexpected BeatMap field {number}");
                match value {
                    Field::Bytes(beat) => frame(&beat),
                    other => bail!("beat is not a message: {other:?}"),
                }
            })
            .collect()
    }

    pub fn key_map(bytes: &[u8]) -> Result<(u8, String, u8)> {
        let parsed = fields(bytes)?;
        let Field::Varint(key) = only(&parsed, 1)? else {
            bail!("global_key is not an enum")
        };
        let Field::Bytes(text) = only(&parsed, 2)? else {
            bail!("global_key_text is not a string")
        };
        let Field::Varint(source) = only(&parsed, 4)? else {
            bail!("source is not an enum")
        };
        Ok((*key as u8, String::from_utf8(text.clone())?, *source as u8))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beat_grid_matches_hand_written_golden_bytes() {
        let mut expected = vec![0x0a, 0x09, 0x09];
        expected.extend_from_slice(&128.0f64.to_le_bytes());
        expected.extend_from_slice(&[0x12, 0x04, 0x08, 0xc4, 0xd8, 0x02]);
        assert_eq!(beat_grid(128.0, 44_100), expected);
        // 128.0 as an IEEE-754 double is 0x4060000000000000.
        assert_eq!(&expected[3..11], &[0, 0, 0, 0, 0, 0, 0x60, 0x40]);
    }

    #[test]
    fn beat_map_matches_hand_written_golden_bytes() {
        assert_eq!(
            beat_map(&[0, 22_050, 44_100]),
            vec![
                0x0a, 0x02, 0x08, 0x00, //
                0x0a, 0x04, 0x08, 0xa2, 0xac, 0x01, //
                0x0a, 0x04, 0x08, 0xc4, 0xd8, 0x02,
            ]
        );
        assert!(beat_map(&[]).is_empty());
    }

    #[test]
    fn key_map_matches_hand_written_golden_bytes() {
        assert_eq!(
            key_map(22, "Am", 2),
            vec![0x08, 0x16, 0x12, 0x02, 0x41, 0x6d, 0x20, 0x02]
        );
    }

    #[test]
    fn negative_frames_are_sign_extended_to_ten_bytes() {
        let bytes = beat_grid(100.0, -1);
        let mut expected_beat = vec![0x12, 0x0b, 0x08];
        expected_beat.extend_from_slice(&[0xff; 9]);
        expected_beat.push(0x01);
        assert!(bytes.ends_with(&expected_beat));
        assert_eq!(decode::beat_grid(&bytes).unwrap(), (100.0, -1));
    }

    #[test]
    fn encoders_round_trip_through_the_independent_reader() {
        for frame in [
            0,
            1,
            127,
            128,
            16_383,
            16_384,
            2_097_152,
            i32::MAX,
            -1,
            i32::MIN,
        ] {
            let bytes = beat_grid(123.456, frame);
            assert_eq!(decode::beat_grid(&bytes).unwrap(), (123.456, frame));
        }
        let frames = [-48, 0, 22_050, 44_100, 1_000_000];
        assert_eq!(decode::beat_map(&beat_map(&frames)).unwrap(), frames);
        for key in 1..=24u8 {
            assert_eq!(
                decode::key_map(&key_map(key, "F#m", 1)).unwrap(),
                (key, "F#m".to_owned(), 1)
            );
        }
    }
}
