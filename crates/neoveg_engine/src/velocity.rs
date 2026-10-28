//! Event velocity: the speed of a video event over time.
//!
//! The velocity envelope of an event (`TrackEvent::velocity`) says how much of the source plays per
//! second of the event: `1.0` is normal, `0.5` half speed, `0.0` a freeze frame and a negative value
//! plays backwards. Its keyframes are in seconds from the start of the event and use the shapes of
//! the other envelopes. The length of the event never changes; the envelope only decides which part
//! of the source is shown (the position is the integral of the speed).
//!
//! Everything here is a pure function; the interface and the video preview ask for the position in
//! the source ([`source_offset`]) of each frame.

use crate::command::EditCommand;
use crate::envelope::{AutomationTarget, EnvelopePoint, EnvelopeShape, TrackEnvelope};
use crate::timeline::{EventId, Timeline, TrackId, TrackKind, TIME_EPSILON};

/// Slowest and fastest speed: `-100 %` (backwards) to `300 %`.
pub const MIN_SPEED: f32 = -1.0;
pub const MAX_SPEED: f32 = 3.0;

/// Speeds the interface offers as presets, with their pt-BR names.
pub const PRESETS: [(&str, f32); 6] = [
    ("100 % para trás", -1.0),
    ("0 % (congelar)", 0.0),
    ("50 %", 0.5),
    ("100 % (normal)", 1.0),
    ("200 %", 2.0),
    ("300 %", 3.0),
];

/// A new velocity envelope that starts at `speed` (the speed of the event so far).
pub fn new_envelope(speed: f32) -> TrackEnvelope {
    TrackEnvelope::new(
        AutomationTarget::EventVelocity,
        speed.clamp(MIN_SPEED, MAX_SPEED),
    )
}

/// The speed at `time` seconds into the event: `1.0` without an envelope or with a disabled one.
pub fn rate_at(envelope: Option<&TrackEnvelope>, time: f64) -> f64 {
    let Some(env) = envelope.filter(|e| e.enabled && !e.points.is_empty()) else {
        return 1.0;
    };
    let value = env.value_at(time, 1.0);
    if value.is_finite() {
        f64::from(value.clamp(MIN_SPEED, MAX_SPEED))
    } else {
        1.0
    }
}

/// Seconds of source played during the first `time` seconds of the event: the integral of the
/// speed. Without an envelope that is `time` itself (normal speed). Negative when the event has been
/// playing backwards.
pub fn source_offset(envelope: Option<&TrackEnvelope>, time: f64) -> f64 {
    let time = if time.is_finite() { time.max(0.0) } else { 0.0 };
    let Some(env) = envelope.filter(|e| e.enabled && !e.points.is_empty()) else {
        return time;
    };
    // The curve is smooth or a step between keyframes, so split at the keyframes and take the
    // midpoint rule on each piece: exact for lines and steps, tiny error for the eased shapes
    let mut breaks = vec![0.0];
    breaks.extend(
        env.points
            .iter()
            .map(|p| p.time)
            .filter(|t| *t > 0.0 && *t < time),
    );
    breaks.push(time);
    breaks.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
    const STEPS: usize = 24;
    let mut total = 0.0;
    for pair in breaks.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let h = (b - a) / STEPS as f64;
        total += (0..STEPS)
            .map(|i| rate_at(Some(env), a + (i as f64 + 0.5) * h))
            .sum::<f64>()
            * h;
    }
    total
}

