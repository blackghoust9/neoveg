//! *Renomear por metadados*: the undoable edit that gives the selected events names built from a
//! template (`neoveg_common::rename_pattern`). The events are numbered in timeline order, so
//! `{n}` follows the order in which they play.

use neoveg_common::rename_pattern::{apply_all, ItemMetadata};

use crate::command::EditCommand;
use crate::selection::EventRef;
use crate::timeline::Timeline;

/// One command that renames the events in `refs` (the ones that exist) to the names `template`
/// gives. `metadata(track, event)` supplies the date and camera of an event (the file behind it);
/// its `name` is overwritten with the current name of the event. `start` is the first `{n}`.
/// `Err` when the template is bad; `Ok(None)` when no name would change.
pub fn rename_command(
    timeline: &Timeline,
    refs: &[EventRef],
    template: &str,
    start: u32,
    metadata: impl Fn(&EventRef) -> ItemMetadata,
) -> Result<Option<EditCommand>, String> {
    let mut found: Vec<(EventRef, f64, String)> = refs
        .iter()
        .filter_map(|r| {
            let event = timeline.track(r.0)?.events.iter().find(|e| e.id == r.1)?;
            Some((*r, event.start, event.name.clone()))
        })
        .collect();
    found.sort_by(|a, b| a.1.total_cmp(&b.1));
    let metas: Vec<ItemMetadata> = found
        .iter()
        .map(|(r, _, name)| ItemMetadata {
            name: name.clone(),
            ..metadata(r)
        })
        .collect();
    let names = apply_all(template, &metas, start)?;
    let changes: Vec<_> = found
        .into_iter()
        .zip(names)
        .filter(|((_, _, before), after)| before != after)
        .map(|(((track, event), _, before), after)| (track, event, before, after))
        .collect();
    Ok((!changes.is_empty()).then_some(EditCommand::RenameEvents { changes }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;
    use crate::timeline::{TrackEvent, TrackKind};

    fn setup() -> (Timeline, Vec<EventRef>) {
        let mut tl = Timeline::new();
        let mut track = tl.new_track(TrackKind::Video);
        let tid = track.id;
        let mut refs = Vec::new();
        // Added out of order on purpose: the numbers follow the time on the timeline
        for (i, start) in [10.0, 0.0, 5.0].into_iter().enumerate() {
            let id = tl.alloc_id();
            track.events.push(TrackEvent {
                id,
                name: format!("clip{i}"),
                start,
                duration: 2.0,
                ..TrackEvent::default()
            });
            refs.push((tid, id));
        }
        tl.tracks.push(track);
        (tl, refs)
    }

    fn names(tl: &Timeline) -> Vec<String> {
        tl.tracks[0].events.iter().map(|e| e.name.clone()).collect()
    }

    #[test]
    fn events_get_numbered_in_timeline_order_and_it_undoes() {
        let (mut tl, refs) = setup();
        let meta = |_: &EventRef| ItemMetadata::from_unix("", 1_000_000_000, Some("CamA".into()));
        let cmd = rename_command(&tl, &refs, "{data}_{camera}_{n:2}", 1, meta)
            .unwrap()
            .expect("names change");
        let mut history = History::new();
        assert!(history.execute(&mut tl, cmd));
        assert_eq!(
            names(&tl),
            vec![
                "2001-09-09_CamA_03",
                "2001-09-09_CamA_01",
                "2001-09-09_CamA_02"
            ]
        );
        assert!(history.undo(&mut tl));
        assert_eq!(names(&tl), vec!["clip0", "clip1", "clip2"]);
    }

    #[test]
    fn a_bad_template_or_no_change_gives_no_command() {
        let (tl, refs) = setup();
        let meta = |_: &EventRef| ItemMetadata::default();
        assert!(rename_command(&tl, &refs, "{oops}", 1, meta).is_err());
        // {nome} keeps each name as it is
        assert_eq!(rename_command(&tl, &refs, "{nome}", 1, meta), Ok(None));
    }
}
