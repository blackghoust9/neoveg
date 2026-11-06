//! Compositing parent and child video tracks (manual v21, "Parent and child tracks").
//!
//! A parent track is the highest of a group of tracks and decides how the tracks under it (its
//! children) are composited together: the children are composited first, on a transparent canvas,
//! then the parent track goes over them in its own compositing mode (so a parent in *Multiply* masks
//! only its children), and the whole group goes over the tracks below it.
//!
//! The tree is kept as a level of indentation on each track (`Track::compose_level`), like the track
//! list shows it: the children of a track are the tracks right under it with a higher level, up to the
//! next track that is not deeper. Reading the levels never fails: [`effective_levels`] puts every
//! level in range (the first video track is at the top level, a track is at most one level deeper
//! than the video track above it), so deleting, inserting or moving tracks cannot leave a tree that
//! is wrong. Only video tracks that hold media take part; audio tracks and adjustment tracks stay out
//! of it, and an adjustment track ends a group.

use crate::command::EditCommand;
use crate::timeline::{Timeline, Track, TrackId, TrackKind};

/// Deepest level a track can be nested to.
pub const MAX_LEVEL: u8 = 8;

/// `true` for the tracks that can be parents or children.
fn takes_part(track: &Track) -> bool {
    track.kind == TrackKind::Video && !track.adjustment
}

/// The level each track is at, one per track of `tracks`: what is stored, put in range. Audio and
/// adjustment tracks are at level 0 and a track after an adjustment track starts again at 0.
pub fn effective_levels(tracks: &[Track]) -> Vec<u8> {
    let mut out = Vec::with_capacity(tracks.len());
    // The level of the video track above (`None` at the top and under an adjustment track)
    let mut above: Option<u8> = None;
    for track in tracks {
        if track.kind != TrackKind::Video {
            out.push(0);
        } else if track.adjustment {
            out.push(0);
            above = None;
        } else {
            let level = match above {
                Some(a) => track.compose_level.min(a + 1).min(MAX_LEVEL),
                None => 0,
            };
            out.push(level);
            above = Some(level);
        }
    }
    out
}

/// Index of `id` in the track list and the levels, when it is there.
fn position(timeline: &Timeline, id: TrackId) -> Option<(usize, Vec<u8>)> {
    let index = timeline.tracks.iter().position(|t| t.id == id)?;
    Some((index, effective_levels(&timeline.tracks)))
}

/// The index just past the last child of the track at `index` (`index + 1` when it has none). Audio
/// tracks in between do not end a group.
fn subtree_end(tracks: &[Track], levels: &[u8], index: usize) -> usize {
    let base = levels[index];
    let mut end = index + 1;
    let mut last_member = index + 1;
    while end < tracks.len() {
        if !takes_part(&tracks[end]) {
            if tracks[end].adjustment {
                break;
            }
            end += 1;
            continue;
        }
        if levels[end] <= base {
            break;
        }
        end += 1;
        last_member = end;
    }
    last_member
}

/// The direct parent of `id`, when it has one.
pub fn parent_of(timeline: &Timeline, id: TrackId) -> Option<TrackId> {
    let (index, levels) = position(timeline, id)?;
    if levels[index] == 0 || !takes_part(&timeline.tracks[index]) {
        return None;
    }
    (0..index)
        .rev()
        .filter(|i| takes_part(&timeline.tracks[*i]))
        .find(|i| levels[*i] < levels[index])
        .map(|i| timeline.tracks[i].id)
}

/// Every parent above `id`, nearest first.
pub fn ancestors(timeline: &Timeline, id: TrackId) -> Vec<TrackId> {
    let mut out = Vec::new();
    let mut current = id;
    while let Some(parent) = parent_of(timeline, current) {
        if out.contains(&parent) {
            break;
        }
        out.push(parent);
        current = parent;
    }
    out
}

