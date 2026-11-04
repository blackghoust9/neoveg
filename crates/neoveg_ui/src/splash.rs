//! Startup screen, drawn as in the design of the program (`layouts-neoveg.vectorcraft`): a rounded
//! card with a blue gradient holding the name, the version and the date of the last update on the
//! left and, on the right, the operating system, processor, graphics adapter, memory and user, then
//! a progress bar with the copyright line under it; the line that says what is being loaded sits
//! under the card.
//!
//! The progress is not a fake timer: the application runs its startup steps one per frame
//! (preferences, graphics, audio device, effects, video decoder, recovery files) and reports each
//! one here. Each step stays on screen for a short minimum time so it can be read, and any key or
//! click skips the pacing. The machine facts are collected on a worker thread, so the card never
//! waits for the system; until they arrive the lines read "Detectando…".
//!
//! The card holds no logo: the "Nv" mark is the icon of the program in the system (executable,
//! window and task bar), not a picture inside this screen.

use std::sync::{Arc, Mutex};

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Rect, Rounding};
use neoveg_common::i18n::tr_args;
use neoveg_common::sys_info::SystemInfo;

// ---- pacing ----------------------------------------------------------------------------------------

/// Delay before the first step starts, so the intro is seen from the beginning.
const INTRO_S: f64 = 0.55;
/// Shortest time a step stays current, so its label can be read.
const MIN_STEP_S: f64 = 0.30;
/// The screen never closes before this much time has passed.
const MIN_TOTAL_S: f64 = 2.8;
/// Time the finished bar stays full before the screen fades out.
const HOLD_S: f64 = 0.35;
const FADE_IN_S: f64 = 0.45;
const FADE_OUT_S: f64 = 0.55;
/// Fade out when skipped.
const SKIP_FADE_S: f64 = 0.20;

/// The state machine behind the progress bar. It knows nothing about egui: the clock is passed in,
/// so it can be tested step by step.
#[derive(Debug, Clone)]
pub struct Pacing {
    steps: usize,
    started: Option<f64>,
    current: usize,
    running: bool,
    work_done: bool,
    step_started: f64,
    skipped_at: Option<f64>,
    all_done: Option<f64>,
}

impl Pacing {
    pub fn new(steps: usize) -> Self {
        Self {
            steps,
            started: None,
            current: 0,
            running: false,
            work_done: false,
            step_started: 0.0,
            skipped_at: None,
            all_done: None,
        }
    }

    fn start(&mut self, now: f64) -> f64 {
        *self.started.get_or_insert(now)
    }

    /// Any key or click: the steps no longer wait for their minimum time and the screen closes
    /// quickly.
    pub fn skip(&mut self, now: f64) {
        self.skipped_at.get_or_insert(now);
    }

    pub fn skipped(&self) -> bool {
        self.skipped_at.is_some()
    }

    fn advance(&mut self, now: f64) {
        if self.running
            && self.work_done
            && (self.skipped() || now - self.step_started >= MIN_STEP_S)
        {
            self.running = false;
            self.work_done = false;
            self.current += 1;
            if self.current >= self.steps {
                self.all_done.get_or_insert(now);
            }
        }
    }

    /// The step the application must run now (once), or `None`.
    pub fn pending_step(&mut self, now: f64) -> Option<usize> {
        let started = self.start(now);
        self.advance(now);
        if self.running || self.current >= self.steps {
            return None;
        }
        if now - started < INTRO_S && !self.skipped() {
            return None;
        }
        self.running = true;
        self.work_done = false;
        self.step_started = now;
        Some(self.current)
    }

    /// The application finished the step it was given.
    pub fn complete(&mut self, now: f64) {
        self.work_done = true;
        self.advance(now);
    }

    /// Index of the step shown as current (the last one stays after the end).
    pub fn current(&self) -> usize {
        self.current.min(self.steps.saturating_sub(1))
    }

