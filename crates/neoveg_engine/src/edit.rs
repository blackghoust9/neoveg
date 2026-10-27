//! Pure geometry rules for mouse editing: moving, trimming, time-stretching, snapping and frame quantization.

use crate::timeline::EventGeometry;

/// Slowest supported event playback rate (`0.25x` = 400% duration).
pub const MIN_PLAYBACK_RATE: f64 = 0.25;
/// Fastest supported event playback rate (`4.0x` = 25% duration).
pub const MAX_PLAYBACK_RATE: f64 = 4.0;

/// Clamps `rate` into [`MIN_PLAYBACK_RATE`]..=[`MAX_PLAYBACK_RATE`], treating non-finite or
/// non-positive values as `1.0`.
pub fn clamp_playback_rate(rate: f64) -> f64 {
    if !rate.is_finite() || rate <= 0.0 {
        1.0
    } else {
        rate.clamp(MIN_PLAYBACK_RATE, MAX_PLAYBACK_RATE)
    }
}

/// Rounds `seconds` to the nearest frame boundary.
pub fn quantize(seconds: f64, fps: f64) -> f64 {
    if fps > 0.0 {
        (seconds * fps).round() / fps
    } else {
        seconds
    }
}

/// Keeps `fade_in + fade_out` inside the event after its length changed.
fn clamp_fades(mut g: EventGeometry) -> EventGeometry {
    g.fade_in = g.fade_in.clamp(0.0, g.duration);
    g.fade_out = g.fade_out.clamp(0.0, (g.duration - g.fade_in).max(0.0));
    g
}

/// Sets the fade-in length (at most the room left before the fade-out starts).
pub fn set_fade_in(g: EventGeometry, length: f64) -> EventGeometry {
    EventGeometry {
        fade_in: length.clamp(0.0, (g.duration - g.fade_out).max(0.0)),
        ..g
    }
}

/// Sets the fade-out length (at most the room left after the fade-in ends).
pub fn set_fade_out(g: EventGeometry, length: f64) -> EventGeometry {
    EventGeometry {
        fade_out: length.clamp(0.0, (g.duration - g.fade_in).max(0.0)),
        ..g
    }
}

/// Moves the event so it starts at `new_start` (never before the timeline origin).
pub fn move_to(g: EventGeometry, new_start: f64) -> EventGeometry {
    EventGeometry {
        start: new_start.max(0.0),
        ..g
    }
}

/// Drags the left edge to `new_start`. The source keeps its alignment (`trim_in` follows the edge), the
/// right edge stays put, the edge cannot reveal media before the source start, and the event keeps at
/// least `min_len` seconds.
pub fn trim_left(g: EventGeometry, new_start: f64, min_len: f64) -> EventGeometry {
    let rate = clamp_playback_rate(g.playback_rate);
    let end = g.end();
    let lowest = (g.start - g.trim_in / rate).max(0.0);
    let highest = (end - min_len).max(g.start);
    let start = new_start.clamp(lowest, highest.max(lowest));
    clamp_fades(EventGeometry {
        start,
        duration: end - start,
        trim_in: g.trim_in + (start - g.start) * rate,
        ..g
    })
}

/// Position inside a looped source: `offset` seconds after the loop start, wrapped to a loop of
/// `loop_len` seconds. A loop length of zero or less means the source does not loop.
pub fn wrap_loop(offset: f64, loop_len: f64) -> f64 {
    if loop_len > 1e-9 {
        offset.rem_euclid(loop_len)
    } else {
        offset
    }
}

/// Length of the part of the source a looping event repeats: from `trim_in` to the end of the media.
pub fn loop_length(source_len: f64, trim_in: f64) -> f64 {
    (source_len - trim_in).max(0.0)
}

/// *Slip*: the event stays where it is on the timeline and its media moves under it, so another part
/// of the source is shown. `delta` is the drag in timeline seconds (positive shows later media).
/// The window cannot leave the source: it stays between its start and `source_len` when that is known.
pub fn slip(g: EventGeometry, delta: f64, source_len: Option<f64>) -> EventGeometry {
    let rate = clamp_playback_rate(g.playback_rate);
    let span = g.duration * rate;
    let lowest = -g.trim_in;
    let highest = source_len
        .map_or(f64::INFINITY, |len| len - span - g.trim_in)
        .max(lowest);
    EventGeometry {
        trim_in: g.trim_in + (delta * rate).clamp(lowest, highest),
        ..g
    }
}