/// The tracks under `id` that are its children, at any depth, in track order.
pub fn descendants(timeline: &Timeline, id: TrackId) -> Vec<TrackId> {
    let Some((index, levels)) = position(timeline, id) else {
        return Vec::new();
    };
    if !takes_part(&timeline.tracks[index]) {
        return Vec::new();
    }
    let end = subtree_end(&timeline.tracks, &levels, index);
    timeline.tracks[index + 1..end]
        .iter()
        .filter(|t| takes_part(t))
        .map(|t| t.id)
        .collect()
}

/// `true` when `id` has children.
pub fn is_parent(timeline: &Timeline, id: TrackId) -> bool {
    !descendants(timeline, id).is_empty()
}

/// The track and its children at any depth.
pub fn subtree(timeline: &Timeline, id: TrackId) -> Vec<TrackId> {
    let mut out = vec![id];
    out.extend(descendants(timeline, id));
    out
}

/// `true` when *Make compositing child* can move `id` one level in.
pub fn can_make_child(timeline: &Timeline, id: TrackId) -> bool {
    let Some((index, levels)) = position(timeline, id) else {
        return false;
    };
    if !takes_part(&timeline.tracks[index]) {
        return false;
    }
    // It needs a video track above to be a child of, at its own level or deeper
    let above = (0..index)
        .rev()
        .find(|i| timeline.tracks[*i].kind == TrackKind::Video);
    match above {
        Some(a) if takes_part(&timeline.tracks[a]) => {
            levels[index] <= levels[a] && levels[index] < MAX_LEVEL
        }
        _ => false,
    }
}

/// `true` when *Make compositing parent* can move `id` one level out.
pub fn can_make_parent(timeline: &Timeline, id: TrackId) -> bool {
    position(timeline, id)
        .is_some_and(|(index, levels)| takes_part(&timeline.tracks[index]) && levels[index] > 0)
}

/// Moves `id` (with all its children when `with_children`) one level in.
pub fn make_child(timeline: &Timeline, id: TrackId, with_children: bool) -> Option<EditCommand> {
    if !can_make_child(timeline, id) {
        return None;
    }
    shift(timeline, id, with_children, 1)
}

/// Moves `id` (with all its children when `with_children`) one level out.
pub fn make_parent(timeline: &Timeline, id: TrackId, with_children: bool) -> Option<EditCommand> {
    if !can_make_parent(timeline, id) {
        return None;
    }
    shift(timeline, id, with_children, -1)
}

/// The edit that changes the level of `id` (and its children) by `delta`, written as the levels the
/// tracks are really at so the stored ones are tidy afterwards.
fn shift(timeline: &Timeline, id: TrackId, with_children: bool, delta: i8) -> Option<EditCommand> {
    let levels = effective_levels(&timeline.tracks);
    let members = if with_children {
        subtree(timeline, id)
    } else {
        vec![id]
    };
    let mut changes = Vec::new();
    for member in members {
        let index = timeline.tracks.iter().position(|t| t.id == member)?;
        let before = timeline.tracks[index].compose_level;
        let now = levels[index];
        let after = if delta > 0 {
            (now + 1).min(MAX_LEVEL)
        } else {
            now.saturating_sub(1)
        };
        if before != after {
            changes.push((member, before, after));
        }
    }
    (!changes.is_empty()).then_some(EditCommand::SetComposeLevels { changes })
}

