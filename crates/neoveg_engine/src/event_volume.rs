//! Event volume: the level of an audio event over time.
//!
//! The volume envelope of an event (`TrackEvent::volume_envelope`) is a curve in decibels whose
//! keyframes are in seconds from the start of the event, so it moves with the event. `0 dB` leaves
//! the sound alone, `-60 dB` and below is silence. It is applied on top of the event's own gain and
//! fades. Everything here is a pure function; the mixer asks for the gain at each moment.

use crate::command::EditCommand;
use crate::envelope::{AutomationTarget, EnvelopePoint, EnvelopeShape, TrackEnvelope};
use crate::timeline::{EventId, Timeline, TrackId, TrackKind, TIME_EPSILON};

/// Lowest and highest level of the envelope, dB (the range of a track fader).
pub const MIN_DB: f32 = -60.0;
pub const MAX_DB: f32 = 12.0;

/// A new volume envelope that starts at `db`.
pub fn new_envelope(db: f32) -> TrackEnvelope {
    TrackEnvelope::new(AutomationTarget::EventVolume, db.clamp(MIN_DB, MAX_DB))
}

/// `true` when `envelope` changes the level: it is there, on, and has keyframes.
pub fn is_active(envelope: Option<&TrackEnvelope>) -> bool {
    envelope.is_some_and(|e| e.enabled && !e.points.is_empty())
}

/// The level in dB at `time` seconds into the event: `0` without an envelope or with a disabled one.
pub fn db_at(envelope: Option<&TrackEnvelope>, time: f64) -> f32 {
    let Some(env) = envelope.filter(|e| is_active(Some(e))) else {
        return 0.0;
    };
    let value = env.value_at(time, 0.0);
    if value.is_finite() {
        value.clamp(MIN_DB, MAX_DB)
    } else {
        0.0
    }
}

/// The linear gain at `time` seconds into the event (`1.0` is unity, `0.0` silence).
pub fn gain_at(envelope: Option<&TrackEnvelope>, time: f64) -> f32 {
    let db = db_at(envelope, time);
    if db <= MIN_DB {
        0.0
    } else {
        10f32.powf(db / 20.0)
    }
}

