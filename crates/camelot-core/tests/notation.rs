use camelot_core::{Mode, MusicalKey};

#[test]
fn all_twenty_four_keys_use_standard_camelot_notation() {
    // Tonic order: C, C#, D, Eb, E, F, F#, G, Ab, A, Bb, B.
    let major = [
        "8B", "3B", "10B", "5B", "12B", "7B", "2B", "9B", "4B", "11B", "6B", "1B",
    ];
    let minor = [
        "5A", "12A", "7A", "2A", "9A", "4A", "11A", "6A", "1A", "8A", "3A", "10A",
    ];
    for tonic in 0..12 {
        for (mode, expected) in [(Mode::Major, major[tonic]), (Mode::Minor, minor[tonic])] {
            let key = MusicalKey::new(tonic as u8, mode).unwrap();
            assert_eq!(key.camelot(), expected);
            assert_eq!(MusicalKey::from_camelot(expected).unwrap(), key);
        }
    }
}

#[test]
fn compatibility_wraps_at_twelve_and_preserves_relative_modes() {
    let key = MusicalKey::from_camelot("12A").unwrap();
    for value in ["12A", "11A", "1A", "12B"] {
        assert!(key.compatible_with(MusicalKey::from_camelot(value).unwrap()));
    }
    for value in ["2A", "11B", "1B"] {
        assert!(!key.compatible_with(MusicalKey::from_camelot(value).unwrap()));
    }
}

#[test]
fn invalid_keys_are_rejected_instead_of_wrapped() {
    assert!(MusicalKey::new(12, Mode::Major).is_err());
    for value in ["0A", "13A", "1C", "8", "", "8BB", "-1A", " 8A"] {
        assert!(MusicalKey::from_camelot(value).is_err(), "{value}");
    }
}
