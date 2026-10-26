//! Subtitles: the "Legendas" tab of the media panel and the text drawn over the video preview.
//!
//! The panel lists the captions of the project, lets the user edit their text and times, add
//! them at the cursor, import or export SubRip / WebVTT files and jump to one. It never touches the
//! project: it returns [`CaptionAction`]s and the application turns each one into one undoable
//! command (`neoveg_engine::captions`).

use std::collections::HashMap;

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Rect};
use neoveg_engine::{Caption, CaptionId};

use crate::icons::{self, Icon};

/// Shortest length a caption may be given by hand, seconds.
const MIN_LENGTH: f64 = 0.05;
/// Length of a caption added at the cursor, seconds.
pub const NEW_CAPTION_SECONDS: f64 = 2.0;
/// Characters per line in the preview before a long line is wrapped.
const WRAP_CHARS: usize = 42;

#[derive(Debug, Clone, PartialEq)]
pub enum CaptionAction {
    /// A new, empty caption at the cursor.
    Add,
    Remove(CaptionId),
    SetText(CaptionId, String),
    SetTimes(CaptionId, f64, f64),
    /// Move the cursor to a time.
    Seek(f64),
    Import,
    Export,
    Clear,
}

#[derive(Debug, Default)]
pub struct CaptionsPanelState {
    /// Text being edited, until the box loses focus.
    text_drafts: HashMap<CaptionId, String>,
    /// Start and end being dragged, until the drag ends.
    time_drafts: HashMap<CaptionId, (f64, f64)>,
}

// ---- pure helpers -------------------------------------------------------------------------------

/// The text of a caption as shown: inline tags (`<i>`, `<b>`, `<v Name>`, `<00:01.000>`) removed and
/// the common character entities decoded.
pub fn plain_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_tag = false;
    for c in text.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// The lines to draw for a caption: its own lines, long ones wrapped at spaces.
