//! Event transition progress: a free curve that says how far the transition of a video event has
//! gone, instead of the gain of its fade.
//!
//! Without it the progress of a transition is the gain of the fade it sits on (`0.0`, the layer is
//! not there, to `1.0`, whole and untouched). With the curve (`TrackEvent::transition_progress`) the
//! progress at each moment is the value of the curve, so a transition can hold half way, go back or
//! run in steps. Its keyframes are in seconds from the start of the event, with the shapes of the
//! other envelopes. The curve drives the transition in when the event has one, otherwise the
//! transition out; the other transition keeps following its fade. Everything here is a pure
//! function; the preview and the renderer ask for the progress of each frame.

use crate::command::EditCommand;
use crate::envelope::{AutomationTarget, EnvelopePoint, EnvelopeShape, TrackEnvelope};
use crate::timeline::{EventId, Timeline, TrackId, TrackKind, TIME_EPSILON};

/// A new curve that grows from nothing to the whole picture in `ramp_seconds` (the length of the
/// fade it replaces, or a second when that is not known), as the fade did.
pub fn new_envelope(ramp_seconds: f64) -> TrackEnvelope {
    let ramp = if ramp_seconds.is_finite() && ramp_seconds > TIME_EPSILON {
        ramp_seconds
    } else {
        1.0
    };
    let mut env = TrackEnvelope::new(AutomationTarget::EventTransitionProgress, 0.0);
    env.points.push(EnvelopePoint {
        time: ramp,
        value: 1.0,
        shape: EnvelopeShape::Linear,
    });
    env
}

/// `true` when `envelope` takes over the progress: it is there, on, and has keyframes.
pub fn is_active(envelope: Option<&TrackEnvelope>) -> bool {
    envelope.is_some_and(|e| e.enabled && !e.points.is_empty())
}

/// The progress `0.0..=1.0` at `time` seconds into the event (`1.0`, no transition, for a value that
/// is not a number).
pub fn progress_at(envelope: &TrackEnvelope, time: f64) -> f32 {
    let value = envelope.value_at(time, 1.0);
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        1.0
    }
}

/// The curve seen from `offset` seconds into the event (the front cut off by a split or a trim): the
/// progress at the cut becomes the first keyframe and the later ones move back by `offset`.
pub fn from_offset(envelope: Option<&TrackEnvelope>, offset: f64) -> Option<TrackEnvelope> {
    let env = envelope?;
    let mut out = env.clone();
    let shape = env
        .points
        .iter()
        .rfind(|p| p.time <= offset)
        .map_or(EnvelopeShape::Linear, |p| p.shape);
    out.points = vec![EnvelopePoint {
        time: 0.0,
        value: progress_at(env, offset),
        shape,
    }];
    out.points.extend(
        env.points
            .iter()
            .filter(|p| p.time > offset + TIME_EPSILON)
            .map(|p| EnvelopePoint {
                time: p.time - offset,
                ..*p
            }),
    );
    Some(out)
}

/// The curve after the start of the event moves `delta` seconds while the picture stays where it
/// is on the timeline: later, the part cut off the front goes with it; earlier, the new head holds
/// the progress the event started with.
pub fn shift_start(envelope: &TrackEnvelope, delta: f64) -> TrackEnvelope {
    if delta >= 0.0 {
        return from_offset(Some(envelope), delta).unwrap_or_else(|| envelope.clone());
    }
    let head = -delta;
    let mut out = envelope.clone();
    out.points = vec![EnvelopePoint {
        time: 0.0,
        value: progress_at(envelope, 0.0),
        shape: EnvelopeShape::Linear,
    }];
    out.points
        .extend(envelope.points.iter().map(|p| EnvelopePoint {
            time: p.time + head,
            ..*p
        }));
    out
}

