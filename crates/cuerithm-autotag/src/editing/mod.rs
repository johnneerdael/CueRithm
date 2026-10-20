mod model;
pub use model::*;

#[cfg(test)]
mod tests {
    #[test]
    fn year_month_date_keeps_its_precision() {
        let value = crate::TransferSnapshot {
            date: Some(crate::TransferDate {
                year: 2026,
                month: Some(10),
                day: None,
            }),
            ..Default::default()
        };
        assert!(
            value.validate().is_ok(),
            "year-month precision must not require an invented day"
        );
    }
}
pub(crate) mod native;
use anyhow::{Result, ensure};
pub use native::{metadata_artwork_bytes, read_metadata_snapshot};
pub fn validate_date(date: &crate::TransferDate) -> Result<()> {
    ensure!(
        (1..=9999).contains(&date.year),
        "Year must be between 1 and 9999"
    );
    ensure!(
        match (date.month, date.day) {
            (None, None) => true,
            (Some(m), None) => (1..=12).contains(&m),
            (Some(m), Some(d)) =>
                chrono::NaiveDate::from_ymd_opt(date.year, m.into(), d.into()).is_some(),
            _ => false,
        },
        "Invalid date precision or calendar date"
    );
    Ok(())
}
impl MetadataPatch {
    /// Exact record restoration is internal to a guarded Undo, never accepted from serialized editor input.
    pub fn reorder_artwork(&mut self, order: Vec<usize>) {
        self.artwork_order = Some(order);
    }
    pub fn restore_rating_record(
        &mut self,
        identity: RatingIdentity,
        record: Option<NativeRating>,
    ) {
        self.restored_ratings.push((identity, record));
    }
    pub fn validate(&self) -> Result<()> {
        let mut bytes = 0usize;
        for edit in self
            .text
            .values()
            .chain(self.comments.iter().map(|c| &c.value))
        {
            let values = match edit {
                TextEdit::Set { values } | TextEdit::Append { values } => values,
                _ => continue,
            };
            ensure!(
                !values.is_empty() && values.len() <= 128,
                "Set/Append requires 1 to 128 values; use Clear explicitly"
            );
            for v in values {
                ensure!(!v.contains('\0'), "Metadata text contains a NUL");
                bytes = bytes.saturating_add(v.len());
            }
        }
        ensure!(
            bytes <= 1_048_576
                && self.comments.len() <= 128
                && self.ratings.len() <= 128
                && self.artwork.len() <= 128,
            "Metadata edit exceeds limits"
        );
        for date in [&self.release_date, &self.original_date] {
            if let ValueEdit::Set { value } = date {
                validate_date(value)?;
            }
        }
        for number in [
            &self.track_number,
            &self.track_total,
            &self.disc_number,
            &self.disc_total,
        ] {
            if let ValueEdit::Set { value } = number {
                ensure!(*value > 0, "Track/disc values must be positive; use Clear");
            }
        }
        if let ValueEdit::Set { value } = self.bpm {
            ensure!(
                value.is_finite() && value > 0. && value < 1000.,
                "Invalid file-tag BPM"
            );
        }
        for rating in &self.ratings {
            ensure!(
                rating.identity.owner.len() <= 1024 && !rating.identity.owner.contains('\0'),
                "Invalid rating owner"
            );
            if let ValueEdit::Set { value } = rating.value {
                ensure!(
                    (1..=5).contains(&value),
                    "Rating must be 1 to 5 stars; use Clear"
                );
            }
        }
        for comment in &self.comments {
            ensure!(
                comment.identity.language.len() <= 3 && comment.identity.description.len() <= 4096,
                "Invalid comment identity"
            );
        }
        let mut selected = std::collections::BTreeSet::new();
        for art in &self.artwork {
            if let Some(target) = &art.target {
                ensure!(
                    selected.insert(target.ordinal),
                    "An artwork record may be selected once per patch"
                );
            }
            if let Some(value) = &art.replacement {
                crate::image::Artwork::validate(value.bytes.clone())?;
                ensure!(
                    value.kind.is_none_or(|k| k <= 20)
                        && value.description.as_ref().is_none_or(|s| s.len() <= 4096),
                    "Invalid artwork identity"
                );
            } else {
                ensure!(
                    art.target.is_some(),
                    "Remove requires an exact artwork identity"
                );
            }
        }
        Ok(())
    }
    pub fn protected_tags(&self) -> Vec<crate::SupportedTag> {
        use crate::SupportedTag as T;
        use MetadataTextField as F;
        let mut out = vec![];
        for (f, e) in &self.text {
            if matches!(e, TextEdit::Keep) {
                continue;
            }
            let tag = match f {
                F::Title => Some(T::Title),
                F::Artist => Some(T::Artist),
                F::Album => Some(T::Album),
                F::AlbumArtist => Some(T::AlbumArtist),
                F::Genre => Some(T::Genre),
                F::Style => Some(T::Style),
                F::Label => Some(T::Label),
                F::Version => Some(T::Version),
                F::Remixer => Some(T::Remixer),
                F::CatalogNumber => Some(T::CatalogNumber),
                F::Isrc => Some(T::ISRC),
                F::Key => Some(T::Key),
                _ => None,
            };
            if let Some(tag) = tag {
                out.push(tag)
            }
        }
        if !matches!(self.release_date, ValueEdit::Keep) {
            out.push(T::ReleaseDate)
        }
        if !matches!(self.original_date, ValueEdit::Keep) {
            out.push(T::PublishDate)
        }
        if !matches!(self.track_number, ValueEdit::Keep) {
            out.push(T::TrackNumber)
        }
        if !matches!(self.track_total, ValueEdit::Keep) {
            out.push(T::TrackTotal)
        }
        if !matches!(self.disc_number, ValueEdit::Keep)
            || !matches!(self.disc_total, ValueEdit::Keep)
        {
            out.push(T::DiscNumber)
        }
        if !matches!(self.bpm, ValueEdit::Keep) {
            out.push(T::BPM)
        }
        if !self.artwork.is_empty() {
            out.push(T::AlbumArt)
        }
        out
    }
    pub(crate) fn apply_tag(
        &self,
        tag: &mut onetagger_tag::Tag,
        format: &onetagger_tag::AudioFileFormat,
    ) -> Result<()> {
        native::apply_tag(tag, format, self)
    }
    pub(crate) fn apply_asf(&self, tag: &mut crate::asf::AsfTag) -> Result<()> {
        native::apply_asf(tag, self)
    }
    pub fn project_release_date(&self, info: &mut cuerithm_media::MediaInfo) {
        if !matches!(self.release_date, ValueEdit::Keep) {
            for key in [
                "releasedate",
                "release_date",
                "release date",
                "release_time",
                "releasetime",
                "tdrl",
                "date",
                "year",
            ] {
                info.tags.remove(key);
            }
            if let ValueEdit::Set { value } = &self.release_date {
                let text = match (value.month, value.day) {
                    (Some(m), Some(d)) => format!("{:04}-{m:02}-{d:02}", value.year),
                    (Some(m), None) => format!("{:04}-{m:02}", value.year),
                    _ => format!("{:04}", value.year),
                };
                info.tags.insert("release_date".into(), text);
            }
        }
    }
}

impl MetadataPatch {
    pub fn is_noop(&self) -> bool {
        self.text.values().all(|e| matches!(e, TextEdit::Keep))
            && matches!(self.release_date, ValueEdit::Keep)
            && matches!(self.original_date, ValueEdit::Keep)
            && matches!(self.track_number, ValueEdit::Keep)
            && matches!(self.track_total, ValueEdit::Keep)
            && matches!(self.disc_number, ValueEdit::Keep)
            && matches!(self.disc_total, ValueEdit::Keep)
            && matches!(self.bpm, ValueEdit::Keep)
            && self.artwork_order.is_none()
            && self.restored_ratings.is_empty()
            && self
                .ratings
                .iter()
                .all(|e| matches!(e.value, ValueEdit::Keep))
            && self
                .comments
                .iter()
                .all(|e| matches!(e.value, TextEdit::Keep))
            && self.artwork.is_empty()
    }
}
pub fn validate_metadata_artwork(bytes: Vec<u8>) -> Result<(String, Vec<u8>)> {
    let art = crate::image::Artwork::validate(bytes)?;
    Ok((art.mime.into(), art.bytes))
}
