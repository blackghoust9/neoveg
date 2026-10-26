//! Loudness panel: momentary, short-term and integrated loudness, loudness range and true peak
//! (ITU-R BS.1770-4 / EBU R128), measured on the samples that really go to the sound card.
//!
//! The numbers come from `neoveg_audio::readout`, fed by the audio callback; nothing here is
//! simulated. This is a separate panel on purpose: the Master Bus panel only holds the L / R peak
//! meters and the master fader.

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Rect, Rounding, Sense, Stroke};
use neoveg_audio::readout::LoudnessSnapshot;

use crate::icons::{self, Icon};

/// A delivery loudness target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Target {
    pub name: &'static str,
    /// Integrated loudness to reach, LUFS.
    pub lufs: f64,
    /// Highest allowed true peak, dBTP.
    pub true_peak_max: f64,
}

pub const TARGETS: [Target; 4] = [
    Target {
        name: "EBU R128 (-23 LUFS)",
        lufs: -23.0,
        true_peak_max: -1.0,
    },
    Target {
        name: "ATSC A/85 (-24 LKFS)",
        lufs: -24.0,
        true_peak_max: -2.0,
    },
    Target {
        name: "Streaming (-14 LUFS)",
        lufs: -14.0,
        true_peak_max: -1.0,
    },
    Target {
        name: "Podcast (-16 LUFS)",
        lufs: -16.0,
        true_peak_max: -1.0,
    },
];

/// Loudness at the bottom and at the top of the bar graph, LUFS.
const SCALE_MIN: f64 = -50.0;
const SCALE_MAX: f64 = 0.0;
/// How far the integrated loudness may be from the target and still count as on target, LU.
const TOLERANCE_LU: f64 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Nothing has been measured yet.
    NoSignal,
    OnTarget,
    /// Off by more than the tolerance but by less than 3 LU.
    Near,
    Off,
}

/// How the integrated loudness compares with `target`.
pub fn verdict(integrated_lufs: f64, target: Target) -> Verdict {
    if !integrated_lufs.is_finite() {
        return Verdict::NoSignal;
    }
    let off = (integrated_lufs - target.lufs).abs();
    if off <= TOLERANCE_LU {
        Verdict::OnTarget
    } else if off <= 3.0 {
        Verdict::Near
    } else {
        Verdict::Off
    }
}

/// `true` when the true peak is above the target's limit.
pub fn true_peak_over(true_peak_dbtp: f64, target: Target) -> bool {
    true_peak_dbtp.is_finite() && true_peak_dbtp > target.true_peak_max
}

/// What the panel shows: the window readings (momentary, short-term) are only meaningful while
/// audio is playing, so they read `-inf` when stopped; the programme readings (integrated, range,
/// true peak, maxima) stay until the next measurement.
pub fn displayed(snapshot: LoudnessSnapshot, playing: bool) -> LoudnessSnapshot {
    if playing {
        snapshot
    } else {
        LoudnessSnapshot {
            momentary_lufs: f64::NEG_INFINITY,
            short_term_lufs: f64::NEG_INFINITY,
            ..snapshot
        }
    }
}

pub fn format_lufs(value: f64) -> String {
    if value.is_finite() {
        format!("{value:.1}")
    } else {
        "-∞".into()
    }
}

/// Height of a bar, `0..=1`, for a loudness on the `-50..0` LUFS scale (silence is empty).
pub fn bar_fraction(lufs: f64) -> f32 {
    if !lufs.is_finite() {
        return 0.0;
    }
    ((lufs - SCALE_MIN) / (SCALE_MAX - SCALE_MIN)).clamp(0.0, 1.0) as f32
}

fn verdict_color(verdict: Verdict) -> Color32 {
    match verdict {
        Verdict::NoSignal => Color32::from_gray(140),
        Verdict::OnTarget => Color32::from_rgb(46, 204, 113),
        Verdict::Near => Color32::from_rgb(241, 196, 15),
        Verdict::Off => Color32::from_rgb(231, 76, 60),
    }
}

