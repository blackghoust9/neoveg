//! Video Scopes panel: histogram, luma waveform, RGB parade and vectorscope of the preview frame.
//!
//! The numbers come from `neoveg_video::scopes` (plain functions); this file only draws them. The
//! grid scopes are painted into a small texture that is rebuilt when the frame or the mode changes.

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, ColorImage, FontId, Rect, Sense, Stroke, TextureHandle,
    TextureOptions,
};
use neoveg_common::i18n::tr;
use neoveg_common::i18n::tr_args;
use neoveg_video::scopes::{ycbcr, Grid, Scopes, GRID};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScopeMode {
    #[default]
    Waveform,
    Parade,
    Histogram,
    Vectorscope,
}

impl ScopeMode {
    pub const ALL: [ScopeMode; 4] = [
        ScopeMode::Waveform,
        ScopeMode::Parade,
        ScopeMode::Histogram,
        ScopeMode::Vectorscope,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ScopeMode::Waveform => tr("Forma de onda"),
            ScopeMode::Parade => tr("Parada RGB"),
            ScopeMode::Histogram => tr("Histograma"),
            ScopeMode::Vectorscope => tr("Vectorscópio"),
        }
    }
}

#[derive(Default)]
pub struct ScopesPanelState {
    pub mode: ScopeMode,
    texture: Option<(ScopeMode, u64, TextureHandle)>,
}

/// Image of `grid` tinted with `tint`, `GRID x GRID`, level 0 at the bottom.
pub fn grid_image(grids: &[(&Grid, Color32)]) -> ColorImage {
    let mut image = ColorImage::new([GRID * grids.len().max(1), GRID], Color32::BLACK);
    let total_w = GRID * grids.len().max(1);
    for (slot, (grid, tint)) in grids.iter().enumerate() {
        for y in 0..GRID {
            for x in 0..GRID {
                let k = grid.intensity(x, y);
                if k > 0.0 {
                    // Level 0 is the bottom row of the picture
                    let row = GRID - 1 - y;
                    let boost = 0.25 + 0.75 * k;
                    image.pixels[row * total_w + slot * GRID + x] = Color32::from_rgb(
                        (f32::from(tint.r()) * boost) as u8,
                        (f32::from(tint.g()) * boost) as u8,
                        (f32::from(tint.b()) * boost) as u8,
                    );
                }
            }
        }
    }
    image
}

/// The `(x, y)` of a color on the vectorscope, as fractions of the scope (`y = 0` at the top).
pub fn vectorscope_point(r: u8, g: u8, b: u8) -> (f32, f32) {
    let (_, cb, cr) = ycbcr(r, g, b);
    (f32::from(cb) / 255.0, 1.0 - f32::from(cr) / 255.0)
}

fn draw_histogram(ui: &mut egui::Ui, scopes: &Scopes, rect: Rect) {
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Color32::from_gray(10));
    let channels = [
        (0usize, Color32::from_rgba_unmultiplied(230, 60, 60, 150)),
        (1, Color32::from_rgba_unmultiplied(60, 210, 90, 150)),
        (2, Color32::from_rgba_unmultiplied(70, 120, 240, 150)),
        (3, Color32::from_rgba_unmultiplied(220, 220, 220, 110)),
    ];
    let peak = (0..4)
        .map(|c| scopes.histogram_peak(c))
        .max()
        .unwrap_or(1)
        .max(1) as f32;
    for (channel, color) in channels {
        for bin in 0..256 {
            let count = scopes.histogram[channel][bin] as f32;
            if count == 0.0 {
                continue;
            }
            // Square root keeps small bins readable next to a tall spike
            let h = (count / peak).sqrt() * rect.height();
            let x = rect.left() + (bin as f32 + 0.5) / 256.0 * rect.width();
            painter.line_segment(
                [pos2(x, rect.bottom()), pos2(x, rect.bottom() - h)],
                Stroke::new((rect.width() / 256.0).max(1.0), color),
            );
        }
    }
    for tick in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let x = rect.left() + tick * rect.width();
        painter.line_segment(
            [pos2(x, rect.top()), pos2(x, rect.bottom())],
            Stroke::new(1.0_f32, Color32::from_white_alpha(25)),
        );
    }
    if scopes.clipped_fraction() > 0.001 {
        painter.text(
            rect.right_top() + vec2(-4.0, 4.0),
            Align2::RIGHT_TOP,
            tr_args(
                "Estourado {} %",
                &[&format!("{:.1}", scopes.clipped_fraction() * 100.0)],
            ),
            FontId::monospace(10.0),
            Color32::from_rgb(231, 76, 60),
        );
    }
}

