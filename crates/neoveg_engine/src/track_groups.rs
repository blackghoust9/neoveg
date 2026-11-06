//! Track groups: named sets of tracks that collapse into one row and mute or solo together, plus a
//! colour for each track.
//!
//! The groups are kept apart from the [`Timeline`] (they are a way of showing and driving the
//! tracks, not part of the edit). Tracks that no longer exist are ignored when reading them, so
//! deleting a track never leaves a broken group.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::timeline::{Timeline, TrackId};

pub type TrackGroupId = u32;

/// Colours offered for tracks and groups.
pub const TRACK_COLORS: [[u8; 3]; 8] = [
    [231, 76, 60],
    [230, 126, 34],
    [241, 196, 15],
    [46, 204, 113],
    [26, 188, 156],
    [52, 152, 219],
    [155, 89, 182],
    [149, 165, 166],
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackGroup {
    pub id: TrackGroupId,
    pub name: String,
    pub tracks: Vec<TrackId>,
    #[serde(default)]
    pub collapsed: bool,
    /// The colour of the group's tracks that have none of their own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[u8; 3]>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TrackGroups {
    #[serde(default)]
    groups: Vec<TrackGroup>,
    #[serde(default)]
    next_id: TrackGroupId,
    /// A colour for each track that has one.
    #[serde(default)]
    track_colors: BTreeMap<TrackId, [u8; 3]>,
}

/// How a group's tracks stand for a switch like mute or solo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupSwitch {
    None,
    Some,
    All,
}

impl TrackGroups {
    pub fn new() -> Self {
        Self::default()
    }

    /// `true` when there is no group and no track colour.
    pub fn is_empty(&self) -> bool {
        self.groups.is_empty() && self.track_colors.is_empty()
    }

    pub fn groups(&self) -> &[TrackGroup] {
        &self.groups
    }

    pub fn group(&self, id: TrackGroupId) -> Option<&TrackGroup> {
        self.groups.iter().find(|g| g.id == id)
    }

    fn group_mut(&mut self, id: TrackGroupId) -> Option<&mut TrackGroup> {
        self.groups.iter_mut().find(|g| g.id == id)
    }

    /// The group a track belongs to.
    pub fn group_of(&self, track: TrackId) -> Option<TrackGroupId> {
        self.groups
            .iter()
            .find(|g| g.tracks.contains(&track))
            .map(|g| g.id)
    }

    /// Makes a group of `tracks` (a track is in one group only, so it leaves the one it was in).
    /// Groups left empty by that are dropped. `None` for no track at all.
    pub fn create(&mut self, name: &str, tracks: &[TrackId]) -> Option<TrackGroupId> {
        let mut members: Vec<TrackId> = Vec::new();
        for t in tracks {
            if !members.contains(t) {
                members.push(*t);
            }
        }
        if members.is_empty() {
            return None;
        }
        for g in &mut self.groups {
            g.tracks.retain(|t| !members.contains(t));
        }
        self.groups.retain(|g| !g.tracks.is_empty());
        let id = self.next_id;
        self.next_id += 1;
        let name = if name.trim().is_empty() {
            format!("Grupo {}", id + 1)
        } else {
            name.trim().to_string()
        };
        self.groups.push(TrackGroup {
            id,
            name,
            tracks: members,
            collapsed: false,
            color: None,
        });
        Some(id)
    }

    pub fn rename(&mut self, id: TrackGroupId, name: &str) -> bool {
        let name = name.trim();
        match self.group_mut(id) {
            Some(g) if !name.is_empty() && g.name != name => {
                g.name = name.to_string();
                true
            }
            _ => false,
        }
    }

    /// Dissolves a group; its tracks stay as they are.
    pub fn ungroup(&mut self, id: TrackGroupId) -> bool {
        let before = self.groups.len();
        self.groups.retain(|g| g.id != id);
        self.groups.len() != before
    }

    pub fn remove_track(&mut self, track: TrackId) {
        for g in &mut self.groups {
            g.tracks.retain(|t| *t != track);
        }
        self.groups.retain(|g| !g.tracks.is_empty());
        self.track_colors.remove(&track);
    }

