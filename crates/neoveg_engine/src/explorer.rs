//! Unified File Explorer Subsystem (results.md §12.11; manual pp. 180–184).
//!
//! Provides directory navigation, bookmarks ("My Favorites"), companion file
//! suppression (.sfk, .sfvp0), preview transport player with rapid file selection
//! buffer flushing, and automatic scene detection for subclip creation.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// File or directory entry in the explorer pane.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileItem {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub size_bytes: u64,
    pub modified: u64,
    pub is_media: bool,
    pub is_favorite: bool,
}

/// Transport playback state of the explorer preview player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportState {
    #[default]
    Stopped,
    Playing,
    Paused,
}

/// Unified File Explorer model with filesystem navigation, favorites, and preview player.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorerModel {
    current_dir: PathBuf,
    favorites: Vec<PathBuf>,
    auto_preview: bool,
    transport_state: TransportState,
    preview_position_sec: f64,
    preview_volume: f32, // 0.0 to 1.0
    active_preview_file: Option<PathBuf>,
    /// Leave the peak files of the sound (`.nvpk`, `.sfk`) out of the listing.
    #[serde(default = "default_hide_peak_files")]
    hide_peak_files: bool,
}

fn default_hide_peak_files() -> bool {
    true
}

impl Default for ExplorerModel {
    fn default() -> Self {
        Self::new()
    }
}

impl ExplorerModel {
    pub fn new() -> Self {
        Self {
            current_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            favorites: Vec::new(),
            auto_preview: true,
            transport_state: TransportState::Stopped,
            preview_position_sec: 0.0,
            preview_volume: 1.0,
            active_preview_file: None,
            hide_peak_files: true,
        }
    }

    /// Whether the listing leaves out peak files.
    pub fn hide_peak_files(&self) -> bool {
        self.hide_peak_files
    }

    /// Shows or hides the peak files in the listing (*ocultar arquivos de picos*).
    pub fn set_hide_peak_files(&mut self, hide: bool) {
        self.hide_peak_files = hide;
    }

    pub fn current_dir(&self) -> &Path {
        &self.current_dir
    }

    pub fn favorites(&self) -> &[PathBuf] {
        &self.favorites
    }

    pub fn auto_preview(&self) -> bool {
        self.auto_preview
    }

    pub fn set_auto_preview(&mut self, enabled: bool) {
        self.auto_preview = enabled;
    }

    pub fn transport_state(&self) -> TransportState {
        self.transport_state
    }

    pub fn preview_position(&self) -> f64 {
        self.preview_position_sec
    }

    pub fn active_preview_file(&self) -> Option<&Path> {
        self.active_preview_file.as_deref()
    }

    /// Adds a folder path to favorites bookmarks.
    pub fn add_favorite(&mut self, path: PathBuf) {
        if !self.favorites.contains(&path) {
            self.favorites.push(path);
        }
    }

    /// Removes a folder path from favorites bookmarks.
    pub fn remove_favorite(&mut self, path: &Path) {
        self.favorites.retain(|p| p != path);
    }