/// *Slide*: the event moves on the timeline while its media keeps its place on it, so the event is a
/// window that slides over the media: its start and its in point move together. `delta` is the drag
/// in timeline seconds. The event cannot start before zero, nor show media before the source start or
/// past its end (when `source_len` is known).
pub fn slide(g: EventGeometry, delta: f64, source_len: Option<f64>) -> EventGeometry {
    let rate = clamp_playback_rate(g.playback_rate);
    let span = g.duration * rate;
    let lowest = (-g.start).max(-g.trim_in / rate);
    let highest = source_len
        .map_or(f64::INFINITY, |len| (len - span - g.trim_in) / rate)
        .max(lowest);
    let moved = delta.clamp(lowest, highest);
    EventGeometry {
        start: g.start + moved,
        trim_in: g.trim_in + moved * rate,
        ..g
    }
}

/// Drags the right edge to `new_end`, limited by the remaining source (`source_len`, when known) and
/// by `min_len`.
pub fn trim_right(
    g: EventGeometry,
    new_end: f64,
    min_len: f64,
    source_len: Option<f64>,
) -> EventGeometry {
    let rate = clamp_playback_rate(g.playback_rate);
    let lowest = g.start + min_len.min(g.duration);
    let highest = source_len
        .map_or(f64::INFINITY, |len| g.start + (len - g.trim_in) / rate)
        .max(lowest);
    let end = new_end.clamp(lowest, highest);
    clamp_fades(EventGeometry {
        duration: end - g.start,
        ..g
    })
}

/// `Ctrl` + drag on the right edge: stretches or compresses the event (`0.25x..=4.0x`) while
/// keeping `start`, `trim_in` and the underlying source media span unchanged.
pub fn stretch_right(g: EventGeometry, new_end: f64, min_len: f64) -> EventGeometry {
    let old_rate = clamp_playback_rate(g.playback_rate);
    let source_span = (g.duration * old_rate).max(1e-6);
    let raw_dur = (new_end - g.start).max(min_len.max(1e-6));
    let new_rate = clamp_playback_rate(source_span / raw_dur);
    let duration = (source_span / new_rate).max(min_len);
    clamp_fades(EventGeometry {
        duration,
        playback_rate: new_rate,
        ..g
    })
}

/// `Ctrl` + drag on the left edge: stretches or compresses the event (`0.25x..=4.0x`) while
/// keeping the right edge (`end`), `trim_in` and the underlying source media span unchanged.
pub fn stretch_left(g: EventGeometry, new_start: f64, min_len: f64) -> EventGeometry {
    let old_rate = clamp_playback_rate(g.playback_rate);
    let end = g.end();
    let source_span = (g.duration * old_rate).max(1e-6);
    let clamped_start = new_start.clamp(0.0, (end - min_len).max(0.0));
    let raw_dur = (end - clamped_start).max(min_len.max(1e-6));
    let new_rate = clamp_playback_rate(source_span / raw_dur);
    let duration = (source_span / new_rate).min(end).max(min_len.min(end));
    let actual_rate = clamp_playback_rate(source_span / duration.max(1e-6));
    clamp_fades(EventGeometry {
        start: (end - duration).max(0.0),
        duration,
        playback_rate: actual_rate,
        ..g
    })
}

/// Changes `playback_rate` explicitly (e.g. from the Inspector or context menu), scaling
/// `duration` inversely so the event continues to play the exact same source media segment.
pub fn set_playback_rate(g: EventGeometry, rate: f64, min_len: f64) -> EventGeometry {
    let old_rate = clamp_playback_rate(g.playback_rate);
    let new_rate = clamp_playback_rate(rate);
    let source_span = (g.duration * old_rate).max(1e-6);
    let duration = (source_span / new_rate).max(min_len);
    clamp_fades(EventGeometry {
        duration,
        playback_rate: new_rate,
        ..g
    })
}

/// Nearest candidate within `threshold` of `time`.
pub fn snap_time(time: f64, candidates: &[f64], threshold: f64) -> Option<f64> {
    candidates
        .iter()
        .copied()
        .filter(|c| (c - time).abs() <= threshold)
        .min_by(|a, b| (a - time).abs().total_cmp(&(b - time).abs()))
}

