//! How the ruler and the cursor read a time (manual v21, "Changing the format of the ruler"):
//! samples, time, seconds, frames, measures and beats, feet and frames, and the SMPTE and film
//! rates. Only the label changes: the ticks stay where they are on the frame grid, and the project
//! keeps its time in seconds.

use serde::{Deserialize, Serialize};

use crate::timecode::{frame_index, frame_to_timecode};

/// Ticks in a beat, for [`RulerFormat::MeasuresBeats`].
pub const TICKS_PER_BEAT: u32 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RulerFormat {
    /// SMPTE timecode at the frame rate of the project (drop-frame for 29.97).
    #[default]
    ProjectTimecode,
    Samples,
    /// `h:mm:ss.mmm`
    Time,
    Seconds,
    /// `hh:mm:ss.ff` with frames of the project.
    TimeFrames,
    AbsoluteFrames,
    /// `measure.beat.tick`, 64 ticks to a beat.
    MeasuresBeats,
    /// Feet and frames of 16 mm film (40 frames a foot, 24 frames a second).
    Feet16mm,
    /// Feet and frames of 35 mm film (16 frames a foot, 24 frames a second).
    Feet35mm,
    Smpte2398,
    Smpte24,
    /// EBU, 25 fps.
    Smpte25,
    Smpte2997NonDrop,
    Smpte2997Drop,
    Smpte30,
    /// Audio CD, 75 frames a second.
    AudioCd,
}

impl RulerFormat {
    pub const ALL: [RulerFormat; 16] = [
        Self::ProjectTimecode,
        Self::Samples,
        Self::Time,
        Self::Seconds,
        Self::TimeFrames,
        Self::AbsoluteFrames,
        Self::MeasuresBeats,
        Self::Feet16mm,
        Self::Feet35mm,
        Self::Smpte2398,
        Self::Smpte24,
        Self::Smpte25,
        Self::Smpte2997NonDrop,
        Self::Smpte2997Drop,
        Self::Smpte30,
        Self::AudioCd,
    ];

    /// Name in the menus (a key of the translation files).
    pub fn label(self) -> &'static str {
        match self {
            Self::ProjectTimecode => "Timecode do projeto",
            Self::Samples => "Amostras",
            Self::Time => "Tempo (h:m:s.ms)",
            Self::Seconds => "Segundos",
            Self::TimeFrames => "Tempo e quadros",
            Self::AbsoluteFrames => "Quadros absolutos",
            Self::MeasuresBeats => "Compassos e tempos",
            Self::Feet16mm => "Pés e quadros (16 mm)",
            Self::Feet35mm => "Pés e quadros (35 mm)",
            Self::Smpte2398 => "SMPTE Film Sync (23,976 fps)",
            Self::Smpte24 => "SMPTE Film Sync (24 fps)",
            Self::Smpte25 => "SMPTE EBU (25 fps)",
            Self::Smpte2997NonDrop => "SMPTE Non-Drop (29,97 fps)",
            Self::Smpte2997Drop => "SMPTE Drop Frame (29,97 fps)",
            Self::Smpte30 => "SMPTE (30 fps)",
            Self::AudioCd => "CD de áudio (75 fps)",
        }
    }

    /// The frame rate and drop-frame flag of the SMPTE and film formats; `None` for the others.
    fn fixed_rate(self) -> Option<(f64, bool)> {
        match self {
            Self::Smpte2398 => Some((24_000.0 / 1001.0, false)),
            Self::Smpte24 => Some((24.0, false)),
            Self::Smpte25 => Some((25.0, false)),
            Self::Smpte2997NonDrop => Some((30_000.0 / 1001.0, false)),
            Self::Smpte2997Drop => Some((30_000.0 / 1001.0, true)),
            Self::Smpte30 => Some((30.0, false)),
            Self::AudioCd => Some((75.0, false)),
            _ => None,
        }
    }
}

/// What a label needs to know about the project.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RulerContext {
    pub fps: f64,
    pub drop_frame: bool,
    pub sample_rate: u32,
    pub bpm: f64,
    pub beats_per_bar: u32,
    /// The time the ruler reads at the origin of the project, seconds.
    pub start: f64,
}

/// The time as the ruler reads it: the offset of the ruler added, never below zero.
pub fn shown_time(seconds: f64, start: f64) -> f64 {
    let seconds = if seconds.is_finite() { seconds } else { 0.0 };
    let start = if start.is_finite() { start } else { 0.0 };
    (seconds + start).max(0.0)
}

