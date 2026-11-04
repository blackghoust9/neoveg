//! Video metadata and pure helpers (no FFmpeg needed).

use std::path::Path;

/// Container extensions treated as video on import.
pub const VIDEO_EXTENSIONS: [&str; 15] = [
    "mp4", "mov", "mkv", "avi", "webm", "m4v", "mxf", "wmv", "mpg", "mpeg", "ts", "mts", "flv",
    "3gp", "ogv",
];

/// `true` when this build can decode container videos (the `ffmpeg` feature). Without it only
/// sound, pictures and generators import, and converting a video would not help.
pub const fn video_decoding_enabled() -> bool {
    cfg!(feature = "ffmpeg")
}

static HARDWARE_DECODING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// Whether videos opened from now on ask the graphics card to decode them (the preference). The
/// decoder falls back to the processor by itself when the card, the driver or the codec cannot.
pub fn hardware_decoding() -> bool {
    HARDWARE_DECODING.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn set_hardware_decoding(on: bool) {
    HARDWARE_DECODING.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub fn is_video_path(path: &Path) -> bool {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .is_some_and(|e| VIDEO_EXTENSIONS.contains(&e.as_str()))
}

/// True for any file that lands on a video track: container videos, still images, `.neovegseq`
/// image-sequence manifests, or `.neoveggen` procedural media generators.
pub fn is_visual_media_path(path: &Path) -> bool {
    is_video_path(path)
        || crate::still::is_image_path(path)
        || crate::still::is_sequence_manifest_path(path)
        || crate::generators::is_generator_manifest_path(path)
}

/// True for media whose source duration is unbounded when trimming the right edge on the timeline
/// (static images and procedural media generators).
pub fn is_unbounded_visual_path(path: &Path) -> bool {
    crate::still::is_image_path(path) || crate::generators::is_generator_manifest_path(path)
}

#[derive(Debug, Clone, PartialEq)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub duration: f64,
}

/// Output size for a frame scaled down to at most `max_width` (even dimensions, aspect kept).
/// `max_width == 0` or a larger value keeps the source size.
pub fn fit_size(width: u32, height: u32, max_width: u32) -> (u32, u32) {
    if max_width == 0 || width <= max_width || width == 0 {
        return (width.max(2) & !1, height.max(2) & !1);
    }
    let w = max_width.max(2) & !1;
    let h = ((height as u64 * w as u64 / width as u64) as u32).max(2) & !1;
    (w, h)
}

/// Nearest standard frame rate (so 29.97002997 becomes 29.97).
pub fn snap_fps(fps: f64) -> f64 {
    const STANDARD: [f64; 10] = [
        23.976, 24.0, 25.0, 29.97, 30.0, 48.0, 50.0, 59.94, 60.0, 120.0,
    ];
    STANDARD
        .iter()
        .copied()
        .filter(|s| (s - fps).abs() < 0.05)
        .min_by(|a, b| (a - fps).abs().total_cmp(&(b - fps).abs()))
        .unwrap_or(if fps.is_finite() && fps > 1.0 {
            fps
        } else {
            29.97
        })
}

/// Presentation timestamp (stream time-base units) for `seconds`.
pub fn target_pts(seconds: f64, time_base_num: i32, time_base_den: i32) -> i64 {
    if time_base_num <= 0 || time_base_den <= 0 {
        return 0;
    }
    (seconds * time_base_den as f64 / time_base_num as f64).round() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_extensions_are_detected_case_insensitively() {
        assert!(is_video_path(Path::new("C:/x/Clip.MP4")));
        assert!(is_video_path(Path::new("a.mkv")));
        assert!(!is_video_path(Path::new("song.wav")));
        assert!(!is_video_path(Path::new("noext")));
    }

    #[test]
    fn fit_size_keeps_aspect_and_even_dimensions() {
        assert_eq!(fit_size(1920, 1080, 480), (480, 270));
        assert_eq!(fit_size(1920, 1080, 0), (1920, 1080));
        assert_eq!(fit_size(640, 360, 1920), (640, 360));
        assert_eq!(fit_size(1921, 1081, 0), (1920, 1080));
        let (w, h) = fit_size(1000, 563, 333);
        assert!(w % 2 == 0 && h % 2 == 0 && w <= 333);
    }

    #[test]
    fn standard_rates_are_snapped() {
        assert_eq!(snap_fps(29.97002997), 29.97);
        assert_eq!(snap_fps(23.976023976), 23.976);
        assert_eq!(snap_fps(25.0), 25.0);
        assert_eq!(snap_fps(0.0), 29.97);
        assert_eq!(snap_fps(37.0), 37.0);
    }

    #[test]
    fn pts_follows_the_time_base() {
        assert_eq!(target_pts(2.0, 1, 90_000), 180_000);
        assert_eq!(target_pts(1.0, 1001, 30_000), 30);
        assert_eq!(target_pts(1.0, 0, 0), 0);
    }
}
