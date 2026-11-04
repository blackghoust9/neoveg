//! The pitch of audio events: a change in semitones, with the formants kept where they were or
//! moved (see `neoveg_common::pitch_change`). Setting it for a selection is one
//! [`EditCommand::ReplaceEvents`], so it is one undo step. What the numbers sound like is
//! `neoveg_audio::pitch_shift`.

use neoveg_common::pitch_change::PitchChange;

use crate::command::{EditCommand, TrackChange};
use crate::selection::EventRef;
use crate::timeline::{Timeline, TrackId, TrackKind};

/// Gives every unlocked audio event among `events` the pitch change `change` (made safe first).
/// Video events and locked events are left alone; `None` when nothing changes.
pub fn set(timeline: &Timeline, events: &[EventRef], change: &PitchChange) -> Option<EditCommand> {
    apply(timeline, events, |_| *change)
}

/// Moves the pitch of every unlocked audio event among `events` by `semitones` more (negative: down),
/// keeping each event's formant settings; `None` when nothing changes.
pub fn nudge(timeline: &Timeline, events: &[EventRef], semitones: f64) -> Option<EditCommand> {
    if !semitones.is_finite() || semitones == 0.0 {
        return None;
    }
    apply(timeline, events, |old| PitchChange {
        semitones: snap_to_cent(old.semitones + semitones),
        ..*old
    })
}

/// Puts the pitch of every unlocked audio event among `events` at `semitones` (`0.0` takes it back to
/// the original pitch), keeping each event's formant settings; `None` when nothing changes.
pub fn set_semitones(
    timeline: &Timeline,
    events: &[EventRef],
    semitones: f64,
) -> Option<EditCommand> {
    if !semitones.is_finite() {
        return None;
    }
    apply(timeline, events, |old| PitchChange {
        semitones: snap_to_cent(semitones),
        ..*old
    })
}

/// `semitones` rounded to a ten-thousandth, so a hundred steps of a cent land on a whole semitone
/// instead of drifting.
fn snap_to_cent(semitones: f64) -> f64 {
    (semitones * 10_000.0).round() / 10_000.0
}

/// `true` when at least one of `events` is an unlocked audio event, the ones the pitch keys act on.
pub fn can_change(timeline: &Timeline, events: &[EventRef]) -> bool {
    events.iter().any(|(track_id, event_id)| {
        timeline.track(*track_id).is_some_and(|track| {
            track.kind == TrackKind::Audio
                && track.events.iter().any(|e| e.id == *event_id && !e.locked)
        })
    })
}

