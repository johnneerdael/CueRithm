use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Major,
    Minor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "KeyFields")]
pub struct MusicalKey {
    tonic: u8,
    mode: Mode,
}

#[derive(Deserialize)]
struct KeyFields {
    tonic: u8,
    mode: Mode,
}

impl TryFrom<KeyFields> for MusicalKey {
    type Error = anyhow::Error;
    fn try_from(value: KeyFields) -> Result<Self> {
        Self::new(value.tonic, value.mode)
    }
}

const MAJOR: [u8; 12] = [8, 3, 10, 5, 12, 7, 2, 9, 4, 11, 6, 1];
const MINOR: [u8; 12] = [5, 12, 7, 2, 9, 4, 11, 6, 1, 8, 3, 10];

impl MusicalKey {
    pub fn new(tonic: u8, mode: Mode) -> Result<Self> {
        ensure!(tonic < 12, "Tonic must be in 0..12");
        Ok(Self { tonic, mode })
    }

    pub fn tonic(self) -> u8 {
        self.tonic
    }

    pub fn mode(self) -> Mode {
        self.mode
    }

    pub fn camelot_number(self) -> u8 {
        match self.mode {
            Mode::Major => MAJOR[self.tonic as usize],
            Mode::Minor => MINOR[self.tonic as usize],
        }
    }

    pub fn camelot(self) -> String {
        format!(
            "{}{}",
            self.camelot_number(),
            if self.mode == Mode::Minor { 'A' } else { 'B' }
        )
    }

    pub fn name(self) -> String {
        let note = [
            "C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B",
        ][self.tonic as usize];
        format!(
            "{note} {}",
            if self.mode == Mode::Major {
                "major"
            } else {
                "minor"
            }
        )
    }

    pub fn from_camelot(value: &str) -> Result<Self> {
        let mode = if value.ends_with('A') {
            Mode::Minor
        } else if value.ends_with('B') {
            Mode::Major
        } else {
            bail!("Camelot key must end with A or B");
        };
        let digits = &value[..value.len() - 1];
        ensure!(
            (1..=2).contains(&digits.len()) && digits.bytes().all(|c| c.is_ascii_digit()),
            "Invalid Camelot number"
        );
        let number: u8 = digits.parse()?;
        let table = if mode == Mode::Major { MAJOR } else { MINOR };
        let tonic = table
            .iter()
            .position(|n| *n == number)
            .ok_or_else(|| anyhow::anyhow!("Camelot number must be 1..12"))?;
        Self::new(tonic as u8, mode)
    }

    pub fn compatible_with(self, other: Self) -> bool {
        let distance = self.camelot_number().abs_diff(other.camelot_number());
        distance == 0 || (self.mode == other.mode && (distance == 1 || distance == 11))
    }
}