/// The velocity envelope of the part of an event that starts `offset` seconds in (the second half of
/// a split): the speed at the cut becomes the first keyframe and the later ones move back by
/// `offset`. `None` stays `None`.
pub fn from_offset(envelope: Option<&TrackEnvelope>, offset: f64) -> Option<TrackEnvelope> {
    let env = envelope?;
    let mut out = env.clone();
    let first = rate_at(Some(env), offset) as f32;
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

/// The envelope with a freeze frame at `time`: the speed up to there is kept, then it drops to `0`
/// and stays there to the end of the event (the keyframes after the cursor are dropped).
pub fn freeze_at(envelope: Option<&TrackEnvelope>, time: f64) -> TrackEnvelope {
    let time = if time.is_finite() { time.max(0.0) } else { 0.0 };
    let mut env = envelope.cloned().unwrap_or_else(|| new_envelope(1.0));
    let speed = rate_at(Some(&env), time) as f32;
    env.enabled = true;
    env.points.retain(|p| p.time < time - TIME_EPSILON);
    // Hold the speed until the cursor, then a step down to a standstill
    env.points.push(EnvelopePoint {
        time,
        value: speed,
        shape: EnvelopeShape::Hold,
    });
    env.points.push(EnvelopePoint {
        time: time + 1e-3,
        value: 0.0,
        shape: EnvelopeShape::Linear,
    });
    env
}

/// The velocity envelope of an event of a video track (`Some(None)` is none), or `None` when the
/// event does not exist or is not on a video track.
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
    Some(event.velocity.as_ref())
}

