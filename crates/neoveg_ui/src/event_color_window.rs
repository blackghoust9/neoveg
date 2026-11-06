//! Exibir > Correção de cor do evento: automatic color from the current preview frame, export and
//! import of the correction as a file, and the mark for video events that have no correction yet.
//!
//! Everything that changes the project is a pure function returning one [`EditCommand`] (so one
//! undo step) and is tested without a window; the window only reports [`ColorAction`]s.

use eframe::egui;
use neoveg_common::i18n::tr;
use neoveg_engine::color_correction::{has_color_correction, ColorCorrectionFile};
use neoveg_engine::timeline::{Timeline, TrackEvent, TrackKind};
use neoveg_engine::{EditCommand, EventId, TrackId};
use neoveg_video::auto_color::{auto_color_grade, AutoColorOptions};
use neoveg_video::frame::FrameBuffer;

/// What the user asked of the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorAction {
    Auto,
    Export,
    Import,
    Reset,
    /// Writes a stabilized copy of the event's picture to a new file.
    Stabilize,
    CancelStabilize,
}

/// What the window needs to know to draw itself.
pub struct ColorView {
    /// Name of the selected event; `None` when no video event is selected.
    pub event_name: Option<String>,
    pub has_correction: bool,
    pub has_frame: bool,
    /// Progress `(done, total)` of a stabilization that is running.
    pub stabilizing: Option<(u64, u64)>,
}

/// The window: only whether it is open.
#[derive(Debug, Clone, Default)]
pub struct EventColorWindow {
    pub open: bool,
}

impl EventColorWindow {
    pub fn show(&mut self, ctx: &egui::Context, view: &ColorView) -> Vec<ColorAction> {
        let mut out = Vec::new();
        let mut open = self.open;
        egui::Window::new(tr("Correção de cor do evento"))
            .id(egui::Id::new("Correção de cor do evento"))
            .open(&mut open)
            .resizable(false)
            .show(ctx, |ui| {
                match &view.event_name {
                    Some(name) => ui.label(format!("{}: {name}", tr("Evento"))),
                    None => ui.label(tr("Selecione um evento de vídeo na linha do tempo.")),
                };
                let selected = view.event_name.is_some();
                ui.add_enabled_ui(selected, |ui| {
                    ui.horizontal(|ui| {
                        let auto = ui.add_enabled(view.has_frame, egui::Button::new(tr("Cor automática")));
                        if auto.on_hover_text(tr("Calcula balanço de branco e níveis a partir do quadro atual da prévia (o cursor deve estar sobre o evento)")).clicked() {
                            out.push(ColorAction::Auto);
                        }
                        if ui
                            .add_enabled(view.has_correction, egui::Button::new(tr("Redefinir")))
                            .clicked()
                        {
                            out.push(ColorAction::Reset);
                        }
                    });
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(view.has_correction, egui::Button::new(tr("Exportar correção…")))
                            .clicked()
                        {
                            out.push(ColorAction::Export);
                        }
                        if ui.button(tr("Importar correção…")).clicked() {
                            out.push(ColorAction::Import);
                        }
                    });
                    match view.stabilizing {
                        Some((done, total)) => {
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::ProgressBar::new(done as f32 / total.max(1) as f32)
                                        .desired_width(160.0),
                                );
                                if ui.button(tr("Cancelar")).clicked() {
                                    out.push(ColorAction::CancelStabilize);
                                }
                            });
                        }
                        None => {
                            if ui
                                .button(tr("Estabilizar evento"))
                                .on_hover_text(tr("Grava uma cópia estabilizada do trecho do evento e a importa na mídia do projeto"))
                                .clicked()
                            {
                                out.push(ColorAction::Stabilize);
                            }
                        }
                    }
                });
            });
        self.open = open;
        out
    }
}