    /// Progress of the bar, `0..=1`: finished steps plus how far the current one has got.
    pub fn progress(&self, now: f64) -> f32 {
        if self.steps == 0 || self.current >= self.steps {
            return 1.0;
        }
        let within = if self.running {
            let time = ((now - self.step_started) / MIN_STEP_S).clamp(0.0, 1.0) as f32;
            // Never the whole step before the work is reported done
            time * if self.work_done { 1.0 } else { 0.6 }
        } else {
            0.0
        };
        ((self.current as f32 + within) / self.steps as f32).clamp(0.0, 1.0)
    }

    /// When the fade-out starts.
    fn closing_at(&self) -> Option<f64> {
        if let Some(skipped) = self.skipped_at {
            return Some(skipped);
        }
        let done = self.all_done?;
        let started = self.started?;
        Some((done + HOLD_S).max(started + MIN_TOTAL_S))
    }

    /// Opacity of the whole screen: fades in at the start, out at the end.
    pub fn opacity(&self, now: f64) -> f32 {
        let Some(started) = self.started else {
            return 0.0;
        };
        let fade_in = ((now - started) / FADE_IN_S).clamp(0.0, 1.0);
        let fade_out = match self.closing_at() {
            Some(at) => {
                let length = if self.skipped() {
                    SKIP_FADE_S
                } else {
                    FADE_OUT_S
                };
                1.0 - ((now - at) / length).clamp(0.0, 1.0)
            }
            None => 1.0,
        };
        (fade_in.min(fade_out)) as f32
    }

    /// `true` once the screen has faded out completely.
    pub fn finished(&self, now: f64) -> bool {
        match self.closing_at() {
            Some(at) => {
                let length = if self.skipped() {
                    SKIP_FADE_S
                } else {
                    FADE_OUT_S
                };
                now >= at + length
            }
            None => false,
        }
    }
}

// ---- the card ----------------------------------------------------------------------------------------
//
// The layout follows the design of the screen (`layouts-neoveg.vectorcraft`, group `splash`): every
// position and size below is in the units of that drawing, where the card is 1466 wide and 995 high,
// and is multiplied by the scale of the card on screen.

/// Version of the program (the workspace version).
const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Date of the build, `dd/mm/aaaa` (written by `build.rs`).
const UPDATED: &str = env!("NEOVEG_BUILD_DATE");
const COPYRIGHT: &str = "© 2026 Neoveg. Código aberto sob MIT OU Apache-2.0.";

const SCREEN_BG: Color32 = Color32::from_rgb(0x0B, 0x0F, 0x14);
const TEXT: Color32 = Color32::WHITE;
/// The track of the progress bar and the part of it that is done.
const BAR_TRACK: Color32 = Color32::from_rgb(0xC1, 0xC1, 0xC1);
const BAR_FILL: Color32 = Color32::from_rgb(0x34, 0x34, 0x34);

/// The drawing of the card: size, corner radius and the places of its parts.
const DESIGN_W: f32 = 1466.0;
const DESIGN_H: f32 = 995.0;
const DESIGN_RADIUS: f32 = 108.0;
const TITLE_AT: (f32, f32) = (75.0, 105.0);
const TITLE_SIZE: f32 = 163.5;
const VERSION_AT: (f32, f32) = (85.0, 326.0);
const UPDATED_AT: (f32, f32) = (85.0, 395.0);
const TEXT_SIZE: f32 = 46.7;
/// The right-hand list: operating system, processor, graphics, memory, user.
const LIST_X: f32 = 864.0;
const LIST_Y: [f32; 5] = [286.0, 353.0, 422.0, 491.0, 559.0];
/// The progress bar (track), the inset of its fill, and the line under the card that says what is
/// being loaded.
const BAR: (f32, f32, f32, f32) = (101.0, 895.0, 1282.0, 913.0);
const BAR_INSET: (f32, f32) = (6.0, 3.0);
const COPYRIGHT_Y: f32 = 927.0;
const COPYRIGHT_SIZE: f32 = 23.4;
const STATUS_AT: (f32, f32) = (107.0, 1067.0);
const STATUS_SIZE: f32 = 27.6;
/// Room the status line takes under the card, in design units.
const STATUS_ROOM: f32 = 150.0;

