use crate::{CueKind, Operator, RuleValue, SmartTrack};
pub(crate) fn matches(track: &SmartTrack, op: Operator, value: &RuleValue) -> bool {
    use Operator::*;
    match op {
        CuepointAnyLabelIncludes | CuepointNoLabelIncludes => {
            let RuleValue::Text(query) = value else {
                unreachable!()
            };
            let found = if query.is_empty() {
                track.cues.iter().any(|c| {
                    c.label
                        .as_deref()
                        .is_none_or(|s| crate::text::trim(s).is_empty())
                })
            } else {
                let needle = crate::text::trim(query).to_lowercase();
                track.cues.iter().any(|c| {
                    c.label
                        .as_ref()
                        .is_some_and(|s| !s.is_empty() && s.to_lowercase().contains(&needle))
                })
            };
            if op == CuepointNoLabelIncludes {
                !found
            } else {
                found
            }
        }
        CuepointAnyColorEquals | CuepointNoColorEquals => {
            let RuleValue::Color(query) = value else {
                unreachable!()
            };
            let found = query.as_ref().is_some_and(|s| {
                !s.is_empty() && track.cues.iter().any(|c| c.color.as_ref() == Some(s))
            });
            if op == CuepointNoColorEquals {
                !found
            } else {
                found
            }
        }
        CuepointHasLoop => track.cues.iter().any(|c| c.kind == CueKind::Loop),
        CuepointHasNoLoop => !track.cues.iter().any(|c| c.kind == CueKind::Loop),
        CuepointHasActiveLoop => track
            .cues
            .iter()
            .any(|c| c.kind == CueKind::Loop && c.active_loop),
        CuepointHasNoActiveLoop => !track
            .cues
            .iter()
            .any(|c| c.kind == CueKind::Loop && c.active_loop),
        CuepointHasMemoryCue => track.cues.iter().any(|c| c.memory),
        CuepointHasNoMemoryCue => !track.cues.iter().any(|c| c.memory),
        _ => unreachable!(),
    }
}
