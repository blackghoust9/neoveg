//! *Alinhar ao transiente* (manual v21, *aligning sound effects*): moves an audio event so the
//! first strong peak of its sound falls on the cursor, or on the marker nearest to the cursor.
//!
//! What is pure lives here: finding the first strong peak in samples, choosing the target time and
//! building the one-undo-step edit. The samples come from the caller (the UI has them decoded).

use crate::command::EditCommand;
use crate::selection::EventRef;
use crate::switches;
use crate::timeline::{Timeline, TrackEvent, TIME_EPSILON};

/// A peak is "strong" when it reaches this fraction of the loudest peak of the event.
pub const STRONG_FRACTION: f32 = 0.5;
/// Sound quieter than this peak (linear) counts as silence: there is no transient to align.
pub const SILENCE_PEAK: f32 = 1e-4;
/// A marker closer than this (seconds) to the cursor takes the sound instead of the cursor.
pub const MARKER_REACH: f64 = 1.0;

/// The seconds, from the first frame of `samples` (interleaved, `channels` per frame), of the
/// first frame where any channel reaches [`STRONG_FRACTION`] of the loudest peak. `None` for
/// silence or no samples.
pub fn first_transient(samples: &[f32], channels: usize, rate: u32) -> Option<f64> {
    let channels = channels.max(1);
    let frames = samples.len() / channels;
    let peak_at = |f: usize| {
        samples[f * channels..(f + 1) * channels]
            .iter()
            .fold(0.0f32, |a, v| a.max(v.abs()))
    };
    let peak = (0..frames).map(peak_at).fold(0.0f32, f32::max);
    if !peak.is_finite() || peak < SILENCE_PEAK {
        return None;
    }
    let threshold = peak * STRONG_FRACTION;
    let frame = (0..frames).find(|&f| peak_at(f) >= threshold)?;
    Some(frame as f64 / f64::from(rate.max(1)))
}

/// Where in the event (seconds of the timeline, from its start) its first strong peak plays, given
/// the whole decoded media of the event (`samples`, interleaved). Only the part the event plays is
/// looked at, at its playback rate.
pub fn transient_in_event(
    event: &TrackEvent,
    samples: &[f32],
    channels: usize,
    rate: u32,
) -> Option<f64> {
    let section = crate::subclip::section_of(event)?;
    let channels = channels.max(1);
    let frames = samples.len() / channels;
    let rate_hz = f64::from(rate.max(1));
    let from = ((section.start * rate_hz).round() as usize).min(frames);
    let to = (((section.start + section.length) * rate_hz).round() as usize).clamp(from, frames);
    let seconds = first_transient(&samples[from * channels..to * channels], channels, rate)?;
    Some(seconds / crate::edit::clamp_playback_rate(event.playback_rate))
}

/// The time the peak is moved to: the marker nearest to `cursor` when one is within
/// [`MARKER_REACH`], else the cursor.
pub fn target_time(timeline: &Timeline, cursor: f64) -> f64 {
    timeline
        .markers
        .iter()
        .map(|m| m.time)
        .filter(|t| (t - cursor).abs() <= MARKER_REACH)
        .min_by(|a, b| (a - cursor).abs().total_cmp(&(b - cursor).abs()))
        .unwrap_or(cursor)
}

