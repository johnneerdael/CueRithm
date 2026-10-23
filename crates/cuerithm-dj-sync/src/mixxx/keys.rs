//! CueRithm key text to Mixxx's `ChromaticKey` id and ID3v2 spelling.
use anyhow::{Context, Result};
use camelot_core::Mode;

/// A key as Mixxx stores it: the `ChromaticKey` enum value (1-12 major, 13-24 minor)
/// and the ID3v2 text used for `library.key` and `KeyMap.global_key_text`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct MixxxKey {
    pub id: u8,
    pub text: &'static str,
}

/// Indexed by `tonic` (C = 0) for major, then again for minor.
const ID3V2: [&str; 24] = [
    "C", "Db", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B", //
    "Cm", "C#m", "Dm", "Ebm", "Em", "Fm", "F#m", "Gm", "G#m", "Am", "Bbm", "Bm",
];

/// Accepts every spelling `formatting::parse_key` does (Camelot, Open Key, musical).
/// Empty text is no key, so an absent CueRithm key never blanks a Mixxx one.
pub(super) fn mixxx_key(text: &str) -> Result<Option<MixxxKey>> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    let key = crate::formatting::parse_key(text)
        .with_context(|| format!("Mixxx cannot store the key {text:?}"))?;
    let index = usize::from(key.tonic()) + if key.mode() == Mode::Minor { 12 } else { 0 };
    Ok(Some(MixxxKey {
        id: index as u8 + 1,
        text: ID3V2[index],
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{KeyNotation, formatting::format_key};
    use camelot_core::MusicalKey;

    #[test]
    fn every_camelot_open_key_and_musical_spelling_maps_to_the_same_chromatic_id() {
        for mode in [Mode::Major, Mode::Minor] {
            for tonic in 0..12u8 {
                let key = MusicalKey::new(tonic, mode).unwrap();
                let expected = tonic + 1 + if mode == Mode::Minor { 12 } else { 0 };
                let camelot = key.camelot();
                let spellings = [
                    camelot.clone(),
                    format_key(&camelot, KeyNotation::OpenKey).unwrap(),
                    format_key(&camelot, KeyNotation::Musical).unwrap(),
                    key.name(),
                ];
                for spelling in spellings {
                    let found = mixxx_key(&spelling).unwrap().unwrap();
                    assert_eq!(found.id, expected, "{spelling}");
                }
            }
        }
    }

    #[test]
    fn key_text_uses_id3v2_spelling() {
        for (input, id, text) in [
            ("8B", 1, "C"),
            ("F#", 7, "F#"),
            ("Db", 2, "Db"),
            ("Ebm", 16, "Ebm"),
            ("G#m", 21, "G#m"),
            ("Bbm", 23, "Bbm"),
            ("8A", 22, "Am"),
            ("Bm", 24, "Bm"),
        ] {
            let found = mixxx_key(input).unwrap().unwrap();
            assert_eq!((found.id, found.text), (id, text), "{input}");
        }
    }

    #[test]
    fn empty_key_is_none() {
        assert_eq!(mixxx_key("").unwrap(), None);
        assert_eq!(mixxx_key("   ").unwrap(), None);
    }

    #[test]
    fn bad_key_is_error_naming_the_value() {
        let error = mixxx_key("not a key").unwrap_err().to_string();
        assert!(error.contains("not a key"), "{error}");
    }
}
