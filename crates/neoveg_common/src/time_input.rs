//! Typed times for the boxes that hold a start, an end or a length (*Início*, *Fim*, *Duração*): a
//! value is absolute (`12.5`, `00:00:12:05`, `1:30`) or relative to what the box holds (`+10`, `-5`),
//! and a bare number is in the unit of the ruler (frames on a frame or timecode ruler, samples on the
//! sample ruler, seconds on a time ruler). A suffix says the unit: `s` seconds, `ms` milliseconds,
//! `f` frames.
//!
//! Everything here is a pure function.

use crate::ruler_format::{RulerContext, RulerFormat};
use crate::timecode::parse_timecode;

/// Seconds in one unit of the ruler: what a bare number means.
pub fn unit_seconds(format: RulerFormat, ctx: &RulerContext) -> f64 {
    let fps = ctx.fps.max(1.0);
    let rate = |frames: f64| 1.0 / frames;
    match format {
        RulerFormat::Samples => 1.0 / f64::from(ctx.sample_rate.max(1)),
        RulerFormat::ProjectTimecode | RulerFormat::TimeFrames | RulerFormat::AbsoluteFrames => {
            rate(fps)
        }
        RulerFormat::Smpte2398 => 1.001 / 24.0,
        RulerFormat::Smpte24 | RulerFormat::Feet16mm | RulerFormat::Feet35mm => rate(24.0),
        RulerFormat::Smpte25 => rate(25.0),
        RulerFormat::Smpte2997NonDrop | RulerFormat::Smpte2997Drop => 1.001 / 30.0,
        RulerFormat::Smpte30 => rate(30.0),
        RulerFormat::AudioCd => rate(75.0),
        RulerFormat::Time | RulerFormat::Seconds | RulerFormat::MeasuresBeats => 1.0,
    }
}

/// A number with an optional unit suffix, in seconds.
fn number(text: &str, bare: f64, fps: f64) -> Option<f64> {
    let (digits, unit) = if let Some(d) = text.strip_suffix("ms") {
        (d, 0.001)
    } else if let Some(d) = text.strip_suffix('s') {
        (d, 1.0)
    } else if let Some(d) = text.strip_suffix('f') {
        (d, 1.0 / fps)
    } else {
        (text, bare)
    };
    let digits = digits.trim();
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        return None;
    }
    let value: f64 = digits.parse().ok()?;
    value.is_finite().then_some(value * unit)
}

/// `h:mm:ss.fff`, `m:ss` or `ss` clock times, in seconds.
fn clock(text: &str) -> Option<f64> {
    let parts: Vec<&str> = text.split(':').collect();
    if parts.len() < 2 || parts.len() > 3 {
        return None;
    }
    let mut seconds = 0.0;
    for (i, part) in parts.iter().enumerate() {
        let last = i == parts.len() - 1;
        let ok = !part.is_empty()
            && part
                .bytes()
                .all(|b| b.is_ascii_digit() || (last && b == b'.'));
        if !ok {
            return None;
        }
        seconds = seconds * 60.0 + part.parse::<f64>().ok()?;
    }
    Some(seconds)
}