/// `true` when the lane should mark this event as having no color correction: the same rule as
/// [`neoveg_engine::color_correction::events_without_color_correction`].
pub fn needs_color_mark(kind: TrackKind, event: &TrackEvent) -> bool {
    kind == TrackKind::Video
        && (event.media_id.is_some() || event.adjustment)
        && !has_color_correction(event)
}

fn find_event(timeline: &Timeline, track_id: TrackId, event_id: EventId) -> Result<&TrackEvent, String> {
    timeline
        .track(track_id)
        .and_then(|t| t.events.iter().find(|e| e.id == event_id))
        .ok_or_else(|| tr("O evento não existe mais.").to_string())
}

fn set_grade(
    track_id: TrackId,
    event_id: EventId,
    event: &TrackEvent,
    after: Option<neoveg_common::grade::EventColorGrade>,
) -> Result<EditCommand, String> {
    if event.color_grade == after {
        return Err(tr("Nada a mudar na correção do evento.").to_string());
    }
    Ok(EditCommand::SetEventColorGrade {
        track_id,
        event_id,
        before: event.color_grade.clone(),
        after,
    })
}

/// Automatic color for an event from the preview `frame` taken at `playhead`. The preview already
/// shows the event's correction, so an event that has one must be reset first (the analysis would
/// otherwise work on a corrected picture and stack a second correction on it).
pub fn auto_color_edit(
    timeline: &Timeline,
    track_id: TrackId,
    event_id: EventId,
    frame: &FrameBuffer,
    playhead: f64,
) -> Result<EditCommand, String> {
    let event = find_event(timeline, track_id, event_id)?;
    if playhead < event.start || playhead >= event.end() {
        return Err(tr("Coloque o cursor sobre o evento para analisar o quadro.").to_string());
    }
    if has_color_correction(event) {
        return Err(tr("O evento já tem correção: redefina-a antes de usar a cor automática.").to_string());
    }
    let grade = auto_color_grade(frame, AutoColorOptions::default())
        .ok_or_else(|| tr("O quadro atual não permite calcular a cor automática.").to_string())?;
    set_grade(track_id, event_id, event, Some(grade))
}

/// What to stabilize for an event: its media, the start in the source and the length in seconds.
/// Only events played at normal speed qualify (the stabilizer reads the source in order).
pub fn stabilize_plan(
    timeline: &Timeline,
    track_id: TrackId,
    event_id: EventId,
) -> Result<(u64, f64, f64), String> {
    let event = find_event(timeline, track_id, event_id)?;
    let media = event
        .media_id
        .filter(|_| !event.adjustment)
        .ok_or_else(|| tr("Só eventos com vídeo podem ser estabilizados.").to_string())?;
    if (event.playback_rate - 1.0).abs() > 1e-6 || event.looping {
        return Err(tr("Estabilizar exige um evento em velocidade normal e sem laço.").to_string());
    }
    Ok((media, event.trim_in, event.duration))
}

/// Removes the correction of the event.
pub fn reset_edit(timeline: &Timeline, track_id: TrackId, event_id: EventId) -> Result<EditCommand, String> {
    let event = find_event(timeline, track_id, event_id)?;
    set_grade(track_id, event_id, event, None)
}

/// The text of the correction file of the event.
pub fn export_text(timeline: &Timeline, track_id: TrackId, event_id: EventId) -> Result<String, String> {
    let event = find_event(timeline, track_id, event_id)?;
    ColorCorrectionFile::from_event(event)
        .map(|f| f.to_json())
        .ok_or_else(|| tr("O evento não tem correção de cor para exportar.").to_string())
}

/// Applies the text of a correction file to the event.
pub fn import_edit(
    timeline: &Timeline,
    track_id: TrackId,
    event_id: EventId,
    text: &str,
) -> Result<EditCommand, String> {
    let event = find_event(timeline, track_id, event_id)?;
    let file = ColorCorrectionFile::from_json(text).map_err(|e| e.to_string())?;
    let mut copy = event.clone();
    file.apply_to(&mut copy);
    set_grade(track_id, event_id, event, copy.color_grade)
}

