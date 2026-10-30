//! The Pan/Crop editor: a window with the source picture, the selection box with its handles, the
//! numeric fields, the keyframe strip and, when asked, the live preview beside it.
//!
//! The geometry (handle positions, hit-testing and what a drag does to the box) is made of pure
//! functions so it is tested without a window. The window itself only returns [`PanCropAction`]s:
//! `Draft` while something is being dragged (the preview shows it at once, the project is not
//! touched) and `Commit` when the gesture ends, which `AppState` turns into one undo step.

use eframe::egui::{self, pos2, vec2, Color32, Pos2, Rect, Sense, Stroke, TextureHandle};
use neoveg_common::i18n::tr;
use neoveg_common::i18n::tr_args;
use neoveg_common::pan_crop::{PanCrop, PanCropFrame, KEY_TOLERANCE};
use neoveg_engine::{EventId, TrackId};

/// How close (in canvas pixels) the pointer has to be to grab a handle.
const GRAB_RADIUS: f32 = 9.0;
/// Distance of the rotation handle above the top edge of the box.
const ROTATE_OFFSET: f32 = 28.0;

/// Directions of the eight resize handles in the box's own axes (`-1..=1` of the half sizes), in
/// the order top-left, top, top-right, right, bottom-right, bottom, bottom-left, left.
pub const HANDLE_DIRS: [(f32, f32); 8] = [
    (-1.0, -1.0),
    (0.0, -1.0),
    (1.0, -1.0),
    (1.0, 0.0),
    (1.0, 1.0),
    (0.0, 1.0),
    (-1.0, 1.0),
    (-1.0, 0.0),
];

/// What the pointer holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handle {
    /// The inside of the box: drag to move it.
    Move,
    /// The round handle above the box: drag to turn it.
    Rotate,
    /// One of the eight resize handles (index into [`HANDLE_DIRS`]).
    Resize(usize),
}

/// Which way the box may move.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Axis {
    #[default]
    Free,
    XOnly,
    YOnly,
}

/// The switches of a drag.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DragOptions {
    /// Keep the box's proportions while resizing.
    pub lock_aspect: bool,
    /// Resize around the centre instead of anchoring the opposite side.
    pub about_center: bool,
    pub axis: Axis,
    /// Turn in steps of 15 degrees.
    pub snap_rotation: bool,
}

impl Default for DragOptions {
    fn default() -> Self {
        Self {
            lock_aspect: true,
            about_center: false,
            axis: Axis::Free,
            snap_rotation: false,
        }
    }
}

fn center_px(frame: &PanCropFrame, canvas: Rect) -> Pos2 {
    pos2(
        canvas.left() + frame.x_center * canvas.width(),
        canvas.top() + frame.y_center * canvas.height(),
    )
}

fn half_sizes_px(frame: &PanCropFrame, canvas: Rect) -> (f32, f32) {
    (
        frame.width * canvas.width() / 2.0,
        frame.height * canvas.height() / 2.0,
    )
}

fn rotate(v: (f32, f32), degrees: f32) -> (f32, f32) {
    let (sin, cos) = degrees.to_radians().sin_cos();
    (v.0 * cos - v.1 * sin, v.0 * sin + v.1 * cos)
}

/// Position on the canvas of the point `(lx, ly)` in the box's own axes (pixels from its centre).
fn to_canvas(frame: &PanCropFrame, canvas: Rect, local: (f32, f32)) -> Pos2 {
    let c = center_px(frame, canvas);
    let r = rotate(local, frame.rotation_deg);
    pos2(c.x + r.0, c.y + r.1)
}

/// The four corners of the box on the canvas, clockwise from the top-left one.
pub fn corners(frame: &PanCropFrame, canvas: Rect) -> [Pos2; 4] {
    let (hw, hh) = half_sizes_px(frame, canvas);
    [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)].map(|p| to_canvas(frame, canvas, p))
}

/// Position of resize handle `index` on the canvas.
pub fn handle_position(frame: &PanCropFrame, canvas: Rect, index: usize) -> Pos2 {
    let (hw, hh) = half_sizes_px(frame, canvas);
    let (dx, dy) = HANDLE_DIRS[index % 8];
    to_canvas(frame, canvas, (dx * hw, dy * hh))
}

/// Position of the rotation handle on the canvas.
pub fn rotate_handle_position(frame: &PanCropFrame, canvas: Rect) -> Pos2 {
    let (_, hh) = half_sizes_px(frame, canvas);
    to_canvas(frame, canvas, (0.0, -hh - ROTATE_OFFSET))
}

