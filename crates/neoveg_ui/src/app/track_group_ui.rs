//! Track groups wired to the project: the window's actions change `Timeline::track_groups`
//! (saved with the project); the mute and solo of a group go through the mixer actions of each
//! track, so they are undoable like the buttons of the track list.

use neoveg_common::i18n::tr_args;

use super::*;
use crate::track_groups_window::GroupAction;

impl AppState {
    /// The tracks of a group that still exist, in timeline order.
    fn group_tracks(&self, id: neoveg_engine::track_groups::TrackGroupId) -> Vec<(TrackId, TrackKind)> {
        let Some(group) = self.timeline.track_groups.group(id) else {
            return Vec::new();
        };
        self.timeline
            .tracks
            .iter()
            .filter(|t| group.tracks.contains(&t.id))
            .map(|t| (t.id, t.kind))
            .collect()
    }

    pub(super) fn apply_group_actions(&mut self, actions: Vec<GroupAction>) {
        for action in actions {
            let changed = match action {
                GroupAction::Create { name, tracks } => {
                    let made = self.timeline.track_groups.create(&name, &tracks);
                    if made.is_some() {
                        self.status = tr_args("Grupo criado com {} faixa(s)", &[&tracks.len()]);
                    }
                    made.is_some()
                }
                GroupAction::Rename(id, name) => self.timeline.track_groups.rename(id, &name),
                GroupAction::Ungroup(id) => self.timeline.track_groups.ungroup(id),
                GroupAction::Collapse(id, on) => self.timeline.track_groups.set_collapsed(id, on),
                GroupAction::GroupColor(id, color) => {
                    self.timeline.track_groups.set_group_color(id, color)
                }
                GroupAction::TrackColor(track, color) => {
                    self.timeline.track_groups.set_track_color(track, color);
                    true
                }
                GroupAction::Mute(id, on) => {
                    for (track, _) in self.group_tracks(id) {
                        let already = self.timeline.track(track).is_some_and(|t| t.muted == on);
                        if !already {
                            self.apply_mix_actions(track, vec![MixAction::Mute(on)]);
                        }
                    }
                    false
                }
                GroupAction::Solo(id, on) => {
                    for (track, kind) in self.group_tracks(id) {
                        let already = self.timeline.track(track).is_some_and(|t| t.solo == on);
                        if kind == TrackKind::Audio && !already {
                            self.apply_mix_actions(track, vec![MixAction::Solo(on)]);
                        }
                    }
                    false
                }
            };
            if changed {
                self.forced_dirty = true;
            }
        }
    }

    /// The track groups window.
    pub(super) fn run_track_groups_window(&mut self, ctx: &egui::Context) {
        if !self.track_groups_window.open {
            return;
        }
        let actions = self.track_groups_window.show(ctx, &self.timeline);
        self.apply_group_actions(actions);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_tracks(s: &mut AppState) -> (TrackId, TrackId) {
        let mut ids = Vec::new();
        for _ in 0..2 {
            let track = s.timeline.new_track(TrackKind::Audio);
            ids.push(track.id);
            s.timeline.tracks.push(track);
        }
        (ids[0], ids[1])
    }

    #[test]
    fn group_actions_change_the_project_and_the_group_mutes_its_tracks_together() {
        let mut s = AppState::new();
        let (a, b) = two_tracks(&mut s);
        s.apply_group_actions(vec![GroupAction::Create {
            name: "Vozes".into(),
            tracks: vec![a, b],
        }]);
        let id = s.timeline.track_groups.group_of(a).expect("grouped");
        assert!(s.forced_dirty, "the project has something to save");
        s.apply_group_actions(vec![
            GroupAction::Mute(id, true),
            GroupAction::GroupColor(id, Some([10, 20, 30])),
            GroupAction::Collapse(id, true),
            GroupAction::Rename(id, "Locução".into()),
        ]);
        assert!(s.timeline.track(a).unwrap().muted && s.timeline.track(b).unwrap().muted);
        assert_eq!(s.timeline.track_groups.color_of(b), Some([10, 20, 30]));
        assert!(s.timeline.track_groups.group(id).unwrap().collapsed);
        assert_eq!(s.timeline.track_groups.group(id).unwrap().name, "Locução");
        s.apply_group_actions(vec![GroupAction::Solo(id, true)]);
        assert!(s.timeline.track(a).unwrap().solo);
        // The groups are saved with the project and old projects load without them
        let json = serde_json::to_string(&s.timeline).unwrap();
        let back: neoveg_engine::Timeline = serde_json::from_str(&json).unwrap();
        assert_eq!(back.track_groups.groups().len(), 1);
        s.apply_group_actions(vec![GroupAction::Ungroup(id)]);
        assert!(s.timeline.track_groups.groups().is_empty());
    }
}