/// Widest the card is, in points, when the window is large enough.
const CARD_WIDTH: f32 = 800.0;

/// The card centred in `screen`, shrunk to fit a small window (the status line sits under it).
pub fn card_rect(screen: Rect) -> Rect {
    let ratio = DESIGN_H / DESIGN_W;
    let by_height = (screen.height() - 24.0) / (ratio + STATUS_ROOM / DESIGN_W);
    let width = CARD_WIDTH
        .min(screen.width() - 24.0)
        .min(by_height)
        .max(320.0);
    Rect::from_center_size(screen.center(), vec2(width, width * ratio))
}

/// The colour of the card at `t` (`0.0` left edge, `1.0` right edge): deep blue at the left, a
/// brighter blue from the middle on.
pub fn gradient_color(t: f32) -> Color32 {
    const FROM: [f32; 3] = [34.0, 34.0, 174.0];
    const TO: [f32; 3] = [44.0, 103.0, 255.0];
    const STOPS: (f32, f32) = (0.0657, 0.5009);
    let k = ((t - STOPS.0) / (STOPS.1 - STOPS.0)).clamp(0.0, 1.0);
    let channel = |i: usize| (FROM[i] + (TO[i] - FROM[i]) * k).round() as u8;
    Color32::from_rgb(channel(0), channel(1), channel(2))
}

/// Fills `rect` with the gradient, rounded at the corners by `radius`: thin vertical strips whose
/// height follows the corner curve.
pub fn paint_gradient(painter: &egui::Painter, rect: Rect, radius: f32, opacity: f32) {
    let radius = radius.min(rect.height() / 2.0).min(rect.width() / 2.0);
    let strip = 2.0;
    let count = (rect.width() / strip).ceil().max(1.0) as usize;
    for i in 0..count {
        let x0 = rect.left() + i as f32 * strip;
        let x1 = (x0 + strip + 0.5).min(rect.right());
        let middle = (x0 + x1) / 2.0;
        let edge = (middle - rect.left()).min(rect.right() - middle);
        let inset = if edge < radius {
            radius - (radius * radius - (radius - edge).powi(2)).max(0.0).sqrt()
        } else {
            0.0
        };
        let t = (middle - rect.left()) / rect.width();
        painter.rect_filled(
            Rect::from_min_max(
                pos2(x0, rect.top() + inset),
                pos2(x1, rect.bottom() - inset),
            ),
            Rounding::ZERO,
            fade(gradient_color(t), opacity),
        );
    }
}

/// The five lines of the right-hand list, in the order of the drawing: operating system, processor,
/// graphics adapter, memory and user. `info` is `None` while the system is still being asked.
pub fn info_lines(info: Option<&SystemInfo>, gpu: &str) -> [String; 5] {
    let waiting = || "Detectando…".to_owned();
    let gpu = if gpu.trim().is_empty() {
        "Desconhecido".to_owned()
    } else {
        gpu.trim().to_owned()
    };
    match info {
        Some(i) => [
            i.os.clone(),
            i.cpu.clone(),
            gpu,
            i.memory.clone(),
            i.user.clone(),
        ],
        None => [waiting(), waiting(), gpu, waiting(), waiting()],
    }
}

/// The line with the version of the program.
pub fn version_line() -> String {
    tr_args("Versão {}", &[&VERSION])
}

/// The line with the date of the last update (the build).
pub fn updated_line() -> String {
    tr_args("Atualizado em {}", &[&UPDATED])
}

pub struct Splash {
    pub pacing: Pacing,
    labels: Vec<String>,
    /// What the last finished step reported (shown under the card).
    detail: String,
    shown_progress: f32,
    /// The machine facts, filled in by a worker thread.
    system: Arc<Mutex<Option<SystemInfo>>>,
    /// The graphics adapter, known once the window exists.
    gpu: String,
}

