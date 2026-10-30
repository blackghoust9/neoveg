//! Copy and paste of event attributes: the properties of one event applied to others, all of them
//! or only the ones picked. The attributes are the event switches, the pitch handling of audio, the
//! playback rate of video, the video effects, the Pan/Crop, the velocity envelope and the
//! transitions. Position, length, fades, name and media never move.
//!
//! Everything here is a pure function; the interface keeps the copied event on the clipboard and
//! shows the dialog that picks the attributes.

use crate::command::{EditCommand, TrackChange};
use crate::selection::EventRef;
use crate::timeline::{Timeline, TrackEvent, TrackId, TrackKind};

/// Which attributes to paste.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Attributes {
    /// Mute, lock, loop and, on audio, invert phase and normalize.
    pub switches: bool,
    /// Audio: keep the pitch when the speed changes.
    pub pitch: bool,
    /// Video: the playback rate of the event.
    pub rate: bool,
    /// Video: the effect chain of the event.
    pub effects: bool,
    /// Video: the Pan/Crop box and its keyframes.
    pub pan_crop: bool,
    /// Video: the velocity envelope.
    pub velocity: bool,
    /// Video: the transitions in and out.
    pub transitions: bool,
    /// Audio: the volume envelope.
    pub volume: bool,
}

impl Attributes {
    /// Every attribute (*Paste event attributes*).
    pub const ALL: Attributes = Attributes {
        switches: true,
        pitch: true,
        rate: true,
        effects: true,
        pan_crop: true,
        velocity: true,
        transitions: true,
        volume: true,
    };

    /// None of them (the start of the *selective* dialog).
    pub const NONE: Attributes = Attributes {
        switches: false,
        pitch: false,
        rate: false,
        effects: false,
        pan_crop: false,
        velocity: false,
        transitions: false,
        volume: false,
    };

    /// `true` when nothing is picked.
    pub fn is_empty(&self) -> bool {
        *self == Self::NONE
    }
}

/// Copies the picked attributes of `source` onto `target` (an event of a `kind` track). Attributes
/// that belong to the other kind of track are left alone. `true` when something changed.
pub fn apply(
    source: &TrackEvent,
    target: &mut TrackEvent,
    kind: TrackKind,
    set: Attributes,
) -> bool {
    let before = target.clone();
    let video = kind == TrackKind::Video;
    if set.switches {
        target.muted = source.muted;
        target.locked = source.locked;
        target.looping = source.looping;
        if video {
            target.resample = source.resample;
        }
        if !video {
            target.invert_phase = source.invert_phase;
            target.normalize = source.normalize;
            target.normalize_gain = source.normalize_gain;
        }
    }
    if set.pitch && !video {
        target.preserve_pitch = source.preserve_pitch;
    }
    if set.volume && !video {
        target.volume_envelope = source.volume_envelope.clone();
    }
    if video {
        if set.rate {
            target.playback_rate = source.playback_rate;
        }
        if set.effects {
            target.video_effects = source.video_effects.clone();
        }
        if set.pan_crop {
            target.pan_crop = source.pan_crop.clone();
        }
        if set.velocity {
            target.velocity = source.velocity.clone();
        }
        if set.transitions {
            target.transition_in = source.transition_in.clone();
            target.transition_out = source.transition_out.clone();
            target.transition_progress = source.transition_progress.clone();
        }
    }
    *target != before
}

