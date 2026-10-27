//! Vector toolbar icons: drawn with egui shapes, plus a library of SVG files for the essential tools.
//!
//! Flat, monochrome pictograms in the spirit of classic NLE toolbars: original artwork, no bitmap
//! assets, so they stay crisp at any scale and monitor DPI and follow the colours of the theme.
//! Every icon is described on a unit square and turned into shapes by [`shapes`], which is pure
//! and testable without a window.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use eframe::egui::{self, vec2, Color32, Pos2, Rect, Response, Rounding, Sense, Shape, Stroke};

macro_rules! icons {
    ($($name:ident),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum Icon {
            $($name),+
        }

        impl Icon {
            /// Every icon, so tests cover new ones automatically.
            #[cfg(test)]
            pub const ALL: &'static [Icon] = &[$(Icon::$name),+];
        }
    };
}

icons! {
    GoToStart,
    Play,
    Pause,
    Stop,
    Split,
    Undo,
    Redo,
    AddAudioTrack,
    AddVideoTrack,
    Loop,
    Group,
    Snap,
    Crossfade,
    ZoomOut,
    ZoomIn,
    Reset,
    Bypass,
    Remove,
    MoveUp,
    MoveDown,
    Add,
    Import,
    Export,
    Mute,
    Solo,
    Phase,
    Record,
    Lock,
}

impl Icon {
    /// The colour of the icon at rest; hovering brightens it.
    pub fn tint(self) -> Color32 {
        match self {
            Icon::Play => Color32::from_rgb(0x6D, 0xD4, 0x7E),
            Icon::Stop => Color32::from_rgb(0xD8, 0x7A, 0x7A),
            Icon::Record => Color32::from_rgb(0xE0, 0x4B, 0x4B),
            Icon::AddAudioTrack => Color32::from_rgb(0x6C, 0xB4, 0xE4),
            Icon::AddVideoTrack => Color32::from_rgb(0xB4, 0x9A, 0xE6),
            _ => Color32::from_gray(205),
        }
    }
}

/// The icon's resting color on the current theme: neutral icons follow the text color and the
/// colored ones are darkened on a light theme so they stay readable.
fn themed_tint(icon: Icon, visuals: &egui::Visuals) -> Color32 {
    let tint = icon.tint();
    if tint == Color32::from_gray(205) {
        visuals.text_color()
    } else if visuals.dark_mode {
        tint
    } else {
        tint.gamma_multiply(0.55)
    }
}

/// Drawing surface over the unit square `0..1` mapped onto the largest centred square of a rect.
struct Canvas {
    origin: Pos2,
    size: f32,
    color: Color32,
    width: f32,
    /// Draws the icon flipped left to right (Redo is a mirrored Undo).
    mirror: bool,
    shapes: Vec<Shape>,
}

type Pt = (f32, f32);

impl Canvas {
    fn new(rect: Rect, color: Color32) -> Self {
        let size = rect.width().min(rect.height());
        Self {
            origin: rect.center() - vec2(size, size) / 2.0,
            size,
            color,
            width: (size * 0.09).max(1.0),
            mirror: false,
            shapes: Vec::new(),
        }
    }

    fn p(&self, (x, y): Pt) -> Pos2 {
        let x = if self.mirror { 1.0 - x } else { x };
        self.origin + vec2(x, y) * self.size
    }

    fn pts(&self, points: &[Pt]) -> Vec<Pos2> {
        points.iter().map(|&p| self.p(p)).collect()
    }

    fn stroke(&self, scale: f32) -> Stroke {
        Stroke::new(self.width * scale, self.color)
    }

    fn line(&mut self, a: Pt, b: Pt, scale: f32) {
        let stroke = self.stroke(scale);
        self.shapes
            .push(Shape::line_segment([self.p(a), self.p(b)], stroke));
    }

    fn path(&mut self, points: &[Pt]) {
        let stroke = self.stroke(1.0);
        self.shapes.push(Shape::line(self.pts(points), stroke));
    }

    /// Filled convex polygon.
    fn fill(&mut self, points: &[Pt]) {
        self.shapes.push(Shape::convex_polygon(
            self.pts(points),
            self.color,
            Stroke::NONE,
        ));
    }