    /// Navigates to a filesystem directory and lists its items.
    ///
    /// Automatically suppresses companion files (.sfk audio peaks and .sfvp0 video proxies)
    /// per results.md §12.11 and manual lines 4142, 9057 (Edge Case 21).
    pub fn navigate_to(&mut self, path: &Path) -> Result<Vec<FileItem>, String> {
        if !path.exists() {
            return Err(format!("Directory does not exist: {}", path.display()));
        }
        if !path.is_dir() {
            return Err(format!("Path is not a directory: {}", path.display()));
        }

        let read_dir = std::fs::read_dir(path)
            .map_err(|e| format!("Cannot read directory {}: {e}", path.display()))?;

        self.current_dir = path.to_path_buf();
        let mut items = Vec::new();

        for entry in read_dir.flatten() {
            let entry_path = entry.path();
            let file_name = entry.file_name().to_string_lossy().to_string();

            // Edge Case 21: Companion file suppression (.sfk, .sfvp0, .nvprv) case-insensitive
            let name_lower = file_name.to_lowercase();
            let peak_file = name_lower.ends_with(".sfk") || name_lower.ends_with(".nvpk");
            if (peak_file && self.hide_peak_files)
                || name_lower.ends_with(".sfvp0")
                || name_lower.ends_with(".nvprv")
            {
                continue;
            }

            let metadata = entry.metadata().ok();
            let is_dir = metadata.as_ref().is_some_and(|m| m.is_dir());
            let size_bytes = metadata.as_ref().map_or(0, |m| m.len());
            let modified = metadata
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs());

            let is_media = is_media_extension(&entry_path);
            let is_favorite = self.favorites.contains(&entry_path);

            items.push(FileItem {
                path: entry_path,
                name: file_name,
                is_dir,
                size_bytes,
                modified,
                is_media,
                is_favorite,
            });
        }

        // Folders first, then media files, then other files, alphabetically
        items.sort_by(|a, b| {
            b.is_dir
                .cmp(&a.is_dir)
                .then(b.is_media.cmp(&a.is_media))
                .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });

        Ok(items)
    }

    /// Starts or updates playback preview for the given media file.
    ///
    /// Edge Case 22: Rapid file selection immediately halts and flushes previous playback.
    pub fn play_preview(&mut self, media_path: &Path) -> Result<(), String> {
        if !media_path.exists() {
            return Err(format!(
                "Media file does not exist: {}",
                media_path.display()
            ));
        }

        // Halt and flush previous playback
        self.stop_preview();

        self.active_preview_file = Some(media_path.to_path_buf());
        self.transport_state = TransportState::Playing;
        self.preview_position_sec = 0.0;
        Ok(())
    }

    pub fn pause_preview(&mut self) {
        if self.transport_state == TransportState::Playing {
            self.transport_state = TransportState::Paused;
        }
    }

    pub fn stop_preview(&mut self) {
        self.transport_state = TransportState::Stopped;
        self.preview_position_sec = 0.0;
        self.active_preview_file = None;
    }

    pub fn scrub_preview(&mut self, position_sec: f64) {
        self.preview_position_sec = position_sec.max(0.0);
    }

    /// Probes media container and stream properties (duration, dimensions, fps).
    pub fn probe_media(&self, media_path: &Path) -> Result<ProbedMediaInfo, String> {
        use std::io::Read;

        if !media_path.exists() {
            return Err(format!(
                "Media file does not exist: {}",
                media_path.display()
            ));
        }

        let metadata = std::fs::metadata(media_path)
            .map_err(|e| format!("Cannot read file metadata {}: {e}", media_path.display()))?;
        if metadata.len() == 0 {
            return Err(format!(
                "Media file is empty (0 bytes): {}",
                media_path.display()
            ));
        }

        // 1. Try ISO-BMFF (MP4 / MOV) demuxer
        if let Ok(file) = std::fs::File::open(media_path) {
            if let Ok(mp4) = neoveg_media::isobmff::open(&file) {
                // Find video track first
                if let Some((_, v_track)) = mp4
                    .tracks
                    .iter()
                    .enumerate()
                    .find(|(_, t)| t.kind == neoveg_media::isobmff::TrackKind::Video)
                {
                    let timescale = v_track.timescale.max(1) as f64;
                    let duration = if v_track.duration > 0 {
                        v_track.duration as f64 / timescale
                    } else {
                        mp4.duration as f64 / (mp4.timescale.max(1) as f64)
                    };
                    if duration <= 0.0 {
                        return Err(format!(
                            "Invalid or zero video duration ({duration}s) in {}",
                            media_path.display()
                        ));
                    }
                    let fps = if !v_track.samples.is_empty() && duration > 0.0 {
                        v_track.samples.len() as f64 / duration
                    } else {
                        29.97
                    };
                    return Ok(ProbedMediaInfo {
                        duration_seconds: duration,
                        width: v_track.width,
                        height: v_track.height,
                        fps,
                        is_video: true,
                    });
                } else if let Some((_, a_track)) = mp4
                    .tracks
                    .iter()
                    .enumerate()
                    .find(|(_, t)| t.kind == neoveg_media::isobmff::TrackKind::Audio)
                {
                    let timescale = a_track.timescale.max(1) as f64;
                    let duration = if a_track.duration > 0 {
                        a_track.duration as f64 / timescale
                    } else {
                        mp4.duration as f64 / (mp4.timescale.max(1) as f64)
                    };
                    if duration <= 0.0 {
                        return Err(format!(
                            "Invalid or zero audio duration ({duration}s) in {}",
                            media_path.display()
                        ));
                    }
                    return Ok(ProbedMediaInfo {
                        duration_seconds: duration,
                        width: 0,
                        height: 0,
                        fps: 0.0,
                        is_video: false,
                    });
                }
            }
        }

        // 2. Check synthetic video fixture format (magic: b"NVVID100")
        if metadata.len() >= 28 {
            let mut head = [0u8; 28];
            if let Ok(mut f) = std::fs::File::open(media_path) {
                if f.read_exact(&mut head).is_ok() && &head[0..8] == b"NVVID100" {
                    let width = u32::from_le_bytes(head[8..12].try_into().unwrap_or([0; 4]));
                    let height = u32::from_le_bytes(head[12..16].try_into().unwrap_or([0; 4]));
                    let fps_num = u32::from_le_bytes(head[16..20].try_into().unwrap_or([0; 4]));
                    let fps_den =
                        u32::from_le_bytes(head[20..24].try_into().unwrap_or([1; 4])).max(1);
                    let frame_count = u32::from_le_bytes(head[24..28].try_into().unwrap_or([0; 4]));
                    let fps = fps_num as f64 / fps_den as f64;
                    let duration = if fps > 0.0 {
                        frame_count as f64 / fps
                    } else {
                        0.0
                    };
                    if duration <= 0.0 {
                        return Err(format!(
                            "Invalid duration in synthetic video {}",
                            media_path.display()
                        ));
                    }
                    return Ok(ProbedMediaInfo {
                        duration_seconds: duration,
                        width,
                        height,
                        fps,
                        is_video: true,
                    });
                }
            }
        }

        // 3. Check WAV audio
        if metadata.len() >= 44 {
            let mut head = [0u8; 44];
            if let Ok(mut f) = std::fs::File::open(media_path) {
                if f.read_exact(&mut head).is_ok()
                    && &head[0..4] == b"RIFF"
                    && &head[8..12] == b"WAVE"
                {
                    let byte_rate =
                        u32::from_le_bytes(head[28..32].try_into().unwrap_or([0; 4])).max(1);
                    let data_len = u32::from_le_bytes(head[40..44].try_into().unwrap_or([0; 4]));
                    let duration = data_len as f64 / byte_rate as f64;
                    if duration <= 0.0 {
                        return Err(format!(
                            "Invalid audio duration in {}",
                            media_path.display()
                        ));
                    }
                    return Ok(ProbedMediaInfo {
                        duration_seconds: duration,
                        width: 0,
                        height: 0,
                        fps: 0.0,
                        is_video: false,
                    });
                }
            }
        }

        // 4. Check still image files
        let ext = media_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();
        if matches!(
            ext.as_str(),
            "png" | "jpg" | "jpeg" | "bmp" | "tif" | "tiff"
        ) {
            let mut head = [0u8; 32];
            if let Ok(mut f) = std::fs::File::open(media_path) {
                let n = f.read(&mut head).unwrap_or(0);
                let (w, h) = if n >= 24 && &head[0..8] == b"\x89PNG\r\n\x1a\n" {
                    let w = u32::from_be_bytes(head[16..20].try_into().unwrap_or([0; 4]));
                    let h = u32::from_be_bytes(head[20..24].try_into().unwrap_or([0; 4]));
                    (w, h)
                } else if n >= 26 && &head[0..2] == b"BM" {
                    let w = u32::from_le_bytes(head[18..22].try_into().unwrap_or([0; 4]));
                    let h = u32::from_le_bytes(head[22..26].try_into().unwrap_or([0; 4]));
                    (w, h)
                } else {
                    (1920, 1080)
                };
                return Ok(ProbedMediaInfo {
                    duration_seconds: 5.0,
                    width: w,
                    height: h,
                    fps: 29.97,
                    is_video: false,
                });
            }
        }

        Err(format!(
            "Unreadable or unsupported media format in {}",
            media_path.display()
        ))
    }

    /// Automatic Scene Detection (results.md lines 12882, 12959–12960).
    ///
    /// Analyzes media file cut points, splitting the file into discrete subclip time ranges.
    /// If no scene boundaries exceed `threshold`, returns single continuous span strictly bounded
    /// by media duration (Edge Case 106).
    pub fn detect_scenes(
        &self,
        media_path: &Path,
        threshold: f32,
    ) -> Result<Vec<(f64, f64)>, String> {
        use std::io::Read;

        let probed = self.probe_media(media_path)?;
        let duration = probed.duration_seconds;
        if duration <= 0.0 {
            return Err(format!("Media duration <= 0.0 in {}", media_path.display()));
        }

        let mut cuts: Vec<f64> = Vec::new();

        // 1. If synthetic video container (NVVID100)
        let mut head = [0u8; 28];
        let mut is_nvvid = false;
        if let Ok(mut f) = std::fs::File::open(media_path) {
            if f.read_exact(&mut head).is_ok() && &head[0..8] == b"NVVID100" {
                is_nvvid = true;
            }
        }

        if is_nvvid {
            let width = u32::from_le_bytes(head[8..12].try_into().unwrap_or([0; 4])) as usize;
            let height = u32::from_le_bytes(head[12..16].try_into().unwrap_or([0; 4])) as usize;
            let fps_num = u32::from_le_bytes(head[16..20].try_into().unwrap_or([0; 4])) as f64;
            let fps_den =
                u32::from_le_bytes(head[20..24].try_into().unwrap_or([1; 4])).max(1) as f64;
            let frame_count =
                u32::from_le_bytes(head[24..28].try_into().unwrap_or([0; 4])) as usize;
            let fps = fps_num / fps_den;
            let frame_bytes = width * height;

            if frame_count > 1 && frame_bytes > 0 && fps > 0.0 {
                use std::io::Seek;
                if let Ok(mut f) = std::fs::File::open(media_path) {
                    let mut prev_luma: Option<f32> = None;
                    let mut prev_hist: Option<[f32; 16]> = None;
                    let mut frame_buf = vec![0u8; frame_bytes];

                    for i in 0..frame_count {
                        let offset = 28 + (i * frame_bytes) as u64;
                        if f.seek(std::io::SeekFrom::Start(offset)).is_err() {
                            break;
                        }
                        if f.read_exact(&mut frame_buf).is_err() {
                            break;
                        }

                        let luma_sum: u64 = frame_buf.iter().map(|&b| b as u64).sum();
                        let mean_luma = (luma_sum as f32) / (frame_bytes as f32 * 255.0);

                        let mut hist = [0.0f32; 16];
                        for &b in &frame_buf {
                            hist[(b as usize) / 16] += 1.0;
                        }
                        for h in &mut hist {
                            *h /= frame_bytes as f32;
                        }

                        if let (Some(pl), Some(ph)) = (prev_luma, prev_hist) {
                            let luma_diff = (mean_luma - pl).abs();
                            let hist_diff: f32 = hist
                                .iter()
                                .zip(ph.iter())
                                .map(|(a, b)| (a - b).abs())
                                .sum::<f32>()
                                * 0.5;
                            let diff = 0.5 * luma_diff + 0.5 * hist_diff;

                            let cut_sec = (i as f64) / fps;
                            if diff > threshold
                                && cut_sec > 0.05
                                && cut_sec < (duration - 0.05)
                                && (cuts.is_empty()
                                    || (cut_sec - cuts.last().copied().unwrap_or(0.0)) >= 0.2)
                            {
                                cuts.push(cut_sec);
                            }
                        }

                        prev_luma = Some(mean_luma);
                        prev_hist = Some(hist);
                    }
                }
            }
        } else if let Ok(file) = std::fs::File::open(media_path) {
            // 2. Try ISO BMFF (MP4 / MOV) demuxer
            if let Ok(mp4) = neoveg_media::isobmff::open(&file) {
                if let Some((v_idx, v_track)) = mp4
                    .tracks
                    .iter()
                    .enumerate()
                    .find(|(_, t)| t.kind == neoveg_media::isobmff::TrackKind::Video)
                {
                    let timescale = v_track.timescale.max(1) as f64;
                    let sample_count = v_track.samples.len();

                    if sample_count > 1 {
                        let step = (sample_count / 300).max(1);
                        let mut prev_sig: Option<(f32, [f32; 16])> = None;

                        for i in (0..sample_count).step_by(step) {
                            let s = &v_track.samples[i];
                            let pts_sec = (s.pts as f64) / timescale;

                            if let Ok(bytes) = mp4.read_sample(&file, v_idx, i) {
                                if !bytes.is_empty() {
                                    let sum: u64 = bytes.iter().map(|&b| b as u64).sum();
                                    let mean = (sum as f32) / (bytes.len() as f32 * 255.0);

                                    let mut hist = [0.0f32; 16];
                                    for &b in &bytes {
                                        hist[(b as usize) / 16] += 1.0;
                                    }
                                    let inv_len = 1.0 / bytes.len() as f32;
                                    for h in &mut hist {
                                        *h *= inv_len;
                                    }

                                    if let Some((prev_mean, prev_hist)) = prev_sig {
                                        let mean_diff = (mean - prev_mean).abs();
                                        let hist_diff: f32 = hist
                                            .iter()
                                            .zip(prev_hist.iter())
                                            .map(|(a, b)| (a - b).abs())
                                            .sum::<f32>()
                                            * 0.5;
                                        let mut diff = 0.4 * mean_diff + 0.6 * hist_diff;

                                        // Keyframes at major shifts boost transition confidence
                                        if s.is_sync {
                                            diff *= 1.25;
                                        }

                                        if diff > threshold
                                            && pts_sec > 0.05
                                            && pts_sec < (duration - 0.05)
                                            && (cuts.is_empty()
                                                || (pts_sec - cuts.last().copied().unwrap_or(0.0))
                                                    >= 0.2)
                                        {
                                            cuts.push(pts_sec);
                                        }
                                    }

                                    prev_sig = Some((mean, hist));
                                }
                            }
                        }
                    }
                }
            } else {
                // 3. Audio or still image fallback cut detection
                // If WAV audio, detect major amplitude transitions
                let ext = media_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_lowercase())
                    .unwrap_or_default();
                if ext == "wav" {
                    if let Ok(mut f) = std::fs::File::open(media_path) {
                        let mut head = [0u8; 44];
                        if f.read_exact(&mut head).is_ok()
                            && &head[0..4] == b"RIFF"
                            && &head[8..12] == b"WAVE"
                        {
                            let byte_rate =
                                u32::from_le_bytes(head[28..32].try_into().unwrap_or([0; 4]))
                                    .max(1);
                            let block_size = (byte_rate / 10).max(1024) as usize; // ~100ms blocks
                            let mut block = vec![0u8; block_size];
                            let mut prev_rms: Option<f32> = None;
                            let mut block_idx = 0usize;

                            while let Ok(n) = f.read(&mut block) {
                                if n < 4 {
                                    break;
                                }
                                let t = (block_idx * block_size) as f64 / byte_rate as f64;
                                block_idx += 1;

                                let sum_sq: f64 = block[..n]
                                    .as_chunks::<2>()
                                    .0
                                    .iter()
                                    .map(|c| {
                                        let sample = i16::from_le_bytes([c[0], c[1]]) as f64;
                                        sample * sample
                                    })
                                    .sum();
                                let count = (n / 2).max(1) as f64;
                                let rms = (sum_sq / count).sqrt() as f32 / 32768.0;

                                if let Some(p_rms) = prev_rms {
                                    let diff = (rms - p_rms).abs() / rms.max(p_rms).max(0.05);
                                    if diff > threshold
                                        && t > 0.05
                                        && t < (duration - 0.05)
                                        && (cuts.is_empty()
                                            || (t - cuts.last().copied().unwrap_or(0.0)) >= 0.2)
                                    {
                                        cuts.push(t);
                                    }
                                }
                                prev_rms = Some(rms);
                            }
                        }
                    }
                }
            }
        }

        // Deduplicate and assemble scene spans
        cuts.retain(|&c| c > 0.05 && c < (duration - 0.05));
        cuts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        cuts.dedup_by(|a, b| (*a - *b).abs() < 0.15);

        if cuts.is_empty() {
            return Ok(vec![(0.0, duration)]);
        }

        let mut scenes = Vec::new();
        let mut start = 0.0;
        for cut in cuts {
            if cut > start && cut < duration {
                scenes.push((start, cut));
                start = cut;
            }
        }
        if start < duration {
            scenes.push((start, duration));
        }

        if scenes.is_empty() {
            scenes.push((0.0, duration));
        }

        Ok(scenes)
    }
}

