use crate::{Candidate, Matcher};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use unicode_normalization::UnicodeNormalization;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReviewTrack {
    pub candidate: Candidate,
    pub extra1: String,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewState {
    #[default]
    Unlocked,
    Locked,
    CopyCues,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GroupEdit {
    Select { track_id: String },
    UseTags { track_id: Option<String> },
    Lock,
    Unlock { force: bool },
    ToggleIgnore { track_id: String },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Preference {
    Folder { path: String },
    MostCuepoints,
    HighestBitrate,
    TitleContains { value: String },
    Extra1Contains { value: String },
    Extra1NotContains { value: String },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReviewGroup {
    pub id: String,
    pub exact_audio: bool,
    pub tracks: Vec<ReviewTrack>,
    pub keeper: Option<String>,
    pub tags_from: Option<String>,
    pub state: ReviewState,
    pub ignored: BTreeSet<String>,
    pub changed_selection: bool,
}
impl ReviewGroup {
    pub fn new(id: String, tracks: Vec<ReviewTrack>, exact_audio: bool) -> Result<Self> {
        ensure!(
            !id.is_empty() && tracks.len() >= 2,
            "Invalid duplicate review group"
        );
        let candidates = tracks
            .iter()
            .map(|t| t.candidate.clone())
            .collect::<Vec<_>>();
        let selected = Matcher::new().preselect(&candidates, exact_audio)?;
        let ignored = candidates
            .iter()
            .filter(|t| t.ignored)
            .map(|t| t.id.clone())
            .collect();
        Ok(Self {
            id,
            exact_audio,
            tracks,
            keeper: selected.keeper,
            tags_from: selected.tags_from,
            state: ReviewState::Unlocked,
            ignored,
            changed_selection: false,
        })
    }
    fn member(&self, id: &str) -> Result<&ReviewTrack> {
        self.tracks
            .iter()
            .find(|t| t.candidate.id == id)
            .context("Track is not in this duplicate group")
    }
    fn selectable(&self, id: &str) -> Result<()> {
        ensure!(
            self.member(id)?.candidate.present && !self.ignored.contains(id),
            "Track is unavailable or ignored"
        );
        Ok(())
    }
    fn unlock(&mut self, force: bool) {
        self.state = if self.state == ReviewState::Locked && self.tags_from.is_some() && !force {
            ReviewState::CopyCues
        } else {
            ReviewState::Unlocked
        };
    }
    pub fn edit(&mut self, edit: GroupEdit) -> Result<()> {
        match edit {
            GroupEdit::Select { track_id } => {
                self.selectable(&track_id)?;
                self.keeper = Some(track_id);
            }
            GroupEdit::UseTags { track_id } => {
                if let Some(id) = &track_id {
                    self.selectable(id)?;
                }
                self.tags_from = track_id.filter(|id| Some(id) != self.keeper.as_ref());
                if self.tags_from.is_none() && self.state == ReviewState::CopyCues {
                    self.state = ReviewState::Unlocked;
                }
            }
            GroupEdit::Lock => self.state = ReviewState::Locked,
            GroupEdit::Unlock { force } => self.unlock(force),
            GroupEdit::ToggleIgnore { track_id } => {
                self.member(&track_id)?;
                if !self.ignored.remove(&track_id) {
                    self.ignored.insert(track_id);
                }
            }
        }
        Ok(())
    }
    pub fn prefer(
        &mut self,
        preference: &Preference,
        override_locked: bool,
        use_best_cues: bool,
    ) -> Result<()> {
        if self.state == ReviewState::Locked && !override_locked {
            return Ok(());
        }
        let available = self
            .tracks
            .iter()
            .filter(|t| t.candidate.present && !self.ignored.contains(&t.candidate.id))
            .collect::<Vec<_>>();
        let current = self
            .keeper
            .as_deref()
            .map(|id| self.member(id))
            .transpose()?;
        let most = available
            .iter()
            .copied()
            .max_by_key(|t| t.candidate.cue_count)
            .filter(|t| t.candidate.cue_count > 0);
        let most = most.and_then(|m| {
            available
                .iter()
                .copied()
                .find(|t| t.candidate.cue_count == m.candidate.cue_count)
        });
        let selected = match preference {
            Preference::Folder { path } => {
                ensure!(
                    !path.is_empty() && path.len() <= 32768,
                    "Invalid preferred folder"
                );
                let path = format!(
                    "{}/",
                    path.replace('\\', "/")
                        .nfc()
                        .collect::<String>()
                        .to_lowercase()
                        .trim_end_matches('/')
                );
                let candidates = available
                    .iter()
                    .copied()
                    .filter(|t| {
                        t.candidate
                            .location
                            .replace('\\', "/")
                            .nfc()
                            .collect::<String>()
                            .to_lowercase()
                            .starts_with(&path)
                    })
                    .map(|t| &t.candidate)
                    .collect::<Vec<_>>();
                crate::preferred_path(&candidates)
                    .and_then(|c| available.iter().copied().find(|t| t.candidate.id == c.id))
            }
            Preference::MostCuepoints => most
                .filter(|t| current.is_none_or(|c| t.candidate.cue_count > c.candidate.cue_count)),
            Preference::HighestBitrate => {
                let maximum = available
                    .iter()
                    .map(|t| t.candidate.bitrate.unwrap_or(0.))
                    .max_by(f64::total_cmp);
                available.iter().copied().find(|t| {
                    Some(t.candidate.bitrate.unwrap_or(0.)) == maximum
                        && current.is_none_or(|c| {
                            t.candidate.bitrate.unwrap_or(0.) > c.candidate.bitrate.unwrap_or(0.)
                        })
                })
            }
            Preference::TitleContains { value }
            | Preference::Extra1Contains { value }
            | Preference::Extra1NotContains { value } => {
                ensure!(value.len() <= 1_048_576, "Preference text exceeds 1 MiB");
                let needle = value.to_lowercase();
                available.iter().copied().find(|t| match preference {
                    Preference::TitleContains { .. } => {
                        t.candidate.title.to_lowercase().contains(&needle)
                    }
                    Preference::Extra1Contains { .. } => t.extra1.to_lowercase().contains(&needle),
                    _ => !t.extra1.to_lowercase().contains(&needle),
                })
            }
        };
        if let Some(selected) = selected {
            let id = selected.candidate.id.clone();
            let donor = most
                .filter(|most| {
                    use_best_cues
                        && !matches!(preference, Preference::MostCuepoints)
                        && most.candidate.id != id
                        && most.candidate.cue_count > selected.candidate.cue_count
                })
                .map(|t| t.candidate.id.clone());
            if self.keeper.as_ref() != Some(&id) {
                self.keeper = Some(id);
                if donor.is_some() {
                    self.tags_from = donor;
                }
                self.changed_selection = true;
            }
            if override_locked {
                self.unlock(true);
            }
        }
        Ok(())
    }
    /// A selected ignored/unavailable keeper remains visible, but cannot be applied.
    pub fn ready(&self) -> bool {
        self.state != ReviewState::Locked
            && self
                .keeper
                .as_deref()
                .is_some_and(|id| self.selectable(id).is_ok())
            && self
                .tags_from
                .as_deref()
                .is_none_or(|id| self.selectable(id).is_ok())
            && (self.state != ReviewState::CopyCues || self.tags_from.is_some())
    }
}
