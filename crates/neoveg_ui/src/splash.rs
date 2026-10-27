//! Startup screen: a rectangular card with the program's name, version and last update on the
//! left, what the machine is on the right (operating system, processor, memory, graphics adapter and
//! user), and a footer with the copyright line and a progress bar.
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

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Rect, Rounding, Stroke};
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

/// Version of the program (the workspace version).
const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Date of the build, `dd/mm/aaaa` (written by `build.rs`).
const UPDATED: &str = env!("NEOVEG_BUILD_DATE");
const COPYRIGHT: &str = "© 2026 Neoveg. Código aberto sob MIT OU Apache-2.0.";
const TAGLINE: &str = "Editor não linear de vídeo e áudio em Rust";

const SCREEN_BG: Color32 = Color32::from_rgb(0x0B, 0x0F, 0x14);
const CARD_BG: Color32 = Color32::from_rgb(0x12, 0x1A, 0x24);
const CARD_BORDER: Color32 = Color32::from_rgb(0x29, 0x80, 0xB9);
const TEXT: Color32 = Color32::from_rgb(0xE6, 0xED, 0xF3);
const LABEL: Color32 = Color32::from_rgb(0x7D, 0x8B, 0x99);
const ACCENT: Color32 = Color32::from_rgb(0x3E, 0xA6, 0xE0);

/// Size of the card, in points, when the window is large enough.
const CARD_SIZE: (f32, f32) = (800.0, 330.0);

/// The card centred in `screen`, shrunk to fit a small window.
pub fn card_rect(screen: Rect) -> Rect {
    let size = vec2(
        CARD_SIZE.0.min(screen.width() - 24.0).max(320.0),
        CARD_SIZE.1.min(screen.height() - 24.0).max(240.0),
    );
    Rect::from_center_size(screen.center(), size)
}

/// One labelled value of the right-hand list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub label: &'static str,
    pub value: String,
}

/// The sections of the right-hand list: the operating system, the machine (processor, memory and
/// graphics adapter) and the user. `info` is `None` while the system is still being asked.
pub fn sections(info: Option<&SystemInfo>, gpu: &str) -> Vec<(&'static str, Vec<Field>)> {
    let waiting = || "Detectando…".to_owned();
    let (os, cpu, memory, user) = match info {
        Some(i) => (
            i.os.clone(),
            i.cpu.clone(),
            i.memory.clone(),
            i.user.clone(),
        ),
        None => (waiting(), waiting(), waiting(), waiting()),
    };
    let gpu = if gpu.trim().is_empty() {
        "Desconhecido".to_owned()
    } else {
        gpu.trim().to_owned()
    };
    vec![
        (
            "SISTEMA OPERACIONAL",
            vec![Field {
                label: "",
                value: os,
            }],
        ),
        (
            "CONFIG. DA MÁQUINA",
            vec![
                Field {
                    label: "CPU",
                    value: cpu,
                },
                Field {
                    label: "MEM",
                    value: memory,
                },
                Field {
                    label: "GPU",
                    value: gpu,
                },
            ],
        ),
        (
            "USUÁRIO",
            vec![Field {
                label: "",
                value: user,
            }],
        ),
    ]
}

pub struct Splash {
    pub pacing: Pacing,
    labels: Vec<String>,
    /// What the last finished step reported (shown under the bar).
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
        painter.rect_filled(card, Rounding::ZERO, fade(CARD_BG, opacity));
        painter.rect_stroke(
            card,
            Rounding::ZERO,
            Stroke::new(1.0_f32, fade(CARD_BORDER, opacity)),
        );
        let pad = 28.0;
        let inner = card.shrink(pad);

        // Left: the name, the version and the date of the last update
        let left_w = (inner.width() * 0.36).min(280.0);
        painter.text(
            inner.left_top(),
            Align2::LEFT_TOP,
            "NEOVEG",
            FontId::proportional(46.0),
            fade(TEXT, opacity),
        );
        let mut y = inner.top() + 64.0;
        painter.text(
            pos2(inner.left(), y),
            Align2::LEFT_TOP,
            TAGLINE,
            FontId::proportional(11.0),
            fade(LABEL, opacity),
        );
        y += 34.0;
        for (label, value) in [("VERSÃO", VERSION), ("ÚLTIMA ATUALIZAÇÃO", UPDATED)] {
            painter.text(
                pos2(inner.left(), y),
                Align2::LEFT_TOP,
                label,
                FontId::proportional(10.0),
                fade(LABEL, opacity),
            );
            painter.text(
                pos2(inner.left(), y + 14.0),
                Align2::LEFT_TOP,
                value,
                FontId::proportional(16.0),
                fade(TEXT, opacity),
            );
            y += 48.0;
        }

        // A thin divider between the two halves
        let divider_x = inner.left() + left_w + 14.0;
        painter.line_segment(
            [
                pos2(divider_x, inner.top() + 4.0),
                pos2(divider_x, inner.bottom() - 70.0),
            ],
            Stroke::new(1.0_f32, fade(LABEL, opacity * 0.35)),
        );

