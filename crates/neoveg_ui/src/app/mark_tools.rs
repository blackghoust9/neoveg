//! Several markers and regions at once on the ruler (Ctrl/Shift+click picks, dragging a picked
//! one moves or resizes them all as one undo step) and the markers a media carries, drawn on the
//! events that play them. The rules are in `neoveg_engine::{mark_selection, media_markers}`.

use neoveg_common::i18n::{tr, tr_args};
use neoveg_engine::mark_selection::{self, RegionEdge};
use neoveg_engine::media_markers;

use super::*;

/// Colour of the markers and regions that are picked.
pub(super) const PICKED_COLOR: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);

impl AppState {
    /// A click in the strip of the ruler. With `additive` (Ctrl or Shift) the mark under it is
    /// added to or taken out of the pick; without it the pick becomes that mark alone, or nothing.
    pub(super) fn click_mark_strip(&mut self, time: f64, tolerance: f64, additive: bool) {
        let hit = markers::hit_test(&self.timeline, time, tolerance).map(|(id, _)| id);
        match (hit, additive) {
            (Some(id), true) => {
                if let Some(at) = self.marks_picked.iter().position(|m| *m == id) {
                    self.marks_picked.remove(at);
                } else {
                    self.marks_picked.push(id);
                }
            }
            (Some(id), false) => {
                if !self.marks_picked.contains(&id) {
                    self.marks_picked = vec![id];
                }
            }
            (None, true) => {}
            (None, false) => self.marks_picked.clear(),
        }
    }

    /// Picks the markers and regions inside the time selection.
    pub(super) fn pick_marks_in_selection(&mut self) {
        if let Some(range) = self.time_selection {
            self.marks_picked = mark_selection::select_in_range(&self.timeline, range, true);
            self.status = tr_args("{} marcador(es)/região(ões) selecionado(s)", &[&self.marks_picked.len()]);
        }
    }

    /// Removes every picked marker and region (one undo step).
    pub(super) fn remove_picked_marks(&mut self) {
        let ids = std::mem::take(&mut self.marks_picked);
        if let Some(command) = mark_selection::remove_selected(&self.timeline, &ids) {
            self.history.execute(&mut self.timeline, command);
            self.status = tr("Marcadores e regiões selecionados removidos").into();
        }
    }

    /// While one picked mark is dragged, the others of the pick follow it.
    pub(super) fn group_preview(&self, drag: &MarkDrag, id: MarkId, stored: (f64, f64)) -> (f64, f64) {
        if self.marks_picked.len() < 2
            || !self.marks_picked.contains(&drag.id)
            || !self.marks_picked.contains(&id)
        {
            return stored;
        }
        match drag.part {
            MarkPart::Marker | MarkPart::RegionBody => {
                let delta = drag.current.0 - drag.original.0;
                ((stored.0 + delta).max(0.0), (stored.1 + delta).max(0.0))
            }
            MarkPart::RegionStart => {
                let delta = drag.current.0 - drag.original.0;
                ((stored.0 + delta).clamp(0.0, stored.1), stored.1)
            }
            MarkPart::RegionEnd => {
                let delta = drag.current.1 - drag.original.1;
                (stored.0, (stored.1 + delta).max(stored.0))
            }
        }
    }

    /// The edit that releasing a drag of one picked mark makes for the whole pick; `None` when
    /// the dragged mark is not part of a pick of two or more.
    pub(super) fn group_mark_command(&self, drag: &MarkDrag) -> Option<neoveg_engine::EditCommand> {
        if self.marks_picked.len() < 2 || !self.marks_picked.contains(&drag.id) {
            return None;
        }
        let fps = self.project.settings.fps;
        let min_len = 1.0 / fps.max(1.0);
        match drag.part {
            MarkPart::Marker | MarkPart::RegionBody => mark_selection::move_selected(
                &self.timeline,
                &self.marks_picked,
                drag.current.0 - drag.original.0,
                fps,
            ),
            MarkPart::RegionStart => mark_selection::resize_selected(
                &self.timeline,
                &self.marks_picked,
                RegionEdge::Start,
                drag.current.0 - drag.original.0,
                min_len,
            ),
            MarkPart::RegionEnd => mark_selection::resize_selected(
                &self.timeline,
                &self.marks_picked,
                RegionEdge::End,
                drag.current.1 - drag.original.1,
                min_len,
            ),
        }
    }

