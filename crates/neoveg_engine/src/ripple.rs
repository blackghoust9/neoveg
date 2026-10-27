//! Ripple edits that do not come from a time selection: deleting events and closing the gap.

use crate::command::{EditCommand, FxTarget};
use crate::ops::{Draft, Outcome};
use crate::selection::EventRef;
use crate::timeline::{Timeline, TrackEvent, TrackId, TIME_EPSILON};

/// Removes the given events and moves every later event of the same track earlier by the length of
/// the gaps that were opened before it (overlapping removals count once). One undo step.
pub fn ripple_delete_events(timeline: &Timeline, events: &[EventRef]) -> Option<Outcome> {
    let events = &crate::switches::unlocked(timeline, events);
    let mut draft = Draft::new(timeline);
    let track_ids: Vec<TrackId> = events.iter().map(|(t, _)| *t).collect();
    let mut seen: Vec<TrackId> = Vec::new();
    for track_id in track_ids {
        if seen.contains(&track_id) {
            continue;
        }
        seen.push(track_id);
        let Some(list) = draft.list(track_id) else {
            continue;
        };
        // The spans being removed, merged where they overlap or touch
        let mut gaps: Vec<(f64, f64)> = list
            .iter()
            .filter(|e| events.contains(&(track_id, e.id)))
            .map(|e| (e.start, e.end()))
            .collect();
        gaps.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut merged: Vec<(f64, f64)> = Vec::new();
        for gap in gaps {
            match merged.last_mut() {
                Some(last) if gap.0 <= last.1 + TIME_EPSILON => last.1 = last.1.max(gap.1),
                _ => merged.push(gap),
            }
        }
        list.retain(|e| !events.contains(&(track_id, e.id)));
        for event in list.iter_mut() {
            event.start -= shift_for(event, &merged);
        }
    }
    draft.finish().map(|command| Outcome {
        command,
        touched: Vec::new(),
    })
}

/// What a post-edit ripple carries along with the edited tracks (Auto Ripple scopes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RippleScope {
    /// Events, keyframes and envelopes of the edited tracks only.
    #[default]
    AffectedTracks,
    /// The edited tracks plus the markers and regions of the ruler.
    AffectedTracksAndMarks,
    /// Every track plus the markers and regions.
    AllTracksAndMarks,
}

impl RippleScope {
    pub const ALL: [RippleScope; 3] = [
        RippleScope::AffectedTracks,
        RippleScope::AffectedTracksAndMarks,
        RippleScope::AllTracksAndMarks,
    ];

    /// `true` when markers and regions move too.
    pub fn includes_marks(self) -> bool {
        !matches!(self, RippleScope::AffectedTracks)
    }

    /// `true` when tracks that were not edited move too.
    pub fn includes_all_tracks(self) -> bool {
        matches!(self, RippleScope::AllTracksAndMarks)
    }

    /// Label shown in the default language (pt-BR).
    pub fn label(self) -> &'static str {
        match self {
            RippleScope::AffectedTracks => "Faixas afetadas",
            RippleScope::AffectedTracksAndMarks => "Faixas afetadas, marcadores e regiões",
            RippleScope::AllTracksAndMarks => "Todas as faixas, marcadores e regiões",
        }
    }
}

