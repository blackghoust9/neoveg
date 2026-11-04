//! Cursor navigation: project end and the edit points (event edges and fade edges) the cursor jumps
//! between. Pure functions over the timeline, so the jumps are testable without a window.

use crate::timeline::{Timeline, TIME_EPSILON};

/// End of the last event, seconds (`0` for an empty timeline).
pub fn project_end(timeline: &Timeline) -> f64 {
    timeline
        .tracks
        .iter()
        .flat_map(|t| t.events.iter().map(|e| e.end()))
        .fold(0.0, f64::max)
}

/// Every edit point of the timeline, sorted and without duplicates: the edges of the events and the
/// edges of their fades.
pub fn edit_points(timeline: &Timeline) -> Vec<f64> {
    let mut points: Vec<f64> = Vec::new();
    for event in timeline.tracks.iter().flat_map(|t| &t.events) {
        points.push(event.start);
        points.push(event.end());
        if event.fade_in > 0.0 {
            points.push(event.start + event.fade_in);
        }
        if event.fade_out > 0.0 {
            points.push(event.end() - event.fade_out);
        }
    }
    points.retain(|p| p.is_finite());
    points.sort_by(f64::total_cmp);
    points.dedup_by(|a, b| (*a - *b).abs() < TIME_EPSILON);
    points
}

/// The closest edit point after `from`.
pub fn next_edit_point(timeline: &Timeline, from: f64) -> Option<f64> {
    edit_points(timeline)
        .into_iter()
        .find(|p| *p > from + TIME_EPSILON)
}

/// The closest edit point before `from`.
pub fn previous_edit_point(timeline: &Timeline, from: f64) -> Option<f64> {
    edit_points(timeline)
        .into_iter()
        .rev()
        .find(|p| *p < from - TIME_EPSILON)
}

/// Reads a time typed in the "go to" box: a timecode (`00:01:02:10`, `;` for drop-frame), minutes
/// and seconds (`1:02.5`), seconds (`62,5` or `62.5s`) or frames (`1500f`). `None` when it is
/// none of these or is negative.
pub fn parse_time(text: &str, fps: f64) -> Option<f64> {
    let text = text.trim();
    if text.is_empty() || fps <= 0.0 {
        return None;
    }
    if let Some((frame, _)) = neoveg_common::timecode::parse_timecode(text, fps) {
        return (frame >= 0).then(|| frame as f64 / fps);
    }
    if let Some(frames) = text.strip_suffix(['f', 'F']) {
        let n: f64 = frames.trim().parse().ok()?;
        return (n >= 0.0 && n.is_finite()).then(|| n / fps);
    }
    let plain = text
        .strip_suffix(['s', 'S'])
        .unwrap_or(text)
        .replace(',', ".");
    let value = match plain.split(':').collect::<Vec<_>>().as_slice() {
        [secs] => secs.trim().parse::<f64>().ok()?,
        [mins, secs] => {
            mins.trim().parse::<f64>().ok()? * 60.0 + secs.trim().parse::<f64>().ok()?
        }
        [hours, mins, secs] => {
            hours.trim().parse::<f64>().ok()? * 3600.0
                + mins.trim().parse::<f64>().ok()? * 60.0
                + secs.trim().parse::<f64>().ok()?
        }
        _ => return None,
    };
    (value >= 0.0 && value.is_finite()).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::{TrackEvent, TrackKind};

    fn timeline() -> Timeline {
        let mut tl = Timeline::new();
        let mut a = tl.new_track(TrackKind::Audio);
        let mut b = tl.new_track(TrackKind::Audio);
        // Built from JSON so the test does not depend on every field the event has grown
        let add = |tl: &mut Timeline,
                   track: &mut crate::timeline::Track,
                   start: f64,
                   duration: f64,
                   fade_in: f64| {
            let event: TrackEvent = serde_json::from_value(serde_json::json!({
                "id": tl.alloc_id(),
                "name": "",
                "media_id": null,
                "start": start,
                "duration": duration,
                "trim_in": 0.0,
                "fade_in": fade_in,
                "fade_out": 0.0,
            }))
            .unwrap();
            track.events.push(event);
        };
        add(&mut tl, &mut a, 1.0, 2.0, 0.5);
        add(&mut tl, &mut b, 2.0, 4.0, 0.0);
        tl.tracks.push(a);
        tl.tracks.push(b);
        tl
    }

    #[test]
    fn edit_points_are_sorted_and_include_fade_edges() {
        assert_eq!(edit_points(&timeline()), vec![1.0, 1.5, 2.0, 3.0, 6.0]);
        assert_eq!(project_end(&timeline()), 6.0);
        assert_eq!(project_end(&Timeline::new()), 0.0);
        assert!(edit_points(&Timeline::new()).is_empty());
    }

    #[test]
    fn typed_times_are_read_in_every_form() {
        let close = |a: Option<f64>, b: f64| a.is_some_and(|v| (v - b).abs() < 1e-6);
        assert!(close(parse_time("00:00:02:12", 24.0), 2.5));
        assert!(close(parse_time("1:02.5", 25.0), 62.5));
        assert!(close(parse_time("62,5", 25.0), 62.5));
        assert!(close(parse_time("62.5s", 25.0), 62.5));
        assert!(close(parse_time("1500f", 25.0), 60.0));
        assert!(close(parse_time("0:1:2", 25.0), 62.0));
        assert!(parse_time("", 25.0).is_none());
        assert!(parse_time("abc", 25.0).is_none());
        assert!(parse_time("-3", 25.0).is_none());
        assert!(parse_time("1:2:3:4:5", 25.0).is_none());
        assert!(parse_time("10", 0.0).is_none());
    }

    #[test]
    fn the_cursor_jumps_to_the_next_and_previous_point() {
        let tl = timeline();
        assert_eq!(next_edit_point(&tl, 0.0), Some(1.0));
        assert_eq!(next_edit_point(&tl, 1.0), Some(1.5));
        assert_eq!(next_edit_point(&tl, 3.0), Some(6.0));
        assert_eq!(next_edit_point(&tl, 6.0), None);
        assert_eq!(previous_edit_point(&tl, 6.0), Some(3.0));
        assert_eq!(previous_edit_point(&tl, 1.2), Some(1.0));
        assert_eq!(previous_edit_point(&tl, 1.0), None);
    }
}
