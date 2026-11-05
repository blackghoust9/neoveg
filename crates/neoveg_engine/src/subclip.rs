//! *Subclipes* and *Reverter evento* (manual v21, *Creating subclips*; *Reverse*): a subclip is a part of
//! a media file written as a media file of its own and added to the media pool; reversing an event
//! writes the part it plays backwards and puts the event on that file.
//!
//! The work on the files is done elsewhere (`neoveg_audio::subclip` for sound, `neoveg_video::subclip`
//! for files with a picture). Here is what is pure: which part of the source an event plays, how the
//! new file is called, and the edit that moves events to the new media.

use std::path::{Path, PathBuf};

use crate::command::{EditCommand, TrackChange};
use crate::edit;
use crate::selection::EventRef;
use crate::timeline::{Timeline, TrackEvent, TrackKind};

/// The longest part of a video that is reversed: `ffmpeg` keeps the whole part in memory to play it
/// backwards (sound alone has no such limit here).
pub const MAX_REVERSE_VIDEO_SECONDS: f64 = 60.0;

/// A part of a media file, in seconds of the file.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Section {
    pub start: f64,
    pub length: f64,
}

/// The part of its media an event plays: from its in point, as much as its length at its playback
/// rate takes. `None` for an event with no media (an adjustment event, or an empty one) or no length.
pub fn section_of(event: &TrackEvent) -> Option<Section> {
    event.media_id?;
    if event.adjustment || !event.duration.is_finite() || event.duration <= 0.0 {
        return None;
    }
    let length = event.duration * edit::clamp_playback_rate(event.playback_rate);
    Some(Section {
        start: event.trim_in.max(0.0),
        length,
    })
}

/// A file name in `dir` for the subclip of `section` of `input` that does not exist yet:
/// `name_1.50s-4.00s.wav` (`_reverso` before the extension when it plays backwards), then `_2`, `_3`…
/// `exists` says whether a path is taken.
pub fn output_path(
    input: &Path,
    dir: &Path,
    section: Section,
    reverse: bool,
    extension: &str,
    exists: impl Fn(&Path) -> bool,
) -> PathBuf {
    let stem: String = input
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "midia".into())
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let tag = if reverse { "_reverso" } else { "" };
    let base = format!(
        "{stem}_{:.2}s-{:.2}s{tag}",
        section.start,
        section.start + section.length
    );
    let first = dir.join(format!("{base}.{extension}"));
    if !exists(&first) {
        return first;
    }
    (2..)
        .map(|n| dir.join(format!("{base}_{n}.{extension}")))
        .find(|p| !exists(p))
        .expect("an unbounded range finds a free name")
}

/// The events among `refs` that play the same part of the same media as `event` (a video file has
/// its picture and its sound on two events that must change together).
pub fn same_section(timeline: &Timeline, refs: &[EventRef], event: EventRef) -> Vec<EventRef> {
    let Some(reference) = timeline
        .track(event.0)
        .and_then(|t| t.events.iter().find(|e| e.id == event.1))
    else {
        return Vec::new();
    };
    refs.iter()
        .copied()
        .filter(|&(track_id, event_id)| {
            timeline
                .track(track_id)
                .and_then(|t| t.events.iter().find(|e| e.id == event_id))
                .is_some_and(|e| {
                    !e.adjustment
                        && e.media_id == reference.media_id
                        && (e.trim_in - reference.trim_in).abs() < 1e-6
                        && (e.duration - reference.duration).abs() < 1e-6
                        && (e.playback_rate - reference.playback_rate).abs() < 1e-9
                })
        })
        .collect()
}

/// The edit that puts the unlocked events among `refs` on the media `media_id`, from its start (the
/// new file holds just the part they played). Position, length, rate, fades, effects and envelopes
/// stay. One undo step; `None` when no event changes.
pub fn move_to_media(timeline: &Timeline, refs: &[EventRef], media_id: u64) -> Option<EditCommand> {
    let mut changes = Vec::new();
    for track in &timeline.tracks {
        let mut after = track.events.clone();
        let mut changed = false;
        for event in &mut after {
            if refs.contains(&(track.id, event.id))
                && !event.locked
                && !track.locked
                && !event.adjustment
                && event.media_id.is_some()
            {
                event.media_id = Some(media_id);
                event.trim_in = 0.0;
                changed = true;
            }
        }
        if changed {
            changes.push(TrackChange {
                track_id: track.id,
                before: track.events.clone(),
                after,
            });
        }
    }
    (!changes.is_empty()).then_some(EditCommand::ReplaceEvents { changes })
}