/// Post-edit ripple: everything at or after `from` moves by `delta` seconds (negative closes a gap,
/// positive opens one) on the tracks `scope` selects. Locked events stay where they are. One undo
/// step; `None` when `delta` is zero or nothing moves.
pub fn ripple_after(
    timeline: &Timeline,
    edited: &[TrackId],
    from: f64,
    delta: f64,
    scope: RippleScope,
) -> Option<Outcome> {
    if !delta.is_finite() || !from.is_finite() || delta.abs() < TIME_EPSILON {
        return None;
    }
    let targets: Vec<TrackId> = timeline
        .tracks
        .iter()
        .map(|t| t.id)
        .filter(|id| scope.includes_all_tracks() || edited.contains(id))
        .collect();
    let moves = |time: f64| time >= from - TIME_EPSILON;
    let shifted = |time: f64| (time + delta).max(0.0);

    let mut commands = Vec::new();
    let mut draft = Draft::new(timeline);
    for &track_id in &targets {
        if let Some(list) = draft.list(track_id) {
            for event in list.iter_mut().filter(|e| !e.locked && moves(e.start)) {
                event.start = shifted(event.start);
            }
        }
    }
    commands.extend(draft.finish());

    for track in timeline.tracks.iter().filter(|t| targets.contains(&t.id)) {
        if !track
            .envelopes
            .iter()
            .any(|e| e.points.iter().any(|p| moves(p.time)))
        {
            continue;
        }
        let mut after = track.envelopes.clone();
        for point in after.iter_mut().flat_map(|e| e.points.iter_mut()) {
            if moves(point.time) {
                point.time = shifted(point.time);
            }
        }
        for envelope in &mut after {
            envelope.sort_points();
        }
        commands.push(EditCommand::SetEnvelopes {
            target: FxTarget::Track(track.id),
            before: track.envelopes.clone(),
            after,
        });
    }

    if scope.includes_marks() {
        let before = timeline.marks();
        let mut after = before.clone();
        for marker in after.markers.iter_mut().filter(|m| moves(m.time)) {
            marker.time = shifted(marker.time);
        }
        for region in &mut after.regions {
            let (start, end) = (region.start, region.end);
            let new_start = if moves(start) { shifted(start) } else { start };
            let new_end = if moves(end) { shifted(end) } else { end };
            // A region squeezed to nothing keeps its length instead of vanishing
            if new_end > new_start + TIME_EPSILON {
                region.start = new_start;
                region.end = new_end;
            }
        }
        after.markers.sort_by(|a, b| a.time.total_cmp(&b.time));
        after.regions.sort_by(|a, b| a.start.total_cmp(&b.start));
        if after != before {
            commands.push(EditCommand::ReplaceMarks { before, after });
        }
    }

    let command = match commands.len() {
        0 => return None,
        1 => commands.remove(0),
        _ => EditCommand::Batch(commands),
    };
    Some(Outcome {
        command,
        touched: Vec::new(),
    })
}

/// *Shuffle*: moves `event` to the slot where `drop_time` falls among the other events of its track
/// and lays the sequence out again from the first event's start, keeping the gap lengths in their
/// original order. Needs a plain sequence: `None` when events overlap, any of them is locked, the
/// event is unknown, or the order would not change. One undo step.
pub fn shuffle_events(
    timeline: &Timeline,
    track_id: TrackId,
    event_id: crate::timeline::EventId,
    drop_time: f64,
) -> Option<Outcome> {
    let track = timeline.tracks.iter().find(|t| t.id == track_id)?;
    let mut events: Vec<TrackEvent> = track.events.clone();
    events.sort_by(|a, b| a.start.total_cmp(&b.start));
    let from = events.iter().position(|e| e.id == event_id)?;
    if events.iter().any(|e| e.locked)
        || events
            .windows(2)
            .any(|w| w[1].start < w[0].end() - TIME_EPSILON)
    {
        return None;
    }
    let origin = events[0].start;
    let gaps: Vec<f64> = events
        .windows(2)
        .map(|w| (w[1].start - w[0].end()).max(0.0))
        .collect();
    let moved = events.remove(from);
    // The slot is the first remaining event whose midpoint lies after the drop point
    let to = events
        .iter()
        .position(|e| e.start + e.duration / 2.0 > drop_time)
        .unwrap_or(events.len());
    if to == from {
        return None;
    }
    events.insert(to, moved);
    let mut cursor = origin;
    for (i, event) in events.iter_mut().enumerate() {
        event.start = cursor;
        cursor += event.duration + gaps.get(i).copied().unwrap_or(0.0);
    }
    let mut draft = Draft::new(timeline);
    *draft.list(track_id)? = events;
    draft.finish().map(|command| Outcome {
        command,
        touched: vec![(track_id, event_id)],
    })
}