/// The edit that moves `event` so its peak, `offset` seconds into it, falls on `target`. `None`
/// for a locked or missing event, a start before the timeline's zero, or when it is already there.
pub fn align(
    timeline: &Timeline,
    event: EventRef,
    offset: f64,
    target: f64,
) -> Option<EditCommand> {
    if !offset.is_finite() || !target.is_finite() || switches::is_locked(timeline, event) {
        return None;
    }
    let current = timeline
        .track(event.0)?
        .events
        .iter()
        .find(|e| e.id == event.1)?;
    let new_start = target - offset;
    if new_start < -TIME_EPSILON || (new_start - current.start).abs() <= TIME_EPSILON {
        return None;
    }
    Some(EditCommand::MoveEvent {
        track_id: event.0,
        event_id: event.1,
        old_start: current.start,
        new_start: new_start.max(0.0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;
    use crate::timeline::TrackKind;

    /// Silence, a soft tick, silence, then a loud hit at frame `hit` (mono, 1 kHz rate).
    fn hit_at(hit: usize) -> Vec<f32> {
        let mut v = vec![0.0f32; 3_000];
        v[100] = 0.1;
        v[hit] = 0.9;
        v[hit + 1] = 0.6;
        v
    }

    #[test]
    fn the_first_strong_peak_is_found_and_soft_sounds_before_it_are_skipped() {
        let t = first_transient(&hit_at(1_500), 1, 1_000).unwrap();
        assert!((t - 1.5).abs() < 1e-9, "{t}");
        assert!(first_transient(&vec![0.0; 100], 1, 1_000).is_none());
        assert!(first_transient(&[], 2, 1_000).is_none());
        // Stereo: the louder channel decides, a frame is both channels
        let mut stereo = vec![0.0f32; 2_000];
        stereo[2 * 700 + 1] = 0.8;
        assert_eq!(first_transient(&stereo, 2, 1_000), Some(0.7));
    }

    fn setup() -> (Timeline, EventRef, TrackEvent) {
        let mut tl = Timeline::new();
        let mut track = tl.new_track(TrackKind::Audio);
        let event = TrackEvent {
            id: 5,
            media_id: Some(1),
            start: 10.0,
            duration: 2.0,
            trim_in: 0.5,
            ..Default::default()
        };
        track.events = vec![event.clone()];
        let id = track.id;
        tl.tracks.push(track);
        (tl, (id, 5), event)
    }

    #[test]
    fn the_peak_offset_counts_from_the_start_of_the_part_the_event_plays() {
        let (_, _, event) = setup();
        // The event plays from 0.5 s; the hit is at 1.5 s of the media, 1.0 s into the event.
        let off = transient_in_event(&event, &hit_at(1_500), 1, 1_000).unwrap();
        assert!((off - 1.0).abs() < 1e-9, "{off}");
        // A hit before the in point is not heard
        assert!(transient_in_event(&event, &hit_at(200), 1, 1_000).is_none());
        let mut fast = event.clone();
        fast.playback_rate = 2.0;
        fast.duration = 1.0;
        let off = transient_in_event(&fast, &hit_at(1_500), 1, 1_000).unwrap();
        assert!((off - 0.5).abs() < 1e-9, "twice the speed: {off}");
    }

    #[test]
    fn the_target_is_the_nearest_marker_within_reach_else_the_cursor() {
        let (mut tl, ..) = setup();
        assert_eq!(target_time(&tl, 4.0), 4.0);
        let mut h = History::new();
        for time in [4.4, 9.0] {
            let c = crate::markers::add_marker(&mut tl, time, None).unwrap();
            assert!(h.execute(&mut tl, c));
        }
        assert_eq!(target_time(&tl, 4.0), 4.4);
        assert_eq!(target_time(&tl, 5.0), 4.4, "4.4 is within reach of 5.0");
        assert_eq!(target_time(&tl, 7.0), 7.0, "nothing within reach");
        assert_eq!(target_time(&tl, 8.6), 9.0);
    }

    #[test]
    fn aligning_moves_the_event_so_the_peak_lands_on_the_target_in_one_undo_step() {
        let (mut tl, event_ref, _) = setup();
        let command = align(&tl, event_ref, 1.0, 20.0).unwrap();
        let mut h = History::new();
        assert!(h.execute(&mut tl, command));
        assert_eq!(tl.track(event_ref.0).unwrap().events[0].start, 19.0);
        assert!(h.undo(&mut tl));
        assert_eq!(tl.track(event_ref.0).unwrap().events[0].start, 10.0);
        // Already there, before zero, locked: nothing to do
        assert!(align(&tl, event_ref, 1.0, 11.0).is_none());
        assert!(align(&tl, event_ref, 1.0, 0.5).is_none());
        tl.track_mut(event_ref.0).unwrap().events[0].locked = true;
        assert!(align(&tl, event_ref, 1.0, 20.0).is_none());
    }
}