/// What is under `pos`: the rotation handle, a resize handle or the inside of the box.
pub fn hit(frame: &PanCropFrame, canvas: Rect, pos: Pos2) -> Option<Handle> {
    if rotate_handle_position(frame, canvas).distance(pos) <= GRAB_RADIUS {
        return Some(Handle::Rotate);
    }
    let nearest = (0..8)
        .map(|i| (i, handle_position(frame, canvas, i).distance(pos)))
        .filter(|(_, d)| *d <= GRAB_RADIUS)
        .min_by(|a, b| a.1.total_cmp(&b.1));
    if let Some((i, _)) = nearest {
        return Some(Handle::Resize(i));
    }
    // Inside: undo the rotation and compare with the half sizes
    let c = center_px(frame, canvas);
    let local = rotate((pos.x - c.x, pos.y - c.y), -frame.rotation_deg);
    let (hw, hh) = half_sizes_px(frame, canvas);
    (local.0.abs() <= hw && local.1.abs() <= hh).then_some(Handle::Move)
}

/// The box after dragging `handle` from `press` to `now`, starting from `start`.
pub fn drag(
    start: &PanCropFrame,
    handle: Handle,
    press: Pos2,
    now: Pos2,
    canvas: Rect,
    options: DragOptions,
) -> PanCropFrame {
    let start = start.sanitized();
    let mut out = start;
    match handle {
        Handle::Move => {
            let (mut dx, mut dy) = (
                (now.x - press.x) / canvas.width(),
                (now.y - press.y) / canvas.height(),
            );
            match options.axis {
                Axis::Free => {}
                Axis::XOnly => dy = 0.0,
                Axis::YOnly => dx = 0.0,
            }
            out.x_center += dx;
            out.y_center += dy;
        }
        Handle::Rotate => {
            let c = center_px(&start, canvas);
            let angle = |p: Pos2| (p.y - c.y).atan2(p.x - c.x).to_degrees();
            let mut rotation = start.rotation_deg + (angle(now) - angle(press));
            if options.snap_rotation {
                rotation = (rotation / 15.0).round() * 15.0;
            }
            out.rotation_deg = rotation;
        }
        Handle::Resize(index) => {
            let (dir_x, dir_y) = HANDLE_DIRS[index % 8];
            // The pointer's move in the box's own axes
            let d = rotate((now.x - press.x, now.y - press.y), -start.rotation_deg);
            let (w0, h0) = (start.width * canvas.width(), start.height * canvas.height());
            let k = if options.about_center { 2.0 } else { 1.0 };
            let (mut w, mut h) = (w0 + dir_x * d.0 * k, h0 + dir_y * d.1 * k);
            if options.lock_aspect {
                let (sx, sy) = (w / w0, h / h0);
                let s = if dir_x != 0.0 && dir_y != 0.0 {
                    if (sx - 1.0).abs() >= (sy - 1.0).abs() {
                        sx
                    } else {
                        sy
                    }
                } else if dir_x != 0.0 {
                    sx
                } else {
                    sy
                };
                w = w0 * s;
                h = h0 * s;
            } else {
                if dir_x == 0.0 {
                    w = w0;
                }
                if dir_y == 0.0 {
                    h = h0;
                }
            }
            let (w, h) = (w.max(1.0), h.max(1.0));
            out.width = w / canvas.width();
            out.height = h / canvas.height();
            if !options.about_center {
                // The opposite side stays where it was: the centre moves half of the growth
                let shift = (dir_x * (w - w0) / 2.0, dir_y * (h - h0) / 2.0);
                let r = rotate(shift, start.rotation_deg);
                out.x_center += r.0 / canvas.width();
                out.y_center += r.1 / canvas.height();
            }
        }
    }
    out.sanitized()
}

/// The Pan/Crop with `frame` put at the current time: the key there is changed, or one is added.
/// Without keys the first edit makes a still box for the whole event (a key at `0`).
pub fn with_frame(pan_crop: &PanCrop, time: f64, frame: PanCropFrame, tolerance: f64) -> PanCrop {
    let mut next = pan_crop.clone();
    let at = if next.keys.is_empty() { 0.0 } else { time };
    next.upsert_key(at, frame, tolerance);
    next
}

/// Height of the box for the given width so its shape on the screen is `target_aspect`
/// (width over height), for a source of `source_aspect`.
pub fn height_for_aspect(width: f32, source_aspect: f32, target_aspect: f32) -> f32 {
    if target_aspect <= 0.0 || !target_aspect.is_finite() {
        return width;
    }
    width * source_aspect / target_aspect
}

// ---- the window --------------------------------------------------------------------------------

/// What the window asks of the application.
#[derive(Debug, Clone, PartialEq)]
pub enum PanCropAction {
    /// Show this Pan/Crop in the preview now; the project is not changed.
    Draft(PanCrop),
    /// The gesture ended: make this the Pan/Crop of the event (one undo step).
    Commit(PanCrop),
    /// Move the cursor to this many seconds into the event.
    SeekTo(f64),
    /// The editing draft was dropped (the preview goes back to the project).
    EndDraft,
    Close,
}

/// What the editor changes: the Pan/Crop of an event or the motion of a video track.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditTarget {
    Event(TrackId, EventId),
    Track(TrackId),
}

/// A live Pan/Crop (or track motion) that the preview must show instead of the project's.
#[derive(Debug, Clone, PartialEq)]
pub struct PanCropDraft {
    pub target: EditTarget,
    pub pan_crop: PanCrop,
}