pub fn display_lines(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for raw in plain_text(text).lines() {
        let mut line = String::new();
        for word in raw.split_whitespace() {
            if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > max_chars {
                lines.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        if !line.is_empty() {
            lines.push(line);
        }
    }
    lines
}

/// A start and end that make a valid caption: not before zero, at least [`MIN_LENGTH`] long.
pub fn valid_times(start: f64, end: f64) -> (f64, f64) {
    let start = start.max(0.0);
    (start, end.max(start + MIN_LENGTH))
}

/// Index of the caption showing at `time`, if any.
pub fn current_index(captions: &[Caption], time: f64) -> Option<usize> {
    captions
        .iter()
        .position(|c| time >= c.start && time < c.end)
}

// ---- drawing ------------------------------------------------------------------------------------------

/// Draws the captions showing at `time` over `frame` (the video picture): white text with a dark
/// outline, at the bottom, sized with the picture.
pub fn paint_overlay(painter: &egui::Painter, frame: Rect, active: &[&Caption]) {
    let size = (frame.height() * 0.052).clamp(10.0, 48.0);
    let font = FontId::proportional(size);
    let mut lines: Vec<String> = Vec::new();
    for caption in active {
        lines.extend(display_lines(&caption.text, WRAP_CHARS));
    }
    let line_height = size * 1.18;
    let mut y = frame.bottom() - frame.height() * 0.06 - line_height * lines.len() as f32;
    for line in lines {
        let at = pos2(frame.center().x, y);
        for (dx, dy) in [
            (-1.5, 0.0),
            (1.5, 0.0),
            (0.0, -1.5),
            (0.0, 1.5),
            (1.2, 1.2),
            (-1.2, -1.2),
        ] {
            painter.text(
                at + vec2(dx, dy),
                Align2::CENTER_TOP,
                &line,
                font.clone(),
                Color32::from_black_alpha(230),
            );
        }
        painter.text(at, Align2::CENTER_TOP, &line, font.clone(), Color32::WHITE);
        y += line_height;
    }
}

/// The captions tab. `playhead` highlights the caption under the cursor.
pub fn captions_ui(
    ui: &mut egui::Ui,
    panel: &mut CaptionsPanelState,
    captions: &[Caption],
    playhead: f64,
) -> Vec<CaptionAction> {
    let mut actions = Vec::new();
    ui.horizontal(|ui| {
        if icons::icon_button(ui, Icon::Add, "Adicionar uma legenda no cursor", false).clicked() {
            actions.push(CaptionAction::Add);
        }
        if icons::icon_button(
            ui,
            Icon::Import,
            "Importar legendas (SRT ou WebVTT)…",
            false,
        )
        .clicked()
        {
            actions.push(CaptionAction::Import);
        }
        let any = !captions.is_empty();
        ui.add_enabled_ui(any, |ui| {
            if icons::icon_button(
                ui,
                Icon::Export,
                "Exportar legendas (SRT ou WebVTT)…",
                false,
            )
            .clicked()
            {
                actions.push(CaptionAction::Export);
            }
            if icons::icon_button(ui, Icon::Remove, "Apagar todas as legendas", false).clicked() {
                actions.push(CaptionAction::Clear);
            }
        });
        ui.weak(format!("{} legenda(s)", captions.len()));
    });
    ui.separator();
    if captions.is_empty() {
        ui.weak("Nenhuma legenda. Adicione uma no cursor ou importe um arquivo SRT ou WebVTT.");
        return actions;
    }

    // Drafts of captions that no longer exist (removed, undone) are dropped
    panel
        .text_drafts
        .retain(|id, _| captions.iter().any(|c| c.id == *id));
    panel
        .time_drafts
        .retain(|id, _| captions.iter().any(|c| c.id == *id));
    let current = current_index(captions, playhead);

    egui::ScrollArea::vertical().show(ui, |ui| {
        for (i, caption) in captions.iter().enumerate() {
            caption_row(ui, panel, caption, current == Some(i), &mut actions);
            ui.separator();
        }
    });
    actions
}

fn caption_row(
    ui: &mut egui::Ui,
    panel: &mut CaptionsPanelState,
    caption: &Caption,
    current: bool,
    actions: &mut Vec<CaptionAction>,
) {
    let id = caption.id;
    ui.horizontal(|ui| {
        if icons::icon_button_sized(ui, Icon::Play, "Ir para esta legenda", current, 22.0).clicked()
        {
            actions.push(CaptionAction::Seek(caption.start));
        }
        let (mut start, mut end) = panel
            .time_drafts
            .get(&id)
            .copied()
            .unwrap_or((caption.start, caption.end));
        let mut finished = false;
        let mut changed = false;
        for (label, value) in [("de", &mut start), ("até", &mut end)] {
            ui.label(label);
            let response = ui.add(
                egui::DragValue::new(value)
                    .speed(0.02)
                    .range(0.0..=36_000.0)
                    .fixed_decimals(3)
                    .suffix(" s"),
            );
            changed |= response.changed();
            finished |= response.drag_stopped() || response.lost_focus();
        }
        if changed {
            panel.time_drafts.insert(id, (start, end));
        }
        if finished {
            panel.time_drafts.remove(&id);
            let (s, e) = valid_times(start, end);
            if (s - caption.start).abs() > 1e-6 || (e - caption.end).abs() > 1e-6 {
                actions.push(CaptionAction::SetTimes(id, s, e));
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icons::icon_button_sized(ui, Icon::Remove, "Apagar esta legenda", false, 22.0)
                .clicked()
            {
                actions.push(CaptionAction::Remove(id));
            }
        });
    });
    let mut text = panel
        .text_drafts
        .get(&id)
        .cloned()
        .unwrap_or_else(|| caption.text.clone());
    let response = ui.add(
        egui::TextEdit::multiline(&mut text)
            .desired_rows(2)
            .desired_width(f32::INFINITY)
            .hint_text("Texto da legenda"),
    );
    if response.changed() {
        panel.text_drafts.insert(id, text.clone());
    }
    if response.lost_focus() {
        panel.text_drafts.remove(&id);
        if text != caption.text {
            actions.push(CaptionAction::SetText(id, text));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caption(id: u64, start: f64, end: f64, text: &str) -> Caption {
        Caption {
            id,
            start,
            end,
            text: text.into(),
            speaker: None,
        }
    }

    #[test]
    fn markup_is_removed_and_entities_decoded() {
        assert_eq!(plain_text("<i>Olá</i>, <b>mundo</b>"), "Olá, mundo");
        assert_eq!(plain_text("<v Ana>Oi &amp; tchau"), "Oi & tchau");
        assert_eq!(plain_text("a &lt;b&gt; c&nbsp;d"), "a <b> c d");
        assert_eq!(plain_text("<00:01.000>palavra"), "palavra");
        assert_eq!(plain_text("sem marcação"), "sem marcação");
        // An unclosed tag swallows the rest instead of showing raw markup
        assert_eq!(plain_text("texto <i"), "texto ");
    }

    #[test]
    fn long_lines_wrap_at_spaces_and_own_lines_are_kept() {
        let lines = display_lines("um dois três quatro cinco seis sete", 15);
        assert_eq!(lines, ["um dois três", "quatro cinco", "seis sete"]);
        assert!(lines.iter().all(|l| l.chars().count() <= 15));
        assert_eq!(
            display_lines("primeira\nsegunda", 40),
            ["primeira", "segunda"]
        );
        assert!(display_lines("   \n  ", 40).is_empty());
        // A word longer than the limit stays whole
        assert_eq!(
            display_lines("supercalifragilístico", 5),
            ["supercalifragilístico"]
        );
    }

    #[test]
    fn hand_edited_times_stay_valid() {
        assert_eq!(valid_times(1.0, 3.0), (1.0, 3.0));
        assert_eq!(valid_times(-2.0, 1.0), (0.0, 1.0));
        let (s, e) = valid_times(5.0, 5.0);
        assert!(e - s >= MIN_LENGTH - 1e-9);
        let (s, e) = valid_times(5.0, 2.0);
        assert!(e > s);
    }

    #[test]
    fn the_caption_under_the_cursor_is_found() {
        let list = [caption(1, 1.0, 2.0, "a"), caption(2, 3.0, 4.0, "b")];
        assert_eq!(current_index(&list, 0.5), None);
        assert_eq!(current_index(&list, 1.0), Some(0));
        assert_eq!(current_index(&list, 2.0), None, "the end is exclusive");
        assert_eq!(current_index(&list, 3.5), Some(1));
    }

    #[test]
    fn the_overlay_draws_every_line_with_its_outline_and_nothing_without_captions() {
        let ctx = egui::Context::default();
        let frame = Rect::from_min_size(pos2(100.0, 50.0), vec2(640.0, 360.0));
        let first = caption(
            1,
            0.0,
            2.0,
            "primeira linha
segunda linha",
        );
        let second = caption(2, 0.0, 2.0, "<i>outra</i> legenda");
        let texts = |shown: &[&Caption]| {
            let output = ctx.run(egui::RawInput::default(), |ctx| {
                let painter = ctx.layer_painter(egui::LayerId::background());
                paint_overlay(&painter, frame, shown);
            });
            output
                .shapes
                .iter()
                .filter(|s| matches!(s.shape, egui::Shape::Text(_)))
                .count()
        };
        // Three lines, each drawn six times for the outline and once for the white text
        assert_eq!(texts(&[&first, &second]), 3 * 7);
        assert_eq!(texts(&[]), 0);
    }
}