    fn fill_rect(&mut self, (x0, y0): Pt, (x1, y1): Pt) {
        self.fill(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)]);
    }

    fn outline_rect(&mut self, (x0, y0): Pt, (x1, y1): Pt) {
        let stroke = self.stroke(1.0);
        let rect = Rect::from_two_pos(self.p((x0, y0)), self.p((x1, y1)));
        self.shapes
            .push(Shape::rect_stroke(rect, Rounding::ZERO, stroke));
    }

    fn circle(&mut self, (cx, cy): Pt, r: f32) {
        let stroke = self.stroke(1.0);
        self.shapes.push(Shape::circle_stroke(
            self.p((cx, cy)),
            r * self.size,
            stroke,
        ));
    }

    /// Triangle whose tip is `tip`, pointing along `dir` (a unit vector in icon space).
    fn arrowhead(&mut self, tip: Pt, dir: Pt, length: f32) {
        let (dx, dy) = dir;
        let (px, py) = (-dy, dx);
        let base = (tip.0 - dx * length, tip.1 - dy * length);
        let half = length * 0.55;
        self.fill(&[
            tip,
            (base.0 + px * half, base.1 + py * half),
            (base.0 - px * half, base.1 - py * half),
        ]);
    }

    fn plus(&mut self, (cx, cy): Pt, half: f32) {
        self.line((cx - half, cy), (cx + half, cy), 1.0);
        self.line((cx, cy - half), (cx, cy + half), 1.0);
    }
}

/// Points of an arc of the circle at `centre`; angles in degrees, measured clockwise on screen
/// from the right (so `270` is the top).
fn arc((cx, cy): Pt, r: f32, from: f32, to: f32, steps: u32) -> Vec<Pt> {
    (0..=steps)
        .map(|i| {
            let a = (from + (to - from) * i as f32 / steps as f32).to_radians();
            (cx + r * a.cos(), cy + r * a.sin())
        })
        .collect()
}

/// An arc with an arrowhead at its end, as on Undo and Reset.
fn arc_arrow(c: &mut Canvas, centre: Pt, r: f32, from: f32, to: f32) {
    c.path(&arc(centre, r, from, to, 20));
    let end = (to).to_radians();
    let tip = (centre.0 + r * end.cos(), centre.1 + r * end.sin());
    // Direction of travel along the arc
    let sweep = if to < from { -1.0 } else { 1.0 };
    let dir = (-end.sin() * sweep, end.cos() * sweep);
    c.arrowhead((tip.0 + dir.0 * 0.07, tip.1 + dir.1 * 0.07), dir, 0.2);
}

