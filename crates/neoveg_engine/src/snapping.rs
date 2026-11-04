//! What an event edge can snap to: the sources the user switches on and the times they give.
//!
//! The playhead, the project origin, the edges of the time selection and the edges of events on the
//! dragged event's own track are always targets while snapping is on; the grid, the markers and
//! regions, and the events of the other tracks have their own switches.

use crate::selection::EventRef;
use crate::timeline::{Timeline, TrackId};

/// The optional snap sources (`Ctrl+F8`, `Shift+F8`, `Ctrl+Shift+F8`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapSources {
    /// The lines of the frame grid.
    pub grid: bool,
    /// Markers and the edges of regions.
    pub markers: bool,
    /// The edges of events on other tracks.
    pub all_events: bool,
}

impl Default for SnapSources {
    fn default() -> Self {
        Self {
            grid: false,
            markers: true,
            all_events: true,
        }
    }
}

/// The two grid lines around `time` (for a `step` in seconds); empty for a step that is not positive.
pub fn grid_candidates(time: f64, step: f64) -> Vec<f64> {
    if !step.is_finite() || step <= 0.0 || !time.is_finite() {
        return Vec::new();
    }
    let below = (time / step).floor() * step;
    vec![below.max(0.0), below + step]
}

/// Snap targets for dragging `dragged`: the origin, the playhead, the edges of the time selection,
/// the events on the dragged tracks and whatever `sources` adds. Grid lines are not listed here (they
/// depend on where the pointer is); use [`grid_candidates`].
pub fn candidates(
    timeline: &Timeline,
    sources: SnapSources,
    dragged: &[EventRef],
    playhead: f64,
    selection: Option<(f64, f64)>,
) -> Vec<f64> {
    let mut times = vec![0.0, playhead];
    if let Some((start, end)) = selection {
        times.extend([start, end]);
    }
    if sources.markers {
        times.extend(timeline.markers.iter().map(|m| m.time));
        for region in &timeline.regions {
            times.extend([region.start, region.end]);
        }
    }
    let own: Vec<TrackId> = dragged.iter().map(|(t, _)| *t).collect();
    for track in &timeline.tracks {
        if !(sources.all_events || own.is_empty() || own.contains(&track.id)) {
            continue;
        }
        for event in &track.events {
            if !dragged.contains(&(track.id, event.id)) {
                times.push(event.start);
                times.push(event.end());
            }
        }
    }
    times
}