    pub fn set_collapsed(&mut self, id: TrackGroupId, collapsed: bool) -> bool {
        match self.group_mut(id) {
            Some(g) if g.collapsed != collapsed => {
                g.collapsed = collapsed;
                true
            }
            _ => false,
        }
    }

    pub fn set_group_color(&mut self, id: TrackGroupId, color: Option<[u8; 3]>) -> bool {
        match self.group_mut(id) {
            Some(g) if g.color != color => {
                g.color = color;
                true
            }
            _ => false,
        }
    }

    pub fn set_track_color(&mut self, track: TrackId, color: Option<[u8; 3]>) {
        match color {
            Some(c) => {
                self.track_colors.insert(track, c);
            }
            None => {
                self.track_colors.remove(&track);
            }
        }
    }

    /// The colour a track is drawn with: its own, else its group's, else none.
    pub fn color_of(&self, track: TrackId) -> Option<[u8; 3]> {
        self.track_colors.get(&track).copied().or_else(|| {
            self.group_of(track)
                .and_then(|g| self.group(g))
                .and_then(|g| g.color)
        })
    }

    /// The tracks of `timeline` that are shown, in order: a collapsed group shows only its first
    /// track (in timeline order) as its row.
    pub fn visible_tracks(&self, timeline: &Timeline) -> Vec<TrackId> {
        let mut shown = Vec::new();
        for track in &timeline.tracks {
            let hidden = self.groups.iter().any(|g| {
                g.collapsed
                    && g.tracks.contains(&track.id)
                    && timeline
                        .tracks
                        .iter()
                        .find(|t| g.tracks.contains(&t.id))
                        .is_some_and(|first| first.id != track.id)
            });
            if !hidden {
                shown.push(track.id);
            }
        }
        shown
    }