impl Splash {
    pub fn new(labels: &[&str], gpu: &str) -> Self {
        let system = Arc::new(Mutex::new(None));
        let slot = Arc::clone(&system);
        // Asking the system for its processors and memory takes a moment: never on this thread
        std::thread::spawn(move || {
            let info = SystemInfo::collect();
            if let Ok(mut slot) = slot.lock() {
                *slot = Some(info);
            }
        });
        Self {
            pacing: Pacing::new(labels.len()),
            labels: labels.iter().map(|s| (*s).to_owned()).collect(),
            detail: String::new(),
            shown_progress: 0.0,
            system,
            gpu: gpu.to_owned(),
        }
    }

    /// The application ran the step it was given and says what it found.
    pub fn step_done(&mut self, now: f64, detail: String) {
        let label = self
            .labels
            .get(self.pacing.current())
            .map_or("", String::as_str);
        self.detail = format!("{label}: {detail}");
        self.pacing.complete(now);
    }

    /// Draws the screen; returns `true` once it has faded out and the application can show.
    pub fn show(&mut self, ctx: &egui::Context, now: f64) -> bool {
        if ctx.input(|i| {
            i.pointer.any_pressed()
                || i.events
                    .iter()
                    .any(|e| matches!(e, egui::Event::Key { pressed: true, .. }))
        }) {
            self.pacing.skip(now);
        }
        if self.pacing.finished(now) {
            return true;
        }
        let opacity = self.pacing.opacity(now);

        // A smooth bar
        let dt = ctx.input(|i| i.stable_dt).min(0.1);
        let target = self.pacing.progress(now);
        self.shown_progress += (target - self.shown_progress) * (1.0 - (-8.0 * dt).exp());

        let info = self.system.lock().ok().and_then(|g| g.clone());
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(fade(SCREEN_BG, opacity)))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let painter = ui.painter_at(rect);
                self.paint(&painter, rect, info.as_ref(), opacity);
            });
        ctx.request_repaint();
        false
    }

    fn step_label(&self) -> String {
        if self.detail.is_empty() {
            "Iniciando…".to_owned()
        } else {
            self.detail.clone()
        }
    }

    fn paint(
        &self,
        painter: &egui::Painter,
        screen: Rect,
        info: Option<&SystemInfo>,
        opacity: f32,
    ) {
        let card = card_rect(screen);
        let s = card.width() / DESIGN_W;
        let at = |(x, y): (f32, f32)| card.min + vec2(x, y) * s;
        let white = fade(TEXT, opacity);
        paint_gradient(painter, card, DESIGN_RADIUS * s, opacity);

        // Left: the name, the version and the date of the last update
        painter.text(
            at(TITLE_AT),
            Align2::LEFT_TOP,
            "NEOVEG",
            FontId::proportional(TITLE_SIZE * s),
            white,
        );
        for (place, text) in [(VERSION_AT, version_line()), (UPDATED_AT, updated_line())] {
            painter.text(
                at(place),
                Align2::LEFT_TOP,
                text,
                FontId::proportional(TEXT_SIZE * s),
                white,
            );
        }

        // Right: what the machine is
        let room = ((card.right() - at((LIST_X, 0.0)).x - 16.0) / (TEXT_SIZE * s * 0.5)) as usize;
        for (y, line) in LIST_Y.iter().zip(info_lines(info, &self.gpu)) {
            painter.text(
                at((LIST_X, *y)),
                Align2::LEFT_TOP,
                clip(&line, room),
                FontId::proportional(TEXT_SIZE * s),
                white,
            );
        }

        // The progress bar, with the copyright line under it
        let track = Rect::from_min_max(at((BAR.0, BAR.1)), at((BAR.2, BAR.3)));
        painter.rect_filled(
            track,
            Rounding::same(track.height() / 2.0),
            fade(BAR_TRACK, opacity),
        );
        let inner = Rect::from_min_max(
            track.min + vec2(BAR_INSET.0, BAR_INSET.1) * s,
            track.max - vec2(BAR_INSET.0, BAR_INSET.1) * s,
        );
        let filled = Rect::from_min_size(
            inner.min,
            vec2(
                inner.width() * self.shown_progress.clamp(0.0, 1.0),
                inner.height(),
            ),
        );
        painter.rect_filled(
            filled,
            Rounding::same(inner.height() / 2.0),
            fade(BAR_FILL, opacity),
        );
        painter.text(
            pos2(track.center().x, card.top() + COPYRIGHT_Y * s),
            Align2::CENTER_TOP,
            COPYRIGHT,
            FontId::proportional(COPYRIGHT_SIZE * s),
            white,
        );

        // What is being loaded sits under the card
        painter.text(
            at(STATUS_AT),
            Align2::LEFT_TOP,
            clip(
                &self.step_label(),
                (card.width() / (STATUS_SIZE * s * 0.5)) as usize,
            ),
            FontId::proportional(STATUS_SIZE * s),
            white,
        );
    }
}

