//! The edit that changes what the user says about a media file (the Media Properties dialog): the
//! undoable command, and how the rest of the engine reads the values.

use neoveg_common::media_props::MediaProps;

use crate::command::EditCommand;
use crate::timeline::Timeline;

/// The properties of media `media_id`: what the user set, or the defaults.
pub fn props_of(timeline: &Timeline, media_id: u64) -> MediaProps {
    timeline
        .media_props
        .get(&media_id)
        .cloned()
        .unwrap_or_default()
}

/// The edit that gives media `media_id` the properties `props` (made safe first); `None` when they
/// are what it already has.
pub fn set(timeline: &Timeline, media_id: u64, props: &MediaProps) -> Option<EditCommand> {
    let props = props.sanitized();
    let before = timeline.media_props.get(&media_id).cloned();
    let after = (!props.is_default()).then_some(props);
    (before != after).then_some(EditCommand::SetMediaProps {
        media_id,
        before,
        after,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;
    use neoveg_common::media_props::{AlphaMode, FieldOrder};

    #[test]
    fn properties_are_set_changed_and_cleared_as_undo_steps() {
        let mut tl = Timeline::new();
        let original = tl.clone();
        let mut history = History::new();
        assert_eq!(props_of(&tl, 7), MediaProps::default());
        // Nothing changed: nothing to do
        assert!(set(&tl, 7, &MediaProps::default()).is_none());
        let props = MediaProps {
            substitute_fps: Some(25.0),
            field_order: FieldOrder::LowerFirst,
            alpha: AlphaMode::Premultiplied,
            ..MediaProps::default()
        };
        let command = set(&tl, 7, &props).unwrap();
        assert!(history.execute(&mut tl, command));
        assert_eq!(props_of(&tl, 7), props);
        assert!(set(&tl, 7, &props).is_none(), "the same again is no edit");
        // Back to the defaults removes the entry, so the project file stays clean
        let command = set(&tl, 7, &MediaProps::default()).unwrap();
        assert!(history.execute(&mut tl, command));
        assert!(tl.media_props.is_empty());
        assert!(history.undo(&mut tl));
        assert_eq!(props_of(&tl, 7), props);
        assert!(history.undo(&mut tl));
        assert_eq!(tl, original);
    }

    #[test]
    fn odd_values_are_made_safe_and_only_the_changed_media_is_written() {
        let mut tl = Timeline::new();
        let mut history = History::new();
        let wild = MediaProps {
            pixel_aspect: 50.0,
            timecode_start: Some(-3.0),
            ..MediaProps::default()
        };
        let command = set(&tl, 1, &wild).unwrap();
        assert!(history.execute(&mut tl, command));
        let stored = props_of(&tl, 1);
        assert_eq!(stored.pixel_aspect, 4.0);
        assert_eq!(stored.timecode_start, Some(0.0));
        assert!(!tl.media_props.contains_key(&2));
        let json = serde_json::to_string(&tl).unwrap();
        assert!(json.contains("media_props"));
        let back: Timeline = serde_json::from_str(&json).unwrap();
        assert_eq!(props_of(&back, 1), stored);
        // A project without the field reads as having none
        let clean = serde_json::to_string(&Timeline::new()).unwrap();
        assert!(!clean.contains("media_props"));
    }
}
