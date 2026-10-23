//! SMPTE timecode math (non-drop and drop-frame), derived from integer frame indices only.

/// 29.97 and 59.94 fps are drop-frame by default.
pub fn is_drop_frame_rate(fps: f64) -> bool {
    (fps - 29.97).abs() < 0.06 || (fps - 59.94).abs() < 0.06
}

/// Nominal (rounded) frame rate used for the HH:MM:SS:FF fields.
pub fn nominal_fps(fps: f64) -> i64 {
    (fps.round() as i64).max(1)
}

/// Frame numbers skipped at each non-tenth minute (2 at 29.97, 4 at 59.94, 0 otherwise).
fn drop_per_minute(fps: f64, drop_frame: bool) -> i64 {
    match (drop_frame, nominal_fps(fps)) {
        (true, 30) => 2,
        (true, 60) => 4,
        _ => 0,
    }
}

/// Frame index to `HH:MM:SS:FF` (`HH:MM:SS;FF` for drop-frame).
pub fn frame_to_timecode(frame: i64, fps: f64, drop_frame: bool) -> String {
    let nominal = nominal_fps(fps);
    let drop = drop_per_minute(fps, drop_frame);
    let mut frame = frame.max(0);

    if drop > 0 {
        let frames_per_10_min = nominal * 600 - 9 * drop;
        let frames_per_minute = nominal * 60 - drop;
        let tens = frame / frames_per_10_min;
        let rem = frame % frames_per_10_min;
        frame += 9 * drop * tens;
        if rem > drop {
            frame += drop * ((rem - drop) / frames_per_minute);
        }
    }

    let ff = frame % nominal;
    let total_seconds = frame / nominal;
    let ss = total_seconds % 60;
    let mm = (total_seconds / 60) % 60;
    let hh = total_seconds / 3600;
    let sep = if drop > 0 { ';' } else { ':' };
    format!("{hh:02}:{mm:02}:{ss:02}{sep}{ff:02}")
}

/// Seconds to a timecode string at the given frame rate.
pub fn seconds_to_timecode(seconds: f64, fps: f64, drop_frame: bool) -> String {
    let frame = (seconds.max(0.0) * fps + 1e-6).floor() as i64;
    frame_to_timecode(frame, fps, drop_frame)
}

/// Frame number at which the timecode reads `timecode_seconds` with `;00` / `:00` frames.
/// At a dropped minute boundary the first frame of that second is `;02` (`;04` at 59.94).
pub fn timecode_seconds_to_frame(timecode_seconds: i64, fps: f64) -> i64 {
    let nominal = nominal_fps(fps);
    let mut frames = timecode_seconds * nominal;
    let drop = drop_per_minute(fps, is_drop_frame_rate(fps));
    if drop > 0 {
        let total_minutes = timecode_seconds / 60;
        frames -= drop * (total_minutes - total_minutes / 10);
        if timecode_seconds % 60 == 0 && total_minutes % 10 != 0 {
            frames += drop;
        }
    }
    frames
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_drop_formats_plainly() {
        assert_eq!(frame_to_timecode(0, 25.0, false), "00:00:00:00");
        assert_eq!(frame_to_timecode(25 * 3661 + 7, 25.0, false), "01:01:01:07");
    }

    #[test]
    fn drop_frame_skips_minute_boundaries() {
        assert_eq!(frame_to_timecode(1799, 29.97, true), "00:00:59;29");
        assert_eq!(frame_to_timecode(1800, 29.97, true), "00:01:00;02");
        // the tenth minute is not dropped
        assert_eq!(frame_to_timecode(17982, 29.97, true), "00:10:00;00");
    }

    #[test]
    fn timecode_second_lands_on_the_matching_frame() {
        for secs in [1_i64, 59, 60, 61, 120, 600, 3600] {
            let frame = timecode_seconds_to_frame(secs, 29.97);
            let tc = frame_to_timecode(frame, 29.97, true);
            let ss = secs % 60;
            assert!(tc.contains(&format!(":{ss:02};")), "{secs}s -> {tc}");
        }
        assert_eq!(timecode_seconds_to_frame(60, 29.97), 1800);
        assert_eq!(timecode_seconds_to_frame(60, 25.0), 1500);
    }
}
