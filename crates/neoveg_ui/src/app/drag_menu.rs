//! Drag-and-copy on the timeline: `Ctrl`+drag of an event body drops a copy; a drag with the right
//! button asks "Mover aqui / Copiar aqui / Embaralhar" when the button is released. The rules are in
//! `neoveg_engine::drag_copy`; this file reads the buttons and shows the little menu.

use neoveg_common::i18n::tr;
use neoveg_engine::drag_copy::{self, DragChoice};

use super::*;

/// A right-button drag that ended and waits for the choice.
pub(super) struct PendingDragMenu {
    track: TrackId,
    event: EventId,
    to_track: TrackId,
    moved: Vec<EventRef>,
    new_start: f64,
    pos: egui::Pos2,
}

/// How the current drag was started, and the menu that may follow it.
#[derive(Default)]
pub(super) struct DragExtras {
    secondary: bool,
    ctrl_at_press: bool,
    menu: Option<PendingDragMenu>,
}

impl AppState {
    /// Called when a drag starts on the lanes: remembers the button and `Ctrl`.
    pub(super) fn note_drag_press(&mut self, ui: &egui::Ui) {
        self.drag_extras.secondary = ui.input(|i| i.pointer.button_down(egui::PointerButton::Secondary));
        self.drag_extras.ctrl_at_press = ui.input(|i| i.modifiers.command);
    }

    /// Called when the mouse button is released: a body drag with `Ctrl` copies, one with the right
    /// button opens the menu; anything else is the ordinary [`AppState::finish_drag`].
    pub(super) fn finish_drag_routed(&mut self, ui: &egui::Ui) {
        let secondary = std::mem::take(&mut self.drag_extras.secondary);
        let ctrl = std::mem::take(&mut self.drag_extras.ctrl_at_press)
            || ui.input(|i| i.modifiers.command);
        let routed = self.drag.as_ref().is_some_and(|d| {
            d.mode == DragMode::Move
                && (d.current != d.origin || d.target_track != d.track_id)
                && (secondary || ctrl)
        });
        if !routed {
            self.finish_drag();
            return;
        }
        let Some(d) = self.drag.take() else {
            return;
        };
        let pending = PendingDragMenu {
            track: d.track_id,
            event: d.event_id,
            to_track: d.target_track,
            moved: d.moving,
            new_start: d.current.start,
            pos: ui
                .input(|i| i.pointer.latest_pos())
                .unwrap_or(egui::Pos2::ZERO),
        };
        if secondary {
            self.drag_extras.menu = Some(pending);
        } else {
            self.apply_drag_choice(DragChoice::Copy, &pending);
        }
    }

    fn apply_drag_choice(&mut self, choice: DragChoice, p: &PendingDragMenu) {
        let outcome = drag_copy::apply_choice(
            &mut self.timeline,
            choice,
            p.track,
            p.event,
            &p.moved,
            p.new_start,
            p.to_track,
        );
        match outcome {
            Some(outcome) => {
                let touched = outcome.touched.clone();
                if self.execute_timeline(outcome.command) {
                    if choice == DragChoice::Copy {
                        if let Some(&first) = touched.first() {
                            self.selected_track = Some(first.0);
                            self.selection.select_many(touched, first);
                        }
                    }
                    self.restart_audio_if_playing();
                    self.last_frame_key = None;
                }
            }
            None => {
                self.status = match choice {
                    DragChoice::Shuffle => {
                        "Embaralhar precisa de eventos em sequência, sem sobreposição nem bloqueio"
                            .into()
                    }
                    _ => "Nada mudou".into(),
                }
            }
        }
    }

    /// Shows the menu of a right-button drag until a choice is made (or `Esc` / a click elsewhere).
    pub(super) fn show_drag_menu(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.drag_extras.menu.take() else {
            return;
        };
        let mut chosen = None;
        let mut keep = true;
        let area = egui::Area::new(egui::Id::new("drag_choice_menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(pending.pos)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    for choice in DragChoice::available(pending.moved.len().max(1)) {
                        if ui.button(tr(choice.label())).clicked() {
                            chosen = Some(choice);
                        }
                    }
                });
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            keep = false;
        }
        if ctx.input(|i| i.pointer.any_pressed()) && !area.response.contains_pointer() {
            keep = false;
        }
        match chosen {
            Some(choice) => self.apply_drag_choice(choice, &pending),
            None if keep => self.drag_extras.menu = Some(pending),
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_choice_from_the_menu_copies_the_dragged_event() {
        let mut s = AppState::new();
        let mut track = s.timeline.new_track(TrackKind::Audio);
        let event = TrackEvent {
            id: s.timeline.alloc_id(),
            start: 1.0,
            duration: 2.0,
            media_id: Some(1),
            ..TrackEvent::default()
        };
        let ev_id = event.id;
        track.events.push(event);
        let track_id = track.id;
        s.timeline.tracks.push(track);
        let pending = PendingDragMenu {
            track: track_id,
            event: ev_id,
            to_track: track_id,
            moved: Vec::new(),
            new_start: 10.0,
            pos: egui::Pos2::ZERO,
        };
        s.apply_drag_choice(DragChoice::Copy, &pending);
        assert_eq!(s.timeline.tracks[0].events.len(), 2);
        assert_eq!(s.selection.len(), 1);
        s.apply_drag_choice(DragChoice::Move, &pending);
        assert_eq!(s.timeline.tracks[0].events.len(), 2);
        assert!(s.timeline.tracks[0].events.iter().any(|e| e.id == ev_id && e.start == 10.0));
    }
}