/// Probed media attributes for an inspected media file.
#[derive(Debug, Clone, PartialEq)]
pub struct ProbedMediaInfo {
    pub duration_seconds: f64,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub is_video: bool,
}

fn is_media_extension(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let ext = ext.to_lowercase();
        matches!(
            ext.as_str(),
            "mp4"
                | "mov"
                | "avi"
                | "mxf"
                | "mkv"
                | "wav"
                | "mp3"
                | "aac"
                | "flac"
                | "png"
                | "jpg"
                | "jpeg"
                | "tif"
                | "tiff"
                | "exr"
                | "dng"
        )
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn companion_files_are_suppressed_case_insensitively() {
        let dir = std::env::temp_dir().join(format!("neoveg_exp_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);

        let video = dir.join("clip.mp4");
        let peak_lower = dir.join("clip.mp4.sfk");
        let proxy_lower = dir.join("clip.mp4.sfvp0");
        let prerender_lower = dir.join("clip.mp4.nvprv");
        let peak_upper = dir.join("clip.mp4.SFK");
        let proxy_upper = dir.join("clip.mp4.SFVP0");
        let prerender_upper = dir.join("clip.mp4.NVPRV");

        let _ = std::fs::write(&video, b"video");
        let _ = std::fs::write(&peak_lower, b"peak");
        let _ = std::fs::write(&proxy_lower, b"proxy");
        let _ = std::fs::write(&prerender_lower, b"prerender");
        let _ = std::fs::write(&peak_upper, b"PEAK");
        let _ = std::fs::write(&proxy_upper, b"PROXY");
        let _ = std::fs::write(&prerender_upper, b"PRERENDER");

        let mut explorer = ExplorerModel::new();
        let items = explorer.navigate_to(&dir).expect("navigate");

        assert_eq!(
            items.len(),
            1,
            "Must suppress all case variations of companion files"
        );
        assert_eq!(items[0].name, "clip.mp4");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn peak_files_show_in_the_listing_only_when_the_option_to_hide_them_is_off() {
        let dir = std::env::temp_dir().join(format!("neoveg_exp_peaks_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for name in ["voz.wav", "voz.wav.nvpk", "voz.wav.sfk"] {
            std::fs::write(dir.join(name), b"x").unwrap();
        }
        let mut explorer = ExplorerModel::new();
        assert!(explorer.hide_peak_files(), "hidden by default");
        assert_eq!(explorer.navigate_to(&dir).unwrap().len(), 1);
        explorer.set_hide_peak_files(false);
        let mut names: Vec<String> = explorer
            .navigate_to(&dir)
            .unwrap()
            .into_iter()
            .map(|i| i.name)
            .collect();
        names.sort();
        assert_eq!(names, ["voz.wav", "voz.wav.nvpk", "voz.wav.sfk"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rapid_preview_halts_previous_playback() {
        let mut explorer = ExplorerModel::new();
        let f1 = std::env::temp_dir().join("a.mp4");
        let f2 = std::env::temp_dir().join("b.mp4");
        let _ = std::fs::write(&f1, b"a");
        let _ = std::fs::write(&f2, b"b");

        explorer.play_preview(&f1).unwrap();
        assert_eq!(explorer.transport_state(), TransportState::Playing);
        assert_eq!(explorer.active_preview_file(), Some(f1.as_path()));

        // Rapid selection of second file flushes first
        explorer.play_preview(&f2).unwrap();
        assert_eq!(explorer.transport_state(), TransportState::Playing);
        assert_eq!(explorer.active_preview_file(), Some(f2.as_path()));

        let _ = std::fs::remove_file(f1);
        let _ = std::fs::remove_file(f2);
    }

    #[test]
    fn favorites_management() {
        let mut explorer = ExplorerModel::new();
        let fav = PathBuf::from("/my/projects");
        explorer.add_favorite(fav.clone());
        assert!(explorer.favorites().contains(&fav));
        explorer.remove_favorite(&fav);
        assert!(!explorer.favorites().contains(&fav));
    }

    #[test]
    fn test_detect_scenes_synthetic_fixture_and_duration_bounding() {
        let fixture_path =
            crate::test_fixtures::BlenderOpenMovieFixture::ensure_fixture("big_buck_bunny")
                .expect("fixture generation");

        let explorer = ExplorerModel::new();

        // 1. Scene detection with default threshold (0.25) detects the 3 distinct shots
        let scenes = explorer
            .detect_scenes(&fixture_path, 0.25)
            .expect("detect scenes");
        assert_eq!(
            scenes.len(),
            3,
            "Big Buck Bunny fixture has 3 distinct luminance shots"
        );
        assert_eq!(scenes[0].0, 0.0);
        assert!((scenes[0].1 - 2.0).abs() < 0.15, "Shot 1 ends near 2.0s");
        assert!((scenes[1].0 - 2.0).abs() < 0.15);
        assert!((scenes[1].1 - 5.0).abs() < 0.15, "Shot 2 ends near 5.0s");
        assert!((scenes[2].0 - 5.0).abs() < 0.15);
        assert!(
            (scenes[2].1 - 7.5).abs() < 0.15,
            "Final shot bounded strictly by 7.5s"
        );

        // 2. High threshold (0.95) returns single continuous span strictly bounded by duration
        let single_scene = explorer
            .detect_scenes(&fixture_path, 0.95)
            .expect("high threshold");
        assert_eq!(single_scene.len(), 1);
        assert_eq!(single_scene[0].0, 0.0);
        assert!(
            (single_scene[0].1 - 7.5).abs() < 0.01,
            "Must be bounded strictly by file duration (7.5s, not 10.0)"
        );

        // 3. Non-existent file returns Err
        let missing = std::path::Path::new("does_not_exist_xyz123.mp4");
        assert!(explorer.detect_scenes(missing, 0.25).is_err());
    }
}