    /// The time of the media that the cursor shows in the event, when the cursor is over it.
    fn media_time_at_cursor(&self, track_id: TrackId, event_id: EventId) -> Option<(u64, f64)> {
        let event = self
            .timeline
            .track(track_id)?
            .events
            .iter()
            .find(|e| e.id == event_id)?;
        let media = event.media_id?;
        if self.playhead < event.start || self.playhead > event.start + event.duration {
            return None;
        }
        let rate = if event.playback_rate > 0.0 { event.playback_rate } else { 1.0 };
        Some((media, event.trim_in + (self.playhead - event.start) * rate))
    }

    /// Puts a marker in the media of the event at the cursor; it shows on every event that plays
    /// that moment.
    pub(super) fn add_media_marker_at_cursor(&mut self, track_id: TrackId, event_id: EventId) {
        let Some((media, time)) = self.media_time_at_cursor(track_id, event_id) else {
            self.status = tr("O cursor precisa estar sobre o evento").into();
            return;
        };
        let name = tr_args("Marcador {}", &[&(self.timeline.media_markers.markers_of(media).len() + 1)]);
        if self.timeline.media_markers.add(media, time, &name) {
            self.forced_dirty = true;
            self.status = tr("Marcador de mídia inserido").into();
        } else {
            self.status = tr("Já há um marcador de mídia neste ponto").into();
        }
    }

    /// Removes the media marker nearest to the cursor (within a tenth of a second).
    pub(super) fn remove_media_marker_at_cursor(&mut self, track_id: TrackId, event_id: EventId) {
        let Some((media, time)) = self.media_time_at_cursor(track_id, event_id) else {
            return;
        };
        let near = self
            .timeline
            .media_markers
            .markers_of(media)
            .iter()
            .map(|m| m.time)
            .filter(|t| (t - time).abs() <= 0.1)
            .min_by(|a, b| (a - time).abs().total_cmp(&(b - time).abs()));
        if let Some(t) = near {
            if self.timeline.media_markers.remove(media, t) {
                self.forced_dirty = true;
                self.status = tr("Marcador de mídia removido").into();
            }
        }
    }

    /// Snap targets with the media markers added when the option is on.
    pub(super) fn with_media_marker_snap(&self, candidates: Vec<f64>) -> Vec<f64> {
        media_markers::with_media_markers(
            candidates,
            &self.timeline,
            &self.timeline.media_markers,
            self.prefs.snap_to_media_markers,
        )
    }
}

/// Items for the ruler's context menu about the pick of marks.
pub(super) fn picked_marks_menu(state: &mut AppState, ui: &mut egui::Ui) {
    if ui
        .add_enabled(
            state.time_selection.is_some(),
            egui::Button::new(tr("Selecionar marcadores e regiões da seleção de tempo")),
        )
        .clicked()
    {
        state.pick_marks_in_selection();
        ui.close_menu();
    }
    let picked = state.marks_picked.len();
    if picked > 0 {
        if ui.button(tr_args("Remover {} selecionado(s)", &[&picked])).clicked() {
            state.remove_picked_marks();
            ui.close_menu();
        }
        if ui.button(tr("Limpar a seleção de marcadores")).clicked() {
            state.marks_picked.clear();
            ui.close_menu();
        }
    }
}

/// Items for the context menu of an event about the markers of its media.
pub(super) fn media_marker_menu(state: &mut AppState, ui: &mut egui::Ui, track_id: TrackId, event_id: EventId) {
    ui.separator();
    if ui.button(tr("Inserir marcador de mídia no cursor")).clicked() {
        state.add_media_marker_at_cursor(track_id, event_id);
        ui.close_menu();
    }
    if ui.button(tr("Remover marcador de mídia no cursor")).clicked() {
        state.remove_media_marker_at_cursor(track_id, event_id);
        ui.close_menu();
    }
    ui.separator();
}