#[cfg(test)]
mod tests {
    use super::*;
    use neoveg_engine::command::History;

    fn warm_frame() -> FrameBuffer {
        FrameBuffer::solid(16, 16, [200, 140, 90, 255])
    }

    fn video_event(id: u64, start: f64, duration: f64, media: u64) -> TrackEvent {
        TrackEvent {
            id,
            name: format!("ev{id}"),
            media_id: Some(media),
            start,
            duration,
            ..TrackEvent::default()
        }
    }

    fn project() -> (Timeline, TrackId, EventId) {
        let mut tl = Timeline::new();
        let mut track = tl.new_track(TrackKind::Video);
        let track_id = track.id;
        track.events.push(video_event(1, 0.0, 4.0, 7));
        tl.tracks.push(track);
        (tl, track_id, 1)
    }

    #[test]
    fn auto_color_is_one_undo_step_and_refuses_outside_the_event_or_over_a_correction() {
        let (mut tl, t, e) = project();
        assert!(auto_color_edit(&tl, t, e, &warm_frame(), 9.0).is_err());
        let command = auto_color_edit(&tl, t, e, &warm_frame(), 1.0).unwrap();
        let mut history = History::default();
        assert!(history.execute(&mut tl, command));
        assert!(tl.tracks[0].events[0].color_grade.is_some());
        assert!(auto_color_edit(&tl, t, e, &warm_frame(), 1.0).is_err());
        assert!(history.undo(&mut tl));
        assert!(tl.tracks[0].events[0].color_grade.is_none());
    }

    #[test]
    fn stabilize_needs_a_video_event_at_normal_speed() {
        let (mut tl, t, e) = project();
        assert_eq!(stabilize_plan(&tl, t, e), Ok((7, 0.0, 4.0)));
        tl.tracks[0].events[0].playback_rate = 2.0;
        assert!(stabilize_plan(&tl, t, e).is_err());
        tl.tracks[0].events[0].playback_rate = 1.0;
        tl.tracks[0].events[0].media_id = None;
        assert!(stabilize_plan(&tl, t, e).is_err());
        assert!(stabilize_plan(&tl, t, 99).is_err());
    }

    #[test]
    fn export_then_import_moves_the_correction_to_another_event() {
        let (mut tl, t, e) = project();
        assert!(export_text(&tl, t, e).is_err());
        let command = auto_color_edit(&tl, t, e, &warm_frame(), 1.0).unwrap();
        History::default().execute(&mut tl, command);
        let text = export_text(&tl, t, e).unwrap();
        tl.tracks[0].events.push(video_event(2, 5.0, 2.0, 8));
        let import = import_edit(&tl, t, 2, &text).unwrap();
        assert!(History::default().execute(&mut tl, import));
        assert_eq!(tl.tracks[0].events[1].color_grade, tl.tracks[0].events[0].color_grade);
        assert!(import_edit(&tl, t, 2, "não é json").is_err());
        let reset = reset_edit(&tl, t, 2).unwrap();
        History::default().execute(&mut tl, reset);
        assert!(reset_edit(&tl, t, 2).is_err());
    }

    #[test]
    fn the_lane_mark_follows_the_engine_list_of_uncorrected_events() {
        let (mut tl, t, e) = project();
        let marked = |tl: &Timeline| -> Vec<(TrackId, EventId)> {
            tl.tracks
                .iter()
                .flat_map(|tr| tr.events.iter().filter(|ev| needs_color_mark(tr.kind, ev)).map(|ev| (tr.id, ev.id)))
                .collect()
        };
        assert_eq!(marked(&tl), vec![(t, e)]);
        assert_eq!(marked(&tl), neoveg_engine::color_correction::events_without_color_correction(&tl));
        let command = auto_color_edit(&tl, t, e, &warm_frame(), 1.0).unwrap();
        History::default().execute(&mut tl, command);
        assert!(marked(&tl).is_empty());
    }
}