/// What pasting does to `targets`: the command (one undo step) and how many events it changes, or
/// `None` when nothing would change. A locked target is not edited, and neither is the source.
pub fn paste_command(
    timeline: &Timeline,
    source: &TrackEvent,
    targets: &[EventRef],
    set: Attributes,
) -> Option<(EditCommand, usize)> {
    if set.is_empty() {
        return None;
    }
    let mut changes: Vec<TrackChange> = Vec::new();
    let mut count = 0;
    let mut tracks: Vec<TrackId> = targets.iter().map(|(t, _)| *t).collect();
    tracks.sort_unstable();
    tracks.dedup();
    for track_id in tracks {
        let Some(track) = timeline.track(track_id) else {
            continue;
        };
        let mut after = track.events.clone();
        let mut changed = false;
        for event in &mut after {
            if event.id == source.id || event.locked || !targets.contains(&(track_id, event.id)) {
                continue;
            }
            if apply(source, event, track.kind, set) {
                changed = true;
                count += 1;
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
    (!changes.is_empty()).then_some((EditCommand::ReplaceEvents { changes }, count))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;
    use crate::envelope::{AutomationTarget, TrackEnvelope};
    use crate::timeline::TrackEffect;
    use neoveg_common::pan_crop::{PanCrop, PanCropFrame};

    /// Two video tracks and an audio one; the first event, rich in attributes, is the source.
    fn setup() -> (Timeline, History, TrackEvent, Vec<EventRef>) {
        let mut tl = Timeline::new();
        let mut h = History::new();
        let mut refs = Vec::new();
        let mut source = None;
        for (i, kind) in [TrackKind::Video, TrackKind::Video, TrackKind::Audio]
            .into_iter()
            .enumerate()
        {
            let track = tl.new_track(kind);
            let tid = track.id;
            assert!(h.execute(&mut tl, EditCommand::AddTrack { track }));
            let id = tl.alloc_id();
            let mut event = TrackEvent {
                id,
                media_id: Some(1),
                name: format!("e{i}"),
                start: 1.0 + i as f64,
                duration: 4.0,
                ..TrackEvent::default()
            };
            if i == 0 {
                event.muted = true;
                event.looping = true;
                event.playback_rate = 2.0;
                event.preserve_pitch = false;
                event.invert_phase = true;
                event.video_effects = vec![TrackEffect::new("vignette")];
                let mut pc = PanCrop::default();
                let frame = PanCropFrame {
                    x_center: 0.4,
                    ..PanCropFrame::FULL
                };
                pc.upsert_key(0.0, frame, 1e-3);
                event.pan_crop = pc;
                event.velocity = Some(TrackEnvelope::new(AutomationTarget::EventVelocity, 0.5));
                event.transition_in = Some(TrackEffect::new("iris"));
                event.transition_out = Some(TrackEffect::new("blinds"));
                source = Some(event.clone());
            } else {
                refs.push((tid, id));
            }
            assert!(h.execute(
                &mut tl,
                EditCommand::AddEvent {
                    track_id: tid,
                    event
                }
            ));
        }
        (tl, h, source.unwrap(), refs)
    }

    fn event(tl: &Timeline, r: EventRef) -> TrackEvent {
        tl.track(r.0)
            .unwrap()
            .events
            .iter()
            .find(|e| e.id == r.1)
            .unwrap()
            .clone()
    }

    #[test]
    fn everything_goes_to_a_video_event_but_position_length_name_and_media_stay() {
        let (mut tl, mut h, source, refs) = setup();
        let before = event(&tl, refs[0]);
        let (cmd, n) = paste_command(&tl, &source, &refs[..1], Attributes::ALL).unwrap();
        assert_eq!(n, 1);
        assert!(h.execute(&mut tl, cmd));
        let after = event(&tl, refs[0]);
        assert!(after.muted && after.looping);
        assert_eq!(after.playback_rate, 2.0);
        assert_eq!(after.video_effects, source.video_effects);
        assert_eq!(after.pan_crop, source.pan_crop);
        assert_eq!(after.velocity, source.velocity);
        assert_eq!(after.transition_in, source.transition_in);
        assert_eq!(after.transition_out, source.transition_out);
        // Audio-only attributes do not reach a video event
        assert!(!after.invert_phase);
        assert!(after.preserve_pitch);
        // What belongs to the event stays
        assert_eq!(
            (after.start, after.duration, after.id),
            (before.start, before.duration, before.id)
        );
        assert_eq!(
            (after.name.as_str(), after.media_id),
            (before.name.as_str(), before.media_id)
        );
        // One undo step puts it all back
        assert!(h.undo(&mut tl));
        assert_eq!(event(&tl, refs[0]), before);
    }

    #[test]
    fn only_the_picked_attributes_move_and_an_audio_event_takes_the_audio_ones() {
        let (mut tl, mut h, source, refs) = setup();
        let only_effects = Attributes {
            effects: true,
            ..Attributes::NONE
        };
        let (cmd, _) = paste_command(&tl, &source, &refs[..1], only_effects).unwrap();
        assert!(h.execute(&mut tl, cmd));
        let v = event(&tl, refs[0]);
        assert_eq!(v.video_effects, source.video_effects);
        assert!(!v.muted && v.velocity.is_none() && v.playback_rate == 1.0);
        // The audio event gets the switches and the pitch handling, no video attribute
        let (cmd, _) = paste_command(&tl, &source, &refs[1..], Attributes::ALL).unwrap();
        assert!(h.execute(&mut tl, cmd));
        let a = event(&tl, refs[1]);
        assert!(a.muted && a.looping && a.invert_phase && !a.preserve_pitch);
        assert!(a.video_effects.is_empty() && a.velocity.is_none() && a.playback_rate == 1.0);
    }

    #[test]
    fn nothing_picked_nothing_to_change_a_locked_target_and_the_source_itself_give_no_command() {
        let (mut tl, mut h, source, refs) = setup();
        assert!(paste_command(&tl, &source, &refs, Attributes::NONE).is_none());
        // The source is never a target of its own paste
        let own = (tl.tracks[0].id, source.id);
        assert!(paste_command(&tl, &source, &[own], Attributes::ALL).is_none());
        // A locked event is left as it is
        tl.track_mut(refs[0].0).unwrap().events[0].locked = true;
        let (_, n) = paste_command(&tl, &source, &refs, Attributes::ALL).unwrap();
        assert_eq!(n, 1, "only the audio event");
        // Pasting twice changes nothing the second time
        let (cmd, _) = paste_command(&tl, &source, &refs[1..], Attributes::ALL).unwrap();
        assert!(h.execute(&mut tl, cmd));
        assert!(paste_command(&tl, &source, &refs[1..], Attributes::ALL).is_none());
    }
}
