//! Command markers (manual v21, "Inserting command markers"): a command, a parameter and a comment
//! pinned to a time of the project. A command marker says when an instruction (show a headline, go
//! to an address, display a caption...) is to happen in a streaming file; here it is kept in the
//! project, drawn on the ruler and edited from a dialog.
//!
//! Like the other marks they live in the [`Timeline`], so each change is one undo step, and the
//! functions are pure: they build an [`EditCommand`] or answer a question.

use serde::{Deserialize, Serialize};

use crate::command::EditCommand;
use crate::timeline::{Timeline, TIME_EPSILON};

pub type CommandId = u64;

/// The commands the dialog suggests; any other text is a custom command.
pub const SUGGESTED: [&str; 4] = ["URL", "Text", "Headline", "Caption"];

/// Longest text of a command, a parameter or a comment, in characters.
pub const MAX_TEXT: usize = 2048;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandMarker {
    pub id: CommandId,
    pub time: f64,
    /// What to do (`URL`, `Text`, or a command of the user's own).
    pub command: String,
    /// The argument of the command: the address of a page, the text to show...
    #[serde(default)]
    pub parameter: String,
    /// A reminder for whoever edits the project.
    #[serde(default)]
    pub comment: String,
}

/// The text cut to [`MAX_TEXT`] characters, without line breaks at the ends.
fn clean(text: &str) -> String {
    text.trim().chars().take(MAX_TEXT).collect()
}

/// The marker made safe: a time that is a number at or after zero, texts that are not too long.
pub fn sanitized(marker: &CommandMarker) -> CommandMarker {
    CommandMarker {
        id: marker.id,
        time: if marker.time.is_finite() {
            marker.time.max(0.0)
        } else {
            0.0
        },
        command: clean(&marker.command),
        parameter: clean(&marker.parameter),
        comment: clean(&marker.comment),
    }
}

fn replace(timeline: &Timeline, mut after: Vec<CommandMarker>) -> Option<EditCommand> {
    after.sort_by(|a, b| a.time.total_cmp(&b.time).then(a.id.cmp(&b.id)));
    (after != timeline.command_markers).then(|| EditCommand::ReplaceCommandMarkers {
        before: timeline.command_markers.clone(),
        after,
    })
}

/// Puts a command marker at `time`. `None` when the command is empty (a marker with no command
/// does nothing).
pub fn insert(
    timeline: &mut Timeline,
    time: f64,
    command: &str,
    parameter: &str,
    comment: &str,
) -> Option<(CommandId, EditCommand)> {
    let command = clean(command);
    if command.is_empty() {
        return None;
    }
    let id = timeline.alloc_id();
    let marker = sanitized(&CommandMarker {
        id,
        time,
        command,
        parameter: parameter.to_owned(),
        comment: comment.to_owned(),
    });
    let mut after = timeline.command_markers.clone();
    after.push(marker);
    replace(timeline, after).map(|command| (id, command))
}

/// Gives the marker `marker.id` the values of `marker`. `None` when it does not exist, the command
/// is empty or nothing changes.
pub fn update(timeline: &Timeline, marker: &CommandMarker) -> Option<EditCommand> {
    let marker = sanitized(marker);
    if marker.command.is_empty() {
        return None;
    }
    let mut after = timeline.command_markers.clone();
    let slot = after.iter_mut().find(|m| m.id == marker.id)?;
    *slot = marker;
    replace(timeline, after)
}

/// Takes the marker away; `None` when it does not exist.
pub fn remove(timeline: &Timeline, id: CommandId) -> Option<EditCommand> {
    let after: Vec<CommandMarker> = timeline
        .command_markers
        .iter()
        .filter(|m| m.id != id)
        .cloned()
        .collect();
    (after.len() != timeline.command_markers.len())
        .then(|| replace(timeline, after))
        .flatten()
}

/// The marker whose tag is at `time`, within `tolerance` seconds (the nearest one).
pub fn hit_test(timeline: &Timeline, time: f64, tolerance: f64) -> Option<CommandId> {
    timeline
        .command_markers
        .iter()
        .map(|m| ((m.time - time).abs(), m.id))
        .filter(|(distance, _)| *distance <= tolerance.max(TIME_EPSILON))
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id)
}