/// The label of `seconds` (a time of the project) in `format`.
pub fn format_time(seconds: f64, format: RulerFormat, ctx: &RulerContext) -> String {
    let t = shown_time(seconds, ctx.start);
    match format {
        RulerFormat::ProjectTimecode => {
            frame_to_timecode(frame_index(t, ctx.fps), ctx.fps, ctx.drop_frame)
        }
        RulerFormat::Samples => {
            format!("{}", (t * f64::from(ctx.sample_rate.max(1))).round() as u64)
        }
        RulerFormat::Time => {
            let ms = (t * 1000.0).round() as u64;
            format!(
                "{}:{:02}:{:02}.{:03}",
                ms / 3_600_000,
                ms / 60_000 % 60,
                ms / 1000 % 60,
                ms % 1000
            )
        }
        RulerFormat::Seconds => format!("{t:.3}"),
        RulerFormat::TimeFrames => {
            let frame = frame_index(t, ctx.fps);
            let nominal = (ctx.fps.round() as i64).max(1);
            let whole = frame / nominal;
            format!(
                "{:02}:{:02}:{:02}.{:02}",
                whole / 3600,
                whole / 60 % 60,
                whole % 60,
                frame % nominal
            )
        }
        RulerFormat::AbsoluteFrames => format!("{}", frame_index(t, ctx.fps)),
        RulerFormat::MeasuresBeats => {
            let beats = t * ctx.bpm.max(1.0) / 60.0;
            let whole = beats.floor() as u64;
            let per_bar = u64::from(ctx.beats_per_bar.max(1));
            let tick = ((beats - beats.floor()) * f64::from(TICKS_PER_BEAT)) as u32;
            format!(
                "{}.{}.{:02}",
                whole / per_bar + 1,
                whole % per_bar + 1,
                tick
            )
        }
        RulerFormat::Feet16mm | RulerFormat::Feet35mm => {
            let per_foot = if format == RulerFormat::Feet16mm {
                40
            } else {
                16
            };
            let frame = frame_index(t, 24.0);
            format!("{}+{:02}", frame / per_foot, frame % per_foot)
        }
        other => match other.fixed_rate() {
            Some((fps, drop)) => frame_to_timecode(frame_index(t, fps), fps, drop),
            None => String::new(),
        },
    }
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

    #[test]
    fn the_plain_formats_read_the_same_moment() {
        let c = ctx();
        let t = 3_723.5; // 1 h 2 min 3.5 s
        assert_eq!(
            format_time(t, RulerFormat::ProjectTimecode, &c),
            "01:02:03:12"
        );
        assert_eq!(format_time(t, RulerFormat::Time, &c), "1:02:03.500");
        assert_eq!(format_time(t, RulerFormat::Seconds, &c), "3723.500");
        assert_eq!(format_time(t, RulerFormat::Samples, &c), "178728000");
        assert_eq!(format_time(t, RulerFormat::TimeFrames, &c), "01:02:03.12");
        assert_eq!(format_time(t, RulerFormat::AbsoluteFrames, &c), "93087");
    }

    #[test]
    fn measures_and_beats_count_from_one_with_64_ticks_to_a_beat() {
        let c = ctx(); // 120 bpm: a beat is half a second, a bar two seconds
        assert_eq!(format_time(0.0, RulerFormat::MeasuresBeats, &c), "1.1.00");
        assert_eq!(format_time(0.5, RulerFormat::MeasuresBeats, &c), "1.2.00");
        assert_eq!(format_time(2.0, RulerFormat::MeasuresBeats, &c), "2.1.00");
        assert_eq!(format_time(2.25, RulerFormat::MeasuresBeats, &c), "2.1.32");
        let waltz = RulerContext {
            beats_per_bar: 3,
            ..c
        };
        assert_eq!(
            format_time(1.5, RulerFormat::MeasuresBeats, &waltz),
            "2.1.00"
        );
    }

    #[test]
    fn film_feet_use_24_frames_a_second_and_the_gauge_of_the_film() {
        let c = ctx();
        // One second is 24 frames: 0 feet + 24 in 16 mm (40 a foot), 1 foot + 8 in 35 mm (16 a foot)
        assert_eq!(format_time(1.0, RulerFormat::Feet16mm, &c), "0+24");
        assert_eq!(format_time(1.0, RulerFormat::Feet35mm, &c), "1+08");
        assert_eq!(format_time(10.0, RulerFormat::Feet35mm, &c), "15+00");
    }

    #[test]
    fn the_smpte_formats_use_their_own_rate_not_the_one_of_the_project() {
        let c = ctx();
        assert_eq!(format_time(1.5, RulerFormat::Smpte24, &c), "00:00:01:12");
        assert_eq!(format_time(1.5, RulerFormat::Smpte30, &c), "00:00:01:15");
        assert_eq!(format_time(1.5, RulerFormat::AudioCd, &c), "00:00:01:37");
        // Drop frame skips frame numbers 00 and 01 at the minute: frame 1800 reads 01:00;02
        let minute = 1800.0 * 1001.0 / 30_000.0;
        assert_eq!(
            format_time(minute, RulerFormat::Smpte2997Drop, &c),
            "00:01:00;02"
        );
        assert_eq!(
            format_time(minute, RulerFormat::Smpte2997NonDrop, &c),
            "00:01:00:00"
        );
    }

    #[test]
    fn the_start_time_moves_every_label_and_nothing_goes_below_zero() {
        let c = RulerContext {
            start: 10.0,
            ..ctx()
        };
        assert_eq!(format_time(0.0, RulerFormat::Seconds, &c), "10.000");
        let before = RulerContext {
            start: -5.0,
            ..ctx()
        };
        assert_eq!(format_time(2.0, RulerFormat::Seconds, &before), "0.000");
        assert_eq!(shown_time(f64::NAN, f64::INFINITY), 0.0);
    }

    #[test]
    fn every_format_has_a_name_and_a_reading() {
        let c = ctx();
        for format in RulerFormat::ALL {
            assert!(!format.label().is_empty());
            assert!(!format_time(1.0, format, &c).is_empty(), "{format:?}");
        }
        let json = serde_json::to_string(&RulerFormat::Feet35mm).unwrap();
        assert_eq!(
            serde_json::from_str::<RulerFormat>(&json).unwrap(),
            RulerFormat::Feet35mm
        );
    }
}