/// The progress curve of an event of a video track (`Some(None)` is none), or `None` when the event
/// does not exist, is not on a video track or is an adjustment event (it has no transitions).
pub fn get(
    timeline: &Timeline,
    track_id: TrackId,
    event_id: EventId,
) -> Option<Option<&TrackEnvelope>> {
    let track = timeline.track(track_id)?;
    if track.kind != TrackKind::Video {
        return None;
    }
    let event = track.events.iter().find(|e| e.id == event_id)?;
    if event.adjustment {
        return None;
    }
    Some(event.transition_progress.as_ref())
}

/// A command that puts `after` as the progress curve, or `None` when nothing changes. Keyframes are
/// kept sorted, from zero on, and their values between `0.0` and `1.0`.
pub fn set(
    timeline: &Timeline,
    track_id: TrackId,
    event_id: EventId,
    after: Option<TrackEnvelope>,
) -> Option<EditCommand> {
    let before = get(timeline, track_id, event_id)?.cloned();
    let mut after = after;
    if let Some(env) = after.as_mut() {
        env.target = AutomationTarget::EventTransitionProgress;
        for p in &mut env.points {
            p.time = if p.time.is_finite() {
                p.time.max(0.0)
            } else {
                0.0
            };
            p.value = if p.value.is_finite() {
                p.value.clamp(0.0, 1.0)
            } else {
                1.0
            };
        }
        env.sort_points();
    }
    (before != after).then_some(EditCommand::SetTransitionProgress {
        track_id,
        event_id,
        before,
        after,
    })
}