fn graticule(ui: &egui::Ui, rect: Rect) {
    let painter = ui.painter_at(rect);
    for (level, label) in [
        (0.0, "0"),
        (0.25, "25"),
        (0.5, "50"),
        (0.75, "75"),
        (1.0, "100"),
    ] {
        let y = rect.bottom() - level * rect.height();
        painter.line_segment(
            [pos2(rect.left(), y), pos2(rect.right(), y)],
            Stroke::new(1.0_f32, Color32::from_white_alpha(35)),
        );
        painter.text(
            pos2(rect.left() + 2.0, y - 1.0),
            Align2::LEFT_BOTTOM,
            label,
            FontId::monospace(9.0),
            Color32::from_white_alpha(110),
        );
    }
}

fn draw_vectorscope_overlay(ui: &egui::Ui, rect: Rect) {
    let painter = ui.painter_at(rect);
    let c = rect.center();
    let r = rect.width().min(rect.height()) / 2.0;
    for k in [0.33, 0.66, 1.0] {
        painter.circle_stroke(
            c,
            r * k,
            Stroke::new(1.0_f32, Color32::from_white_alpha(30)),
        );
    }
    painter.line_segment(
        [pos2(rect.left(), c.y), pos2(rect.right(), c.y)],
        Stroke::new(1.0_f32, Color32::from_white_alpha(30)),
    );
    painter.line_segment(
        [pos2(c.x, rect.top()), pos2(c.x, rect.bottom())],
        Stroke::new(1.0_f32, Color32::from_white_alpha(30)),
    );
    // Targets of the 75 % color bars: red, magenta, blue, cyan, green, yellow
    for (name, (r8, g8, b8)) in [
        ("R", (191u8, 0u8, 0u8)),
        ("M", (191, 0, 191)),
        ("B", (0, 0, 191)),
        ("C", (0, 191, 191)),
        ("G", (0, 191, 0)),
        ("Y", (191, 191, 0)),
    ] {
        let (fx, fy) = vectorscope_point(r8, g8, b8);
        let p = pos2(
            rect.left() + fx * rect.width(),
            rect.top() + fy * rect.height(),
        );
        painter.rect_stroke(
            Rect::from_center_size(p, vec2(10.0, 10.0)),
            0.0,
            Stroke::new(1.0_f32, Color32::from_white_alpha(120)),
        );
        painter.text(
            p + vec2(7.0, -7.0),
            Align2::LEFT_BOTTOM,
            name,
            FontId::monospace(9.0),
            Color32::from_white_alpha(140),
        );
    }
}