/// The colour with its alpha scaled by `opacity` (the fade in and out of the whole screen).
fn fade(color: Color32, opacity: f32) -> Color32 {
    let a = (f32::from(color.a()) * opacity.clamp(0.0, 1.0)).round() as u8;
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), a)
}

/// `text` cut to `max` characters with an ellipsis (a long adapter name must not leave the card).
fn clip(text: &str, max: usize) -> String {
    let max = max.max(4);
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        let cut: String = text.chars().take(max - 1).collect();
        format!("{cut}…")
    }
}

#[cfg(test)]
mod card_tests {
    use super::*;

    fn info() -> SystemInfo {
        SystemInfo {
            os: "Windows 11 Pro 64-bit".into(),
            cpu: "Intel Core i9-13900K".into(),
            memory: "64 GB".into(),
            user: "ana".into(),
        }
    }

    #[test]
    fn the_right_hand_list_has_the_five_lines_of_the_design_in_order() {
        let lines = info_lines(Some(&info()), "NVIDIA GeForce RTX 4090 · Vulkan");
        assert_eq!(
            lines,
            [
                "Windows 11 Pro 64-bit",
                "Intel Core i9-13900K",
                "NVIDIA GeForce RTX 4090 · Vulkan",
                "64 GB",
                "ana"
            ]
        );
    }

    #[test]
    fn until_the_system_answers_the_lines_say_so_and_an_unknown_gpu_is_named() {
        let waiting = info_lines(None, "");
        assert_eq!(waiting[0], "Detectando…");
        assert_eq!(waiting[2], "Desconhecido", "no adapter reported");
        assert_eq!(waiting[3], "Detectando…");
        assert_eq!(waiting[4], "Detectando…");
    }