/// Auto Ripple for a trim or a stretch: the event takes the geometry `new` but keeps its start (a
/// left-edge trim closes up on the left), and everything after the old end moves by the change in
/// length, on the tracks `scope` selects. The resize and the ripple are one undo step; `None` when
/// the length does not change or the event is missing or locked.
pub fn ripple_resize(
    timeline: &Timeline,
    track_id: TrackId,
    event_id: crate::timeline::EventId,
    new: crate::timeline::EventGeometry,
    scope: RippleScope,
) -> Option<Outcome> {
    let event = timeline
        .track(track_id)?
        .events
        .iter()
        .find(|e| e.id == event_id)?;
    if event.locked {
        return None;
    }
    let old = event.geometry();
    let delta = new.duration - old.duration;
    if delta.abs() < TIME_EPSILON {
        return None;
    }
    let resized = crate::timeline::EventGeometry {
        start: old.start,
        ..new
    };
    let edit = EditCommand::EditEvent {
        from_track: track_id,
        to_track: track_id,
        event_id,
        old,
        new: resized,
    };
    // The ripple is computed on the timeline as it is after the resize
    let mut after = timeline.clone();
    let mut scratch = crate::command::History::new();
    if !scratch.execute(&mut after, edit.clone()) {
        return None;
    }
    let ripple = ripple_after(&after, &[track_id], old.end(), delta, scope)?;
    Some(Outcome {
        command: EditCommand::Batch(vec![edit, ripple.command]),
        touched: vec![(track_id, event_id)],
    })
}

/// What a manual post-edit ripple (`F`) should do after an edit that left a gap or an overlap.
#[derive(Debug, Clone, PartialEq)]
pub struct RippleHint {
    pub tracks: Vec<TrackId>,
    /// Everything at or after this time moves.
    pub from: f64,
    /// By how much (negative closes a gap).
    pub delta: f64,
}

impl RippleHint {
    /// The ripple this hint stands for, under `scope`.
    pub fn outcome(&self, timeline: &Timeline, scope: RippleScope) -> Option<Outcome> {
        ripple_after(timeline, &self.tracks, self.from, self.delta, scope)
    }
}

/// The hint for removing `events` without rippling: only for one contiguous span on one track.
/// Call it *before* the removal, while the events still exist.
pub fn removal_hint(timeline: &Timeline, events: &[EventRef]) -> Option<RippleHint> {
    let events = crate::switches::unlocked(timeline, events);
    let track_id = events.first()?.0;
    if events.iter().any(|(t, _)| *t != track_id) {
        return None;
    }
    let track = timeline.track(track_id)?;
    let spans = merged_spans(
        track
            .events
            .iter()
            .filter(|e| events.contains(&(track_id, e.id)))
            .map(|e| (e.start, e.end())),
    );
    match spans.as_slice() {
        [(start, end)] => Some(RippleHint {
            tracks: vec![track_id],
            from: *end,
            delta: -(end - start),
        }),
        _ => None,
    }
}

/// The hint for an event that changed length from `old` to `new` without rippling.
pub fn resize_hint(
    track_id: TrackId,
    old: crate::timeline::EventGeometry,
    new: crate::timeline::EventGeometry,
) -> Option<RippleHint> {
    let delta = new.duration - old.duration;
    (delta.abs() >= TIME_EPSILON).then(|| RippleHint {
        tracks: vec![track_id],
        from: old.end(),
        delta,
    })
}

/// Merged `(start, end)` spans of the given events, sorted by start.
fn merged_spans(spans: impl Iterator<Item = (f64, f64)>) -> Vec<(f64, f64)> {
    let mut spans: Vec<(f64, f64)> = spans.collect();
    spans.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut merged: Vec<(f64, f64)> = Vec::new();
    for span in spans {
        match merged.last_mut() {
            Some(last) if span.0 <= last.1 + TIME_EPSILON => last.1 = last.1.max(span.1),
            _ => merged.push(span),
        }
    }
    merged
}

/// Where `time` lands once every span in `spans` is closed; a time inside a span collapses onto its start.
fn close_spans(time: f64, spans: &[(f64, f64)]) -> f64 {
    let removed: f64 = spans
        .iter()
        .map(|&(start, end)| {
            if end <= time + TIME_EPSILON {
                end - start
            } else if start < time {
                time - start
            } else {
                0.0
            }
        })
        .sum();
    (time - removed).max(0.0)
}

