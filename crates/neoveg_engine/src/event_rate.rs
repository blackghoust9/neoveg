//! The playback rate of events changed in steps from the keyboard (`Alt+=` / `Alt+-`): the event
//! keeps its start and its source span, so a faster rate makes it shorter and a slower one longer.
//! Setting it for a selection is one [`EditCommand::ReplaceEvents`], so it is one undo step.

use crate::command::{EditCommand, TrackChange};
use crate::edit::{clamp_playback_rate, MAX_PLAYBACK_RATE, MIN_PLAYBACK_RATE};
use crate::selection::EventRef;
use crate::timeline::{Timeline, TrackId};

/// Each key press changes the rate by this fraction (5 %).
pub const RATE_STEP: f64 = 0.05;

/// Multiplies the rate of every unlocked event among `events` by `factor` (above 1: faster), within
/// the allowed rates, keeping the start and the part of the source that is played. `None` when
/// nothing changes (already at the limit, or nothing unlocked selected).
pub fn nudge(timeline: &Timeline, events: &[EventRef], factor: f64) -> Option<EditCommand> {
    if !factor.is_finite() || factor <= 0.0 || factor == 1.0 {
        return None;
    }
    let mut changes: Vec<TrackChange> = Vec::new();
    let mut tracks: Vec<TrackId> = events.iter().map(|(t, _)| *t).collect();
    tracks.sort_unstable();
    tracks.dedup();
    for track_id in tracks {
        let Some(track) = timeline.track(track_id) else {
            continue;
        };
        let mut after = track.events.clone();
        let mut changed = false;
        for event in &mut after {
            if !events.contains(&(track_id, event.id)) || event.locked {
                continue;
            }
            let old = clamp_playback_rate(event.playback_rate);
            let wanted = round_rate(clamp_playback_rate(old * factor));
            if (wanted - old).abs() < 1e-9 {
                continue;
            }
            // The same stretch of source plays, so the length changes the other way
            event.duration *= old / wanted;
            event.playback_rate = wanted;
            let length = event.duration;
            event.fade_in = event.fade_in.min(length);
            event.fade_out = event.fade_out.min(length);
            changed = true;
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

/// The rate rounded to a thousandth, so steps up and down do not drift.
fn round_rate(rate: f64) -> f64 {
    if rate <= MIN_PLAYBACK_RATE || rate >= MAX_PLAYBACK_RATE {
        return rate;
    }
    (rate * 1000.0).round() / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;
    use crate::timeline::{TrackEvent, TrackKind};

    fn timeline() -> (Timeline, TrackId, u64) {
        let mut tl = Timeline::new();
        let mut track = tl.new_track(TrackKind::Video);
        let event = TrackEvent {
            id: tl.alloc_id(),
            start: 2.0,
            duration: 10.0,
            media_id: Some(1),
            ..TrackEvent::default()
        };
        let id = event.id;
        track.events.push(event);
        let tid = track.id;
        tl.tracks.push(track);
        (tl, tid, id)
    }

    #[test]
    fn a_faster_rate_shortens_the_event_and_keeps_its_start() {
        let (mut tl, track, id) = timeline();
        let mut h = History::new();
        let c = nudge(&tl, &[(track, id)], 1.0 + RATE_STEP).unwrap();
        assert!(h.execute(&mut tl, c));
        let e = &tl.tracks[0].events[0];
        assert_eq!(e.start, 2.0);
        assert!((e.playback_rate - 1.05).abs() < 1e-9);
        assert!((e.duration - 10.0 / 1.05).abs() < 1e-9);
        assert!(h.undo(&mut tl));
        assert_eq!(tl.tracks[0].events[0].duration, 10.0);
    }

    #[test]
    fn the_rate_stops_at_the_limits_and_locked_events_stay() {
        let (mut tl, track, id) = timeline();
        assert!(nudge(&tl, &[(track, id)], 1.0).is_none());
        tl.tracks[0].events[0].playback_rate = MAX_PLAYBACK_RATE;
        assert!(nudge(&tl, &[(track, id)], 1.05).is_none());
        tl.tracks[0].events[0].playback_rate = 1.0;
        tl.tracks[0].events[0].locked = true;
        assert!(nudge(&tl, &[(track, id)], 0.95).is_none());
        tl.tracks[0].events[0].locked = false;
        tl.tracks[0].events[0].playback_rate = 0.26;
        let c = nudge(&tl, &[(track, id)], 0.5).unwrap();
        let mut h = History::new();
        assert!(h.execute(&mut tl, c));
        assert_eq!(tl.tracks[0].events[0].playback_rate, MIN_PLAYBACK_RATE);
    }
}