/// For a block `[start, start + duration]`, the start that aligns the nearest of its two edges to a
/// candidate within `threshold`.
pub fn snap_block(start: f64, duration: f64, candidates: &[f64], threshold: f64) -> Option<f64> {
    // (distance moved, resulting start); computed from the candidate so the edge lands exactly on it
    let by_start = snap_time(start, candidates, threshold).map(|c| ((c - start).abs(), c));
    let by_end = snap_time(start + duration, candidates, threshold)
        .map(|c| ((c - (start + duration)).abs(), c - duration));
    match (by_start, by_end) {
        (Some(a), Some(b)) => Some(if a.0 <= b.0 { a.1 } else { b.1 }),
        (a, b) => a.or(b).map(|(_, new_start)| new_start),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geo(start: f64, duration: f64, trim_in: f64) -> EventGeometry {
        EventGeometry {
            start,
            duration,
            trim_in,
            ..Default::default()
        }
    }

    #[test]
    fn stretch_right_and_left_scale_rate_inversely_and_clamp_bounds() {
        let g = geo(4.0, 4.0, 1.0);
        // Dragging right edge from 8.0 to 12.0 doubles duration (8 s) -> 0.5x playback rate
        let sr = stretch_right(g, 12.0, 0.04);
        assert_eq!(sr.start, 4.0);
        assert!((sr.duration - 8.0).abs() < 1e-9);
        assert!((sr.playback_rate - 0.5).abs() < 1e-9);
        assert_eq!(sr.trim_in, 1.0);

        // Dragging right edge way out clamps at MIN_PLAYBACK_RATE (0.25x -> 16 s)
        let max_slow = stretch_right(g, 999.0, 0.04);
        assert!((max_slow.playback_rate - MIN_PLAYBACK_RATE).abs() < 1e-9);
        assert!((max_slow.duration - 16.0).abs() < 1e-9);

        // Dragging left edge from 4.0 to 6.0 halves duration (2 s), keeps end at 8.0 -> 2.0x
        let sl = stretch_left(g, 6.0, 0.04);
        assert!((sl.end() - 8.0).abs() < 1e-9);
        assert!((sl.duration - 2.0).abs() < 1e-9);
        assert!((sl.playback_rate - 2.0).abs() < 1e-9);

        // Explicit rate update scales duration inversely
        let explicit = set_playback_rate(g, 2.0, 0.04);
        assert!((explicit.duration - 2.0).abs() < 1e-9);
        assert!((explicit.playback_rate - 2.0).abs() < 1e-9);
    }

    #[test]
    fn quantize_rounds_to_frames() {
        assert_eq!(quantize(1.011, 25.0), 1.0);
        assert_eq!(quantize(1.03, 25.0), 1.04);
        assert_eq!(quantize(1.0, 0.0), 1.0);
    }

    #[test]
    fn move_never_goes_before_zero() {
        assert_eq!(move_to(geo(2.0, 3.0, 0.0), -1.0).start, 0.0);
        assert_eq!(move_to(geo(2.0, 3.0, 1.0), 5.0), geo(5.0, 3.0, 1.0));
    }

    #[test]
    fn trim_left_keeps_the_right_edge_and_source_alignment() {
        let t = trim_left(geo(10.0, 8.0, 2.0), 12.0, 0.04);
        assert_eq!((t.start, t.duration, t.trim_in), (12.0, 6.0, 4.0));
        assert_eq!(t.end(), 18.0);
    }

    #[test]
    fn trim_left_cannot_reveal_media_before_the_source_start() {
        // 2 s of source already hidden: the edge can go back at most 2 s
        let t = trim_left(geo(10.0, 8.0, 2.0), 0.0, 0.04);
        assert_eq!((t.start, t.trim_in), (8.0, 0.0));
        assert_eq!(t.end(), 18.0);
        // and never before the timeline origin
        let t = trim_left(geo(1.0, 4.0, 3.0), -5.0, 0.04);
        assert_eq!(t.start, 0.0);
        assert_eq!(t.trim_in, 2.0);
    }

    #[test]
    fn trim_left_keeps_a_minimum_length() {
        let t = trim_left(geo(10.0, 8.0, 0.0), 99.0, 0.5);
        assert!((t.duration - 0.5).abs() < 1e-9);
        assert_eq!(t.end(), 18.0);
    }

    #[test]
    fn trim_right_is_limited_by_the_source() {
        // 10 s source, 4 s already trimmed at the start: only 6 s remain after trim_in
        let g = geo(5.0, 2.0, 4.0);
        assert_eq!(trim_right(g, 100.0, 0.04, Some(10.0)).end(), 11.0);
        assert_eq!(trim_right(g, 100.0, 0.04, None).end(), 100.0);
        assert!((trim_right(g, 0.0, 0.5, None).duration - 0.5).abs() < 1e-9);
    }

    #[test]
    fn fades_never_exceed_the_event() {
        let g = EventGeometry {
            fade_in: 1.0,
            fade_out: 1.0,
            ..geo(0.0, 4.0, 0.0)
        };
        let t = trim_right(g, 1.5, 0.04, None);
        assert_eq!(t.fade_in, 1.0);
        assert!((t.fade_out - 0.5).abs() < 1e-9);
    }

    #[test]
    fn fade_lengths_are_limited_by_each_other_and_the_event() {
        let g = geo(0.0, 4.0, 0.0);
        let g = set_fade_in(g, 1.5);
        assert_eq!(g.fade_in, 1.5);
        assert_eq!(set_fade_out(g, 10.0).fade_out, 2.5);
        assert_eq!(set_fade_in(g, -1.0).fade_in, 0.0);
        assert_eq!(set_fade_in(set_fade_out(g, 2.0), 9.0).fade_in, 2.0);
    }

    #[test]
    fn snapping_picks_the_nearest_candidate_in_range() {
        let c = [1.0, 5.0, 5.2];
        assert_eq!(snap_time(5.1, &c, 0.15), Some(5.0));
        assert_eq!(snap_time(3.0, &c, 0.15), None);
    }

    #[test]
    fn block_snaps_by_whichever_edge_is_closer() {
        let c = [10.0];
        // end at 9.9 is 0.1 from 10: start moves so the end lands on 10
        assert!((snap_block(5.9, 4.0, &c, 0.2).unwrap() - 6.0).abs() < 1e-9);
        // start at 10.05 snaps to 10
        assert!((snap_block(10.05, 2.0, &c, 0.2).unwrap() - 10.0).abs() < 1e-9);
        assert_eq!(snap_block(1.0, 2.0, &c, 0.2), None);
    }

    fn window(start: f64, duration: f64, trim_in: f64) -> EventGeometry {
        EventGeometry {
            start,
            duration,
            trim_in,
            fade_in: 0.0,
            fade_out: 0.0,
            fade_in_curve: Default::default(),
            fade_out_curve: Default::default(),
            playback_rate: 1.0,
            preserve_pitch: true,
        }
    }

    #[test]
    fn slip_shifts_the_media_and_keeps_the_event_where_it_is() {
        let g = window(10.0, 4.0, 5.0);
        let slipped = slip(g, 2.0, Some(30.0));
        assert_eq!(
            (slipped.start, slipped.duration, slipped.trim_in),
            (10.0, 4.0, 7.0)
        );
        let back = slip(g, -2.0, Some(30.0));
        assert_eq!(back.trim_in, 3.0);
    }

    #[test]
    fn slip_cannot_leave_the_source() {
        let g = window(10.0, 4.0, 5.0);
        assert_eq!(
            slip(g, -99.0, Some(30.0)).trim_in,
            0.0,
            "not before the start of the media"
        );
        // 30 s of media, 4 s shown: the window can start at 26 s at the latest
        assert_eq!(slip(g, 99.0, Some(30.0)).trim_in, 26.0);
        assert_eq!(
            slip(g, 99.0, None).trim_in,
            104.0,
            "an unbounded source has no end"
        );
    }

    #[test]
    fn slip_counts_the_media_that_plays_per_timeline_second() {
        // At double speed a drag of 1 s moves 2 s of media
        let mut g = window(0.0, 4.0, 5.0);
        g.playback_rate = 2.0;
        assert_eq!(slip(g, 1.0, Some(60.0)).trim_in, 7.0);
        // and the 8 s of media shown must still fit: 60 - 8 = 52 at the latest
        assert_eq!(slip(g, 500.0, Some(60.0)).trim_in, 52.0);
    }

    #[test]
    fn slide_moves_the_event_and_keeps_the_media_in_place() {
        let g = window(10.0, 4.0, 5.0);
        let slid = slide(g, 3.0, Some(30.0));
        assert_eq!((slid.start, slid.trim_in), (13.0, 8.0));
        // The media is where it was: timeline time t shows the same source time as before
        let before = (12.0 - g.start) + g.trim_in;
        let after = (12.0 - slid.start) + slid.trim_in;
        assert_eq!(before, after);
        assert_eq!(slid.duration, g.duration);
    }

    #[test]
    fn slide_is_bounded_by_zero_the_source_start_and_the_source_end() {
        let g = window(2.0, 4.0, 5.0);
        // Not before time zero
        assert_eq!(slide(g, -99.0, Some(30.0)).start, 0.0);
        // Not before the start of the media: the in point reaches 0 at a slide of -5, the start at -2
        let g2 = window(8.0, 4.0, 3.0);
        let slid = slide(g2, -99.0, Some(30.0));
        assert_eq!((slid.start, slid.trim_in), (5.0, 0.0));
        // Not past the end of the media
        let slid = slide(g, 99.0, Some(30.0));
        assert_eq!(slid.trim_in, 26.0);
        assert_eq!(slid.start, 23.0);
    }
}