/// Span of a moving selection `(earliest_start, left_snap_time, rightmost_end)`:
/// `earliest_start` is the first event's `start`, `left_snap_time` uses any snap offset established
/// on the leftmost event(s), and `rightmost_end` is the last event's `end()`.
pub fn move_span(timeline: &Timeline, events: &[EventRef]) -> Option<(f64, f64, f64)> {
    let mut found = Vec::new();
    for (track_idx, track) in timeline.tracks.iter().enumerate() {
        for ev in &track.events {
            if events.contains(&(track.id, ev.id)) {
                found.push((track_idx, ev));
            }
        }
    }
    if found.is_empty() {
        return None;
    }
    let earliest = found
        .iter()
        .map(|(_, e)| e.start)
        .fold(f64::INFINITY, f64::min);
    let rightmost = found
        .iter()
        .map(|(_, e)| e.end())
        .fold(f64::NEG_INFINITY, f64::max);
    let left_snap = found
        .iter()
        .filter(|(_, e)| (e.start - earliest).abs() <= crate::timeline::TIME_EPSILON)
        .min_by(|(ta, ea), (tb, eb)| {
            let a_has = ea.snap_offset > crate::timeline::TIME_EPSILON;
            let b_has = eb.snap_offset > crate::timeline::TIME_EPSILON;
            b_has
                .cmp(&a_has)
                .then_with(|| ta.cmp(tb))
                .then_with(|| ea.id.cmp(&eb.id))
        })
        .map_or(earliest, |(_, e)| e.snap_point());
    Some((earliest.max(0.0), left_snap, rightmost))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markers::{Marker, Region};
    use crate::timeline::{TrackEvent, TrackKind};

    /// Track 1: events 0-2 and 3-5; track 2: event 10-12; marker at 7, region 20..22.
    fn timeline() -> (Timeline, EventRef) {
        let mut tl = Timeline::new();
        let mut first = None;
        for (i, spans) in [vec![(0.0, 2.0), (3.0, 2.0)], vec![(10.0, 2.0)]]
            .into_iter()
            .enumerate()
        {
            let mut track = tl.new_track(TrackKind::Audio);
            for (start, duration) in spans {
                let event = TrackEvent {
                    id: tl.alloc_id(),
                    start,
                    duration,
                    media_id: Some(1),
                    ..TrackEvent::default()
                };
                if i == 0 && first.is_none() {
                    first = Some((track.id, event.id));
                }
                track.events.push(event);
            }
            tl.tracks.push(track);
        }
        tl.markers.push(Marker {
            id: 1,
            time: 7.0,
            name: "m".into(),
        });
        tl.regions.push(Region {
            id: 2,
            start: 20.0,
            end: 22.0,
            name: "r".into(),
        });
        (tl, first.unwrap())
    }

    fn has(times: &[f64], t: f64) -> bool {
        times.iter().any(|x| (x - t).abs() < 1e-9)
    }

    #[test]
    fn defaults_keep_every_event_and_add_markers_but_not_the_grid() {
        let (tl, dragged) = timeline();
        let times = candidates(&tl, SnapSources::default(), &[dragged], 1.5, None);
        assert!(has(&times, 0.0) && has(&times, 1.5), "origin and playhead");
        assert!(has(&times, 3.0) && has(&times, 5.0), "same track");
        assert!(has(&times, 10.0) && has(&times, 12.0), "other track");
        assert!(
            has(&times, 7.0) && has(&times, 20.0) && has(&times, 22.0),
            "markers and regions"
        );
        assert!(
            !has(&times, 2.0),
            "the dragged event does not snap to itself"
        );
    }

    #[test]
    fn each_switch_removes_only_its_own_targets() {
        let (tl, dragged) = timeline();
        let off = SnapSources {
            grid: false,
            markers: false,
            all_events: false,
        };
        let times = candidates(&tl, off, &[dragged], 1.5, Some((8.0, 9.0)));
        assert!(
            has(&times, 3.0) && has(&times, 8.0) && has(&times, 9.0),
            "own track and selection stay"
        );
        assert!(!has(&times, 10.0), "other tracks need Snap to All Events");
        assert!(
            !has(&times, 7.0) && !has(&times, 20.0),
            "marks need Snap to Markers"
        );
    }

    #[test]
    fn grid_lines_are_the_two_multiples_around_the_time() {
        assert_eq!(grid_candidates(1.05, 0.5), vec![1.0, 1.5]);
        assert_eq!(grid_candidates(1.0, 0.5), vec![1.0, 1.5]);
        assert!(grid_candidates(1.0, 0.0).is_empty());
        assert!(grid_candidates(f64::NAN, 1.0).is_empty());
    }

    #[test]
    fn move_span_uses_leftmost_snap_offset_and_rightmost_end() {
        let mut tl = Timeline::new();
        let mut v_track = tl.new_track(TrackKind::Video);
        let mut a_track = tl.new_track(TrackKind::Audio);
        let v_id = tl.alloc_id();
        let a_id = tl.alloc_id();
        let tail_id = tl.alloc_id();
        v_track.events.push(TrackEvent {
            id: v_id,
            start: 2.0,
            duration: 3.0,
            snap_offset: 0.0,
            ..TrackEvent::default()
        });
        a_track.events.push(TrackEvent {
            id: a_id,
            start: 2.0,
            duration: 4.0,
            snap_offset: 1.25,
            ..TrackEvent::default()
        });
        v_track.events.push(TrackEvent {
            id: tail_id,
            start: 5.0,
            duration: 2.5,
            snap_offset: 0.8,
            ..TrackEvent::default()
        });
        let vt = v_track.id;
        let at = a_track.id;
        tl.tracks.push(v_track);
        tl.tracks.push(a_track);

        // Synchronous A/V group plus trailing clip: leftmost is at 2.0 with audio snap_offset 1.25 -> 3.25; rightmost end is 7.5
        let (earliest, left_snap, rightmost) =
            move_span(&tl, &[(vt, v_id), (at, a_id), (vt, tail_id)]).unwrap();
        assert!((earliest - 2.0).abs() < 1e-9);
        assert!((left_snap - 3.25).abs() < 1e-9);
        assert!((rightmost - 7.5).abs() < 1e-9);

        // L-cut where video starts earlier (1.0, snap_offset = 0.0) than audio (2.0, snap_offset = 1.25): leftmost is video -> 1.0
        tl.tracks[0].events[0].start = 1.0;
        let (earliest, left_snap, _) = move_span(&tl, &[(vt, v_id), (at, a_id)]).unwrap();
        assert!((earliest - 1.0).abs() < 1e-9);
        assert!((left_snap - 1.0).abs() < 1e-9);
    }
}
