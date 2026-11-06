//! Spoken-word search: a window with a search box and the list of places where the words are
//! said in the media of the project (`neoveg_engine::speech_index`, filled when the speech is
//! transcribed). Clicking a hit, or the previous / next buttons, moves the cursor there.

use eframe::egui;
use neoveg_common::i18n::{tr, tr_args};
use neoveg_common::project::MediaRef;
use neoveg_engine::speech_index::{
    jump_points, next_jump, timeline_positions, SpeechHit, SpeechIndex,
};
use neoveg_engine::timeline::Timeline;

#[derive(Default)]
pub struct SpeechSearchState {
    pub open: bool,
    pub query: String,
    /// The last word of the query also matches longer words.
    pub prefix: bool,
    hits: Vec<SpeechHit>,
    /// What `hits` was searched with: (query, prefix, words in the index).
    searched: Option<(String, bool, usize)>,
}

/// Where the cursor goes for a hit: its first place on the timeline after `cursor_s`, else the
/// first one. `None` when no event plays that moment of the media.
pub fn seek_target(timeline: &Timeline, hit: &SpeechHit, cursor_s: f64) -> Option<f64> {
    let positions = timeline_positions(timeline, hit.media_id, hit.start_s);
    positions
        .iter()
        .copied()
        .find(|&p| p > cursor_s + 1e-6)
        .or_else(|| positions.first().copied())
}

fn clock(seconds: f64) -> String {
    let s = seconds.max(0.0);
    format!("{}:{:04.1}", (s / 60.0) as u64, s % 60.0)
}

impl SpeechSearchState {
    /// Searches again when the query, the option or the index changed.
    pub fn refresh(&mut self, index: &SpeechIndex) {
        let key = (self.query.clone(), self.prefix, index.word_count());
        if self.searched.as_ref() == Some(&key) {
            return;
        }
        self.hits = index.search(&self.query, self.prefix);
        self.searched = Some(key);
    }

    #[allow(dead_code)]
    pub fn hits(&self) -> &[SpeechHit] {
        &self.hits
    }

    /// Renders the window; returns the timeline position the cursor should jump to.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        index: &SpeechIndex,
        timeline: &Timeline,
        media: &[MediaRef],
        cursor_s: f64,
    ) -> Option<f64> {
        if !self.open {
            return None;
        }
        self.refresh(index);
        let mut open = self.open;
        let mut jump: Option<f64> = None;
        crate::dialogs::dialog(ctx, tr("Busca de palavra falada"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(480.0)
            .show(ctx, |ui| {
                if index.is_empty() {
                    ui.weak(tr(
                        "Nada indexado ainda: transcreva a fala (painel de legendas) para poder buscar palavras.",
                    ));
                } else {
                    ui.weak(tr_args(
                        "{} palavra(s) indexada(s) em {} mídia(s).",
                        &[&index.word_count(), &index.media_count()],
                    ));
                }
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.query)
                            .hint_text(tr("Palavra ou frase"))
                            .desired_width(260.0),
                    );
                    ui.checkbox(&mut self.prefix, tr("Começa com"));
                });
                let points = jump_points(timeline, &self.hits);
                ui.horizontal(|ui| {
                    let any = !points.is_empty();
                    if ui.add_enabled(any, egui::Button::new(tr("Anterior"))).clicked() {
                        jump = next_jump(&points, cursor_s, false);
                    }
                    if ui.add_enabled(any, egui::Button::new(tr("Próximo"))).clicked() {
                        jump = next_jump(&points, cursor_s, true);
                    }
                    ui.weak(tr_args("{} acerto(s)", &[&self.hits.len()]));
                });
                ui.separator();
                egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                    for hit in self.hits.iter().take(500) {
                        let name = media
                            .iter()
                            .find(|m| m.id == hit.media_id)
                            .and_then(|m| std::path::Path::new(&m.path).file_name().map(|n| n.to_string_lossy().to_string()))
                            .unwrap_or_else(|| format!("#{}", hit.media_id));
                        let target = seek_target(timeline, hit, cursor_s);
                        let label = format!("{name}  {}  {}", clock(hit.start_s), hit.context);
                        let response = ui.add_enabled(target.is_some(), egui::Button::new(label).frame(false));
                        if response.clicked() {
                            jump = target;
                        }
                        if target.is_none() {
                            response.on_disabled_hover_text(tr("Este trecho da mídia não está na linha do tempo."));
                        }
                    }
                });
            });
        self.open = open;
        jump
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neoveg_common::transcript::TranscriptSegment;
    use neoveg_engine::timeline::{TrackEvent, TrackKind};

    fn timeline_with_media_1() -> Timeline {
        let mut tl = Timeline::new();
        let mut track = tl.new_track(TrackKind::Audio);
        track.events.push(TrackEvent {
            id: tl.alloc_id(),
            media_id: Some(1),
            start: 20.0,
            duration: 10.0,
            trim_in: 0.0,
            ..TrackEvent::default()
        });
        tl.tracks.push(track);
        tl
    }

    fn index() -> SpeechIndex {
        let mut idx = SpeechIndex::new();
        idx.add_media(
            1,
            &[TranscriptSegment {
                start_s: 0.0,
                end_s: 4.0,
                text: "bom dia amigos".into(),
            }],
        );
        idx
    }

    #[test]
    fn the_window_state_searches_and_the_hit_leads_to_the_timeline() {
        let tl = timeline_with_media_1();
        let mut state = SpeechSearchState {
            query: "AMIGOS".into(),
            ..Default::default()
        };
        state.refresh(&index());
        assert_eq!(state.hits().len(), 1);
        let target = seek_target(&tl, &state.hits()[0], 0.0).unwrap();
        assert!((target - (20.0 + 4.0 * 2.0 / 3.0)).abs() < 1e-9);
        // The same hit is still the only one after the cursor passes it: it wraps
        let again = seek_target(&tl, &state.hits()[0], 100.0).unwrap();
        assert_eq!(again, target);
        // A change of query searches again
        state.query = "dia".into();
        state.refresh(&index());
        assert_eq!(state.hits().len(), 1);
        state.query = "nada".into();
        state.refresh(&index());
        assert!(state.hits().is_empty());
        // A hit in a part of the media no event plays has no place to go
        let off = SpeechHit {
            media_id: 9,
            start_s: 1.0,
            end_s: 2.0,
            word_index: 0,
            context: String::new(),
        };
        assert_eq!(seek_target(&tl, &off, 0.0), None);
    }
}