    fn live_members<'a>(&'a self, timeline: &'a Timeline, id: TrackGroupId) -> Vec<TrackId> {
        self.group(id)
            .map(|g| {
                g.tracks
                    .iter()
                    .copied()
                    .filter(|t| timeline.track(*t).is_some())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn switch_state(
        &self,
        timeline: &Timeline,
        id: TrackGroupId,
        on: impl Fn(&crate::timeline::Track) -> bool,
    ) -> GroupSwitch {
        let members = self.live_members(timeline, id);
        let count = members
            .iter()
            .filter(|t| timeline.track(**t).is_some_and(&on))
            .count();
        match count {
            0 => GroupSwitch::None,
            n if n == members.len() => GroupSwitch::All,
            _ => GroupSwitch::Some,
        }
    }

    pub fn mute_state(&self, timeline: &Timeline, id: TrackGroupId) -> GroupSwitch {
        self.switch_state(timeline, id, |t| t.muted)
    }

    pub fn solo_state(&self, timeline: &Timeline, id: TrackGroupId) -> GroupSwitch {
        self.switch_state(timeline, id, |t| t.solo)
    }

    /// Mutes or unmutes every track of the group; returns how many changed.
    pub fn set_muted(&self, timeline: &mut Timeline, id: TrackGroupId, muted: bool) -> usize {
        let mut changed = 0;
        for t in self.live_members(timeline, id) {
            if let Some(track) = timeline.track_mut(t) {
                if track.muted != muted {
                    track.muted = muted;
                    changed += 1;
                }
            }
        }
        changed
    }

    /// Solos or un-solos every track of the group; returns how many changed.
    pub fn set_solo(&self, timeline: &mut Timeline, id: TrackGroupId, solo: bool) -> usize {
        let mut changed = 0;
        for t in self.live_members(timeline, id) {
            if let Some(track) = timeline.track_mut(t) {
                if track.solo != solo {
                    track.solo = solo;
                    changed += 1;
                }
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::TrackKind;

    fn timeline(n: usize) -> (Timeline, Vec<TrackId>) {
        let mut tl = Timeline::new();
        let mut ids = Vec::new();
        for _ in 0..n {
            let track = tl.new_track(TrackKind::Audio);
            ids.push(track.id);
            tl.tracks.push(track);
        }
        (tl, ids)
    }

    #[test]
    fn a_track_is_in_one_group_and_empty_groups_disappear() {
        let (_, ids) = timeline(4);
        let mut g = TrackGroups::new();
        let a = g.create("  Vozes ", &ids[0..2]).unwrap();
        assert_eq!(g.group(a).unwrap().name, "Vozes");
        assert_eq!(g.group_of(ids[1]), Some(a));
        // Taking both tracks into another group empties the first one
        let b = g.create("", &ids[0..2]).unwrap();
        assert!(g.group(a).is_none());
        assert_eq!(g.group(b).unwrap().name, "Grupo 2");
        assert!(g.create("x", &[]).is_none());
        // A repeated track counts once
        let c = g.create("Música", &[ids[3], ids[3]]).unwrap();
        assert_eq!(g.group(c).unwrap().tracks, vec![ids[3]]);
        assert!(g.rename(c, "Trilha") && !g.rename(c, "Trilha") && !g.rename(c, " "));
        assert!(g.ungroup(c) && !g.ungroup(c));
        assert_eq!(g.group_of(ids[3]), None);
    }

    #[test]
    fn a_collapsed_group_shows_only_its_first_track() {
        let (tl, ids) = timeline(5);
        let mut g = TrackGroups::new();
        let a = g.create("Vozes", &[ids[3], ids[1], ids[2]]).unwrap();
        assert_eq!(g.visible_tracks(&tl), ids);
        assert!(g.set_collapsed(a, true) && !g.set_collapsed(a, true));
        // The row left is the group's first track in timeline order (ids[1])
        assert_eq!(g.visible_tracks(&tl), vec![ids[0], ids[1], ids[4]]);
        assert!(g.set_collapsed(a, false));
        assert_eq!(g.visible_tracks(&tl).len(), 5);
    }

    #[test]
    fn mute_and_solo_apply_to_the_whole_group() {
        let (mut tl, ids) = timeline(3);
        let mut g = TrackGroups::new();
        let a = g.create("Vozes", &ids[0..2]).unwrap();
        assert_eq!(g.mute_state(&tl, a), GroupSwitch::None);
        assert_eq!(g.set_muted(&mut tl, a, true), 2);
        assert!(tl.track(ids[0]).unwrap().muted && tl.track(ids[1]).unwrap().muted);
        assert!(!tl.track(ids[2]).unwrap().muted, "outside the group");
        assert_eq!(g.mute_state(&tl, a), GroupSwitch::All);
        tl.track_mut(ids[1]).unwrap().muted = false;
        assert_eq!(g.mute_state(&tl, a), GroupSwitch::Some);
        assert_eq!(g.set_muted(&mut tl, a, true), 1);
        assert_eq!(g.set_solo(&mut tl, a, true), 2);
        assert_eq!(g.solo_state(&tl, a), GroupSwitch::All);
        assert_eq!(g.set_solo(&mut tl, a, false), 2);
        // A track deleted from the timeline is skipped
        tl.tracks.retain(|t| t.id != ids[0]);
        assert_eq!(g.mute_state(&tl, a), GroupSwitch::All);
        assert_eq!(g.set_muted(&mut tl, a, false), 1);
    }

    #[test]
    fn colours_are_the_tracks_own_or_the_groups_and_everything_is_saved() {
        let (_, ids) = timeline(3);
        let mut g = TrackGroups::new();
        let a = g.create("Vozes", &ids[0..2]).unwrap();
        assert_eq!(g.color_of(ids[0]), None);
        assert!(g.set_group_color(a, Some(TRACK_COLORS[5])));
        assert_eq!(g.color_of(ids[0]), Some(TRACK_COLORS[5]));
        g.set_track_color(ids[0], Some(TRACK_COLORS[0]));
        assert_eq!(g.color_of(ids[0]), Some(TRACK_COLORS[0]), "its own wins");
        assert_eq!(g.color_of(ids[2]), None);
        let json = serde_json::to_string(&g).unwrap();
        assert_eq!(serde_json::from_str::<TrackGroups>(&json).unwrap(), g);
        g.remove_track(ids[0]);
        assert_eq!(g.color_of(ids[0]), None);
        g.remove_track(ids[1]);
        assert!(g.groups().is_empty(), "the group lost its last track");
    }
}