fn draw(c: &mut Canvas, icon: Icon) {
    match icon {
        Icon::GoToStart => {
            c.fill_rect((0.2, 0.2), (0.3, 0.8));
            c.fill(&[(0.8, 0.2), (0.8, 0.8), (0.36, 0.5)]);
        }
        Icon::Play => c.fill(&[(0.28, 0.16), (0.28, 0.84), (0.82, 0.5)]),
        Icon::Pause => {
            c.fill_rect((0.24, 0.2), (0.42, 0.8));
            c.fill_rect((0.58, 0.2), (0.76, 0.8));
        }
        Icon::Stop => c.fill_rect((0.22, 0.22), (0.78, 0.78)),
        Icon::Split => {
            // The cursor line with a clip on each side of it
            c.fill(&[(0.38, 0.08), (0.62, 0.08), (0.5, 0.24)]);
            c.line((0.5, 0.22), (0.5, 0.92), 1.0);
            c.outline_rect((0.12, 0.38), (0.42, 0.78));
            c.outline_rect((0.58, 0.38), (0.88, 0.78));
        }
        Icon::Undo => arc_arrow(c, (0.5, 0.58), 0.27, 25.0, -170.0),
        Icon::Redo => {
            c.mirror = true;
            arc_arrow(c, (0.5, 0.58), 0.27, 25.0, -170.0);
        }
        Icon::AddAudioTrack => {
            c.fill_rect((0.08, 0.44), (0.24, 0.68));
            c.fill(&[(0.24, 0.44), (0.44, 0.3), (0.44, 0.82), (0.24, 0.68)]);
            c.path(&arc((0.44, 0.56), 0.2, -50.0, 50.0, 8));
            c.plus((0.78, 0.22), 0.12);
        }
        Icon::AddVideoTrack => {
            c.outline_rect((0.08, 0.42), (0.6, 0.88));
            c.fill(&[(0.27, 0.54), (0.27, 0.76), (0.44, 0.65)]);
            c.fill(&[(0.6, 0.56), (0.84, 0.46), (0.84, 0.84), (0.6, 0.74)]);
            c.plus((0.74, 0.2), 0.12);
        }
        Icon::Loop => {
            c.path(&[(0.2, 0.5), (0.2, 0.3), (0.66, 0.3)]);
            c.arrowhead((0.84, 0.3), (1.0, 0.0), 0.2);
            c.path(&[(0.8, 0.5), (0.8, 0.7), (0.34, 0.7)]);
            c.arrowhead((0.16, 0.7), (-1.0, 0.0), 0.2);
        }
        Icon::Group => {
            c.outline_rect((0.1, 0.1), (0.58, 0.58));
            let faded = c.color;
            c.color = faded.gamma_multiply(0.45);
            c.fill_rect((0.42, 0.42), (0.9, 0.9));
            c.color = faded;
            c.outline_rect((0.42, 0.42), (0.9, 0.9));
        }
        Icon::Snap => {
            // Horseshoe magnet with two lit poles
            c.path(&arc((0.5, 0.5), 0.26, 180.0, 360.0, 16));
            c.line((0.24, 0.5), (0.24, 0.72), 1.0);
            c.line((0.76, 0.5), (0.76, 0.72), 1.0);
            c.fill_rect((0.14, 0.7), (0.34, 0.86));
            c.fill_rect((0.66, 0.7), (0.86, 0.86));
        }
        Icon::Crossfade => {
            c.outline_rect((0.1, 0.18), (0.9, 0.82));
            c.line((0.2, 0.7), (0.8, 0.3), 1.0);
            c.line((0.2, 0.3), (0.8, 0.7), 1.0);
        }
        Icon::ZoomOut | Icon::ZoomIn => {
            c.circle((0.42, 0.42), 0.26);
            c.line((0.62, 0.62), (0.86, 0.86), 1.7);
            c.line((0.3, 0.42), (0.54, 0.42), 1.0);
            if icon == Icon::ZoomIn {
                c.line((0.42, 0.3), (0.42, 0.54), 1.0);
            }
        }
        Icon::Reset => arc_arrow(c, (0.5, 0.5), 0.3, -50.0, -330.0),
        Icon::Bypass => {
            // Power symbol: a ring open at the top and a stroke through the gap
            c.path(&arc((0.5, 0.56), 0.3, -60.0, 240.0, 20));
            c.line((0.5, 0.12), (0.5, 0.52), 1.2);
        }
        Icon::Record => {
            // A filled disc
            let points = arc((0.5, 0.5), 0.3, 0.0, 360.0, 24);
            c.fill(&points);
        }
        Icon::Mute => {
            // A speaker with a cross over its sound
            c.fill(&[
                (0.14, 0.38),
                (0.3, 0.38),
                (0.5, 0.2),
                (0.5, 0.8),
                (0.3, 0.62),
                (0.14, 0.62),
            ]);
            c.line((0.62, 0.38), (0.88, 0.62), 1.0);
            c.line((0.88, 0.38), (0.62, 0.62), 1.0);
        }
        Icon::Solo => {
            // Headphones
            c.path(&arc((0.5, 0.56), 0.32, 180.0, 360.0, 16));
            c.fill_rect((0.14, 0.54), (0.3, 0.82));
            c.fill_rect((0.7, 0.54), (0.86, 0.82));
        }
        Icon::Phase => {
            // A circle with a slash: polarity inversion
            c.circle((0.5, 0.5), 0.32);
            c.line((0.26, 0.74), (0.74, 0.26), 1.0);
        }
        Icon::Lock => {
            // A padlock: solid body under a square-topped shackle
            c.fill_rect((0.26, 0.46), (0.74, 0.84));
            c.path(&[(0.36, 0.46), (0.36, 0.2), (0.64, 0.2), (0.64, 0.46)]);
        }
        Icon::Remove => {
            c.line((0.24, 0.24), (0.76, 0.76), 1.1);
            c.line((0.24, 0.76), (0.76, 0.24), 1.1);
        }
        Icon::MoveUp => c.path(&[(0.22, 0.64), (0.5, 0.34), (0.78, 0.64)]),
        Icon::MoveDown => c.path(&[(0.22, 0.36), (0.5, 0.66), (0.78, 0.36)]),
        Icon::Add => c.plus((0.5, 0.5), 0.3),
        Icon::Import => {
            // An arrow going down into a tray
            c.line((0.5, 0.12), (0.5, 0.56), 1.0);
            c.arrowhead((0.5, 0.7), (0.0, 1.0), 0.24);
            c.path(&[(0.16, 0.6), (0.16, 0.86), (0.84, 0.86), (0.84, 0.6)]);
        }
        Icon::Export => {
            // An arrow coming up out of a tray
            c.line((0.5, 0.88), (0.5, 0.44), 1.0);
            c.arrowhead((0.5, 0.18), (0.0, -1.0), 0.24);
            c.path(&[(0.16, 0.6), (0.16, 0.86), (0.84, 0.86), (0.84, 0.6)]);
        }
    }
}

