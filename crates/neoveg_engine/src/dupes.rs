//! *Dupe detection*: finds the events that repeat the same part of one media file, so footage
//! that is cleared or paid for by the use is not counted twice by mistake. Pure and tested without a window.

use crate::edit::clamp_playback_rate;
use crate::selection::EventRef;
use crate::timeline::{Timeline, TrackKind, TIME_EPSILON};

/// Events of one kind of track that use overlapping parts of the same media.
#[derive(Debug, Clone, PartialEq)]
pub struct DupeGroup {
    pub media_id: u64,
    /// The events, in time order.
    pub events: Vec<EventRef>,
    /// The part of the media that at least two of them show, in seconds of the source.
    pub shared: (f64, f64),
}

struct Span {
    event: EventRef,
    kind: TrackKind,
    media: u64,
    start: f64,
    from: f64,
    to: f64,
}

fn root(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

/// Groups of events that repeat a part of a media file by more than `min_overlap` seconds of the
/// source. A picture and its own sound (same media, different kinds of track) are not duplicates of
/// each other. Groups come in the order of the media id, events in time order.
pub fn find_dupes(timeline: &Timeline, min_overlap: f64) -> Vec<DupeGroup> {
    let mut spans = Vec::new();
    for track in &timeline.tracks {
        for e in &track.events {
            let Some(media) = e.media_id else { continue };
            if e.adjustment || e.duration <= 0.0 {
                continue;
            }
            let len = e.duration * clamp_playback_rate(e.playback_rate);
            spans.push(Span {
                event: (track.id, e.id),
                kind: track.kind,
                media,
                start: e.start,
                from: e.trim_in,
                to: e.trim_in + len,
            });
        }
    }
    let n = spans.len();
    let mut parent: Vec<usize> = (0..n).collect();
    let threshold = min_overlap.max(0.0) + TIME_EPSILON;
    let mut shared: Vec<Option<(f64, f64)>> = vec![None; n];
    for i in 0..n {
        for j in (i + 1)..n {
            let (a, b) = (&spans[i], &spans[j]);
            if a.media != b.media || a.kind != b.kind {
                continue;
            }
            let (lo, hi) = (a.from.max(b.from), a.to.min(b.to));
            if hi - lo > threshold {
                let (ri, rj) = (root(&mut parent, i), root(&mut parent, j));
                parent[rj] = ri;
                for k in [i, j] {
                    shared[k] = Some(match shared[k] {
                        Some((l, h)) => (l.min(lo), h.max(hi)),
                        None => (lo, hi),
                    });
                }
            }
        }
    }
    let mut groups: Vec<(u64, Vec<usize>)> = Vec::new();
    let mut by_root: std::collections::BTreeMap<usize, usize> = Default::default();
    for i in 0..n {
        if shared[i].is_none() {
            continue;
        }
        let r = root(&mut parent, i);
        let slot = *by_root.entry(r).or_insert_with(|| {
            groups.push((spans[i].media, Vec::new()));
            groups.len() - 1
        });
        groups[slot].1.push(i);
    }
    let mut out: Vec<DupeGroup> = groups
        .into_iter()
        .map(|(media_id, mut members)| {
            members.sort_by(|&a, &b| spans[a].start.total_cmp(&spans[b].start));
            let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
            for &m in &members {
                if let Some((l, h)) = shared[m] {
                    lo = lo.min(l);
                    hi = hi.max(h);
                }
            }
            DupeGroup {
                media_id,
                events: members.iter().map(|&m| spans[m].event).collect(),
                shared: (lo, hi),
            }
        })
        .collect();
    out.sort_by_key(|g| g.media_id);
    out
}

/// Every event that is part of a group, to select them.
pub fn all_events(groups: &[DupeGroup]) -> Vec<EventRef> {
    groups
        .iter()
        .flat_map(|g| g.events.iter().copied())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::TrackEvent;

    fn add(tl: &mut Timeline, track: usize, media: u64, start: f64, dur: f64, trim: f64) {
        let id = tl.alloc_id();
        tl.tracks[track].events.push(TrackEvent {
            id,
            media_id: Some(media),
            start,
            duration: dur,
            trim_in: trim,
            ..TrackEvent::default()
        });
    }

    fn timeline() -> Timeline {
        let mut tl = Timeline::new();
        let v = tl.new_track(TrackKind::Video);
        let a = tl.new_track(TrackKind::Audio);
        tl.tracks.extend([v, a]);
        tl
    }

    #[test]
    fn events_that_repeat_a_part_of_the_same_media_form_a_group() {
        let mut tl = timeline();
        add(&mut tl, 0, 1, 0.0, 4.0, 0.0); // source 0..4
        add(&mut tl, 0, 1, 10.0, 4.0, 3.0); // source 3..7: repeats 3..4
        add(&mut tl, 0, 1, 20.0, 2.0, 50.0); // alone
        add(&mut tl, 0, 2, 30.0, 2.0, 0.0); // other media
        let groups = find_dupes(&tl, 0.0);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].media_id, 1);
        assert_eq!(groups[0].events.len(), 2);
        assert_eq!(groups[0].shared, (3.0, 4.0));
        assert_eq!(all_events(&groups).len(), 2);
    }

    #[test]
    fn touching_parts_and_a_picture_with_its_own_sound_are_not_dupes() {
        let mut tl = timeline();
        add(&mut tl, 0, 1, 0.0, 4.0, 0.0);
        add(&mut tl, 0, 1, 10.0, 4.0, 4.0); // continues where the first ends
        add(&mut tl, 1, 1, 0.0, 4.0, 0.0); // the sound of the first
        assert!(find_dupes(&tl, 0.0).is_empty());
    }

    #[test]
    fn the_minimum_overlap_and_the_speed_are_respected() {
        let mut tl = timeline();
        add(&mut tl, 0, 1, 0.0, 2.0, 0.0);
        add(&mut tl, 0, 1, 10.0, 2.0, 1.5); // overlaps 0.5 s
        assert_eq!(find_dupes(&tl, 0.0).len(), 1);
        assert!(find_dupes(&tl, 1.0).is_empty());
        // At double speed the first one shows 4 s of the source
        tl.tracks[0].events[0].playback_rate = 2.0;
        assert_eq!(find_dupes(&tl, 1.0).len(), 1);
    }

    #[test]
    fn three_events_on_one_part_make_one_group() {
        let mut tl = timeline();
        for s in [0.0, 10.0, 20.0] {
            add(&mut tl, 0, 1, s, 3.0, 0.0);
        }
        let groups = find_dupes(&tl, 0.0);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].events.len(), 3);
    }
}