/// Draws the media markers on an event: a small flag at the top and a thin line. `shift` is how
/// far the event is from its place in the project while it is dragged.
pub(super) fn paint_event_media_markers(
    painter: &egui::Painter,
    state: &AppState,
    event: &TrackEvent,
    ev_rect: Rect,
    shift: f64,
    lanes_left: f32,
    lanes_right: f32,
) {
    if state.timeline.media_markers.is_empty() {
        return;
    }
    for marker in media_markers::markers_on_event(&state.timeline.media_markers, event) {
        let x = lanes_left
            + ((marker.timeline_time + shift - state.scroll_seconds) * state.pixels_per_second) as f32;
        if x < lanes_left || x > lanes_right {
            continue;
        }
        let color = Color32::from_rgb(0x4F, 0xD1, 0xC5);
        painter.line_segment(
            [pos2(x, ev_rect.top()), pos2(x, ev_rect.bottom())],
            Stroke::new(1.0_f32, color.gamma_multiply(0.7)),
        );
        painter.add(Shape::convex_polygon(
            vec![
                pos2(x, ev_rect.top()),
                pos2(x + 7.0, ev_rect.top() + 3.5),
                pos2(x, ev_rect.top() + 7.0),
            ],
            color,
            Stroke::NONE,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_with_marks() -> (AppState, [MarkId; 3]) {
        let mut s = AppState::new();
        let mut ids = Vec::new();
        for t in [1.0, 3.0, 8.0] {
            let cmd = markers::add_marker(&mut s.timeline, t, Some("m")).unwrap();
            s.history.execute(&mut s.timeline, cmd);
            ids.push(s.timeline.markers.iter().find(|m| (m.time - t).abs() < 1e-9).unwrap().id);
        }
        (s, [ids[0], ids[1], ids[2]])
    }

    #[test]
    fn ctrl_click_picks_several_marks_and_dragging_one_moves_them_all_in_one_undo_step() {
        let (mut s, ids) = state_with_marks();
        s.click_mark_strip(1.0, 0.05, true);
        s.click_mark_strip(3.0, 0.05, true);
        assert_eq!(s.marks_picked, vec![ids[0], ids[1]]);
        // Ctrl+click again takes it out; a plain click on an empty place clears the pick
        s.click_mark_strip(3.0, 0.05, true);
        assert_eq!(s.marks_picked, vec![ids[0]]);
        s.click_mark_strip(3.0, 0.05, true);
        // Drag the first one by 2 s: the other picked one follows, the third stays
        assert!(s.begin_mark_drag(1.0, 0.05));
        s.update_mark_drag(3.0);
        let d = s.mark_drag.as_ref().unwrap();
        let preview = s.group_preview(d, ids[1], (3.0, 3.0));
        assert!((preview.0 - 5.0).abs() < 0.05, "{preview:?}");
        assert_eq!(s.group_preview(d, ids[2], (8.0, 8.0)), (8.0, 8.0));
        let revision = s.history.revision();
        s.finish_mark_drag();
        assert_eq!(s.history.revision(), revision + 1, "one step for the whole pick");
        let times: Vec<f64> = s.timeline.markers.iter().map(|m| m.time).collect();
        assert!((times[0] - 3.0).abs() < 0.05 && times[1] > 4.9 && times.len() == 3, "{times:?}");
        s.history.undo(&mut s.timeline);
        assert!((s.timeline.markers[0].time - 1.0).abs() < 1e-9);
        // A click on empty ruler clears the pick
        s.click_mark_strip(9.0, 0.05, false);
        assert!(s.marks_picked.is_empty());
    }

    #[test]
    fn marks_in_the_time_selection_are_picked_and_removed_together() {
        let (mut s, _) = state_with_marks();
        s.time_selection = Some(neoveg_engine::TimeRange::new(0.5, 3.5));
        s.pick_marks_in_selection();
        assert_eq!(s.marks_picked.len(), 2);
        s.remove_picked_marks();
        assert_eq!(s.timeline.markers.len(), 1);
        assert!(s.marks_picked.is_empty());
    }

    #[test]
    fn a_media_marker_is_added_at_the_cursor_and_snaps_only_when_the_option_is_on() {
        let mut s = AppState::new();
        let track = s.timeline.new_track(TrackKind::Audio);
        let track_id = track.id;
        s.timeline.tracks.push(track);
        let event = TrackEvent {
            id: s.timeline.alloc_id(),
            media_id: Some(4),
            start: 10.0,
            duration: 5.0,
            trim_in: 2.0,
            ..TrackEvent::default()
        };
        let event_id = event.id;
        s.timeline.track_mut(track_id).unwrap().events.push(event);
        s.playhead = 12.0;
        s.add_media_marker_at_cursor(track_id, event_id);
        // Media time 2 + (12 - 10) = 4
        assert_eq!(s.timeline.media_markers.markers_of(4)[0].time, 4.0);
        assert!(s.forced_dirty);
        s.playhead = 99.0;
        s.add_media_marker_at_cursor(track_id, event_id);
        assert_eq!(s.timeline.media_markers.markers_of(4).len(), 1, "cursor off the event");
        // The option decides whether the marker is a snap target
        assert!(!s.with_media_marker_snap(vec![0.0]).contains(&12.0));
        s.prefs.snap_to_media_markers = true;
        assert!(s.with_media_marker_snap(vec![0.0]).contains(&12.0));
        // And it is saved with the project
        let back: neoveg_engine::Timeline =
            serde_json::from_str(&serde_json::to_string(&s.timeline).unwrap()).unwrap();
        assert_eq!(back.media_markers.markers_of(4).len(), 1);
        s.playhead = 12.02;
        s.remove_media_marker_at_cursor(track_id, event_id);
        assert!(s.timeline.media_markers.is_empty());
    }
}
