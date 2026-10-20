use crate::{KeyNotation, SyncLibrary, SyncOptions, SyncTextField, SyncTrack};
use anyhow::{Context, Result, ensure};
use camelot_core::{Mode, MusicalKey};

pub fn parse_key(value: &str) -> Result<MusicalKey> {
    let value = value.trim().replace('♭', "b").replace('♯', "#");
    let upper = value.to_uppercase();
    if upper.len() > 1
        && upper.ends_with(['A', 'B'])
        && upper[..upper.len() - 1].bytes().all(|b| b.is_ascii_digit())
    {
        return MusicalKey::from_camelot(&upper);
    }
    if let Some(digits) = value.strip_suffix('d').or_else(|| value.strip_suffix('m'))
        && let Ok(n) = digits.parse::<u8>()
    {
        ensure!((1..=12).contains(&n), "Invalid Open Key");
        return MusicalKey::from_camelot(&format!(
            "{}{}",
            (n + 6) % 12 + 1,
            if value.ends_with('d') { 'B' } else { 'A' }
        ));
    }
    let lower = value.to_lowercase();
    let (note, mode) = if let Some(note) = lower.strip_suffix("minor") {
        (note.trim(), Mode::Minor)
    } else if let Some(note) = lower.strip_suffix("major") {
        (note.trim(), Mode::Major)
    } else if let Some(note) = lower.strip_suffix('m') {
        (note, Mode::Minor)
    } else {
        (lower.as_str(), Mode::Major)
    };
    let tonic = match note {
        "c" | "b#" => 0,
        "c#" | "db" => 1,
        "d" => 2,
        "d#" | "eb" => 3,
        "e" | "fb" => 4,
        "f" | "e#" => 5,
        "f#" | "gb" => 6,
        "g" => 7,
        "g#" | "ab" => 8,
        "a" => 9,
        "a#" | "bb" => 10,
        "b" | "cb" => 11,
        _ => anyhow::bail!("Unknown musical key: {value}"),
    };
    MusicalKey::new(tonic, mode)
}
pub fn format_key(value: &str, notation: KeyNotation) -> Result<String> {
    if notation == KeyNotation::Original || value.trim().is_empty() {
        return Ok(value.into());
    }
    let key = parse_key(value)?;
    Ok(match notation {
        KeyNotation::Original => value.into(),
        KeyNotation::Camelot => key.camelot(),
        KeyNotation::OpenKey => format!(
            "{}{}",
            (key.camelot_number() + 4) % 12 + 1,
            if key.mode() == Mode::Minor { 'm' } else { 'd' }
        ),
        KeyNotation::Musical => format!(
            "{}{}",
            [
                "C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"
            ][usize::from(key.tonic())],
            if key.mode() == Mode::Minor { "m" } else { "" }
        ),
    })
}
fn text(track: &SyncTrack, field: SyncTextField) -> &str {
    match field {
        SyncTextField::Title => &track.title,
        SyncTextField::Artist => &track.artist,
        SyncTextField::Album => &track.album,
        SyncTextField::Genre => &track.genre,
        SyncTextField::Label => &track.label,
        SyncTextField::Comment => &track.comment,
        SyncTextField::Key => &track.key,
    }
}
/// Apply mappings simultaneously, so swapping fields never duplicates a value.
pub fn format_library(library: &SyncLibrary, options: &SyncOptions) -> Result<SyncLibrary> {
    let mut output = library.clone();
    for (track, original) in output.tracks.iter_mut().zip(&library.tracks) {
        for (&destination, &source) in &options.field_mappings {
            let value = text(original, source).to_owned();
            *match destination {
                SyncTextField::Title => &mut track.title,
                SyncTextField::Artist => &mut track.artist,
                SyncTextField::Album => &mut track.album,
                SyncTextField::Genre => &mut track.genre,
                SyncTextField::Label => &mut track.label,
                SyncTextField::Comment => &mut track.comment,
                SyncTextField::Key => &mut track.key,
            } = value;
        }
        track.key = format_key(&track.key, options.key_notation)
            .with_context(|| format!("Cannot convert key for sync track {}", track.id))?;
    }
    Ok(output)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_key_notations_roundtrip_and_field_swaps_are_simultaneous() {
        for n in 1..=12 {
            for letter in ['A', 'B'] {
                let key = format!("{n}{letter}");
                for notation in [KeyNotation::Musical, KeyNotation::OpenKey] {
                    assert_eq!(
                        format_key(&format_key(&key, notation).unwrap(), KeyNotation::Camelot)
                            .unwrap(),
                        key
                    );
                }
            }
        }
        let library = SyncLibrary {
            tracks: vec![SyncTrack {
                title: "Title".into(),
                artist: "Artist".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let options = SyncOptions {
            field_mappings: std::collections::BTreeMap::from([
                (SyncTextField::Title, SyncTextField::Artist),
                (SyncTextField::Artist, SyncTextField::Title),
            ]),
            ..Default::default()
        };
        let output = format_library(&library, &options).unwrap();
        assert_eq!(output.tracks[0].title, "Artist");
        assert_eq!(output.tracks[0].artist, "Title");
        assert_eq!(library.tracks[0].title, "Title");
    }
}