/// The time `text` stands for, in seconds of the project, or `None` when it is not a time. `current`
/// is what the box holds (a relative value moves it); an absolute value is read on the ruler, so the
/// offset of the ruler is taken off. The result is never below zero.
pub fn parse(text: &str, current: f64, format: RulerFormat, ctx: &RulerContext) -> Option<f64> {
    let text = text.trim().replace(',', ".");
    let (sign, body) = match text.as_bytes().first()? {
        b'+' => (Some(1.0), &text[1..]),
        b'-' => (Some(-1.0), &text[1..]),
        _ => (None, text.as_str()),
    };
    let body = body.trim();
    let fps = ctx.fps.max(1.0);
    // Timecode has four fields (`:` `;` or a last `.`), a clock two or three, a number none
    let seconds = if body.matches([':', ';']).count() == 3
        || (body.matches(':').count() == 2 && body.matches(';').count() == 1)
    {
        let (frame, _) = parse_timecode(body, fps)?;
        frame as f64 / fps
    } else if body.contains(':') {
        clock(body)?
    } else {
        number(body, unit_seconds(format, ctx), fps)?
    };
    let value = match sign {
        Some(s) => current + s * seconds,
        None => seconds - ctx.start,
    };
    value.is_finite().then_some(value.max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> RulerContext {
        RulerContext {
            fps: 25.0,
            drop_frame: false,
            sample_rate: 48_000,
            bpm: 120.0,
            beats_per_bar: 4,
            start: 0.0,
        }
    }

    fn p(text: &str, current: f64, format: RulerFormat) -> Option<f64> {
        parse(text, current, format, &ctx())
    }

    #[test]
    fn a_bare_number_is_in_the_unit_of_the_ruler() {
        assert_eq!(p("12.5", 0.0, RulerFormat::Seconds), Some(12.5));
        assert_eq!(
            p("12,5", 0.0, RulerFormat::Time),
            Some(12.5),
            "a comma is a point"
        );
        assert_eq!(
            p("50", 0.0, RulerFormat::AbsoluteFrames),
            Some(2.0),
            "50 frames at 25 fps"
        );
        assert_eq!(p("50", 0.0, RulerFormat::ProjectTimecode), Some(2.0));
        assert_eq!(p("48000", 0.0, RulerFormat::Samples), Some(1.0));
        assert_eq!(p("75", 0.0, RulerFormat::AudioCd), Some(1.0));
    }

    #[test]
    fn a_sign_makes_the_value_relative_to_what_the_box_holds() {
        assert_eq!(p("+10", 5.0, RulerFormat::Seconds), Some(15.0));
        assert_eq!(p("-5", 8.0, RulerFormat::Seconds), Some(3.0));
        assert_eq!(
            p("+25", 1.0, RulerFormat::AbsoluteFrames),
            Some(2.0),
            "frames of the ruler"
        );
        assert_eq!(
            p("-100", 3.0, RulerFormat::Seconds),
            Some(0.0),
            "never below zero"
        );
        assert_eq!(p("+00:00:01:00", 2.0, RulerFormat::Seconds), Some(3.0));
    }

    #[test]
    fn a_suffix_names_the_unit_whatever_the_ruler() {
        assert_eq!(p("2s", 0.0, RulerFormat::AbsoluteFrames), Some(2.0));
        assert_eq!(p("500ms", 0.0, RulerFormat::Seconds), Some(0.5));
        assert_eq!(p("50f", 0.0, RulerFormat::Seconds), Some(2.0));
        assert_eq!(p("+10f", 1.0, RulerFormat::Seconds), Some(1.4));
    }

    #[test]
    fn timecodes_and_clocks_are_absolute_times() {
        assert_eq!(p("00:00:12:05", 0.0, RulerFormat::Seconds), Some(12.2));
        assert_eq!(p("00:01:00;00", 0.0, RulerFormat::Seconds), Some(60.0));
        assert_eq!(p("1:30", 0.0, RulerFormat::AbsoluteFrames), Some(90.0));
        assert_eq!(p("0:01:30.5", 0.0, RulerFormat::Seconds), Some(90.5));
    }

    #[test]
    fn an_absolute_value_is_read_on_the_ruler_so_its_offset_comes_off() {
        let shifted = RulerContext {
            start: 3600.0,
            ..ctx()
        };
        assert_eq!(
            parse("3610", 0.0, RulerFormat::Seconds, &shifted),
            Some(10.0)
        );
        assert_eq!(
            parse("+10", 4.0, RulerFormat::Seconds, &shifted),
            Some(14.0)
        );
        assert_eq!(
            parse("5", 0.0, RulerFormat::Seconds, &shifted),
            Some(0.0),
            "before the origin"
        );
    }

    #[test]
    fn what_is_not_a_time_is_refused() {
        for bad in [
            "",
            "  ",
            "abc",
            "1:",
            ":5",
            "1:2:3:4:5",
            "5x",
            "--3",
            "1.2.3",
            "+",
            "1e3",
            "99:99:99:99",
        ] {
            assert_eq!(p(bad, 1.0, RulerFormat::Seconds), None, "{bad:?}");
        }
        assert_eq!(p("inf", 1.0, RulerFormat::Seconds), None);
    }
}