fn apply(
    timeline: &Timeline,
    events: &[EventRef],
    new_pitch: impl Fn(&PitchChange) -> PitchChange,
) -> Option<EditCommand> {
    let mut changes: Vec<TrackChange> = Vec::new();
    let mut tracks: Vec<TrackId> = events.iter().map(|(t, _)| *t).collect();
    tracks.sort_unstable();
    tracks.dedup();
    for track_id in tracks {
        let Some(track) = timeline.track(track_id) else {
            continue;
        };
        if track.kind != TrackKind::Audio {
            continue;
        }
        let mut after = track.events.clone();
        let mut changed = false;
        for event in &mut after {
            if !events.contains(&(track_id, event.id)) || event.locked {
                continue;
            }
            let wanted = new_pitch(&event.pitch).sanitized();
            if wanted != event.pitch {
                event.pitch = wanted;
                changed = true;
            }
        }
        if changed {
            changes.push(TrackChange {
                track_id,
                before: track.events.clone(),
                after,
            });
        }
    }
    (!changes.is_empty()).then_some(EditCommand::ReplaceEvents { changes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;
    use crate::timeline::TrackEvent;

    #[test]
    fn a_hundred_cents_make_a_semitone_without_drift() {
        let (mut tl, audio, _video, _locked) = timeline();
        let mut history = History::new();
        for _ in 0..100 {
            let command = nudge(&tl, &[audio], 0.01).expect("a cent moves the pitch");
            assert!(history.execute(&mut tl, command));
        }
        assert_eq!(pitch_of(&tl, audio).semitones, 1.0);
        // The octave keys stop at the limit instead of passing it
        let up = nudge(&tl, &[audio], 12.0).unwrap();
        assert!(history.execute(&mut tl, up));
        let up = nudge(&tl, &[audio], 12.0).unwrap();
        assert!(history.execute(&mut tl, up));
        assert_eq!(pitch_of(&tl, audio).semitones, 24.0);
        assert!(nudge(&tl, &[audio], 1.0).is_none(), "already at the top");
    }

    #[test]
    fn the_pitch_goes_back_to_zero_keeping_the_formant_settings() {
        let (mut tl, audio, video, locked) = timeline();
        let mut history = History::new();
        let shifted = PitchChange {
            semitones: 5.0,
            preserve_formants: true,
            formant_shift: -2.0,
        };
        let set_all = set(&tl, &[audio, locked], &shifted).unwrap();
        assert!(history.execute(&mut tl, set_all));
        let reset = set_semitones(&tl, &[audio, video, locked], 0.0).unwrap();
        assert!(history.execute(&mut tl, reset));
        let back = pitch_of(&tl, audio);
        assert_eq!(back.semitones, 0.0);
        assert!(back.preserve_formants && back.formant_shift == -2.0);
        assert!(
            set_semitones(&tl, &[audio], 0.0).is_none(),
            "nothing to reset"
        );
        assert!(set_semitones(&tl, &[audio], f64::NAN).is_none());
        // One undo step takes the pitch back to 5
        assert!(history.undo(&mut tl));
        assert_eq!(pitch_of(&tl, audio).semitones, 5.0);
    }

    #[test]
    fn the_keys_act_only_when_an_unlocked_audio_event_is_selected() {
        let (tl, audio, video, locked) = timeline();
        assert!(can_change(&tl, &[audio]));
        assert!(can_change(&tl, &[video, audio]));
        assert!(!can_change(&tl, &[video]));
        assert!(!can_change(&tl, &[locked]));
        assert!(!can_change(&tl, &[]));
    }

    fn timeline() -> (Timeline, EventRef, EventRef, EventRef) {
        let mut tl = Timeline::new();
        let mut audio = tl.new_track(TrackKind::Audio);
        let mut video = tl.new_track(TrackKind::Video);
        let mut refs = Vec::new();
        for track in [&mut audio, &mut video] {
            let event = TrackEvent {
                id: tl.alloc_id(),
                duration: 4.0,
                media_id: Some(1),
                ..TrackEvent::default()
            };
            refs.push((track.id, event.id));
            track.events.push(event);
        }
        let mut locked = TrackEvent {
            id: tl.alloc_id(),
            start: 5.0,
            duration: 2.0,
            media_id: Some(1),
            ..TrackEvent::default()
        };
        locked.locked = true;
        let locked_ref = (audio.id, locked.id);
        audio.events.push(locked);
        tl.tracks.push(audio);
        tl.tracks.push(video);
        (tl, refs[0], refs[1], locked_ref)
    }

    fn pitch_of(tl: &Timeline, (track, event): EventRef) -> PitchChange {
        tl.track(track)
            .unwrap()
            .events
            .iter()
            .find(|e| e.id == event)
            .unwrap()
            .pitch
    }

    #[test]
    fn the_pitch_is_set_for_audio_events_in_one_undo_step_and_video_and_locked_ones_are_left_alone()
    {
        let (mut tl, audio, video, locked) = timeline();
        let original = tl.clone();
        let change = PitchChange {
            semitones: 5.0,
            preserve_formants: true,
            formant_shift: -2.0,
        };
        let command = set(&tl, &[audio, video, locked], &change).unwrap();
        let mut history = History::new();
        assert!(history.execute(&mut tl, command));
        assert_eq!(pitch_of(&tl, audio), change);
        assert_eq!(
            pitch_of(&tl, video),
            PitchChange::default(),
            "video has no pitch"
        );
        assert_eq!(pitch_of(&tl, locked), PitchChange::default(), "locked");
        // The same again is not an edit
        assert!(set(&tl, &[audio], &change).is_none());
        assert!(history.undo(&mut tl));
        assert_eq!(tl, original);
    }

    #[test]
    fn nudging_adds_semitones_and_keeps_the_formant_settings() {
        let (mut tl, audio, _, _) = timeline();
        let mut history = History::new();
        let kept = PitchChange {
            semitones: 2.0,
            preserve_formants: true,
            formant_shift: 1.5,
        };
        let command = set(&tl, &[audio], &kept).unwrap();
        assert!(history.execute(&mut tl, command));
        let command = nudge(&tl, &[audio], 1.0).unwrap();
        assert!(history.execute(&mut tl, command));
        let now = pitch_of(&tl, audio);
        assert_eq!(now.semitones, 3.0);
        assert!(now.preserve_formants && now.formant_shift == 1.5);
        // A cent is a hundredth of a semitone, and zero or nonsense is no edit
        let command = nudge(&tl, &[audio], 0.01).unwrap();
        assert!(history.execute(&mut tl, command));
        assert!((pitch_of(&tl, audio).semitones - 3.01).abs() < 1e-12);
        assert!(nudge(&tl, &[audio], 0.0).is_none() && nudge(&tl, &[audio], f64::NAN).is_none());
    }

    #[test]
    fn a_pitch_out_of_range_is_held_at_the_limit_and_stored_values_stay_clean() {
        let (mut tl, audio, _, _) = timeline();
        let wild = PitchChange {
            semitones: 400.0,
            preserve_formants: false,
            formant_shift: 9.0,
        };
        let command = set(&tl, &[audio], &wild).unwrap();
        let mut history = History::new();
        assert!(history.execute(&mut tl, command));
        let stored = pitch_of(&tl, audio);
        assert_eq!(stored.semitones, neoveg_common::pitch_change::MAX_SEMITONES);
        assert_eq!(stored.formant_shift, 0.0);
        // The project file leaves the default out and keeps the rest
        let json = serde_json::to_string(&tl).unwrap();
        assert!(json.contains("\"pitch\""));
        let reset = set(&tl, &[audio], &PitchChange::default()).unwrap();
        assert!(history.execute(&mut tl, reset));
        assert!(!serde_json::to_string(&tl).unwrap().contains("\"pitch\""));
    }
}