    #[test]
    fn the_card_keeps_the_proportions_of_the_design_is_centred_and_fits_small_windows() {
        let big = Rect::from_min_size(pos2(0.0, 0.0), vec2(1600.0, 900.0));
        let card = card_rect(big);
        assert_eq!(card.center(), big.center());
        assert_eq!(card.width(), CARD_WIDTH);
        assert!((card.width() / card.height() - DESIGN_W / DESIGN_H).abs() < 1e-3);
        let small = Rect::from_min_size(pos2(0.0, 0.0), vec2(500.0, 300.0));
        let shrunk = card_rect(small);
        assert!(
            small.contains_rect(shrunk),
            "the card stays inside a small window"
        );
        assert!((shrunk.width() / shrunk.height() - DESIGN_W / DESIGN_H).abs() < 1e-3);
        // The status line has room under the card in a window that is tall enough
        let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 700.0));
        let card = card_rect(screen);
        let status = card.top() + STATUS_AT.1 * (card.width() / DESIGN_W) + STATUS_SIZE;
        assert!(status < screen.bottom(), "{status}");
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn every_part_of_the_design_sits_inside_the_card_except_the_status_line() {
        for (x, y) in [TITLE_AT, VERSION_AT, UPDATED_AT] {
            assert!(x < DESIGN_W && y < DESIGN_H);
        }
        for y in LIST_Y {
            assert!(y + TEXT_SIZE < DESIGN_H);
        }
        assert!(BAR.2 < DESIGN_W && BAR.3 < DESIGN_H);
        assert!(COPYRIGHT_Y + COPYRIGHT_SIZE < DESIGN_H);
        assert!(STATUS_AT.1 > DESIGN_H, "under the card");
        // The list does not run into the left column
        assert!(LIST_X > VERSION_AT.0 + 500.0);
    }

    #[test]
    fn the_gradient_goes_from_deep_blue_to_a_brighter_blue_by_the_middle() {
        let left = gradient_color(0.0);
        assert_eq!((left.r(), left.g(), left.b()), (34, 34, 174));
        assert_eq!(gradient_color(0.0657), left, "flat up to the first stop");
        let right = gradient_color(1.0);
        assert_eq!((right.r(), right.g(), right.b()), (44, 103, 255));
        assert_eq!(
            gradient_color(0.5009),
            right,
            "flat from the second stop on"
        );
        // In between it only gets brighter
        let mut last = 0;
        for i in 0..=20 {
            let c = gradient_color(0.0657 + (0.5009 - 0.0657) * i as f32 / 20.0);
            let brightness = u32::from(c.r()) + u32::from(c.g()) + u32::from(c.b());
            assert!(brightness >= last);
            last = brightness;
        }
    }

    #[test]
    fn the_version_and_the_date_of_the_build_are_there() {
        assert!(!VERSION.is_empty() && VERSION.contains('.'));
        let parts: Vec<&str> = UPDATED.split('/').collect();
        assert_eq!(parts.len(), 3, "dd/mm/aaaa: {UPDATED}");
        assert_eq!(parts[2].len(), 4);
        assert!(version_line().contains(VERSION));
        assert!(updated_line().contains(UPDATED));
    }

    #[test]
    fn a_long_adapter_name_is_cut_with_an_ellipsis_and_a_short_one_is_kept() {
        assert_eq!(clip("Intel UHD", 20), "Intel UHD");
        let cut = clip("NVIDIA GeForce RTX 4090 Laptop GPU (Vulkan 1.3)", 20);
        assert_eq!(cut.chars().count(), 20);
        assert!(cut.ends_with('…'));
    }

    #[test]
    fn fading_scales_the_alpha_of_the_colour() {
        let c = fade(Color32::from_rgb(10, 20, 30), 0.5);
        // egui keeps colours premultiplied by alpha, so the channels can only get darker
        assert!(c.r() <= 10 && c.g() <= 20 && c.b() <= 30);
        assert!((i32::from(c.a()) - 128).abs() <= 1);
        assert_eq!(fade(Color32::from_rgb(1, 2, 3), 0.0).a(), 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs the pacing the way the application does, at 60 frames per second. `work` is how long
    /// each step takes to report done (0 = the same frame).
    fn run(pacing: &mut Pacing, frames: usize, skip_at: Option<f64>) -> (Vec<usize>, f64) {
        let mut ran = Vec::new();
        let mut finished_at = f64::NAN;
        for frame in 0..frames {
            let now = frame as f64 / 60.0;
            if skip_at.is_some_and(|s| now >= s) {
                pacing.skip(now);
            }
            if let Some(step) = pacing.pending_step(now) {
                ran.push(step);
                pacing.complete(now);
            }
            if pacing.finished(now) && finished_at.is_nan() {
                finished_at = now;
            }
        }
        (ran, finished_at)
    }

    #[test]
    fn every_step_runs_once_in_order_and_the_screen_ends() {
        let mut pacing = Pacing::new(6);
        let (ran, finished_at) = run(&mut pacing, 600, None);
        assert_eq!(ran, [0, 1, 2, 3, 4, 5]);
        assert!(finished_at.is_finite());
        assert!(
            finished_at >= MIN_TOTAL_S + FADE_OUT_S - 0.05,
            "{finished_at}"
        );
        assert!(finished_at < 6.0, "it must not drag on: {finished_at}");
    }

    #[test]
    fn a_step_stays_current_for_its_minimum_time() {
        let mut pacing = Pacing::new(3);
        let mut starts = Vec::new();
        for frame in 0..400 {
            let now = f64::from(frame) / 60.0;
            if let Some(step) = pacing.pending_step(now) {
                starts.push((step, now));
                pacing.complete(now);
            }
        }
        assert_eq!(starts.len(), 3);
        for pair in starts.windows(2) {
            assert!(pair[1].1 - pair[0].1 >= MIN_STEP_S - 1e-9, "{pair:?}");
        }
        assert!(starts[0].1 >= INTRO_S - 1e-9, "the intro plays first");
    }

    #[test]
    fn progress_only_goes_up_stays_in_range_and_waits_for_the_work() {
        let mut pacing = Pacing::new(4);
        let mut last = 0.0;
        for frame in 0..400 {
            let now = f64::from(frame) / 60.0;
            if let Some(_step) = pacing.pending_step(now) {
                // The work takes a while: the bar must not reach the end of the step yet
                assert!(pacing.progress(now) < 1.0);
            }
            if pacing.running && !pacing.work_done && now - pacing.step_started > 0.1 {
                pacing.complete(now);
            }
            let p = pacing.progress(now);
            assert!((0.0..=1.0).contains(&p));
            assert!(p >= last - 1e-6, "progress went back: {last} -> {p}");
            last = p;
        }
        assert_eq!(last, 1.0);
    }

    #[test]
    fn a_slow_step_holds_the_bar_below_its_end() {
        let mut pacing = Pacing::new(2);
        // The first call starts the clock; the intro has to pass before a step begins
        assert_eq!(pacing.pending_step(0.0), None);
        let started = pacing.pending_step(1.0).unwrap();
        assert_eq!(started, 0);
        // The step has not reported: even after a long time the bar is short of the first half
        assert!(pacing.progress(5.0) <= 0.3 + 1e-6);
        pacing.complete(5.0);
        assert!(pacing.progress(5.0) >= 0.5 - 1e-6);
    }

    #[test]
    fn skipping_runs_the_remaining_steps_at_once_and_closes_quickly() {
        let mut pacing = Pacing::new(6);
        let (ran, finished_at) = run(&mut pacing, 600, Some(0.1));
        assert_eq!(ran, [0, 1, 2, 3, 4, 5], "no step is lost by skipping");
        assert!(finished_at < 1.0, "{finished_at}");
        assert!(pacing.skipped());
    }

    #[test]
    fn the_screen_fades_in_stays_opaque_and_fades_out() {
        let mut pacing = Pacing::new(2);
        assert_eq!(pacing.opacity(0.0), 0.0, "not started yet");
        run(&mut pacing, 600, None);
        // Replay the same clock to read the opacity at known times
        let mut p = Pacing::new(2);
        let opacity_at = |p: &mut Pacing, now: f64| {
            if p.pending_step(now).is_some() {
                p.complete(now);
            }
            p.opacity(now)
        };
        assert!(opacity_at(&mut p, 0.0) < 0.05);
        assert!(opacity_at(&mut p, 0.2) < 1.0);
        assert!(opacity_at(&mut p, 1.0) > 0.99);
        for k in 1..400 {
            opacity_at(&mut p, 1.0 + f64::from(k) / 60.0);
        }
        assert_eq!(p.opacity(8.0), 0.0);
        assert!(p.finished(8.0));
    }

    #[test]
    fn a_pacing_without_steps_just_runs_the_clock() {
        let mut pacing = Pacing::new(0);
        assert_eq!(pacing.pending_step(0.0), None);
        assert_eq!(pacing.progress(0.0), 1.0);
    }
}