/// The shapes of `icon` inside `rect`, in `color`.
pub fn shapes(icon: Icon, rect: Rect, color: Color32) -> Vec<Shape> {
    let mut canvas = Canvas::new(rect, color);
    draw(&mut canvas, icon);
    canvas.shapes
}

/// Side of the square toolbar buttons, in points.
pub const BUTTON_SIZE: f32 = 26.0;

/// A flat, square-cornered icon button; the name and shortcut go in `tooltip`. `selected` shows
/// the pressed state of a toggle.
pub fn icon_button(ui: &mut egui::Ui, icon: Icon, tooltip: &str, selected: bool) -> Response {
    icon_button_sized(ui, icon, tooltip, selected, BUTTON_SIZE)
}

pub fn icon_button_sized(
    ui: &mut egui::Ui,
    icon: Icon,
    tooltip: &str,
    selected: bool,
    side: f32,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(side, side), Sense::click());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let hovered = response.hovered();
        let fill = if selected {
            ui.visuals().selection.bg_fill
        } else if hovered {
            ui.visuals().widgets.hovered.bg_fill
        } else {
            Color32::TRANSPARENT
        };
        painter.rect_filled(rect, Rounding::ZERO, fill);
        if selected {
            painter.rect_stroke(
                rect,
                Rounding::ZERO,
                Stroke::new(1.0_f32, ui.visuals().selection.stroke.color),
            );
        }
        let color = if !ui.is_enabled() {
            ui.visuals().weak_text_color()
        } else {
            let tint = themed_tint(icon, ui.visuals());
            if hovered || selected {
                tint.gamma_multiply(1.15)
            } else {
                tint
            }
        };
        let inner = rect.shrink(side * 0.18);
        // Icons that have an SVG in the library are drawn from it; the rest (or an SVG that cannot
        // be rendered) use the shape drawing
        let drawn = svg_for(icon).is_some_and(|svg| paint_svg(ui, svg, inner, color));
        if !drawn {
            ui.painter().extend(shapes(icon, inner, color));
        }
    }
    response.on_hover_text(tooltip)
}

// ---- the SVG library ------------------------------------------------------------------------------

/// The SVG files in `assets/icons`: 24x24 viewBox, `currentColor`, flat and square-cornered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SvgIcon {
    PanCrop,
    EventFx,
    Snapping,
    AutoRipple,
    Split,
    TransportPlay,
    TransportPause,
    TransportStop,
}

impl SvgIcon {
    pub const ALL: [SvgIcon; 8] = [
        SvgIcon::PanCrop,
        SvgIcon::EventFx,
        SvgIcon::Snapping,
        SvgIcon::AutoRipple,
        SvgIcon::Split,
        SvgIcon::TransportPlay,
        SvgIcon::TransportPause,
        SvgIcon::TransportStop,
    ];

    /// File name without the extension.
    pub fn name(self) -> &'static str {
        match self {
            SvgIcon::PanCrop => "pan_crop",
            SvgIcon::EventFx => "event_fx",
            SvgIcon::Snapping => "snapping",
            SvgIcon::AutoRipple => "auto_ripple",
            SvgIcon::Split => "split",
            SvgIcon::TransportPlay => "transport_play",
            SvgIcon::TransportPause => "transport_pause",
            SvgIcon::TransportStop => "transport_stop",
        }
    }

    /// The file as written (with `currentColor`).
    pub fn source(self) -> &'static str {
        match self {
            SvgIcon::PanCrop => include_str!("../assets/icons/pan_crop.svg"),
            SvgIcon::EventFx => include_str!("../assets/icons/event_fx.svg"),
            SvgIcon::Snapping => include_str!("../assets/icons/snapping.svg"),
            SvgIcon::AutoRipple => include_str!("../assets/icons/auto_ripple.svg"),
            SvgIcon::Split => include_str!("../assets/icons/split.svg"),
            SvgIcon::TransportPlay => include_str!("../assets/icons/transport_play.svg"),
            SvgIcon::TransportPause => include_str!("../assets/icons/transport_pause.svg"),
            SvgIcon::TransportStop => include_str!("../assets/icons/transport_stop.svg"),
        }
    }

    /// Identifier the image loader caches the rasterized icon under.
    pub fn uri(self) -> String {
        format!("bytes://neoveg/icons/{}.svg", self.name())
    }
}