/// Ripple delete that honours a [`RippleScope`]: the events are removed and the gap closes on their
/// tracks (as [`ripple_delete_events`]); the keyframes and envelopes of those tracks follow, and the
/// scope adds the markers and regions and/or every other track, closing the union of the removed
/// spans. Events of other tracks that start inside a removed span collapse onto its start. One undo step.
pub fn ripple_delete_scoped(
    timeline: &Timeline,
    events: &[EventRef],
    scope: RippleScope,
) -> Option<Outcome> {
    let events = crate::switches::unlocked(timeline, events);
    let mut edited: Vec<TrackId> = Vec::new();
    let mut all_spans: Vec<(f64, f64)> = Vec::new();
    for track in &timeline.tracks {
        let spans = merged_spans(
            track
                .events
                .iter()
                .filter(|e| events.contains(&(track.id, e.id)))
                .map(|e| (e.start, e.end())),
        );
        if !spans.is_empty() {
            edited.push(track.id);
            all_spans.extend(spans);
        }
    }
    if edited.is_empty() {
        return None;
    }
    let union = merged_spans(all_spans.into_iter());
    let mut commands = Vec::new();

    // Events: edited tracks close their own gaps, other tracks follow only for the all-tracks scope
    let mut draft = Draft::new(timeline);
    for track in &timeline.tracks {
        let own = edited.contains(&track.id);
        if !own && !scope.includes_all_tracks() {
            continue;
        }
        let spans = if own {
            merged_spans(
                track
                    .events
                    .iter()
                    .filter(|e| events.contains(&(track.id, e.id)))
                    .map(|e| (e.start, e.end())),
            )
        } else {
            union.clone()
        };
        if let Some(list) = draft.list(track.id) {
            list.retain(|e| !(own && events.contains(&(track.id, e.id))));
            for event in list.iter_mut().filter(|e| own || !e.locked) {
                event.start = close_spans(event.start, &spans);
            }
        }
    }
    commands.extend(draft.finish());

    // Envelopes follow the tracks that moved
    for track in &timeline.tracks {
        if !(edited.contains(&track.id) || scope.includes_all_tracks()) {
            continue;
        }
        let spans = if edited.contains(&track.id) {
            merged_spans(
                track
                    .events
                    .iter()
                    .filter(|e| events.contains(&(track.id, e.id)))
                    .map(|e| (e.start, e.end())),
            )
        } else {
            union.clone()
        };
        let mut after = track.envelopes.clone();
        for point in after.iter_mut().flat_map(|e| e.points.iter_mut()) {
            point.time = close_spans(point.time, &spans);
        }
        for envelope in &mut after {
            envelope.sort_points();
        }
        if after != track.envelopes {
            commands.push(EditCommand::SetEnvelopes {
                target: FxTarget::Track(track.id),
                before: track.envelopes.clone(),
                after,
            });
        }
    }

    if scope.includes_marks() {
        let before = timeline.marks();
        let mut after = before.clone();
        for marker in &mut after.markers {
            marker.time = close_spans(marker.time, &union);
        }
        for region in &mut after.regions {
            let (start, end) = (
                close_spans(region.start, &union),
                close_spans(region.end, &union),
            );
            if end > start + TIME_EPSILON {
                region.start = start;
                region.end = end;
            }
        }
        after.markers.sort_by(|a, b| a.time.total_cmp(&b.time));
        after.regions.sort_by(|a, b| a.start.total_cmp(&b.start));
        if after != before {
            commands.push(EditCommand::ReplaceMarks { before, after });
        }
    }

    let command = match commands.len() {
        0 => return None,
        1 => commands.remove(0),
        _ => EditCommand::Batch(commands),
    };
    Some(Outcome {
        command,
        touched: Vec::new(),
    })
}