/// Lets a left-edge trim keep the progress where it is on events that have a progress curve: when the
/// start and the in point move together, the front of the curve is cut (or extended) by the same
/// amount, as one more command in a batch so undo puts both back. A slide, which moves the event over
/// the same part of the media, leaves the curve alone because it moves with the event. Other commands,
/// and events without an active curve, come back as they were.
pub fn keep_progress(timeline: &Timeline, command: EditCommand) -> EditCommand {
    match command {
        EditCommand::Batch(list) => EditCommand::Batch(
            list.into_iter()
                .map(|c| keep_progress(timeline, c))
                .collect(),
        ),
        EditCommand::EditEvent {
            from_track,
            to_track,
            event_id,
            old,
            new,
        } => {
            let envelope = timeline
                .track(from_track)
                .filter(|t| t.kind == TrackKind::Video)
                .and_then(|t| t.events.iter().find(|e| e.id == event_id))
                .and_then(|e| e.transition_progress.as_ref())
                .filter(|p| is_active(Some(p)))
                .cloned();
            let rate = crate::edit::clamp_playback_rate(old.playback_rate);
            let delta = new.start - old.start;
            let anchored = delta.abs() > TIME_EPSILON
                && (new.trim_in - old.trim_in - rate * delta).abs() <= 1e-6
                && (new.playback_rate - old.playback_rate).abs() < 1e-9;
            let Some(envelope) = envelope.filter(|_| anchored) else {
                return EditCommand::EditEvent {
                    from_track,
                    to_track,
                    event_id,
                    old,
                    new,
                };
            };
            let after = shift_start(&envelope, delta);
            EditCommand::Batch(vec![
                EditCommand::EditEvent {
                    from_track,
                    to_track,
                    event_id,
                    old,
                    new,
                },
                EditCommand::SetTransitionProgress {
                    track_id: to_track,
                    event_id,
                    before: Some(envelope),
                    after: Some(after),
                },
            ])
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;
    use crate::timeline::TrackEvent;

    const LIN: EnvelopeShape = EnvelopeShape::Linear;

    fn env(points: &[(f64, f32, EnvelopeShape)]) -> TrackEnvelope {
        let mut e = new_envelope(1.0);
        e.points = points
            .iter()
            .map(|&(time, value, shape)| EnvelopePoint { time, value, shape })
            .collect();
        e
    }

    fn video_event(duration: f64) -> (Timeline, History, TrackId, EventId) {
        let mut tl = Timeline::new();
        let mut h = History::new();
        let track = tl.new_track(TrackKind::Video);
        let tid = track.id;
        assert!(h.execute(&mut tl, EditCommand::AddTrack { track }));
        let eid = tl.alloc_id();
        let event = TrackEvent {
            id: eid,
            media_id: Some(1),
            start: 10.0,
            duration,
            trim_in: 2.0,
            ..TrackEvent::default()
        };
        assert!(h.execute(
            &mut tl,
            EditCommand::AddEvent {
                track_id: tid,
                event
            }
        ));
        (tl, h, tid, eid)
    }

    #[test]
    fn a_new_curve_grows_from_nothing_to_whole_like_the_fade_it_replaces() {
        let e = new_envelope(2.0);
        assert_eq!(progress_at(&e, 0.0), 0.0);
        assert!((progress_at(&e, 1.0) - 0.5).abs() < 1e-6);
        assert_eq!(progress_at(&e, 2.0), 1.0);
        assert_eq!(progress_at(&e, 9.0), 1.0, "whole after the last keyframe");
        assert_eq!(new_envelope(f64::NAN).points[1].time, 1.0);
        assert_eq!(new_envelope(-3.0).points[1].time, 1.0);
        assert!(!is_active(None));
        let mut off = new_envelope(1.0);
        off.enabled = false;
        assert!(!is_active(Some(&off)));
        assert!(is_active(Some(&e)));
    }

    #[test]
    fn the_progress_can_hold_go_back_and_step_and_a_bad_value_cannot_break_it() {
        // Up to half, a hold, then back down and up again
        let e = env(&[
            (0.0, 0.0, LIN),
            (1.0, 0.5, EnvelopeShape::Hold),
            (3.0, 0.5, LIN),
            (4.0, 0.2, LIN),
            (5.0, 1.0, LIN),
        ]);
        assert!((progress_at(&e, 0.5) - 0.25).abs() < 1e-6);
        assert_eq!(progress_at(&e, 2.0), 0.5, "held");
        assert!((progress_at(&e, 3.5) - 0.35).abs() < 1e-6, "going back");
        let mut bad = env(&[(0.0, 0.5, LIN)]);
        bad.points[0].value = f32::NAN;
        assert_eq!(progress_at(&bad, 1.0), 1.0);
        bad.points[0].value = 7.0;
        assert_eq!(progress_at(&bad, 1.0), 1.0, "clamped");
        bad.points[0].value = -2.0;
        assert_eq!(progress_at(&bad, 1.0), 0.0);
    }

    #[test]
    fn the_second_half_of_a_cut_continues_at_the_same_progress() {
        let e = env(&[(0.0, 0.0, LIN), (8.0, 1.0, LIN)]);
        let right = from_offset(Some(&e), 4.0).unwrap();
        assert_eq!(right.points[0].time, 0.0);
        assert!((right.points[0].value - 0.5).abs() < 1e-6);
        for t in [0.0, 1.0, 3.5, 4.0] {
            assert!(
                (progress_at(&right, t) - progress_at(&e, t + 4.0)).abs() < 1e-5,
                "t = {t}"
            );
        }
        assert!(from_offset(None, 1.0).is_none());
    }

    #[test]
    fn a_front_trim_cuts_the_curve_and_an_earlier_start_holds_the_first_progress() {
        let e = env(&[(0.0, 0.0, LIN), (10.0, 1.0, LIN)]);
        let later = shift_start(&e, 5.0);
        assert!((progress_at(&later, 0.0) - 0.5).abs() < 1e-6);
        assert!((progress_at(&later, 5.0) - 1.0).abs() < 1e-6);
        let earlier = shift_start(&e, -3.0);
        assert_eq!(progress_at(&earlier, 2.0), 0.0, "the head holds");
        assert!((progress_at(&earlier, 8.0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn a_change_is_one_undo_step_and_only_video_events_have_a_curve() {
        let (mut tl, mut h, tid, eid) = video_event(10.0);
        assert_eq!(get(&tl, tid, eid), Some(None));
        assert!(set(&tl, tid, eid, None).is_none(), "nothing changes");
        let cmd = set(&tl, tid, eid, Some(new_envelope(2.0))).unwrap();
        assert!(h.execute(&mut tl, cmd));
        assert!(get(&tl, tid, eid).unwrap().is_some());
        assert!(h.undo(&mut tl));
        assert_eq!(get(&tl, tid, eid), Some(None));
        assert!(h.redo(&mut tl));
        // Wild keyframes are made safe on the way in
        let mut wild = new_envelope(1.0);
        wild.points.push(EnvelopePoint {
            time: -4.0,
            value: 50.0,
            shape: LIN,
        });
        let cmd = set(&tl, tid, eid, Some(wild)).unwrap();
        assert!(h.execute(&mut tl, cmd));
        let stored = get(&tl, tid, eid).unwrap().unwrap();
        assert!(stored
            .points
            .iter()
            .all(|p| p.time >= 0.0 && (0.0..=1.0).contains(&p.value)));
        let audio = tl.new_track(TrackKind::Audio);
        let aid = audio.id;
        assert!(h.execute(&mut tl, EditCommand::AddTrack { track: audio }));
        assert!(get(&tl, aid, eid).is_none());
        assert!(set(&tl, 999, eid, Some(new_envelope(1.0))).is_none());
    }

    #[test]
    fn splitting_and_trimming_the_front_keep_the_progress_over_the_same_picture() {
        let (mut tl, mut h, tid, eid) = video_event(10.0);
        let whole = env(&[(0.0, 0.0, LIN), (10.0, 1.0, LIN)]);
        let cmd = set(&tl, tid, eid, Some(whole.clone())).unwrap();
        assert!(h.execute(&mut tl, cmd));
        // A split gives the right half its part of the curve
        let new_id = tl.alloc_id();
        assert!(h.execute(
            &mut tl,
            EditCommand::SplitEvent {
                track_id: tid,
                event_id: eid,
                at: 14.0,
                new_event_id: new_id,
                original_duration: 10.0,
                original_fade_out: 0.0,
            }
        ));
        let events = &tl.track(tid).unwrap().events;
        assert_eq!(events[0].transition_progress.as_ref(), Some(&whole));
        let right = events[1].transition_progress.as_ref().unwrap();
        assert!((progress_at(right, 1.0) - progress_at(&whole, 5.0)).abs() < 1e-5);
        assert!(h.undo(&mut tl));
        // A front trim moves the start and the in point together
        let old = tl.track(tid).unwrap().events[0].geometry();
        let mut new = old;
        new.start += 4.0;
        new.duration -= 4.0;
        new.trim_in += 4.0;
        assert!(h.execute(
            &mut tl,
            EditCommand::EditEvent {
                from_track: tid,
                to_track: tid,
                event_id: eid,
                old,
                new,
            }
        ));
        let event = &tl.track(tid).unwrap().events[0];
        let cut = event.transition_progress.as_ref().unwrap();
        assert!((progress_at(cut, 0.0) - progress_at(&whole, 4.0)).abs() < 1e-5);
        assert!(h.undo(&mut tl));
        assert_eq!(
            tl.track(tid).unwrap().events[0]
                .transition_progress
                .as_ref(),
            Some(&whole)
        );
        // A slide (the in point stays) leaves the curve alone
        let mut slid = old;
        slid.start += 3.0;
        assert!(h.execute(
            &mut tl,
            EditCommand::EditEvent {
                from_track: tid,
                to_track: tid,
                event_id: eid,
                old,
                new: slid,
            }
        ));
        assert_eq!(
            tl.track(tid).unwrap().events[0]
                .transition_progress
                .as_ref(),
            Some(&whole)
        );
    }
}