/// The envelope seen from `offset` seconds into the event (the front cut off by a split or a trim):
/// the level at the cut becomes the first keyframe and the later ones move back by `offset`. `None`
/// stays `None`.
pub fn from_offset(envelope: Option<&TrackEnvelope>, offset: f64) -> Option<TrackEnvelope> {
    let env = envelope?;
    let mut out = env.clone();
    let first = db_at(Some(env), offset);
    let shape = env
        .points
        .iter()
        .rfind(|p| p.time <= offset)
        .map_or(EnvelopeShape::Linear, |p| p.shape);
    out.points = vec![EnvelopePoint {
        time: 0.0,
        value: first,
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

/// The envelope after the start of the event moves `delta` seconds while the sound stays where it is
/// on the timeline: later, the part cut off the front goes with it; earlier, the new head holds the
/// level the event started with.
pub fn shift_start(envelope: &TrackEnvelope, delta: f64) -> TrackEnvelope {
    if delta >= 0.0 {
        return from_offset(Some(envelope), delta).unwrap_or_else(|| envelope.clone());
    }
    let head = -delta;
    let mut out = envelope.clone();
    out.points = vec![EnvelopePoint {
        time: 0.0,
        value: db_at(Some(envelope), 0.0),
        shape: EnvelopeShape::Linear,
    }];
    out.points
        .extend(envelope.points.iter().map(|p| EnvelopePoint {
            time: p.time + head,
            ..*p
        }));
    out
}

/// The volume envelope of an event of an audio track (`Some(None)` is none), or `None` when the
/// event does not exist or is not on an audio track.
pub fn get(
    timeline: &Timeline,
    track_id: TrackId,
    event_id: EventId,
) -> Option<Option<&TrackEnvelope>> {
    let track = timeline.track(track_id)?;
    if track.kind != TrackKind::Audio {
        return None;
    }
    let event = track.events.iter().find(|e| e.id == event_id)?;
    Some(event.volume_envelope.as_ref())
}

/// A command that puts `after` as the volume envelope, or `None` when nothing changes. Keyframes are
/// kept sorted, from zero on, and their levels inside the fader range.
pub fn set(
    timeline: &Timeline,
    track_id: TrackId,
    event_id: EventId,
    after: Option<TrackEnvelope>,
) -> Option<EditCommand> {
    let before = get(timeline, track_id, event_id)?.cloned();
    let mut after = after;
    if let Some(env) = after.as_mut() {
        env.target = AutomationTarget::EventVolume;
        for p in &mut env.points {
            p.time = if p.time.is_finite() {
                p.time.max(0.0)
            } else {
                0.0
            };
            p.value = if p.value.is_finite() {
                p.value.clamp(MIN_DB, MAX_DB)
            } else {
                0.0
            };
        }
        env.sort_points();
    }
    (before != after).then_some(EditCommand::SetEventVolume {
        track_id,
        event_id,
        before,
        after,
    })
}

/// Lets a left-edge trim keep the level where it is on events that have a volume envelope: when the
/// start and the in point move together, the front of the envelope is cut (or extended) by the same
/// amount, as one more command in a batch so undo puts both back. A slide, which moves the event over
/// the same part of the media, leaves the envelope alone because it moves with the event. Other
/// commands, and events without an active envelope, come back as they were.
pub fn keep_level(timeline: &Timeline, command: EditCommand) -> EditCommand {
    match command {
        EditCommand::Batch(list) => {
            EditCommand::Batch(list.into_iter().map(|c| keep_level(timeline, c)).collect())
        }
        EditCommand::EditEvent {
            from_track,
            to_track,
            event_id,
            old,
            new,
        } => {
            let envelope = timeline
                .track(from_track)
                .filter(|t| t.kind == TrackKind::Audio)
                .and_then(|t| t.events.iter().find(|e| e.id == event_id))
                .and_then(|e| e.volume_envelope.as_ref())
                .filter(|v| is_active(Some(v)))
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
                EditCommand::SetEventVolume {
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
        let mut e = new_envelope(0.0);
        e.points = points
            .iter()
            .map(|&(time, value, shape)| EnvelopePoint { time, value, shape })
            .collect();
        e
    }

    fn audio_event(duration: f64) -> (Timeline, History, TrackId, EventId) {
        let mut tl = Timeline::new();
        let mut h = History::new();
        let track = tl.new_track(TrackKind::Audio);
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
    fn without_an_envelope_the_level_is_unity_and_minus_sixty_is_silence() {
        assert_eq!(db_at(None, 3.0), 0.0);
        assert_eq!(gain_at(None, 3.0), 1.0);
        let mut off = env(&[(0.0, -20.0, LIN)]);
        off.enabled = false;
        assert_eq!(
            gain_at(Some(&off), 1.0),
            1.0,
            "a disabled envelope is ignored"
        );
        assert_eq!(gain_at(Some(&env(&[(0.0, MIN_DB, LIN)])), 1.0), 0.0);
        let six = gain_at(Some(&env(&[(0.0, 6.0, LIN)])), 0.0);
        assert!((six - 1.995_262_3).abs() < 1e-5, "{six}");
        let half = gain_at(Some(&env(&[(0.0, -6.0, LIN)])), 0.0);
        assert!((half - 0.501_187_2).abs() < 1e-5, "{half}");
    }

    #[test]
    fn the_level_follows_the_keyframes_and_their_shapes() {
        // A ramp from 0 to -12 dB over 4 s: -6 dB half way, and the last level holds afterwards
        let ramp = env(&[(0.0, 0.0, LIN), (4.0, -12.0, LIN)]);
        assert!((db_at(Some(&ramp), 2.0) + 6.0).abs() < 1e-5);
        assert_eq!(db_at(Some(&ramp), 9.0), -12.0);
        // A step holds the first level until the second key
        let step = env(&[(0.0, 0.0, EnvelopeShape::Hold), (3.0, -20.0, LIN)]);
        assert_eq!(db_at(Some(&step), 2.9), 0.0);
        assert_eq!(db_at(Some(&step), 3.0), -20.0);
    }

    #[test]
    fn a_bad_stored_level_cannot_make_a_bad_gain() {
        let mut bad = env(&[(0.0, 0.0, LIN)]);
        bad.points[0].value = f32::NAN;
        assert_eq!(db_at(Some(&bad), 1.0), 0.0);
        bad.points[0].value = 99.0;
        assert_eq!(db_at(Some(&bad), 1.0), MAX_DB, "clamped to +12 dB");
        bad.points[0].value = -500.0;
        assert_eq!(gain_at(Some(&bad), 1.0), 0.0);
    }

    #[test]
    fn the_second_half_of_a_cut_continues_at_the_same_level() {
        let e = env(&[(0.0, 0.0, LIN), (8.0, -16.0, LIN)]);
        let right = from_offset(Some(&e), 4.0).unwrap();
        assert_eq!(right.points[0].time, 0.0);
        assert!(
            (right.points[0].value + 8.0).abs() < 1e-5,
            "the level at the cut"
        );
        assert_eq!(right.points.last().unwrap().time, 4.0);
        for t in [0.0, 1.0, 3.5, 4.0, 6.0] {
            assert!(
                (db_at(Some(&right), t) - db_at(Some(&e), t + 4.0)).abs() < 1e-5,
                "t = {t}"
            );
        }
        assert!(from_offset(None, 3.0).is_none());
    }

    #[test]
    fn a_front_trim_cuts_the_envelope_and_an_earlier_start_holds_the_first_level() {
        let e = env(&[(0.0, -6.0, LIN), (10.0, -26.0, LIN)]);
        let later = shift_start(&e, 5.0);
        assert!((db_at(Some(&later), 0.0) + 16.0).abs() < 1e-5);
        assert!((db_at(Some(&later), 5.0) + 26.0).abs() < 1e-5);
        let earlier = shift_start(&e, -3.0);
        assert!(
            (db_at(Some(&earlier), 0.0) + 6.0).abs() < 1e-5,
            "the head holds"
        );
        assert!((db_at(Some(&earlier), 3.0) + 6.0).abs() < 1e-5);
        assert!((db_at(Some(&earlier), 13.0) + 26.0).abs() < 1e-5);
    }

    #[test]
    fn a_volume_change_is_one_undo_step_and_only_audio_events_have_one() {
        let (mut tl, mut h, tid, eid) = audio_event(10.0);
        assert_eq!(get(&tl, tid, eid), Some(None));
        assert!(set(&tl, tid, eid, None).is_none(), "nothing changes");

        let cmd = set(&tl, tid, eid, Some(new_envelope(-6.0))).unwrap();
        assert!(h.execute(&mut tl, cmd));
        assert_eq!(db_at(get(&tl, tid, eid).unwrap(), 1.0), -6.0);
        assert!(h.undo(&mut tl));
        assert_eq!(get(&tl, tid, eid), Some(None));
        assert!(h.redo(&mut tl));
        assert!(get(&tl, tid, eid).unwrap().is_some());

        // Out-of-range levels and times are made safe on the way in
        let mut wild = new_envelope(0.0);
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
            .all(|p| p.time >= 0.0 && p.value <= MAX_DB));

        let video = tl.new_track(TrackKind::Video);
        let vid = video.id;
        assert!(h.execute(&mut tl, EditCommand::AddTrack { track: video }));
        assert!(get(&tl, vid, eid).is_none());
        assert!(set(&tl, 999, eid, Some(new_envelope(0.0))).is_none());
    }

    #[test]
    fn splitting_an_event_gives_each_half_its_part_of_the_curve() {
        let (mut tl, mut h, tid, eid) = audio_event(10.0);
        let whole = env(&[(0.0, 0.0, LIN), (10.0, -20.0, LIN)]);
        let cmd = set(&tl, tid, eid, Some(whole.clone())).unwrap();
        assert!(h.execute(&mut tl, cmd));
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
        let track = tl.track(tid).unwrap();
        let (left, right) = (&track.events[0], &track.events[1]);
        // The left half keeps the curve it had; the right one starts at the level of the cut
        assert_eq!(left.volume_envelope.as_ref(), Some(&whole));
        for t in [0.0, 1.5, 3.0, 6.0] {
            assert!(
                (db_at(right.volume_envelope.as_ref(), t) - db_at(Some(&whole), t + 4.0)).abs()
                    < 1e-4,
                "t = {t}"
            );
        }
        assert!(h.undo(&mut tl));
        assert_eq!(tl.track(tid).unwrap().events.len(), 1);
    }

    #[test]
    fn trimming_the_front_keeps_the_level_over_the_same_sound() {
        let (mut tl, mut h, tid, eid) = audio_event(10.0);
        let whole = env(&[(0.0, 0.0, LIN), (10.0, -20.0, LIN)]);
        let cmd = set(&tl, tid, eid, Some(whole.clone())).unwrap();
        assert!(h.execute(&mut tl, cmd));
        let old = tl.track(tid).unwrap().events[0].geometry();
        // The front moves 4 s later and the in point moves with it
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
        // The sound that played at 4 s into the old event is now at its start
        assert!(
            (db_at(event.volume_envelope.as_ref(), 0.0) - db_at(Some(&whole), 4.0)).abs() < 1e-4
        );
        assert!(
            (db_at(event.volume_envelope.as_ref(), 3.0) - db_at(Some(&whole), 7.0)).abs() < 1e-4
        );
        // One undo puts the geometry and the curve back
        assert!(h.undo(&mut tl));
        let event = &tl.track(tid).unwrap().events[0];
        assert_eq!(event.volume_envelope.as_ref(), Some(&whole));
        assert_eq!(event.start, 10.0);
    }

    #[test]
    fn a_slide_leaves_the_curve_alone_because_it_moves_with_the_event() {
        let (mut tl, mut h, tid, eid) = audio_event(10.0);
        let whole = env(&[(0.0, 0.0, LIN), (10.0, -20.0, LIN)]);
        let cmd = set(&tl, tid, eid, Some(whole.clone())).unwrap();
        assert!(h.execute(&mut tl, cmd));
        let old = tl.track(tid).unwrap().events[0].geometry();
        let mut new = old;
        new.start += 3.0; // the in point stays: the event moved over the same media
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
        assert_eq!(event.volume_envelope.as_ref(), Some(&whole));
    }
}
