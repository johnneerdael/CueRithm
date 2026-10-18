use crate::{EvaluationContext, Operator, PlaylistReference, SmartTrack};
use anyhow::{Context, Result};
pub(crate) fn matches(
    track: &SmartTrack,
    context: &EvaluationContext,
    op: Operator,
    reference: &PlaylistReference,
) -> Result<bool> {
    let members = context
        .memberships
        .get(&reference.id)
        .with_context(|| format!("Playlist membership unavailable: {}", reference.id))?;
    let found = members.contains(&track.id);
    Ok(
        if matches!(
            op,
            Operator::PlaylistNotInSpecificPlaylist
                | Operator::PlaylistNotInSpecificPlaylistFolder
                | Operator::PlaylistNotInSpecificSmartlist
        ) {
            !found
        } else {
            found
        },
    )
}