/// How far `event` moves left: the total length of the removed spans that end at or before its start.
fn shift_for(event: &TrackEvent, gaps: &[(f64, f64)]) -> f64 {
    gaps.iter()
        .filter(|(_, end)| *end <= event.start + TIME_EPSILON)
        .map(|(start, end)| end - start)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;
    use crate::timeline::TrackKind;

    fn timeline() -> (Timeline, TrackId, Vec<u64>) {
        let mut tl = Timeline::new();
        let mut track = tl.new_track(TrackKind::Audio);
        let mut ids = Vec::new();
        for (start, duration) in [(0.0, 2.0), (2.0, 3.0), (6.0, 1.0), (10.0, 2.0)] {
            let event = TrackEvent {
                id: tl.alloc_id(),
                start,
                duration,
                media_id: Some(1),
                ..TrackEvent::default()
            };
            ids.push(event.id);
            track.events.push(event);
        }
        let id = track.id;
        tl.tracks.push(track);
        (tl, id, ids)
    }

    fn starts(tl: &Timeline) -> Vec<f64> {
        tl.tracks[0].events.iter().map(|e| e.start).collect()
    }

    #[test]
    fn later_events_close_the_gap_of_a_deleted_one() {
        let (mut tl, track, ids) = timeline();
        let outcome = ripple_delete_events(&tl, &[(track, ids[1])]).unwrap();
        let mut history = History::new();
        assert!(history.execute(&mut tl, outcome.command));
        // The 3 s event is gone; the two after it move 3 s earlier
        assert_eq!(starts(&tl), vec![0.0, 3.0, 7.0]);
        assert!(history.undo(&mut tl));
        assert_eq!(starts(&tl), vec![0.0, 2.0, 6.0, 10.0]);
    }

    #[test]
    fn several_deletions_add_up_and_only_count_gaps_before_each_event() {
        let (tl, track, ids) = timeline();
        let outcome = ripple_delete_events(&tl, &[(track, ids[0]), (track, ids[2])]).unwrap();
        let mut copy = tl.clone();
        let mut history = History::new();
        assert!(history.execute(&mut copy, outcome.command));
        // Removed: 0-2 (2 s) and 6-7 (1 s). Event at 2.0 moves 2 s, the one at 10.0 moves 3 s
        assert_eq!(starts(&copy), vec![0.0, 7.0]);
    }

    #[test]
    fn other_tracks_and_unknown_events_are_left_alone() {
        let (mut tl, track, ids) = timeline();
        let mut other = tl.new_track(TrackKind::Video);
        other.events.push(TrackEvent {
            id: tl.alloc_id(),
            start: 8.0,
            duration: 1.0,
            ..TrackEvent::default()
        });
        tl.tracks.push(other);
        let outcome = ripple_delete_events(&tl, &[(track, ids[1])]).unwrap();
        let mut history = History::new();
        assert!(history.execute(&mut tl, outcome.command));
        assert_eq!(
            tl.tracks[1].events[0].start, 8.0,
            "the video track did not ripple"
        );
        assert!(ripple_delete_events(&tl, &[(track, 9999)]).is_none());
        assert!(ripple_delete_events(&tl, &[]).is_none());
    }

    /// Audio track (events at 0, 2, 6, 10) + a video track with one event at 8, a marker at 7,
    /// a region 5..9 and a volume envelope point at 6.
    fn rich_timeline() -> (Timeline, TrackId, TrackId) {
        use crate::envelope::{AutomationTarget, EnvelopePoint, EnvelopeShape, TrackEnvelope};
        use crate::markers::{Marker, Region};
        let (mut tl, audio, _) = timeline();
        let mut video = tl.new_track(TrackKind::Video);
        video.events.push(TrackEvent {
            id: tl.alloc_id(),
            start: 8.0,
            duration: 1.0,
            ..TrackEvent::default()
        });
        let video_id = video.id;
        tl.tracks.push(video);
        let mut envelope = TrackEnvelope::new(AutomationTarget::Volume, 0.0);
        envelope.points = vec![
            EnvelopePoint {
                time: 1.0,
                value: 0.0,
                shape: EnvelopeShape::default(),
            },
            EnvelopePoint {
                time: 6.0,
                value: -6.0,
                shape: EnvelopeShape::default(),
            },
        ];
        tl.tracks[0].envelopes.push(envelope);
        tl.markers.push(Marker {
            id: 1,
            time: 7.0,
            name: "m".into(),
        });
        tl.regions.push(Region {
            id: 2,
            start: 5.0,
            end: 9.0,
            name: "r".into(),
        });
        (tl, audio, video_id)
    }

    fn run(tl: &mut Timeline, outcome: Option<Outcome>) -> History {
        let mut history = History::new();
        assert!(history.execute(tl, outcome.expect("ripple produced a command").command));
        history
    }

    #[test]
    fn affected_tracks_moves_only_the_edited_track_and_its_envelope() {
        let (mut tl, audio, _) = rich_timeline();
        let outcome = ripple_after(&tl, &[audio], 5.0, -2.0, RippleScope::AffectedTracks);
        let mut history = run(&mut tl, outcome);
        assert_eq!(starts(&tl), vec![0.0, 2.0, 4.0, 8.0]);
        assert_eq!(tl.tracks[1].events[0].start, 8.0, "other track untouched");
        let times: Vec<f64> = tl.tracks[0].envelopes[0]
            .points
            .iter()
            .map(|p| p.time)
            .collect();
        assert_eq!(times, vec![1.0, 4.0]);
        assert_eq!(
            (tl.markers[0].time, tl.regions[0].start),
            (7.0, 5.0),
            "marks untouched"
        );
        assert!(history.undo(&mut tl));
        assert_eq!(starts(&tl), vec![0.0, 2.0, 6.0, 10.0]);
        assert_eq!(tl.tracks[0].envelopes[0].points[1].time, 6.0);
    }

    #[test]
    fn marks_scope_moves_markers_and_stretches_a_straddling_region() {
        let (mut tl, audio, _) = rich_timeline();
        let outcome = ripple_after(&tl, &[audio], 6.0, 3.0, RippleScope::AffectedTracksAndMarks);
        let mut history = run(&mut tl, outcome);
        assert_eq!(tl.markers[0].time, 10.0);
        assert_eq!((tl.regions[0].start, tl.regions[0].end), (5.0, 12.0));
        assert_eq!(
            tl.tracks[1].events[0].start, 8.0,
            "video track not affected"
        );
        assert!(history.undo(&mut tl));
        assert_eq!((tl.markers[0].time, tl.regions[0].end), (7.0, 9.0));
    }

    #[test]
    fn all_tracks_scope_moves_untouched_tracks_too() {
        let (mut tl, audio, _) = rich_timeline();
        let outcome = ripple_after(&tl, &[audio], 6.0, -1.0, RippleScope::AllTracksAndMarks);
        run(&mut tl, outcome);
        assert_eq!(tl.tracks[1].events[0].start, 7.0);
        assert_eq!(tl.markers[0].time, 6.0);
    }

    #[test]
    fn locked_events_stay_and_nothing_goes_before_zero() {
        let (mut tl, audio, _) = rich_timeline();
        tl.tracks[0].events[2].locked = true;
        let outcome = ripple_after(&tl, &[audio], 1.0, -5.0, RippleScope::AffectedTracks);
        run(&mut tl, outcome);
        // 2.0 -> 0.0 (clamped), 6.0 locked, 10.0 -> 5.0; the list stays sorted by start
        assert_eq!(starts(&tl), vec![0.0, 0.0, 5.0, 6.0]);
    }

    #[test]
    fn no_delta_or_nothing_after_the_point_is_not_an_edit() {
        let (tl, audio, _) = rich_timeline();
        assert!(ripple_after(&tl, &[audio], 0.0, 0.0, RippleScope::AffectedTracks).is_none());
        assert!(ripple_after(&tl, &[audio], 50.0, 2.0, RippleScope::AllTracksAndMarks).is_none());
        assert!(ripple_after(&tl, &[audio], f64::NAN, 2.0, RippleScope::AffectedTracks).is_none());
    }

    #[test]
    fn shuffle_reorders_the_sequence_and_keeps_the_gaps() {
        // Events: 0-2, 2-5, 6-7, 10-12 (gaps 0, 1, 3)
        let (mut tl, track, ids) = timeline();
        let outcome = shuffle_events(&tl, track, ids[0], 6.5).unwrap();
        let mut history = History::new();
        assert!(history.execute(&mut tl, outcome.command));
        let order: Vec<u64> = tl.tracks[0].events.iter().map(|e| e.id).collect();
        assert_eq!(order, vec![ids[1], ids[2], ids[0], ids[3]]);
        // 3 s + gap 0, 1 s + gap 1, 2 s + gap 3, then the last 2 s event
        assert_eq!(starts(&tl), vec![0.0, 3.0, 5.0, 10.0]);
        assert!(history.undo(&mut tl));
        assert_eq!(starts(&tl), vec![0.0, 2.0, 6.0, 10.0]);
    }

    #[test]
    fn shuffle_refuses_what_is_not_a_plain_sequence() {
        let (mut tl, track, ids) = timeline();
        assert!(
            shuffle_events(&tl, track, ids[0], 0.5).is_none(),
            "same slot"
        );
        assert!(
            shuffle_events(&tl, track, 9999, 5.0).is_none(),
            "unknown event"
        );
        tl.tracks[0].events[2].locked = true;
        assert!(
            shuffle_events(&tl, track, ids[0], 11.5).is_none(),
            "locked neighbour"
        );
        tl.tracks[0].events[2].locked = false;
        tl.tracks[0].events[1].start = 1.0;
        assert!(
            shuffle_events(&tl, track, ids[0], 11.5).is_none(),
            "overlap"
        );
    }

    #[test]
    fn scoped_delete_moves_envelopes_marks_and_other_tracks_by_scope() {
        // Audio: 0-2, 2-5, 6-7, 10-12; envelope points at 1 and 6; marker 7; region 5..9; video event at 8.
        let (tl, audio, _) = rich_timeline();
        let ids: Vec<u64> = tl.tracks[0].events.iter().map(|e| e.id).collect();
        let target = [(audio, ids[1])]; // removes 2-5 (3 s)

        let mut own = tl.clone();
        run(
            &mut own,
            ripple_delete_scoped(&tl, &target, RippleScope::AffectedTracks),
        );
        assert_eq!(starts(&own), vec![0.0, 3.0, 7.0]);
        assert_eq!(
            own.tracks[0].envelopes[0].points[1].time, 3.0,
            "envelope follows"
        );
        assert_eq!(
            (own.markers[0].time, own.tracks[1].events[0].start),
            (7.0, 8.0)
        );

        let mut marks = tl.clone();
        run(
            &mut marks,
            ripple_delete_scoped(&tl, &target, RippleScope::AffectedTracksAndMarks),
        );
        assert_eq!(marks.markers[0].time, 4.0);
        assert_eq!((marks.regions[0].start, marks.regions[0].end), (2.0, 6.0));
        assert_eq!(marks.tracks[1].events[0].start, 8.0);

        let mut all = tl.clone();
        let mut history = run(
            &mut all,
            ripple_delete_scoped(&tl, &target, RippleScope::AllTracksAndMarks),
        );
        assert_eq!(all.tracks[1].events[0].start, 5.0);
        assert!(history.undo(&mut all), "everything undoes in one step");
        assert_eq!(all, tl);
    }

    #[test]
    fn scoped_delete_ignores_locked_and_unknown_events() {
        let (mut tl, audio, _) = rich_timeline();
        let id = tl.tracks[0].events[1].id;
        assert!(
            ripple_delete_scoped(&tl, &[(audio, 9999)], RippleScope::AllTracksAndMarks).is_none()
        );
        tl.tracks[0].events[1].locked = true;
        assert!(
            ripple_delete_scoped(&tl, &[(audio, id)], RippleScope::AllTracksAndMarks).is_none()
        );
    }

    #[test]
    fn ripple_resize_shifts_what_follows_by_the_change_in_length() {
        let (tl, audio, _) = rich_timeline();
        let ids: Vec<u64> = tl.tracks[0].events.iter().map(|e| e.id).collect();
        // Event 2-5 loses 1 s from its end: everything after moves 1 s earlier
        let mut g = tl.tracks[0].events[1].geometry();
        g.duration = 2.0;
        let mut shorter = tl.clone();
        let mut history = run(
            &mut shorter,
            ripple_resize(&tl, audio, ids[1], g, RippleScope::AffectedTracksAndMarks),
        );
        assert_eq!(starts(&shorter), vec![0.0, 2.0, 5.0, 9.0]);
        assert_eq!(shorter.tracks[0].events[1].duration, 2.0);
        assert_eq!(shorter.markers[0].time, 6.0);
        assert_eq!(
            shorter.tracks[1].events[0].start, 8.0,
            "video track untouched"
        );
        assert!(history.undo(&mut shorter));
        assert_eq!(shorter, tl, "one undo step restores resize and ripple");

        // Growing opens room instead
        g.duration = 4.5;
        let mut longer = tl.clone();
        run(
            &mut longer,
            ripple_resize(&tl, audio, ids[1], g, RippleScope::AffectedTracks),
        );
        assert_eq!(starts(&longer), vec![0.0, 2.0, 7.5, 11.5]);
    }

    #[test]
    fn ripple_resize_from_the_left_edge_keeps_the_event_start() {
        let (tl, audio, _) = rich_timeline();
        let id = tl.tracks[0].events[1].id;
        // A left trim of 1 s: start would move to 3.0, trim_in 1.0, duration 2.0
        let mut g = tl.tracks[0].events[1].geometry();
        g.start = 3.0;
        g.trim_in += 1.0;
        g.duration = 2.0;
        let mut after = tl.clone();
        run(
            &mut after,
            ripple_resize(&tl, audio, id, g, RippleScope::AffectedTracks),
        );
        let e = &after.tracks[0].events[1];
        assert_eq!((e.start, e.duration, e.trim_in), (2.0, 2.0, 1.0));
        assert_eq!(after.tracks[0].events[2].start, 5.0);
    }

    #[test]
    fn hints_describe_a_removal_or_a_resize_and_replay_it_as_a_ripple() {
        let (tl, audio, _) = rich_timeline();
        let ids: Vec<u64> = tl.tracks[0].events.iter().map(|e| e.id).collect();
        let hint = removal_hint(&tl, &[(audio, ids[1])]).unwrap();
        assert_eq!((hint.from, hint.delta), (5.0, -3.0));
        // Two separate spans or two tracks: no single ripple describes them
        assert!(removal_hint(&tl, &[(audio, ids[0]), (audio, ids[2])]).is_none());
        assert!(removal_hint(&tl, &[]).is_none());

        // The delete itself leaves the gap; the hint then closes it with the chosen scope
        let mut after = tl.clone();
        run(
            &mut after,
            crate::ops::delete_events(&tl, &[(audio, ids[1])]),
        );
        assert_eq!(starts(&after), vec![0.0, 6.0, 10.0]);
        let replay = hint.outcome(&after, RippleScope::AffectedTracksAndMarks);
        run(&mut after, replay);
        assert_eq!(starts(&after), vec![0.0, 3.0, 7.0]);
        assert_eq!(after.markers[0].time, 4.0);

        let old = tl.tracks[0].events[1].geometry();
        let mut new = old;
        new.duration = 1.0;
        let resize = resize_hint(audio, old, new).unwrap();
        assert_eq!((resize.from, resize.delta), (5.0, -2.0));
        assert!(resize_hint(audio, old, old).is_none());
    }

    #[test]
    fn ripple_resize_ignores_same_length_locked_and_missing_events() {
        let (mut tl, audio, _) = rich_timeline();
        let id = tl.tracks[0].events[1].id;
        let same = tl.tracks[0].events[1].geometry();
        assert!(ripple_resize(&tl, audio, id, same, RippleScope::AffectedTracks).is_none());
        let mut g = same;
        g.duration = 1.0;
        assert!(ripple_resize(&tl, audio, 9999, g, RippleScope::AffectedTracks).is_none());
        tl.tracks[0].events[1].locked = true;
        assert!(ripple_resize(&tl, audio, id, g, RippleScope::AffectedTracks).is_none());
    }
}
