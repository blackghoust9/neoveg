//! Pure geometry rules for mouse editing: moving, trimming, snapping and frame quantization.

use crate::timeline::EventGeometry;

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
    let end = g.end();
    let lowest = (g.start - g.trim_in).max(0.0);
    let highest = (end - min_len).max(g.start);
    let start = new_start.clamp(lowest, highest.max(lowest));
    clamp_fades(EventGeometry {
        start,
        duration: end - start,
        trim_in: g.trim_in + (start - g.start),
        ..g
    })
}

/// Drags the right edge to `new_end`, limited by the remaining source (`source_len`, when known) and
/// by `min_len`.
pub fn trim_right(
    g: EventGeometry,
    new_end: f64,
    min_len: f64,
    source_len: Option<f64>,
) -> EventGeometry {
    let lowest = g.start + min_len.min(g.duration);
    let highest = source_len
        .map_or(f64::INFINITY, |len| g.start + (len - g.trim_in))
        .max(lowest);
    let end = new_end.clamp(lowest, highest);
    clamp_fades(EventGeometry {
        duration: end - g.start,
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
            fade_in: 0.0,
            fade_out: 0.0,
            fade_in_curve: Default::default(),
            fade_out_curve: Default::default(),
        }
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
}