/// The SVG for a shape icon, when the library has one.
pub fn svg_for(icon: Icon) -> Option<SvgIcon> {
    match icon {
        Icon::Play => Some(SvgIcon::TransportPlay),
        Icon::Pause => Some(SvgIcon::TransportPause),
        Icon::Stop => Some(SvgIcon::TransportStop),
        Icon::Split => Some(SvgIcon::Split),
        Icon::Snap => Some(SvgIcon::Snapping),
        _ => None,
    }
}

/// The SVG with every `currentColor` made white, so the image can be tinted with any theme color
/// (a white image multiplied by a tint is that tint).
fn tintable_bytes(icon: SvgIcon) -> Arc<[u8]> {
    static CACHE: OnceLock<HashMap<&'static str, Arc<[u8]>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| {
        SvgIcon::ALL
            .iter()
            .map(|i| {
                let text = i.source().replace("currentColor", "#ffffff");
                (i.name(), Arc::from(text.into_bytes()))
            })
            .collect()
    });
    Arc::clone(&cache[icon.name()])
}

/// Installs the image loaders the SVG icons need; call once when the window is created.
pub fn install(ctx: &egui::Context) {
    egui_extras::install_image_loaders(ctx);
}

/// Draws `icon` into `rect` tinted with `color`. `false` when it could not be rendered (the
/// caller then falls back to another drawing); a pending load asks for a repaint and counts as drawn.
pub fn paint_svg(ui: &egui::Ui, icon: SvgIcon, rect: Rect, color: Color32) -> bool {
    let image = egui::Image::from_bytes(icon.uri(), tintable_bytes(icon)).tint(color);
    match image.load_for_size(ui.ctx(), rect.size()) {
        Ok(egui::load::TexturePoll::Ready { .. }) => {
            image.paint_at(ui, rect);
            true
        }
        Ok(egui::load::TexturePoll::Pending { .. }) => {
            ui.ctx().request_repaint();
            true
        }
        Err(e) => {
            tracing::warn!("[WARN] cannot render the icon {}: {e}", icon.name());
            false
        }
    }
}

