use crate::KeyMixingMode;
/// Parse Camelot, Open Key and the observed standard musical aliases.
/// Key values are metadata; this never predicts a musical key from audio.
fn key(value: &str) -> Option<(u8, bool)> {
    let v = value.to_lowercase();
    if let Some(suffix) = v.chars().last().filter(|c| "abdm".contains(*c)) {
        let digits = &v[..v.len() - 1];
        if (1..=2).contains(&digits.len()) && digits.bytes().all(|c| c.is_ascii_digit()) {
            let n: u8 = digits.parse().ok()?;
            if !(1..=12).contains(&n) {
                return None;
            }
            return Some((
                if matches!(suffix, 'd' | 'm') {
                    (n + 6) % 12 + 1
                } else {
                    n
                },
                matches!(suffix, 'a' | 'm'),
            ));
        }
    }
    let (note, minor) = if let Some(n) = v
        .strip_suffix(" minor")
        .or_else(|| v.strip_suffix("min"))
        .or_else(|| v.strip_suffix('m'))
    {
        (n, true)
    } else {
        (
            v.strip_suffix(" major")
                .or_else(|| v.strip_suffix("maj"))
                .unwrap_or(&v),
            false,
        )
    };
    let tonic = match note {
        "c" => 0,
        "c#" | "db" => 1,
        "d" => 2,
        "d#" | "eb" => 3,
        "e" => 4,
        "f" => 5,
        "f#" | "gb" => 6,
        "g" => 7,
        "g#" | "ab" => 8,
        "a" => 9,
        "a#" | "bb" => 10,
        "b" => 11,
        _ => return None,
    };
    let numbers = if minor {
        [5, 12, 7, 2, 9, 4, 11, 6, 1, 8, 3, 10]
    } else {
        [8, 3, 10, 5, 12, 7, 2, 9, 4, 11, 6, 1]
    };
    Some((numbers[tonic], minor))
}
pub fn camelot(value: &str) -> Option<String> {
    key(value).map(|(n, m)| format!("{n}{}", if m { 'A' } else { 'B' }))
}
pub fn compatible(left: &str, right: &str, mode: KeyMixingMode) -> bool {
    let (Some((a, minor_a)), Some((b, minor_b))) = (key(left), key(right)) else {
        return false;
    };
    let distance = a.abs_diff(b);
    match mode {
        KeyMixingMode::Harmonic => {
            distance == 0 || (minor_a == minor_b && matches!(distance, 1 | 11))
        }
        KeyMixingMode::Fuzzy => matches!(distance, 0 | 1 | 11),
    }
}
