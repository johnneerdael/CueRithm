//! Explicit edits override catalog values before cleanup and query projection.
use cuerithm_autotag::{MetadataPatch, MetadataTextField as F, TextEdit, ValueEdit};
pub(crate) fn overlay(mut base: MetadataPatch, edit: &MetadataPatch) -> MetadataPatch {
    base.allow_id3_upgrade |= edit.allow_id3_upgrade;
    base.text.extend(
        edit.text
            .iter()
            .filter(|(_, edit)| !matches!(edit, TextEdit::Keep))
            .map(|(field, edit)| (*field, edit.clone())),
    );
    macro_rules! value {($($name:ident),*)=>{$(if !matches!(edit.$name,ValueEdit::Keep){base.$name=edit.$name.clone();})*};}
    value!(
        release_date,
        original_date,
        track_number,
        track_total,
        disc_number,
        disc_total,
        bpm
    );
    for rating in &edit.ratings {
        base.ratings.retain(|r| r.identity != rating.identity);
        base.ratings.push(rating.clone());
    }
    for comment in &edit.comments {
        base.comments.retain(|r| r.identity != comment.identity);
        base.comments.push(comment.clone());
    }
    for artwork in &edit.artwork {
        base.artwork.retain(|r| r.target != artwork.target);
        base.artwork.push(artwork.clone());
    }
    base
}
pub(crate) fn project_query(input: &mut cuerithm_autotag::TagInput, patch: &MetadataPatch) {
    for (field, edit) in &patch.text {
        let key = match field {
            F::Title => "title",
            F::Artist => "artist",
            F::Album => "album",
            F::AlbumArtist => "album_artist",
            F::Genre => "genre",
            F::Version => "version",
            F::Isrc => "isrc",
            F::Key => "key",
            _ => continue,
        };
        if matches!(edit, TextEdit::Keep) {
            continue;
        }
        let existing = input
            .media
            .tags
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.clone());
        input.media.tags.retain(|k, _| {
            !(k.eq_ignore_ascii_case(key) || key == "version" && k.eq_ignore_ascii_case("mix"))
        });
        let value = match edit {
            TextEdit::Set { values } => values.join("; "),
            TextEdit::Append { values } => existing
                .into_iter()
                .chain(values.iter().cloned())
                .collect::<Vec<_>>()
                .join("; "),
            _ => String::new(),
        };
        if !value.is_empty() {
            input.media.tags.insert(key.into(), value);
        }
    }
    patch.project_release_date(&mut input.media);
}
