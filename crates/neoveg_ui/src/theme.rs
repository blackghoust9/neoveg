//! Applies a `neoveg_common::theme::Theme` to egui and keeps the timeline palette.

use eframe::egui::{self, Color32, Rounding, Stroke};
use neoveg_common::theme::{Rgb, Theme};
use std::sync::RwLock;

/// Colors the custom-painted timeline reads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    pub timeline_bg: Color32,
    pub lane: Color32,
    pub lane_alt: Color32,
    pub grid_minor: Color32,
    pub grid_major: Color32,
    pub header: Color32,
    pub header_selected: Color32,
    pub audio_clip: Color32,
    pub video_clip: Color32,
    pub accent: Color32,
    /// A lighter accent for the playhead outline and guide.
    pub accent_bright: Color32,
}

fn color(c: Rgb) -> Color32 {
    Color32::from_rgb(c.0[0], c.0[1], c.0[2])
}

fn lighten(c: Color32, amount: f32) -> Color32 {
    let mix = |v: u8| (v as f32 + (255.0 - v as f32) * amount).round() as u8;
    Color32::from_rgb(mix(c.r()), mix(c.g()), mix(c.b()))
}

impl Palette {
    pub fn from_theme(theme: &Theme) -> Self {
        let c = &theme.colors;
        Self {
            timeline_bg: color(c.timeline_bg),
            lane: color(c.lane),
            lane_alt: color(c.lane_alt),
            grid_minor: color(c.grid_minor),
            grid_major: color(c.grid_major),
            header: color(c.header),
            header_selected: color(c.header_selected),
            audio_clip: color(c.audio_clip),
            video_clip: color(c.video_clip),
            accent: color(c.accent),
            accent_bright: lighten(color(c.accent), 0.18),
        }
    }
}

static PALETTE: RwLock<Option<Palette>> = RwLock::new(None);

/// The palette of the theme applied last (Modern Carbon Dark before any is applied).
pub fn palette() -> Palette {
    PALETTE
        .read()
        .ok()
        .and_then(|p| *p)
        .unwrap_or_else(|| Palette::from_theme(&Theme::default()))
}

/// Installs the theme: egui visuals for the widgets and the timeline palette.
pub fn apply(ctx: &egui::Context, theme: &Theme) {
    let c = &theme.colors;
    let mut v = if theme.dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    let (panel, recessed, text) = (color(c.panel), color(c.recessed), color(c.text));
    let accent = color(c.accent);
    v.panel_fill = panel;
    v.window_fill = panel;
    v.faint_bg_color = color(c.lane_alt);
    v.extreme_bg_color = recessed;
    v.override_text_color = Some(text);
    v.hyperlink_color = accent;
    v.selection.bg_fill = accent.linear_multiply(0.45);
    v.selection.stroke = Stroke::new(1.0_f32, accent);
    v.window_stroke = Stroke::new(1.0_f32, color(c.grid_major));
    for (w, fill) in [
        (&mut v.widgets.noninteractive, panel),
        (&mut v.widgets.inactive, color(c.widget)),
        (&mut v.widgets.hovered, color(c.widget_hover)),
        (&mut v.widgets.active, color(c.widget_active)),
        (&mut v.widgets.open, color(c.widget_hover)),
    ] {
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.fg_stroke.color = text;
        w.rounding = Rounding::ZERO;
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, color(c.grid_minor));
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, accent);
    ctx.set_visuals(v);
    if let Ok(mut slot) = PALETTE.write() {
        *slot = Some(Palette::from_theme(theme));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neoveg_common::theme::BUILTIN_IDS;

    #[test]
    fn the_carbon_palette_keeps_the_original_timeline_colors() {
        let p = Palette::from_theme(&Theme::modern_carbon_dark());
        assert_eq!(p.grid_minor, Color32::from_rgb(0x2C, 0x30, 0x38));
        assert_eq!(p.grid_major, Color32::from_rgb(0x4A, 0x50, 0x5E));
        assert_eq!(p.accent, Color32::from_rgb(0x29, 0x80, 0xB9));
    }

    #[test]
    fn every_builtin_builds_a_palette_with_a_lighter_bright_accent() {
        for id in BUILTIN_IDS {
            let p = Palette::from_theme(&Theme::builtin(id).unwrap());
            let sum = |c: Color32| c.r() as u32 + c.g() as u32 + c.b() as u32;
            assert!(sum(p.accent_bright) >= sum(p.accent), "{id}");
        }
    }

    #[test]
    fn applying_a_theme_updates_the_visuals_and_the_palette() {
        let ctx = egui::Context::default();
        let silver = Theme::classic_silver();
        apply(&ctx, &silver);
        assert!(!ctx.style().visuals.dark_mode);
        assert_eq!(palette(), Palette::from_theme(&silver));
        apply(&ctx, &Theme::default());
        assert!(ctx.style().visuals.dark_mode);
    }
}