impl PanCropDraft {
    /// A number that changes whenever the draft does (part of the preview's frame key).
    pub fn stamp(draft: Option<&PanCropDraft>) -> u64 {
        use std::hash::{Hash, Hasher};
        let Some(d) = draft else {
            return 0;
        };
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (d.target, d.pan_crop.stretch_to_fill).hash(&mut h);
        for key in &d.pan_crop.keys {
            key.time.to_bits().hash(&mut h);
            key.smoothness.to_bits().hash(&mut h);
            for v in [
                key.frame.x_center,
                key.frame.y_center,
                key.frame.width,
                key.frame.height,
                key.frame.rotation_deg,
            ] {
                v.to_bits().hash(&mut h);
            }
        }
        h.finish() | 1
    }
}

/// What the window needs to know about what it edits.
pub struct PanCropView<'a> {
    /// Name of the event or of the track.
    pub title: &'a str,
    /// The Pan/Crop stored in the project.
    pub pan_crop: &'a PanCrop,
    /// Seconds from the start of the event to the cursor, clamped to the event.
    pub time: f64,
    pub duration: f64,
    pub fps: f64,
    /// Width over height of the source picture and of the project.
    pub source_aspect: f32,
    pub project_aspect: f32,
    /// The composite preview, for the side-by-side view.
    pub preview: Option<&'a TextureHandle>,
}

/// The picture under the box.
pub struct SourceImage {
    pub texture: TextureHandle,
    pub aspect: f32,
}

struct DragState {
    handle: Handle,
    press: Pos2,
    start: PanCropFrame,
}

pub struct PanCropWindow {
    pub open: bool,
    pub target: Option<EditTarget>,
    pub source: Option<SourceImage>,
    /// Key of the source frame already asked for or shown: media path and frame number.
    pub source_key: Option<(std::path::PathBuf, i64)>,
    pub lock_aspect: bool,
    pub about_center: bool,
    pub axis: Axis,
    pub show_preview: bool,
    draft: Option<PanCrop>,
    dragging: Option<DragState>,
}

impl Default for PanCropWindow {
    fn default() -> Self {
        Self {
            open: false,
            target: None,
            source: None,
            source_key: None,
            lock_aspect: true,
            about_center: false,
            axis: Axis::Free,
            show_preview: true,
            draft: None,
            dragging: None,
        }
    }
}

fn fit_rect(area: Rect, aspect: f32) -> Rect {
    let aspect = if aspect.is_finite() && aspect > 0.05 {
        aspect
    } else {
        16.0 / 9.0
    };
    let (mut w, mut h) = (area.width(), area.width() / aspect);
    if h > area.height() {
        h = area.height();
        w = h * aspect;
    }
    Rect::from_center_size(area.center(), vec2(w.max(8.0), h.max(8.0)))
}

impl PanCropWindow {
    /// Opens the editor on an event; the source picture is asked for again.
    pub fn open_event(&mut self, track_id: TrackId, event_id: EventId) {
        self.open_target(EditTarget::Event(track_id, event_id));
    }

    /// Opens the editor on the motion of a video track.
    pub fn open_track(&mut self, track_id: TrackId) {
        self.open_target(EditTarget::Track(track_id));
    }

    fn open_target(&mut self, target: EditTarget) {
        if self.target != Some(target) {
            self.source = None;
            self.source_key = None;
            self.draft = None;
            self.dragging = None;
        }
        self.target = Some(target);
        self.open = true;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.draft = None;
        self.dragging = None;
    }

