//! Automatic crossfades between overlapping events on the same track.
//!
//! Two events crossfade when they overlap *partially*: the later one starts inside the earlier one and
//! ends after it. The earlier event fades out and the later one fades in over the overlap, using the
//! later event's fade-in curve (mirrored for the fade-out). An event lying completely inside another
//! is not crossfaded.

use crate::timeline::{EventId, TrackEvent, TIME_EPSILON};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Crossfade {
    pub earlier: EventId,
    pub later: EventId,
    pub start: f64,
    pub end: f64,
}

impl Crossfade {
    pub fn length(&self) -> f64 {
        self.end - self.start
    }
}

pub fn find_crossfades(events: &[TrackEvent]) -> Vec<Crossfade> {
    let mut found = Vec::new();
    for a in events {
        for b in events {
            if a.id == b.id {
                continue;
            }
            let b_starts_inside_a =
                b.start > a.start + TIME_EPSILON && b.start < a.end() - TIME_EPSILON;
            let b_ends_after_a = b.end() > a.end() + TIME_EPSILON;
            if b_starts_inside_a && b_ends_after_a {
                found.push(Crossfade {
                    earlier: a.id,
                    later: b.id,
                    start: b.start,
                    end: a.end(),
                });
            }
        }
    }
    found.sort_by(|x, y| x.start.total_cmp(&y.start));
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use neoveg_common::fade::FadeCurve;

    fn ev(id: u64, start: f64, duration: f64) -> TrackEvent {
        TrackEvent {
            group: None,
            id,
            name: format!("e{id}"),
            media_id: Some(1),
            start,
            duration,
            trim_in: 0.0,
            fade_in: 0.0,
            fade_out: 0.0,
            fade_in_curve: FadeCurve::default(),
            fade_out_curve: FadeCurve::default(),
        }
    }

    #[test]
    fn partial_overlap_crossfades_over_the_overlap() {
        let found = find_crossfades(&[ev(1, 0.0, 10.0), ev(2, 8.0, 6.0)]);
        assert_eq!(
            found,
            vec![Crossfade {
                earlier: 1,
                later: 2,
                start: 8.0,
                end: 10.0
            }]
        );
        assert!((found[0].length() - 2.0).abs() < 1e-12);
    }

    #[test]
    fn order_in_the_list_does_not_matter() {
        assert_eq!(
            find_crossfades(&[ev(2, 8.0, 6.0), ev(1, 0.0, 10.0)]),
            find_crossfades(&[ev(1, 0.0, 10.0), ev(2, 8.0, 6.0)])
        );
    }

    #[test]
    fn touching_separate_and_nested_events_do_not_crossfade() {
        assert!(find_crossfades(&[ev(1, 0.0, 5.0), ev(2, 5.0, 5.0)]).is_empty());
        assert!(find_crossfades(&[ev(1, 0.0, 5.0), ev(2, 6.0, 5.0)]).is_empty());
        // event 2 is entirely inside event 1
        assert!(find_crossfades(&[ev(1, 0.0, 10.0), ev(2, 2.0, 3.0)]).is_empty());
        // same start: neither starts inside the other
        assert!(find_crossfades(&[ev(1, 0.0, 10.0), ev(2, 0.0, 12.0)]).is_empty());
    }

    #[test]
    fn a_chain_of_overlaps_gives_one_crossfade_per_junction() {
        let found = find_crossfades(&[ev(1, 0.0, 6.0), ev(2, 5.0, 6.0), ev(3, 10.0, 6.0)]);
        assert_eq!(found.len(), 2);
        assert_eq!((found[0].earlier, found[0].later), (1, 2));
        assert_eq!((found[1].earlier, found[1].later), (2, 3));
    }
}