/// A command that puts `after` as the velocity envelope, or `None` when nothing changes. Keyframes are
/// kept sorted, inside the event and their values inside the speed range.
pub fn set(
    timeline: &Timeline,
    track_id: TrackId,
    event_id: EventId,
    after: Option<TrackEnvelope>,
) -> Option<EditCommand> {
    let before = get(timeline, track_id, event_id)?.cloned();
    let mut after = after;
    if let Some(env) = after.as_mut() {
        for p in &mut env.points {
            p.time = if p.time.is_finite() {
                p.time.max(0.0)
            } else {
                0.0
            };
            p.value = if p.value.is_finite() {
                p.value.clamp(MIN_SPEED, MAX_SPEED)
            } else {
                1.0
            };
        }
        env.sort_points();
    }
    (before != after).then_some(EditCommand::SetVelocity {
        track_id,
        event_id,
        before,
        after,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;
    use crate::timeline::TrackEvent;

    fn env(points: &[(f64, f32, EnvelopeShape)]) -> TrackEnvelope {
        let mut e = TrackEnvelope::new(AutomationTarget::EventVelocity, 1.0);
        e.points = points
            .iter()
            .map(|&(time, value, shape)| EnvelopePoint { time, value, shape })
            .collect();
        e
    }

    const LIN: EnvelopeShape = EnvelopeShape::Linear;

    #[test]
    fn without_an_envelope_the_event_plays_at_normal_speed() {
        assert_eq!(rate_at(None, 3.0), 1.0);
        assert_eq!(source_offset(None, 3.0), 3.0);
        let mut off = env(&[(0.0, 0.5, LIN)]);
        off.enabled = false;
        assert_eq!(
            source_offset(Some(&off), 4.0),
            4.0,
            "a disabled envelope is ignored"
        );
        assert_eq!(source_offset(None, f64::NAN), 0.0);
        assert_eq!(source_offset(None, -2.0), 0.0);
    }

    #[test]
    fn a_constant_speed_gives_a_proportional_position_and_reverse_goes_back() {
        let half = env(&[(0.0, 0.5, LIN)]);
        assert!(
            (source_offset(Some(&half), 10.0) - 5.0).abs() < 1e-9,
            "half speed: 5 s of source in 10 s"
        );
        let fast = env(&[(0.0, 3.0, LIN)]);
        assert!((source_offset(Some(&fast), 10.0) - 30.0).abs() < 1e-9);
        let back = env(&[(0.0, -1.0, LIN)]);
        assert!((source_offset(Some(&back), 4.0) - -4.0).abs() < 1e-9);
        let freeze = env(&[(0.0, 0.0, LIN)]);
        assert_eq!(source_offset(Some(&freeze), 7.0), 0.0);
    }

    #[test]
    fn a_ramp_is_integrated_exactly_and_a_step_holds_until_the_next_key() {
        // From 0 to 2 over 4 s: the position is the area of a triangle, 4
        let ramp = env(&[(0.0, 0.0, LIN), (4.0, 2.0, LIN)]);
        assert!((source_offset(Some(&ramp), 4.0) - 4.0).abs() < 1e-6);
        // Half way: speed 1 at 2 s, area 1
        assert!((source_offset(Some(&ramp), 2.0) - 1.0).abs() < 1e-6);
        assert!((rate_at(Some(&ramp), 2.0) - 1.0).abs() < 1e-6);
        // After the last key the speed holds: 4 + 2 per second
        assert!((source_offset(Some(&ramp), 6.0) - 8.0).abs() < 1e-6);
        // A step: 1 until 3 s, then 3: at 5 s the source is at 3 + 2*3 = 9
        let step = env(&[(0.0, 1.0, EnvelopeShape::Hold), (3.0, 3.0, LIN)]);
        assert!((source_offset(Some(&step), 5.0) - 9.0).abs() < 1e-4);
        // A smooth ease between two speeds lands between the linear answers
        let eased = env(&[(0.0, 0.0, EnvelopeShape::Smooth), (4.0, 2.0, LIN)]);
        assert!(
            (source_offset(Some(&eased), 4.0) - 4.0).abs() < 0.01,
            "smoothstep keeps the area of a ramp"
        );
    }

    #[test]
    fn the_position_never_decreases_while_the_speed_is_positive_and_is_continuous() {
        let e = env(&[
            (0.0, 0.2, LIN),
            (3.0, 2.5, EnvelopeShape::Smooth),
            (6.0, 0.4, LIN),
        ]);
        let mut last = 0.0;
        for step in 0..=120 {
            let pos = source_offset(Some(&e), step as f64 * 0.05);
            assert!(pos >= last, "not monotonic at step {step}");
            assert!(pos - last < 0.2, "a jump at step {step}");
            last = pos;
        }
    }

    #[test]
    fn a_bad_stored_value_cannot_make_a_bad_position() {
        let mut bad = env(&[(0.0, 1.0, LIN)]);
        bad.points[0].value = f32::NAN;
        assert_eq!(rate_at(Some(&bad), 1.0), 1.0);
        bad.points[0].value = 99.0;
        assert_eq!(rate_at(Some(&bad), 1.0), 3.0, "clamped to 300 %");
        assert!(source_offset(Some(&bad), 2.0).is_finite());
    }

    #[test]
    fn the_second_half_of_a_split_continues_from_the_same_picture_at_the_same_speed() {
        let e = env(&[(0.0, 0.5, LIN), (8.0, 2.5, LIN)]);
        let right = from_offset(Some(&e), 4.0).unwrap();
        assert_eq!(right.points[0].time, 0.0);
        assert!(
            (right.points[0].value - 1.5).abs() < 1e-6,
            "the speed at the cut"
        );
        assert_eq!(right.points.last().unwrap().time, 4.0);
        // The right half's position equals the whole event's, measured from the cut
        let whole = |t: f64| source_offset(Some(&e), t);
        for t in [0.5, 1.0, 2.5, 4.0] {
            let expected = whole(4.0 + t) - whole(4.0);
            assert!(
                (source_offset(Some(&right), t) - expected).abs() < 1e-3,
                "at {t}"
            );
        }
        assert!(from_offset(None, 3.0).is_none());
    }

    #[test]
    fn a_freeze_frame_keeps_the_speed_until_the_cursor_then_stops_for_good() {
        let e = env(&[(0.0, 2.0, LIN), (6.0, 1.0, LIN), (9.0, 3.0, LIN)]);
        let frozen = freeze_at(Some(&e), 3.0);
        let at_cursor = source_offset(Some(&frozen), 3.0);
        // The speed falls from 2 to 1.5 on the way to 3 s: the area is 3 * (2 + 1.5) / 2
        assert!((at_cursor - 5.25).abs() < 0.01, "{at_cursor}");
        // From the cursor on the picture does not move
        assert!((source_offset(Some(&frozen), 8.0) - at_cursor).abs() < 0.01);
        assert_eq!(rate_at(Some(&frozen), 5.0), 0.0);
        assert!(
            frozen.points.iter().all(|p| p.time < 3.01),
            "the later keys are gone"
        );
        // With no envelope yet a freeze at 4 s plays normally until then
        let fresh = freeze_at(None, 4.0);
        assert!((source_offset(Some(&fresh), 4.0) - 4.0).abs() < 0.01);
        assert!((source_offset(Some(&fresh), 9.0) - 4.0).abs() < 0.01);
    }

    #[test]
    fn a_velocity_change_is_one_undo_step_and_only_video_events_have_one() {
        let mut tl = Timeline::new();
        let mut h = History::new();
        let track = tl.new_track(TrackKind::Video);
        let tid = track.id;
        assert!(h.execute(&mut tl, EditCommand::AddTrack { track }));
        let eid = tl.alloc_id();
        let event = TrackEvent {
            id: eid,
            media_id: Some(1),
            duration: 10.0,
            ..TrackEvent::default()
        };
        assert!(h.execute(
            &mut tl,
            EditCommand::AddEvent {
                track_id: tid,
                event
            }
        ));
        assert_eq!(get(&tl, tid, eid), Some(None));
        assert!(set(&tl, tid, eid, None).is_none(), "nothing changes");

        let cmd = set(&tl, tid, eid, Some(new_envelope(0.5))).unwrap();
        assert!(h.execute(&mut tl, cmd));
        assert_eq!(rate_at(get(&tl, tid, eid).unwrap(), 1.0), 0.5);
        assert!(h.undo(&mut tl));
        assert_eq!(get(&tl, tid, eid), Some(None));
        assert!(h.redo(&mut tl));
        assert!(get(&tl, tid, eid).unwrap().is_some());

        // Out-of-range values and times are made safe on the way in
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
            .all(|p| p.time >= 0.0 && p.value <= MAX_SPEED));

        let audio = tl.new_track(TrackKind::Audio);
        let aid = audio.id;
        assert!(h.execute(&mut tl, EditCommand::AddTrack { track: audio }));
        assert!(get(&tl, aid, eid).is_none());
        assert!(set(&tl, 999, eid, Some(new_envelope(1.0))).is_none());
    }

    #[test]
    fn a_split_keeps_the_picture_continuous_and_old_projects_load_without_velocity() {
        let mut tl = Timeline::new();
        let mut h = History::new();
        let track = tl.new_track(TrackKind::Video);
        let tid = track.id;
        assert!(h.execute(&mut tl, EditCommand::AddTrack { track }));
        let eid = tl.alloc_id();
        let event = TrackEvent {
            id: eid,
            media_id: Some(1),
            duration: 8.0,
            velocity: Some(env(&[(0.0, 2.0, LIN)])),
            ..TrackEvent::default()
        };
        assert!(h.execute(
            &mut tl,
            EditCommand::AddEvent {
                track_id: tid,
                event
            }
        ));
        let new_id = tl.alloc_id();
        assert!(h.execute(
            &mut tl,
            EditCommand::SplitEvent {
                track_id: tid,
                event_id: eid,
                new_event_id: new_id,
                at: 4.0,
                original_duration: 8.0,
                original_fade_out: 0.0,
            }
        ));
        let right = tl
            .track(tid)
            .unwrap()
            .events
            .iter()
            .find(|e| e.id == new_id)
            .unwrap();
        assert!(
            (right.trim_in - 8.0).abs() < 1e-6,
            "2x for 4 s: the source is at 8 s at the cut"
        );
        assert!((rate_at(right.velocity.as_ref(), 1.0) - 2.0).abs() < 1e-6);
        assert!(h.undo(&mut tl));
        assert_eq!(tl.track(tid).unwrap().events.len(), 1);

        let old: TrackEvent = serde_json::from_str(
            r#"{"id":1,"name":"x","media_id":null,"start":0.0,"duration":1.0,"trim_in":0.0,"fade_in":0.0,"fade_out":0.0}"#,
        )
        .unwrap();
        assert!(old.velocity.is_none());
        assert!(!serde_json::to_string(&old).unwrap().contains("velocity"));
        let mut with = old.clone();
        with.velocity = Some(new_envelope(0.5));
        let back: TrackEvent =
            serde_json::from_str(&serde_json::to_string(&with).unwrap()).unwrap();
        assert_eq!(back.velocity, with.velocity);
    }
}