        // Right: what the machine is
        let list_left = divider_x + 28.0;
        let mut y = inner.top() + 2.0;
        for (heading, fields) in sections(info, &self.gpu) {
            painter.text(
                pos2(list_left, y),
                Align2::LEFT_TOP,
                heading,
                FontId::proportional(10.0),
                fade(ACCENT, opacity),
            );
            y += 16.0;
            for field in fields {
                if !field.label.is_empty() {
                    painter.text(
                        pos2(list_left, y),
                        Align2::LEFT_TOP,
                        field.label,
                        FontId::monospace(12.0),
                        fade(LABEL, opacity),
                    );
                }
                let value_x = if field.label.is_empty() {
                    list_left
                } else {
                    list_left + 44.0
                };
                painter.text(
                    pos2(value_x, y),
                    Align2::LEFT_TOP,
                    clip(&field.value, ((inner.right() - value_x) / 7.2) as usize),
                    FontId::proportional(13.0),
                    fade(TEXT, opacity),
                );
                y += 19.0;
            }
            y += 12.0;
        }

        // Footer: the copyright line and the progress bar
        let footer_top = inner.bottom() - 58.0;
        painter.text(
            pos2(inner.left(), footer_top),
            Align2::LEFT_TOP,
            COPYRIGHT,
            FontId::proportional(10.0),
            fade(LABEL, opacity),
        );
        painter.text(
            pos2(inner.right(), footer_top),
            Align2::RIGHT_TOP,
            format!("{:.0}%", self.shown_progress * 100.0),
            FontId::monospace(10.0),
            fade(TEXT, opacity),
        );
        let bar = Rect::from_min_size(
            pos2(inner.left(), footer_top + 20.0),
            vec2(inner.width(), 6.0),
        );
        painter.rect_filled(
            bar,
            Rounding::ZERO,
            fade(Color32::from_rgb(0x24, 0x31, 0x40), opacity),
        );
        let filled = Rect::from_min_size(
            bar.min,
            vec2(
                bar.width() * self.shown_progress.clamp(0.0, 1.0),
                bar.height(),
            ),
        );
        painter.rect_filled(filled, Rounding::ZERO, fade(ACCENT, opacity));
        painter.text(
            pos2(inner.left(), bar.bottom() + 6.0),
            Align2::LEFT_TOP,
            clip(&self.step_label(), (inner.width() / 6.0) as usize),
            FontId::proportional(10.0),
            fade(LABEL, opacity),
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
    fn the_right_hand_list_has_the_sections_and_fields_of_the_model() {
        let list = sections(Some(&info()), "NVIDIA GeForce RTX 4090 · Vulkan");
        let headings: Vec<&str> = list.iter().map(|(h, _)| *h).collect();
        assert_eq!(
            headings,
            vec!["SISTEMA OPERACIONAL", "CONFIG. DA MÁQUINA", "USUÁRIO"]
        );
        assert_eq!(list[0].1[0].value, "Windows 11 Pro 64-bit");
        let machine: Vec<(&str, &str)> = list[1]
            .1
            .iter()
            .map(|f| (f.label, f.value.as_str()))
            .collect();
        assert_eq!(
            machine,
            vec![
                ("CPU", "Intel Core i9-13900K"),
                ("MEM", "64 GB"),
                ("GPU", "NVIDIA GeForce RTX 4090 · Vulkan")
            ]
        );
        assert_eq!(list[2].1[0].value, "ana");
    }

    #[test]
    fn until_the_system_answers_the_lines_say_so_and_an_unknown_gpu_is_named() {
        let waiting = sections(None, "");
        assert_eq!(waiting[0].1[0].value, "Detectando…");
        assert_eq!(waiting[1].1[2].value, "Desconhecido", "no adapter reported");
        assert_eq!(waiting[1].1[0].value, "Detectando…");
    }

    #[test]
    fn the_card_is_centred_fits_small_windows_and_holds_no_logo_text() {
        let big = Rect::from_min_size(pos2(0.0, 0.0), vec2(1600.0, 900.0));
        let card = card_rect(big);
        assert_eq!(card.center(), big.center());
        assert_eq!((card.width(), card.height()), CARD_SIZE);
        let small = Rect::from_min_size(pos2(0.0, 0.0), vec2(500.0, 300.0));
        let shrunk = card_rect(small);
        assert!(
            small.contains_rect(shrunk),
            "the card stays inside a small window"
        );
        // The mark is the program's icon in the system, not part of this screen
        let text = format!("{TAGLINE}{COPYRIGHT}");
        assert!(!text.contains("Nv"));
    }

    #[test]
    fn the_version_and_the_date_of_the_build_are_there() {
        assert!(!VERSION.is_empty() && VERSION.contains('.'));
        let parts: Vec<&str> = UPDATED.split('/').collect();
        assert_eq!(parts.len(), 3, "dd/mm/aaaa: {UPDATED}");
        assert_eq!(parts[2].len(), 4);
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