/// Draws the panel. `scopes` is the frame at the cursor (or the color bars), `version` changes
/// whenever it does so the texture is rebuilt only then.
pub fn scopes_ui(
    ui: &mut egui::Ui,
    state: &mut ScopesPanelState,
    scopes: Option<&Scopes>,
    version: u64,
) {
    egui::ComboBox::from_id_salt("scope_mode")
        .selected_text(state.mode.label())
        .width(130.0)
        .show_ui(ui, |ui| {
            for mode in ScopeMode::ALL {
                ui.selectable_value(&mut state.mode, mode, mode.label());
            }
        });
    let Some(scopes) = scopes else {
        ui.weak(tr(
            "Sem imagem no cursor. Ative as barras SMPTE ou posicione o cursor sobre um vídeo.",
        ));
        return;
    };
    let size = ui.available_size().max(vec2(120.0, 140.0));
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    match state.mode {
        ScopeMode::Histogram => draw_histogram(ui, scopes, rect),
        mode => {
            let stale = state
                .texture
                .as_ref()
                .is_none_or(|(m, v, _)| *m != mode || *v != version);
            if stale {
                let image = match mode {
                    ScopeMode::Waveform => {
                        grid_image(&[(&scopes.waveform, Color32::from_rgb(90, 255, 120))])
                    }
                    ScopeMode::Parade => grid_image(&[
                        (&scopes.parade[0], Color32::from_rgb(255, 90, 90)),
                        (&scopes.parade[1], Color32::from_rgb(90, 255, 110)),
                        (&scopes.parade[2], Color32::from_rgb(110, 150, 255)),
                    ]),
                    _ => grid_image(&[(&scopes.vectorscope, Color32::from_rgb(120, 255, 170))]),
                };
                let handle = ui
                    .ctx()
                    .load_texture("video_scope", image, TextureOptions::LINEAR);
                state.texture = Some((mode, version, handle));
            }
            if let Some((_, _, texture)) = &state.texture {
                let painter = ui.painter_at(rect);
                painter.rect_filled(rect, 0.0, Color32::from_gray(8));
                // The vectorscope is square; the others fill the panel
                let target = if mode == ScopeMode::Vectorscope {
                    let side = rect.width().min(rect.height());
                    Rect::from_center_size(rect.center(), vec2(side, side))
                } else {
                    rect
                };
                painter.image(
                    texture.id(),
                    target,
                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
                if mode == ScopeMode::Vectorscope {
                    draw_vectorscope_overlay(ui, target);
                } else {
                    graticule(ui, target);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neoveg_video::frame::FrameBuffer;

    #[test]
    fn a_grid_image_puts_level_zero_at_the_bottom_and_tints_hits() {
        let scopes = Scopes::from_frame(&FrameBuffer::solid(16, 16, [0, 0, 0, 255]));
        let image = grid_image(&[(&scopes.waveform, Color32::from_rgb(100, 200, 100))]);
        assert_eq!(image.size, [GRID, GRID]);
        // Black pixels sit at level 0 = the last row
        let last_row = &image.pixels[(GRID - 1) * GRID..];
        assert!(last_row.iter().any(|p| *p != Color32::BLACK));
        assert!(image.pixels[..GRID].iter().all(|p| *p == Color32::BLACK));
    }

    #[test]
    fn three_grids_are_laid_side_by_side() {
        let scopes = Scopes::from_frame(&FrameBuffer::solid(16, 16, [255, 255, 255, 255]));
        let tint = Color32::WHITE;
        let image = grid_image(&[
            (&scopes.parade[0], tint),
            (&scopes.parade[1], tint),
            (&scopes.parade[2], tint),
        ]);
        assert_eq!(image.size, [GRID * 3, GRID]);
        // Full-level hits are in the top row of every third
        for slot in 0..3 {
            assert!(image.pixels[slot * GRID..(slot + 1) * GRID]
                .iter()
                .any(|p| *p != Color32::BLACK));
        }
    }

    #[test]
    fn the_bar_targets_land_where_the_colors_belong() {
        let (rx, ry) = vectorscope_point(191, 0, 0);
        assert!(rx < 0.5 && ry < 0.5, "red is up-left: {rx} {ry}");
        let (bx, by) = vectorscope_point(0, 0, 191);
        assert!(bx > 0.5 && by > 0.5, "blue is down-right: {bx} {by}");
        let (gx, gy) = vectorscope_point(128, 128, 128);
        assert!((gx - 0.5).abs() < 0.01 && (gy - 0.5).abs() < 0.01);
    }
}
