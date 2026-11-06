//! Exibir > Storyboard: the events of a video track as cards on a grid. The user picks a card and
//! moves it earlier or later, then sends the new order to the timeline (one undo step). The rules
//! are in `neoveg_engine::storyboard`; this file is only the screen.

use eframe::egui;
use neoveg_common::i18n::{tr, tr_args};
use neoveg_engine::storyboard::{self, Card};
use neoveg_engine::{EditCommand, Timeline, TrackId};

const CARD_WIDTH: f32 = 150.0;
const CARD_HEIGHT: f32 = 56.0;
const GAP: f32 = 8.0;

#[derive(Default)]
pub struct StoryboardWindow {
    pub open: bool,
    track: Option<TrackId>,
    cards: Vec<Card>,
    selected: Option<usize>,
    /// The order differs from the timeline.
    dirty: bool,
}

impl StoryboardWindow {
    /// Loads the cards of `track` from the timeline, dropping any unsent change.
    pub fn reload(&mut self, timeline: &Timeline, track: TrackId) {
        self.track = Some(track);
        self.cards = storyboard::cards_of_track(timeline, track);
        self.selected = None;
        self.dirty = false;
    }

    /// Moves the selected card by `delta` places.
    pub fn move_selected(&mut self, delta: i32) {
        let Some(from) = self.selected else {
            return;
        };
        let to = (from as i64 + i64::from(delta)).clamp(0, self.cards.len() as i64 - 1) as usize;
        if to != from {
            self.selected = Some(storyboard::move_card(&mut self.cards, from, to));
            self.dirty = true;
        }
    }

    /// The edit that sends the order to the timeline, if it changes anything.
    pub fn pending_edit(&self, timeline: &Timeline) -> Option<EditCommand> {
        let order: Vec<_> = self.cards.iter().map(|c| c.event_id).collect();
        storyboard::send_to_timeline(timeline, self.track?, &order)
    }

    /// Draws the window; the edit is returned the moment *Enviar para a linha do tempo* is pressed.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        timeline: &Timeline,
        track: Option<TrackId>,
    ) -> Option<EditCommand> {
        if !self.open {
            return None;
        }
        if self.track != track && !self.dirty {
            match track {
                Some(t) => self.reload(timeline, t),
                None => self.cards.clear(),
            }
        }
        let mut open = true;
        let mut send = false;
        let mut reload = false;
        egui::Window::new(tr("Storyboard"))
            .open(&mut open)
            .default_size([560.0, 380.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.add_enabled(self.selected.is_some(), egui::Button::new(tr("◀ Mais cedo"))).clicked() {
                        self.move_selected(-1);
                    }
                    if ui.add_enabled(self.selected.is_some(), egui::Button::new(tr("Mais tarde ▶"))).clicked() {
                        self.move_selected(1);
                    }
                    if ui.add_enabled(self.dirty, egui::Button::new(tr("Enviar para a linha do tempo"))).clicked() {
                        send = true;
                    }
                    if ui.button(tr("Recarregar")).clicked() {
                        reload = true;
                    }
                });
                ui.separator();
                if self.cards.is_empty() {
                    ui.weak(tr("Não há eventos de vídeo para montar."));
                    return;
                }
                let columns = storyboard::columns_for(ui.available_width(), CARD_WIDTH, GAP);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    egui::Grid::new("storyboard_grid").spacing([GAP, GAP]).show(ui, |ui| {
                        for (i, card) in self.cards.iter().enumerate() {
                            let (_, col) = storyboard::grid_cell(i, columns);
                            let label = format!("{}. {}\n{}", i + 1, card.name, tr_args("{} s", &[&format!("{:.2}", card.duration)]));
                            let button = egui::Button::new(label)
                                .min_size(egui::vec2(CARD_WIDTH, CARD_HEIGHT))
                                .selected(self.selected == Some(i));
                            if ui.add(button).clicked() {
                                self.selected = Some(i);
                            }
                            if col + 1 == columns {
                                ui.end_row();
                            }
                        }
                    });
                });
            });
        self.open = open;
        if reload {
            if let Some(t) = self.track {
                self.reload(timeline, t);
            }
        }
        if send {
            let edit = self.pending_edit(timeline);
            self.dirty = false;
            return edit;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neoveg_engine::{TrackEvent, TrackKind};

    fn timeline() -> (Timeline, TrackId) {
        let mut tl = Timeline::new();
        let track = tl.new_track(TrackKind::Video);
        let id = track.id;
        tl.tracks.push(track);
        for (name, start) in [("A", 0.0), ("B", 2.0), ("C", 4.0)] {
            let event = TrackEvent { id: tl.alloc_id(), name: name.into(), start, duration: 2.0, ..TrackEvent::default() };
            tl.track_mut(id).unwrap().events.push(event);
        }
        (tl, id)
    }

    #[test]
    fn moving_the_selected_card_makes_an_edit_that_swaps_the_events() {
        let (tl, track) = timeline();
        let mut w = StoryboardWindow::default();
        w.reload(&tl, track);
        assert!(w.pending_edit(&tl).is_none(), "nothing changed yet");
        w.move_selected(1); // nothing selected
        assert!(!w.dirty);
        w.selected = Some(0);
        w.move_selected(1);
        assert!(w.dirty);
        assert_eq!(w.cards.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["B", "A", "C"]);
        assert!(w.pending_edit(&tl).is_some());
        w.move_selected(-5); // clamps to the first place
        assert_eq!(w.selected, Some(0));
    }
}