/// The marker with `id`.
pub fn get(timeline: &Timeline, id: CommandId) -> Option<&CommandMarker> {
    timeline.command_markers.iter().find(|m| m.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;

    fn run(
        tl: &mut Timeline,
        history: &mut History,
        build: impl FnOnce(&Timeline) -> Option<EditCommand>,
    ) {
        let command = build(tl).expect("an edit");
        assert!(history.execute(tl, command));
    }

    #[test]
    fn a_command_marker_is_inserted_edited_and_removed_as_undo_steps_and_kept_in_time_order() {
        let mut tl = Timeline::new();
        let mut original = tl.clone();
        let mut history = History::new();
        let (late, command) = insert(&mut tl, 8.0, "URL", "https://example.org", "page").unwrap();
        run(&mut tl, &mut history, |_| Some(command));
        let (early, command) = insert(&mut tl, 2.0, "Text", "Olá", "").unwrap();
        run(&mut tl, &mut history, |_| Some(command));
        let ids: Vec<CommandId> = tl.command_markers.iter().map(|m| m.id).collect();
        assert_eq!(ids, vec![early, late], "ordered by time");
        // Edit: new text and a new time that swaps the order
        let mut edited = get(&tl, early).unwrap().clone();
        edited.time = 9.0;
        edited.parameter = "Tchau".into();
        run(&mut tl, &mut history, |t| update(t, &edited));
        assert_eq!(tl.command_markers[1].id, early);
        assert_eq!(get(&tl, early).unwrap().parameter, "Tchau");
        assert!(update(&tl, &edited).is_none(), "the same again is no edit");
        // Remove
        run(&mut tl, &mut history, |t| remove(t, late));
        assert_eq!(tl.command_markers.len(), 1);
        assert!(remove(&tl, late).is_none());
        // Everything undoes, one step at a time
        for _ in 0..4 {
            assert!(history.undo(&mut tl));
        }
        original.adopt_ids_of(&tl);
        assert_eq!(tl, original);
    }

    #[test]
    fn an_empty_command_and_odd_values_are_refused_or_made_safe() {
        let mut tl = Timeline::new();
        assert!(insert(&mut tl, 1.0, "   ", "x", "").is_none(), "no command");
        let (id, command) = insert(&mut tl, f64::NAN, "  Text  ", &"x".repeat(5000), "c").unwrap();
        let mut history = History::new();
        assert!(history.execute(&mut tl, command));
        let marker = get(&tl, id).unwrap();
        assert_eq!(marker.time, 0.0);
        assert_eq!(marker.command, "Text");
        assert_eq!(marker.parameter.chars().count(), MAX_TEXT);
        let mut blank = marker.clone();
        blank.command = String::new();
        assert!(update(&tl, &blank).is_none());
        let mut ghost = marker.clone();
        ghost.id = 999;
        assert!(update(&tl, &ghost).is_none(), "no such marker");
    }

    #[test]
    fn the_tag_under_the_pointer_is_found_within_the_tolerance_and_the_nearest_wins() {
        let mut tl = Timeline::new();
        let mut history = History::new();
        let (a, c) = insert(&mut tl, 5.0, "Text", "", "").unwrap();
        run(&mut tl, &mut history, |_| Some(c));
        let (b, c) = insert(&mut tl, 5.2, "Text", "", "").unwrap();
        run(&mut tl, &mut history, |_| Some(c));
        assert_eq!(hit_test(&tl, 5.01, 0.3), Some(a));
        assert_eq!(hit_test(&tl, 5.19, 0.3), Some(b));
        assert_eq!(hit_test(&tl, 7.0, 0.3), None);
        assert_eq!(hit_test(&Timeline::new(), 1.0, 1.0), None);
    }

    #[test]
    fn the_markers_are_saved_with_the_project_and_an_old_project_has_none() {
        let mut tl = Timeline::new();
        let mut history = History::new();
        let (_, c) = insert(&mut tl, 3.0, "URL", "https://example.org", "x").unwrap();
        run(&mut tl, &mut history, |_| Some(c));
        let json = serde_json::to_string(&tl).unwrap();
        let back: Timeline = serde_json::from_str(&json).unwrap();
        assert_eq!(back.command_markers, tl.command_markers);
        let clean = serde_json::to_string(&Timeline::new()).unwrap();
        assert!(!clean.contains("command_markers"));
        let old: Timeline = serde_json::from_str(&clean).unwrap();
        assert!(old.command_markers.is_empty());
    }
}