    /// Installs the decoded source picture.
    pub fn set_source(&mut self, ctx: &egui::Context, frame: &neoveg_video::FrameBuffer) {
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [frame.width as usize, frame.height as usize],
            &frame.rgba,
        );
        let texture = ctx.load_texture("pan_crop_source", image, egui::TextureOptions::LINEAR);
        self.source = Some(SourceImage {
            texture,
            aspect: frame.width as f32 / frame.height.max(1) as f32,
        });
    }

    /// Draws the window and returns what the user did.
    pub fn show(&mut self, ctx: &egui::Context, view: &PanCropView) -> Vec<PanCropAction> {
        let mut actions = Vec::new();
        if !self.open {
            return actions;
        }
        let mut open = true;
        let committed = view.pan_crop.clone();
        let working = self.draft.clone().unwrap_or_else(|| committed.clone());
        let tolerance = (0.5 / view.fps.max(1.0)).max(KEY_TOLERANCE);
        let current = working.frame_at(view.time);
        let mut next: Option<(PanCrop, bool)> = None; // (new value, gesture finished)

        let track_mode = matches!(self.target, Some(EditTarget::Track(_)));
        let window_title = if track_mode {
            "Movimento da trilha"
        } else {
            "Pan/Crop do evento"
        };
        crate::dialogs::dialog(ctx, window_title)
            .open(&mut open)
            .default_size([1000.0, 580.0])
            .resizable(true)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.strong(view.title);
                    ui.weak(format!("· {:.2} s de {:.2} s", view.time, view.duration));
                    ui.separator();
                    if ui.button(tr("Restaurar")).on_hover_text(if track_mode { "A camada volta a ocupar o quadro inteiro, sem quadros-chave" } else { "Volta a mostrar a imagem inteira, sem quadros-chave" }).clicked() {
                        next = Some((PanCrop { stretch_to_fill: working.stretch_to_fill, ..PanCrop::default() }, true));
                    }
                    if ui.button(tr("Centralizar")).clicked() {
                        let f = PanCropFrame { x_center: 0.5, y_center: 0.5, ..current };
                        next = Some((with_frame(&working, view.time, f, tolerance), true));
                    }
                    if !track_mode && ui.button(tr("Proporção da origem")).on_hover_text(tr("A caixa passa a ter o formato da imagem de origem")).clicked() {
                        let f = PanCropFrame { height: current.width, ..current };
                        next = Some((with_frame(&working, view.time, f, tolerance), true));
                    }
                    if ui.button(tr("Proporção do projeto")).on_hover_text(if track_mode { "A caixa passa a ter o formato do projeto" } else { "A caixa passa a ter o formato do projeto: sem faixas pretas" }).clicked() {
                        let height = height_for_aspect(current.width, view.source_aspect, view.project_aspect);
                        let f = PanCropFrame { height, ..current };
                        next = Some((with_frame(&working, view.time, f, tolerance), true));
                    }
                    ui.separator();
                    ui.checkbox(&mut self.show_preview, tr("Prévia ao lado"));
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.lock_aspect, tr("Travar proporção"));
                    ui.checkbox(&mut self.about_center, tr("Redimensionar pelo centro"));
                    ui.label(tr("Mover:"));
                    ui.selectable_value(&mut self.axis, Axis::Free, tr("livre"));
                    ui.selectable_value(&mut self.axis, Axis::XOnly, "só em X");
                    ui.selectable_value(&mut self.axis, Axis::YOnly, "só em Y");
                    let mut stretch = working.stretch_to_fill;
                    if !track_mode
                        && ui
                        .checkbox(&mut stretch, "Esticar para preencher o quadro")
                        .on_hover_text(tr("Ligado: a caixa vira o quadro (zoom). Desligado: a imagem mantém o tamanho e o que fica fora da caixa é cortado."))
                        .changed()
                    {
                        let mut p = working.clone();
                        p.stretch_to_fill = stretch;
                        next = Some((p, true));
                    }
                });
                ui.separator();

                ui.horizontal_top(|ui| {
                    // numeric fields
                    ui.vertical(|ui| {
                        ui.set_width(210.0);
                        let mut f = current;
                        let mut changed = false;
                        let mut finished = false;
                        let mut field = |ui: &mut egui::Ui, label: &str, value: &mut f32, range: std::ops::RangeInclusive<f32>, suffix: &str, scale: f32| {
                            ui.horizontal(|ui| {
                                ui.add_sized([86.0, 18.0], egui::Label::new(label));
                                let mut shown = *value * scale;
                                let r = ui.add(
                                    egui::DragValue::new(&mut shown)
                                        .speed(0.2)
                                        .range(range.start() * scale..=range.end() * scale)
                                        .suffix(suffix),
                                );
                                if r.changed() {
                                    *value = shown / scale;
                                    changed = true;
                                }
                                if r.drag_stopped() || r.lost_focus() {
                                    finished = true;
                                }
                            });
                        };
                        field(ui, "Centro X", &mut f.x_center, -1.0..=2.0, " %", 100.0);
                        field(ui, "Centro Y", &mut f.y_center, -1.0..=2.0, " %", 100.0);
                        field(ui, "Largura", &mut f.width, 0.02..=4.0, " %", 100.0);
                        field(ui, "Altura", &mut f.height, 0.02..=4.0, " %", 100.0);
                        field(ui, "Rotação", &mut f.rotation_deg, -180.0..=180.0, " °", 1.0);
                        // The last frame of a drag has no change, only the end of it: commit the draft
                        if changed || (finished && self.draft.is_some()) {
                            next = Some((with_frame(&working, view.time, f, tolerance), finished));
                        }
                        ui.separator();
                        if let Some(i) = working.key_near(view.time, tolerance) {
                            let mut smooth = working.keys[i].smoothness;
                            ui.horizontal(|ui| {
                                ui.add_sized([86.0, 18.0], egui::Label::new(tr("Suavidade")));
                                let r = ui.add(egui::Slider::new(&mut smooth, 0.0..=1.0).show_value(true));
                                if r.changed() {
                                    let mut p = working.clone();
                                    p.keys[i].smoothness = smooth;
                                    next = Some((p, false));
                                }
                                if r.drag_stopped() || r.lost_focus() {
                                    let mut p = working.clone();
                                    p.keys[i].smoothness = smooth;
                                    next = Some((p, true));
                                }
                            });
                        } else {
                            ui.weak(tr("Sem quadro-chave neste instante: a caixa é interpolada."));
                        }
                    });

                    // the canvas
                    let right = if self.show_preview && view.preview.is_some() { 0.42 } else { 0.0 };
                    let avail = ui.available_size();
                    let canvas_area = ui.allocate_exact_size(
                        vec2((avail.x * (1.0 - right) - 10.0).max(120.0), (avail.y - 70.0).max(120.0)),
                        Sense::hover(),
                    ).0;
                    let aspect = self.source.as_ref().map_or(view.source_aspect, |s| s.aspect);
                    let canvas = fit_rect(canvas_area, aspect);
                    self.paint_canvas(ui, canvas, &current, working.stretch_to_fill, track_mode);
                    self.handle_pointer(ui, canvas, &current, view, &working, tolerance, &mut next);

                    if right > 0.0 {
                        if let Some(texture) = view.preview {
                            let area = ui.available_rect_before_wrap();
                            let rect = fit_rect(area.shrink(4.0), view.project_aspect);
                            ui.painter().rect_filled(rect, 0.0, Color32::BLACK);
                            ui.painter().image(
                                texture.id(),
                                rect,
                                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                                Color32::WHITE,
                            );
                        }
                    }
                });

                ui.separator();
                self.key_strip(ui, view, &working, tolerance, &mut next, &mut actions);
            });

        if !open {
            actions.push(PanCropAction::Close);
        }
        if let Some((value, finished)) = next {
            if finished {
                self.draft = None;
                actions.push(PanCropAction::Commit(value));
                actions.push(PanCropAction::EndDraft);
            } else {
                self.draft = Some(value.clone());
                actions.push(PanCropAction::Draft(value));
            }
        }
        actions
    }

    fn paint_canvas(
        &self,
        ui: &egui::Ui,
        canvas: Rect,
        frame: &PanCropFrame,
        stretch: bool,
        track_mode: bool,
    ) {
        let painter = ui.painter_at(canvas.expand(40.0));
        painter.rect_filled(canvas, 0.0, Color32::from_gray(24));
        match &self.source {
            Some(source) => {
                painter.image(
                    source.texture.id(),
                    canvas,
                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            None => {
                if track_mode {
                    // The frame of the project, with the thirds, since a track has no single source
                    for i in 1..3 {
                        let f = i as f32 / 3.0;
                        let line = Stroke::new(1.0_f32, Color32::from_gray(48));
                        painter.line_segment(
                            [
                                pos2(canvas.left() + f * canvas.width(), canvas.top()),
                                pos2(canvas.left() + f * canvas.width(), canvas.bottom()),
                            ],
                            line,
                        );
                        painter.line_segment(
                            [
                                pos2(canvas.left(), canvas.top() + f * canvas.height()),
                                pos2(canvas.right(), canvas.top() + f * canvas.height()),
                            ],
                            line,
                        );
                    }
                }
                painter.text(
                    canvas.center(),
                    egui::Align2::CENTER_CENTER,
                    if track_mode {
                        "Quadro do projeto"
                    } else {
                        "Sem imagem de origem neste instante"
                    },
                    egui::FontId::proportional(13.0),
                    Color32::GRAY,
                );
            }
        }
        // Shade what the box leaves out (a rotated box is shown by its outline only)
        let corners = corners(frame, canvas);
        let accent = Color32::from_rgb(0xF1, 0xC4, 0x0F);
        if frame.rotation_deg.abs() < 0.01 && stretch {
            let (a, b) = (corners[0], corners[2]);
            let inner = Rect::from_min_max(a, b).intersect(canvas);
            let shade = Color32::from_black_alpha(130);
            painter.rect_filled(
                Rect::from_min_max(canvas.min, pos2(canvas.right(), inner.top())),
                0.0,
                shade,
            );
            painter.rect_filled(
                Rect::from_min_max(pos2(canvas.left(), inner.bottom()), canvas.max),
                0.0,
                shade,
            );
            painter.rect_filled(
                Rect::from_min_max(
                    pos2(canvas.left(), inner.top()),
                    pos2(inner.left(), inner.bottom()),
                ),
                0.0,
                shade,
            );
            painter.rect_filled(
                Rect::from_min_max(
                    pos2(inner.right(), inner.top()),
                    pos2(canvas.right(), inner.bottom()),
                ),
                0.0,
                shade,
            );
        }
        painter.add(egui::Shape::closed_line(
            corners.to_vec(),
            Stroke::new(2.0_f32, accent),
        ));
        for i in 0..8 {
            let p = handle_position(frame, canvas, i);
            painter.rect_filled(Rect::from_center_size(p, vec2(8.0, 8.0)), 0.0, accent);
        }
        let top = handle_position(frame, canvas, 1);
        let knob = rotate_handle_position(frame, canvas);
        painter.line_segment([top, knob], Stroke::new(1.0_f32, accent));
        painter.circle_filled(knob, 5.0, accent);
        let c = center_px(frame, canvas);
        painter.line_segment(
            [c - vec2(6.0, 0.0), c + vec2(6.0, 0.0)],
            Stroke::new(1.0_f32, accent),
        );
        painter.line_segment(
            [c - vec2(0.0, 6.0), c + vec2(0.0, 6.0)],
            Stroke::new(1.0_f32, accent),
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_pointer(
        &mut self,
        ui: &mut egui::Ui,
        canvas: Rect,
        current: &PanCropFrame,
        view: &PanCropView,
        working: &PanCrop,
        tolerance: f64,
        next: &mut Option<(PanCrop, bool)>,
    ) {
        let response = ui.interact(
            canvas.expand(40.0),
            ui.id().with("pan_crop_canvas"),
            Sense::click_and_drag(),
        );
        let options = DragOptions {
            lock_aspect: self.lock_aspect,
            about_center: self.about_center,
            axis: self.axis,
            snap_rotation: ui.input(|i| i.modifiers.shift),
        };
        if let Some(pos) = response.hover_pos() {
            if let Some(handle) = hit(current, canvas, pos) {
                ui.output_mut(|o| {
                    o.cursor_icon = match handle {
                        Handle::Move => egui::CursorIcon::Grab,
                        Handle::Rotate => egui::CursorIcon::Alias,
                        Handle::Resize(i) => match HANDLE_DIRS[i] {
                            (0.0, _) => egui::CursorIcon::ResizeVertical,
                            (_, 0.0) => egui::CursorIcon::ResizeHorizontal,
                            (a, b) if a * b > 0.0 => egui::CursorIcon::ResizeNwSe,
                            _ => egui::CursorIcon::ResizeNeSw,
                        },
                    }
                });
            }
        }
        if response.drag_started() {
            if let Some(press) = ui.input(|i| i.pointer.press_origin()) {
                if let Some(handle) = hit(current, canvas, press) {
                    self.dragging = Some(DragState {
                        handle,
                        press,
                        start: *current,
                    });
                }
            }
        }
        if let (Some(d), Some(now)) = (&self.dragging, response.interact_pointer_pos()) {
            let frame = drag(&d.start, d.handle, d.press, now, canvas, options);
            let finished = response.drag_stopped();
            *next = Some((with_frame(working, view.time, frame, tolerance), finished));
        }
        if response.drag_stopped() {
            self.dragging = None;
            // A release with no pointer position still ends the gesture with what was drafted
            if next.is_none() {
                if let Some(draft) = &self.draft {
                    *next = Some((draft.clone(), true));
                }
            }
        }
    }

    fn key_strip(
        &mut self,
        ui: &mut egui::Ui,
        view: &PanCropView,
        working: &PanCrop,
        tolerance: f64,
        next: &mut Option<(PanCrop, bool)>,
        actions: &mut Vec<PanCropAction>,
    ) {
        ui.horizontal(|ui| {
            let here = working.key_near(view.time, tolerance);
            if ui
                .button("◀")
                .on_hover_text(tr("Quadro-chave anterior"))
                .clicked()
            {
                if let Some(k) = working
                    .keys
                    .iter()
                    .rev()
                    .find(|k| k.time < view.time - tolerance)
                {
                    actions.push(PanCropAction::SeekTo(k.time));
                }
            }
            if ui
                .button("▶")
                .on_hover_text(tr("Próximo quadro-chave"))
                .clicked()
            {
                if let Some(k) = working.keys.iter().find(|k| k.time > view.time + tolerance) {
                    actions.push(PanCropAction::SeekTo(k.time));
                }
            }
            if ui
                .add_enabled(
                    here.is_none(),
                    egui::Button::new(tr("Adicionar quadro-chave no cursor")),
                )
                .clicked()
            {
                let frame = working.frame_at(view.time);
                let mut p = working.clone();
                p.upsert_key(
                    if p.keys.is_empty() { 0.0 } else { view.time },
                    frame,
                    tolerance,
                );
                *next = Some((p, true));
            }
            if ui
                .add_enabled(
                    here.is_some(),
                    egui::Button::new(tr("Remover quadro-chave")),
                )
                .clicked()
            {
                if let Some(i) = here {
                    let mut p = working.clone();
                    p.remove_key(i);
                    *next = Some((p, true));
                }
            }
            ui.weak(tr_args("{} quadro(s)-chave", &[&working.keys.len()]));
        });
        // The strip: the whole event, a diamond per key, a line at the cursor
        let (rect, _) =
            ui.allocate_exact_size(vec2(ui.available_width().max(100.0), 22.0), Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::from_gray(30));
        let x_of = |t: f64| {
            rect.left() + (t / view.duration.max(1e-6)).clamp(0.0, 1.0) as f32 * rect.width()
        };
        for key in &working.keys {
            let c = pos2(x_of(key.time), rect.center().y);
            painter.add(egui::Shape::convex_polygon(
                vec![
                    c + vec2(0.0, -6.0),
                    c + vec2(6.0, 0.0),
                    c + vec2(0.0, 6.0),
                    c + vec2(-6.0, 0.0),
                ],
                Color32::from_rgb(0xF1, 0xC4, 0x0F),
                Stroke::NONE,
            ));
        }
        let cursor = x_of(view.time);
        painter.line_segment(
            [pos2(cursor, rect.top()), pos2(cursor, rect.bottom())],
            Stroke::new(1.0_f32, Color32::from_rgb(0x29, 0x80, 0xB9)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canvas() -> Rect {
        Rect::from_min_size(pos2(100.0, 50.0), vec2(400.0, 200.0))
    }

    fn boxed(x: f32, y: f32, w: f32, h: f32, rot: f32) -> PanCropFrame {
        PanCropFrame {
            x_center: x,
            y_center: y,
            width: w,
            height: h,
            rotation_deg: rot,
        }
    }

    #[test]
    fn the_handles_sit_on_the_corners_and_edges_of_an_unrotated_box() {
        let f = boxed(0.5, 0.5, 0.5, 0.5, 0.0);
        // The box covers x 200..400 and y 100..200 on this canvas
        assert_eq!(handle_position(&f, canvas(), 0), pos2(200.0, 100.0));
        assert_eq!(handle_position(&f, canvas(), 4), pos2(400.0, 200.0));
        assert_eq!(handle_position(&f, canvas(), 1), pos2(300.0, 100.0));
        assert_eq!(handle_position(&f, canvas(), 7), pos2(200.0, 150.0));
        let knob = rotate_handle_position(&f, canvas());
        assert_eq!(knob, pos2(300.0, 100.0 - ROTATE_OFFSET));
        // A quarter turn clockwise moves the top handle to the right of the centre
        let turned = boxed(0.5, 0.5, 0.5, 0.5, 90.0);
        let top = handle_position(&turned, canvas(), 1);
        assert!(
            (top.x - 350.0).abs() < 1e-3 && (top.y - 150.0).abs() < 1e-3,
            "{top:?}"
        );
    }

    #[test]
    fn the_pointer_finds_the_handle_the_inside_or_nothing() {
        let f = boxed(0.5, 0.5, 0.5, 0.5, 0.0);
        assert_eq!(hit(&f, canvas(), pos2(301.0, 151.0)), Some(Handle::Move));
        assert_eq!(
            hit(&f, canvas(), pos2(204.0, 103.0)),
            Some(Handle::Resize(0))
        );
        assert_eq!(hit(&f, canvas(), pos2(300.0, 70.0)), Some(Handle::Rotate));
        assert_eq!(hit(&f, canvas(), pos2(120.0, 60.0)), None);
        // Inside a rotated box means inside its rotated outline
        let turned = boxed(0.5, 0.5, 0.8, 0.1, 90.0);
        assert_eq!(
            hit(&turned, canvas(), pos2(300.0, 70.0 + 14.0)),
            Some(Handle::Move)
        );
        assert_eq!(hit(&turned, canvas(), pos2(380.0, 150.0)), None);
    }

    #[test]
    fn moving_follows_the_pointer_and_the_axis_lock_holds_one_axis() {
        let f = boxed(0.5, 0.5, 0.5, 0.5, 0.0);
        let (press, now) = (pos2(300.0, 150.0), pos2(340.0, 170.0));
        let free = drag(
            &f,
            Handle::Move,
            press,
            now,
            canvas(),
            DragOptions::default(),
        );
        assert!((free.x_center - 0.6).abs() < 1e-5 && (free.y_center - 0.6).abs() < 1e-5);
        let x_only = drag(
            &f,
            Handle::Move,
            press,
            now,
            canvas(),
            DragOptions {
                axis: Axis::XOnly,
                ..DragOptions::default()
            },
        );
        assert!((x_only.x_center - 0.6).abs() < 1e-5 && x_only.y_center == 0.5);
        let y_only = drag(
            &f,
            Handle::Move,
            press,
            now,
            canvas(),
            DragOptions {
                axis: Axis::YOnly,
                ..DragOptions::default()
            },
        );
        assert!(y_only.x_center == 0.5 && (y_only.y_center - 0.6).abs() < 1e-5);
    }

    #[test]
    fn resizing_keeps_the_opposite_side_or_the_centre_and_can_lock_the_proportion() {
        let f = boxed(0.5, 0.5, 0.5, 0.5, 0.0); // x 200..400, y 100..200
        let free = DragOptions {
            lock_aspect: false,
            ..DragOptions::default()
        };
        // Right edge 40 px to the right: wider by 40 px, the left side stays at 200
        let r = drag(
            &f,
            Handle::Resize(3),
            pos2(400.0, 150.0),
            pos2(440.0, 150.0),
            canvas(),
            free,
        );
        assert!((r.width - 0.6).abs() < 1e-5 && r.height == 0.5);
        assert!(
            (r.x_center - 0.55).abs() < 1e-5,
            "the left side stays: {r:?}"
        );
        // About the centre the same drag widens both sides
        let c = drag(
            &f,
            Handle::Resize(3),
            pos2(400.0, 150.0),
            pos2(440.0, 150.0),
            canvas(),
            DragOptions {
                about_center: true,
                ..free
            },
        );
        assert!((c.width - 0.7).abs() < 1e-5 && c.x_center == 0.5);
        // A locked corner drag keeps the box's proportions on the screen
        let l = drag(
            &f,
            Handle::Resize(4),
            pos2(400.0, 200.0),
            pos2(440.0, 200.0),
            canvas(),
            DragOptions::default(),
        );
        let ratio_before = (f.width * 400.0) / (f.height * 200.0);
        let ratio_after = (l.width * 400.0) / (l.height * 200.0);
        assert!(
            (ratio_before - ratio_after).abs() < 1e-4,
            "{ratio_before} vs {ratio_after}"
        );
        assert!(l.width > f.width);
        // The left edge dragged to the left grows the box and moves the centre left
        let g = drag(
            &f,
            Handle::Resize(7),
            pos2(200.0, 150.0),
            pos2(160.0, 150.0),
            canvas(),
            free,
        );
        assert!((g.width - 0.6).abs() < 1e-5 && (g.x_center - 0.45).abs() < 1e-5);
    }

    #[test]
    fn a_box_never_shrinks_below_the_minimum_or_to_a_bad_number() {
        let f = boxed(0.5, 0.5, 0.5, 0.5, 0.0);
        let tiny = drag(
            &f,
            Handle::Resize(3),
            pos2(400.0, 150.0),
            pos2(-5000.0, 150.0),
            canvas(),
            DragOptions {
                lock_aspect: false,
                ..DragOptions::default()
            },
        );
        assert!(tiny.width >= neoveg_common::pan_crop::MIN_SIZE);
        assert!(tiny.x_center.is_finite());
    }

    #[test]
    fn dragging_the_rotation_handle_turns_the_box_with_optional_15_degree_steps() {
        let f = boxed(0.5, 0.5, 0.5, 0.5, 0.0);
        let center = pos2(300.0, 150.0);
        // From straight above to straight right of the centre is a quarter turn clockwise
        let quarter = drag(
            &f,
            Handle::Rotate,
            pos2(300.0, 100.0),
            pos2(350.0, 150.0),
            canvas(),
            DragOptions::default(),
        );
        assert!(
            (quarter.rotation_deg - 90.0).abs() < 1e-3,
            "{}",
            quarter.rotation_deg
        );
        let _ = center;
        let near = drag(
            &f,
            Handle::Rotate,
            pos2(300.0, 100.0),
            pos2(340.0, 105.0),
            canvas(),
            DragOptions {
                snap_rotation: true,
                ..DragOptions::default()
            },
        );
        assert!((near.rotation_deg / 15.0 - (near.rotation_deg / 15.0).round()).abs() < 1e-4);
    }

    #[test]
    fn an_edit_changes_the_key_at_the_cursor_adds_one_elsewhere_and_a_first_edit_is_a_still_box() {
        let still = with_frame(
            &PanCrop::default(),
            3.0,
            boxed(0.3, 0.5, 0.5, 0.5, 0.0),
            0.02,
        );
        assert_eq!(still.keys.len(), 1);
        assert_eq!(
            still.keys[0].time, 0.0,
            "the first edit holds for the whole event"
        );
        let animated = with_frame(&still, 4.0, boxed(0.7, 0.5, 0.5, 0.5, 0.0), 0.02);
        assert_eq!(animated.keys.len(), 2);
        let changed = with_frame(&animated, 4.01, boxed(0.9, 0.5, 0.5, 0.5, 0.0), 0.02);
        assert_eq!(
            changed.keys.len(),
            2,
            "within the tolerance it is the same key"
        );
        assert_eq!(changed.keys[1].frame.x_center, 0.9);
    }

    #[test]
    fn matching_the_project_shape_keeps_the_picture_undistorted() {
        // A 4:3 source in a 16:9 project: a box 0.5 wide must be 0.375 high to show 16:9
        let h = height_for_aspect(0.5, 4.0 / 3.0, 16.0 / 9.0);
        assert!((h - 0.375).abs() < 1e-5);
        assert_eq!(height_for_aspect(0.5, 1.5, 0.0), 0.5);
    }

    #[test]
    fn the_draft_stamp_follows_every_number_of_the_pan_crop() {
        let d = |x: f32| PanCropDraft {
            target: EditTarget::Event(1, 2),
            pan_crop: with_frame(&PanCrop::default(), 0.0, boxed(x, 0.5, 0.5, 0.5, 0.0), 0.02),
        };
        assert_eq!(PanCropDraft::stamp(None), 0);
        assert_ne!(
            PanCropDraft::stamp(Some(&d(0.3))),
            PanCropDraft::stamp(Some(&d(0.31)))
        );
        assert_eq!(
            PanCropDraft::stamp(Some(&d(0.3))),
            PanCropDraft::stamp(Some(&d(0.3)))
        );
    }
}