/// Whether the event is on a video track (its media has a picture).
pub fn on_video_track(timeline: &Timeline, track_id: u64) -> bool {
    timeline
        .track(track_id)
        .is_some_and(|t| t.kind == TrackKind::Video)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;

    fn event(id: u64, trim_in: f64, duration: f64, rate: f64) -> TrackEvent {
        TrackEvent {
            id,
            media_id: Some(4),
            trim_in,
            duration,
            playback_rate: rate,
            ..Default::default()
        }
    }

    #[test]
    fn an_event_plays_the_part_of_its_media_its_length_and_rate_take() {
        let plain = section_of(&event(1, 1.5, 2.0, 1.0)).unwrap();
        assert_eq!(
            plain,
            Section {
                start: 1.5,
                length: 2.0
            }
        );
        let fast = section_of(&event(1, 0.0, 2.0, 2.0)).unwrap();
        assert_eq!(fast.length, 4.0, "twice the speed uses twice the source");
        let mut none = event(1, 0.0, 2.0, 1.0);
        none.media_id = None;
        assert!(section_of(&none).is_none());
        let mut adjust = event(1, 0.0, 2.0, 1.0);
        adjust.adjustment = true;
        assert!(section_of(&adjust).is_none());
        assert!(section_of(&event(1, 0.0, 0.0, 1.0)).is_none());
        assert_eq!(section_of(&event(1, -3.0, 1.0, 1.0)).unwrap().start, 0.0);
    }

    #[test]
    fn the_name_says_the_part_and_the_direction_and_never_takes_a_file_that_exists() {
        let dir = Path::new("sub");
        let s = Section {
            start: 1.5,
            length: 2.5,
        };
        assert_eq!(
            output_path(Path::new("a/meu filme.mp4"), dir, s, false, "mp4", |_| {
                false
            }),
            dir.join("meu_filme_1.50s-4.00s.mp4")
        );
        assert_eq!(
            output_path(Path::new("voz.wav"), dir, s, true, "wav", |_| false),
            dir.join("voz_1.50s-4.00s_reverso.wav")
        );
        let taken = [
            dir.join("voz_1.50s-4.00s.wav"),
            dir.join("voz_1.50s-4.00s_2.wav"),
        ];
        assert_eq!(
            output_path(Path::new("voz.wav"), dir, s, false, "wav", |p| taken
                .contains(&p.to_path_buf())),
            dir.join("voz_1.50s-4.00s_3.wav")
        );
    }

    fn timeline() -> (Timeline, u64, u64) {
        let mut tl = Timeline::new();
        let mut v = tl.new_track(TrackKind::Video);
        let mut a = tl.new_track(TrackKind::Audio);
        v.events = vec![event(1, 2.0, 3.0, 1.0)];
        a.events = vec![event(2, 2.0, 3.0, 1.0), event(3, 9.0, 1.0, 1.0)];
        let ids = (v.id, a.id);
        tl.tracks.extend([v, a]);
        (tl, ids.0, ids.1)
    }

    #[test]
    fn the_picture_and_the_sound_of_one_part_are_found_together_and_others_are_not() {
        let (tl, v, a) = timeline();
        let refs = [(v, 1), (a, 2), (a, 3)];
        let same = same_section(&tl, &refs, (v, 1));
        assert_eq!(same, vec![(v, 1), (a, 2)]);
        assert!(same_section(&tl, &refs, (a, 99)).is_empty());
    }

    #[test]
    fn events_move_to_the_new_media_from_its_start_in_one_undo_step_and_locked_ones_stay() {
        let (mut tl, v, a) = timeline();
        tl.track_mut(a).unwrap().events[0].locked = true;
        let command = move_to_media(&tl, &[(v, 1), (a, 2)], 77).unwrap();
        let mut h = History::new();
        assert!(h.execute(&mut tl, command));
        let moved = &tl.track(v).unwrap().events[0];
        assert_eq!(
            (moved.media_id, moved.trim_in, moved.duration),
            (Some(77), 0.0, 3.0)
        );
        let locked = &tl.track(a).unwrap().events[0];
        assert_eq!((locked.media_id, locked.trim_in), (Some(4), 2.0));
        assert!(
            move_to_media(&tl, &[(a, 2)], 77).is_none(),
            "nothing left to move"
        );
        assert!(h.undo(&mut tl));
        let back = &tl.track(v).unwrap().events[0];
        assert_eq!((back.media_id, back.trim_in), (Some(4), 2.0));
        assert!(on_video_track(&tl, v) && !on_video_track(&tl, a));
    }
}