/// A flat, square-cornered button showing an SVG icon (the same look as [`icon_button`]).
#[allow(dead_code)] // for the event-header buttons (pan/crop, event FX) and the auto ripple toggle
pub fn svg_button(ui: &mut egui::Ui, icon: SvgIcon, tooltip: &str, selected: bool) -> Response {
    let side = BUTTON_SIZE;
    let (rect, response) = ui.allocate_exact_size(vec2(side, side), Sense::click());
    if ui.is_rect_visible(rect) {
        let fill = if selected {
            ui.visuals().selection.bg_fill
        } else if response.hovered() {
            ui.visuals().widgets.hovered.bg_fill
        } else {
            Color32::TRANSPARENT
        };
        ui.painter().rect_filled(rect, Rounding::ZERO, fill);
        if selected {
            ui.painter().rect_stroke(
                rect,
                Rounding::ZERO,
                Stroke::new(1.0_f32, ui.visuals().selection.stroke.color),
            );
        }
        let color = if ui.is_enabled() {
            ui.visuals().text_color()
        } else {
            ui.visuals().weak_text_color()
        };
        paint_svg(ui, icon, rect.shrink(side * 0.18), color);
    }
    response.on_hover_text(tooltip)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_svg_follows_the_house_style() {
        for icon in SvgIcon::ALL {
            let text = icon.source();
            let name = icon.name();
            assert!(text.contains("viewBox=\"0 0 24 24\""), "{name}: viewBox");
            assert!(text.contains("currentColor"), "{name}: currentColor");
            // Colors only through currentColor, so the theme decides them
            for banned in ["#", "rgb(", "url(", "<image", "<text"] {
                assert!(!text.contains(banned), "{name}: {banned}");
            }
            // Flat and square: no rounded joins or caps, no rounded rectangles
            assert!(!text.contains("round"), "{name}: round");
            assert!(!text.contains("rx="), "{name}: rx");
            assert!(text.trim_end().ends_with("</svg>"), "{name}");
            assert_eq!(text.matches("<svg").count(), 1, "{name}");
        }
    }

    #[test]
    fn svg_names_are_unique_and_match_their_files() {
        let mut names: Vec<_> = SvgIcon::ALL.iter().map(|i| i.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), SvgIcon::ALL.len());
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons");
        for icon in SvgIcon::ALL {
            let on_disk = std::fs::read_to_string(format!("{dir}/{}.svg", icon.name())).unwrap();
            assert_eq!(on_disk, icon.source());
        }
    }

    #[test]
    fn the_tinted_bytes_drop_current_color_and_keep_the_drawing() {
        for icon in SvgIcon::ALL {
            let text = String::from_utf8(tintable_bytes(icon).to_vec()).unwrap();
            assert!(!text.contains("currentColor"), "{}", icon.name());
            assert!(text.contains("#ffffff"), "{}", icon.name());
            assert!(text.contains("<svg") && text.contains("</svg>"));
        }
    }

    #[test]
    fn the_transport_and_edit_icons_use_their_svgs() {
        assert_eq!(svg_for(Icon::Play), Some(SvgIcon::TransportPlay));
        assert_eq!(svg_for(Icon::Pause), Some(SvgIcon::TransportPause));
        assert_eq!(svg_for(Icon::Stop), Some(SvgIcon::TransportStop));
        assert_eq!(svg_for(Icon::Split), Some(SvgIcon::Split));
        assert_eq!(svg_for(Icon::Snap), Some(SvgIcon::Snapping));
        assert_eq!(svg_for(Icon::Undo), None);
    }
    use eframe::egui::pos2;

    fn frame() -> Rect {
        Rect::from_min_size(pos2(10.0, 20.0), vec2(24.0, 24.0))
    }

    #[test]
    fn every_icon_draws_something_inside_its_frame() {
        let bounds = frame().expand(1.0);
        for &icon in Icon::ALL {
            let shapes = shapes(icon, frame(), Color32::WHITE);
            assert!(!shapes.is_empty(), "{icon:?} draws nothing");
            for shape in &shapes {
                let r = shape.visual_bounding_rect();
                assert!(
                    bounds.contains_rect(r),
                    "{icon:?} leaves its frame: {r:?} outside {bounds:?}"
                );
            }
        }
    }

    #[test]
    fn icons_scale_with_their_frame_and_stay_centred() {
        for &icon in Icon::ALL {
            let small = Rect::from_min_size(pos2(0.0, 0.0), vec2(16.0, 16.0));
            let big = Rect::from_min_size(pos2(100.0, 100.0), vec2(64.0, 64.0));
            let union = |rect| {
                shapes(icon, rect, Color32::WHITE)
                    .iter()
                    .map(Shape::visual_bounding_rect)
                    .fold(Rect::NOTHING, Rect::union)
            };
            let (s, b) = (union(small), union(big));
            assert!(b.width() > s.width() * 3.0, "{icon:?} does not scale");
            // A non-square frame draws in its largest centred square
            let wide = Rect::from_min_size(pos2(0.0, 0.0), vec2(40.0, 16.0));
            let w = union(wide);
            assert!(
                (w.center().x - wide.center().x).abs() < 4.0,
                "{icon:?} is off centre"
            );
        }
    }

    #[test]
    fn redo_is_the_mirror_image_of_undo() {
        let bounds = |icon| {
            shapes(icon, frame(), Color32::WHITE)
                .iter()
                .map(Shape::visual_bounding_rect)
                .fold(Rect::NOTHING, Rect::union)
        };
        let (undo, redo) = (bounds(Icon::Undo), bounds(Icon::Redo));
        let centre = frame().center().x;
        assert!(((undo.left() - centre) + (redo.right() - centre)).abs() < 0.5);
        assert!((undo.height() - redo.height()).abs() < 0.01);
    }

    #[test]
    fn zoom_in_adds_the_vertical_stroke() {
        let count = |icon| shapes(icon, frame(), Color32::WHITE).len();
        assert_eq!(count(Icon::ZoomIn), count(Icon::ZoomOut) + 1);
    }

    #[test]
    fn tints_are_visible_on_the_dark_toolbar() {
        for &icon in Icon::ALL {
            let c = icon.tint();
            let luma = (u32::from(c.r()) + u32::from(c.g()) + u32::from(c.b())) / 3;
            assert!(luma > 120, "{icon:?} is too dark: {c:?}");
        }
    }
}