/// The tracks with their level for a track list: `(track, level)` in order, for drawing the indent.
pub fn indented(timeline: &Timeline) -> Vec<(TrackId, u8)> {
    effective_levels(&timeline.tracks)
        .into_iter()
        .zip(&timeline.tracks)
        .map(|(level, track)| (track.id, level))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;

    /// V1 V2 V3 V4 (video) and A1 (audio), all at the top level.
    fn timeline() -> (Timeline, [TrackId; 5]) {
        let mut tl = Timeline::new();
        let mut ids = [0; 5];
        for (i, kind) in [
            TrackKind::Video,
            TrackKind::Video,
            TrackKind::Video,
            TrackKind::Video,
            TrackKind::Audio,
        ]
        .into_iter()
        .enumerate()
        {
            let track = tl.new_track(kind);
            ids[i] = track.id;
            tl.tracks.push(track);
        }
        (tl, ids)
    }

    fn run(
        tl: &mut Timeline,
        history: &mut History,
        build: impl FnOnce(&Timeline) -> Option<EditCommand>,
    ) {
        let command = build(tl).expect("an edit");
        assert!(history.execute(tl, command));
    }

    #[test]
    fn a_flat_list_has_no_parents_and_the_first_track_cannot_be_a_child() {
        let (tl, ids) = timeline();
        assert_eq!(effective_levels(&tl.tracks), vec![0; 5]);
        assert!(!can_make_child(&tl, ids[0]), "nothing above it");
        assert!(can_make_child(&tl, ids[1]));
        assert!(!can_make_child(&tl, ids[4]), "audio takes no part");
        assert!(!can_make_parent(&tl, ids[1]), "already at the top");
        assert!(parent_of(&tl, ids[1]).is_none() && !is_parent(&tl, ids[0]));
    }

    #[test]
    fn making_a_child_nests_it_under_the_track_above_and_one_more_level_goes_deeper() {
        let (mut tl, ids) = timeline();
        let mut history = History::new();
        run(&mut tl, &mut history, |t| make_child(t, ids[1], false));
        assert_eq!(effective_levels(&tl.tracks), vec![0, 1, 0, 0, 0]);
        assert_eq!(parent_of(&tl, ids[1]), Some(ids[0]));
        assert_eq!(descendants(&tl, ids[0]), vec![ids[1]]);
        assert!(is_parent(&tl, ids[0]));
        // V3 under V2 makes a nested group: V1 > V2 > V3
        run(&mut tl, &mut history, |t| make_child(t, ids[2], false));
        run(&mut tl, &mut history, |t| make_child(t, ids[2], false));
        assert_eq!(effective_levels(&tl.tracks), vec![0, 1, 2, 0, 0]);
        assert_eq!(parent_of(&tl, ids[2]), Some(ids[1]));
        assert_eq!(ancestors(&tl, ids[2]), vec![ids[1], ids[0]]);
        assert_eq!(subtree(&tl, ids[0]), vec![ids[0], ids[1], ids[2]]);
        // It cannot go deeper than one under the track above
        assert!(!can_make_child(&tl, ids[2]));
        // And one level out comes back
        run(&mut tl, &mut history, |t| make_parent(t, ids[2], false));
        assert_eq!(effective_levels(&tl.tracks), vec![0, 1, 1, 0, 0]);
        // Undo walks back, one step each
        assert!(history.undo(&mut tl));
        assert_eq!(effective_levels(&tl.tracks), vec![0, 1, 2, 0, 0]);
    }

    #[test]
    fn with_the_children_a_whole_group_moves_one_level_together() {
        let (mut tl, ids) = timeline();
        let mut history = History::new();
        // V2 > V3 under V1: levels 0 1 2 0
        for id in [ids[1], ids[2], ids[2]] {
            run(&mut tl, &mut history, |t| make_child(t, id, false));
        }
        assert_eq!(effective_levels(&tl.tracks), vec![0, 1, 2, 0, 0]);
        // Take V2 and its child V3 out one level: V2 at 0, V3 at 1
        run(&mut tl, &mut history, |t| make_parent(t, ids[1], true));
        assert_eq!(effective_levels(&tl.tracks), vec![0, 0, 1, 0, 0]);
        // Without the children V3 would stay put
        let (mut other, oids) = timeline();
        let mut h2 = History::new();
        for id in [oids[1], oids[2], oids[2]] {
            run(&mut other, &mut h2, |t| make_child(t, id, false));
        }
        run(&mut other, &mut h2, |t| make_parent(t, oids[1], false));
        assert_eq!(effective_levels(&other.tracks), vec![0, 0, 1, 0, 0]);
        // Putting V4 in and then the group of V2 one in with its children
        run(&mut tl, &mut history, |t| make_child(t, ids[1], true));
        assert_eq!(effective_levels(&tl.tracks), vec![0, 1, 2, 0, 0]);
    }

    #[test]
    fn stale_levels_never_make_a_wrong_tree() {
        let (mut tl, ids) = timeline();
        // A track list edited by hand: the first track at level 3 and a jump of two levels
        tl.track_mut(ids[0]).unwrap().compose_level = 3;
        tl.track_mut(ids[1]).unwrap().compose_level = 5;
        tl.track_mut(ids[2]).unwrap().compose_level = 1;
        assert_eq!(effective_levels(&tl.tracks), vec![0, 1, 1, 0, 0]);
        // Deleting the parent leaves the child at the top
        tl.tracks.remove(0);
        assert_eq!(effective_levels(&tl.tracks)[0], 0);
        // An audio track in between does not break the group, an adjustment track does
        let (mut tl, ids) = timeline();
        tl.track_mut(ids[1]).unwrap().compose_level = 1;
        tl.tracks.swap(3, 4); // V1 V2 V3 A1 V4
        tl.track_mut(ids[3]).unwrap().compose_level = 1;
        assert_eq!(effective_levels(&tl.tracks), vec![0, 1, 0, 0, 1]);
        assert_eq!(
            descendants(&tl, ids[2]),
            vec![ids[3]],
            "A1 sits inside the group"
        );
        tl.track_mut(ids[2]).unwrap().adjustment = true;
        assert_eq!(
            effective_levels(&tl.tracks),
            vec![0, 1, 0, 0, 0],
            "under an adjustment track a group starts again"
        );
    }

    #[test]
    fn adjustment_and_audio_tracks_take_no_part_and_the_depth_has_a_limit() {
        let (mut tl, ids) = timeline();
        tl.track_mut(ids[1]).unwrap().adjustment = true;
        assert!(!can_make_child(&tl, ids[1]));
        assert!(
            !can_make_child(&tl, ids[2]),
            "an adjustment track is not a parent"
        );
        assert!(make_child(&tl, ids[4], false).is_none());
        assert!(subtree(&tl, ids[4]) == vec![ids[4]] && descendants(&tl, ids[4]).is_empty());
        // MAX_LEVEL
        let mut tl = Timeline::new();
        for _ in 0..12 {
            let t = tl.new_track(TrackKind::Video);
            tl.tracks.push(t);
        }
        let mut history = History::new();
        for _ in 0..40 {
            let ids: Vec<TrackId> = tl.tracks.iter().map(|t| t.id).collect();
            for id in ids {
                if let Some(command) = make_child(&tl, id, false) {
                    history.execute(&mut tl, command);
                }
            }
        }
        let levels = effective_levels(&tl.tracks);
        assert!(levels.iter().all(|l| *l <= MAX_LEVEL));
        assert_eq!(*levels.iter().max().unwrap(), MAX_LEVEL);
        // A track that reached the maximum depth cannot take a child, whatever its position
        let deepest = tl
            .tracks
            .iter()
            .zip(&levels)
            .find(|(_, level)| **level == MAX_LEVEL)
            .map(|(track, _)| track.id)
            .expect("a track reaches the maximum depth");
        assert!(!can_make_child(&tl, deepest));
    }

    #[test]
    fn the_levels_are_stored_only_when_they_are_not_zero() {
        let (mut tl, ids) = timeline();
        assert!(!serde_json::to_string(&tl)
            .unwrap()
            .contains("compose_level"));
        let mut history = History::new();
        run(&mut tl, &mut history, |t| make_child(t, ids[1], false));
        let json = serde_json::to_string(&tl).unwrap();
        assert!(json.contains("\"compose_level\":1"));
        let back: Timeline = serde_json::from_str(&json).unwrap();
        assert_eq!(effective_levels(&back.tracks), vec![0, 1, 0, 0, 0]);
        assert_eq!(indented(&back)[1], (ids[1], 1));
    }
}