/// Draws the panel. Returns `true` when the user asked to restart the measurement.
pub fn loudness_ui(
    ui: &mut egui::Ui,
    snapshot: LoudnessSnapshot,
    playing: bool,
    target_index: &mut usize,
) -> bool {
    *target_index = (*target_index).min(TARGETS.len() - 1);
    let target = TARGETS[*target_index];
    let shown = displayed(snapshot, playing);
    let mut reset = false;

    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("loudness_target")
            .selected_text(target.name)
            .width(150.0)
            .show_ui(ui, |ui| {
                for (i, t) in TARGETS.iter().enumerate() {
                    ui.selectable_value(target_index, i, t.name);
                }
            });
        if icons::icon_button(
            ui,
            Icon::Reset,
            "Zerar a medição (ela também recomeça a cada reprodução)",
            false,
        )
        .clicked()
        {
            reset = true;
        }
    });

    // Bars: momentary and short-term, with the target line and the integrated loudness marker
    let bars_h = (ui.available_height() - 150.0).max(70.0);
    let (rect, _) = ui.allocate_exact_size(vec2(190.0, bars_h), Sense::hover());
    let painter = ui.painter_at(rect);
    let (top, bottom) = (rect.top() + 6.0, rect.bottom() - 6.0);
    let y_of = |lufs: f64| bottom - bar_fraction(lufs) * (bottom - top);

    for db in [0.0, -9.0, -18.0, -27.0, -36.0, -50.0] {
        painter.text(
            pos2(rect.left() + 30.0, y_of(db)),
            Align2::RIGHT_CENTER,
            format!("{db:.0}"),
            FontId::monospace(10.0),
            Color32::from_gray(150),
        );
    }
    let bars = [("M", shown.momentary_lufs), ("S", shown.short_term_lufs)];
    for (i, (label, value)) in bars.into_iter().enumerate() {
        let x = rect.left() + 40.0 + i as f32 * 44.0;
        let bar = Rect::from_min_max(pos2(x, top), pos2(x + 32.0, bottom));
        painter.rect_filled(bar, Rounding::ZERO, Color32::from_gray(14));
        if value.is_finite() {
            let color = if value > target.lufs + 6.0 {
                Color32::from_rgb(231, 76, 60)
            } else if value > target.lufs + 2.0 {
                Color32::from_rgb(241, 196, 15)
            } else {
                Color32::from_rgb(46, 204, 113)
            };
            painter.rect_filled(
                Rect::from_min_max(pos2(x, y_of(value)), bar.right_bottom()),
                Rounding::ZERO,
                color,
            );
        }
        painter.text(
            pos2(x + 16.0, bottom + 1.0),
            Align2::CENTER_TOP,
            label,
            FontId::monospace(10.0),
            Color32::from_gray(170),
        );
    }
    let bars_right = rect.left() + 40.0 + 44.0 + 32.0;
    // Target line across both bars
    let target_y = y_of(target.lufs);
    painter.line_segment(
        [
            pos2(rect.left() + 34.0, target_y),
            pos2(bars_right + 10.0, target_y),
        ],
        Stroke::new(1.5_f32, Color32::from_rgb(0x3A, 0x9B, 0xDC)),
    );
    painter.text(
        pos2(bars_right + 14.0, target_y),
        Align2::LEFT_CENTER,
        format!("{:.0}", target.lufs),
        FontId::monospace(10.0),
        Color32::from_rgb(0x3A, 0x9B, 0xDC),
    );
    // Integrated loudness marker
    if shown.integrated_lufs.is_finite() {
        let y = y_of(shown.integrated_lufs);
        painter.line_segment(
            [pos2(rect.left() + 34.0, y), pos2(bars_right + 10.0, y)],
            Stroke::new(1.0_f32, Color32::WHITE),
        );
        painter.text(
            pos2(bars_right + 14.0, y + 9.0),
            Align2::LEFT_CENTER,
            "I",
            FontId::monospace(10.0),
            Color32::WHITE,
        );
    }

    ui.monospace(format!(
        "M {:>6} LUFS   S {:>6} LUFS",
        format_lufs(shown.momentary_lufs),
        format_lufs(shown.short_term_lufs)
    ));
    let state = verdict(shown.integrated_lufs, target);
    ui.horizontal(|ui| {
        ui.monospace(format!("I {:>6} LUFS", format_lufs(shown.integrated_lufs)));
        if state != Verdict::NoSignal {
            let off = shown.integrated_lufs - target.lufs;
            ui.colored_label(verdict_color(state), format!("{off:+.1} LU"));
        }
    });
    ui.monospace(format!("LRA {:>5.1} LU", shown.range_lu));
    let over = true_peak_over(shown.true_peak_dbtp, target);
    let tp_color = if over {
        Color32::from_rgb(231, 76, 60)
    } else {
        ui.visuals().text_color()
    };
    ui.label(
        egui::RichText::new(format!(
            "TP {:>6} dBTP (máx. {:.0})",
            format_lufs(shown.true_peak_dbtp),
            target.true_peak_max
        ))
        .monospace()
        .color(tp_color),
    );
    ui.monospace(format!(
        "Máx. M {:>6}   Máx. S {:>6}",
        format_lufs(shown.max_momentary_lufs),
        format_lufs(shown.max_short_term_lufs)
    ));
    reset
}

#[cfg(test)]
mod tests {
    use super::*;

    const EBU: Target = TARGETS[0];

    fn snapshot() -> LoudnessSnapshot {
        LoudnessSnapshot {
            momentary_lufs: -20.0,
            short_term_lufs: -21.0,
            integrated_lufs: -23.4,
            range_lu: 6.0,
            true_peak_dbtp: -3.0,
            max_momentary_lufs: -15.0,
            max_short_term_lufs: -17.0,
        }
    }

    #[test]
    fn the_verdict_follows_the_tolerance_around_the_target() {
        assert_eq!(verdict(f64::NEG_INFINITY, EBU), Verdict::NoSignal);
        assert_eq!(verdict(-23.0, EBU), Verdict::OnTarget);
        assert_eq!(verdict(-22.0, EBU), Verdict::OnTarget);
        assert_eq!(verdict(-24.5, EBU), Verdict::Near);
        assert_eq!(verdict(-18.0, EBU), Verdict::Off);
        assert_eq!(verdict(-14.0, TARGETS[2]), Verdict::OnTarget);
    }

    #[test]
    fn the_true_peak_limit_is_per_target() {
        assert!(!true_peak_over(f64::NEG_INFINITY, EBU));
        assert!(!true_peak_over(-1.5, EBU));
        assert!(true_peak_over(-0.5, EBU));
        // ATSC allows less headroom
        assert!(true_peak_over(-1.5, TARGETS[1]));
    }

    #[test]
    fn stopped_audio_blanks_only_the_window_readings() {
        let live = displayed(snapshot(), true);
        assert_eq!(live, snapshot());

        let stopped = displayed(snapshot(), false);
        assert!(stopped.momentary_lufs.is_infinite() && stopped.short_term_lufs.is_infinite());
        assert_eq!(stopped.integrated_lufs, -23.4);
        assert_eq!(stopped.true_peak_dbtp, -3.0);
        assert_eq!(stopped.range_lu, 6.0);
    }

    #[test]
    fn bars_fill_between_the_scale_limits_and_silence_is_empty() {
        assert_eq!(bar_fraction(f64::NEG_INFINITY), 0.0);
        assert_eq!(bar_fraction(-60.0), 0.0);
        assert_eq!(bar_fraction(-50.0), 0.0);
        assert!((bar_fraction(-25.0) - 0.5).abs() < 1e-6);
        assert_eq!(bar_fraction(0.0), 1.0);
        assert_eq!(bar_fraction(6.0), 1.0);
    }

    #[test]
    fn numbers_have_one_decimal_and_silence_is_minus_infinity() {
        assert_eq!(format_lufs(-23.04), "-23.0");
        assert_eq!(format_lufs(f64::NEG_INFINITY), "-∞");
    }

    #[test]
    fn the_targets_are_distinct_and_sane() {
        for (i, t) in TARGETS.iter().enumerate() {
            assert!(t.lufs < 0.0 && t.true_peak_max < 0.0, "{}", t.name);
            assert!(TARGETS.iter().skip(i + 1).all(|o| o.name != t.name));
        }
    }
}
