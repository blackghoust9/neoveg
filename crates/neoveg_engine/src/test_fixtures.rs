//! Blender Open Movie test fixture generator and cache.
//!
//! Provides real-world and synthetic video/audio test fixtures based on Blender Open Movies
//! (Big Buck Bunny, Sintel, Tears of Steel) cached in `target/test_fixtures/blender/`.
//! Ensures tests run completely offline and deterministically without bloating git commits.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct BlenderOpenMovieFixture;

impl BlenderOpenMovieFixture {
    /// Directory where Blender fixtures are cached (inside target/, so gitignored).
    pub fn cache_dir() -> PathBuf {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        root.join("target").join("test_fixtures").join("blender")
    }

    /// Returns path to a Blender Open Movie test fixture, creating a lightweight synthetic
    /// multi-shot clip offline if not already present on disk.
    ///
    /// Available names:
    /// - "big_buck_bunny": 3-shot clip (clearing daylight, rabbit hole dark, forest green)
    /// - "sintel": 2-shot clip (snow blizzard, dragon canyon)
    /// - "tears_of_steel": 2-shot clip (robot lab bright, street night)
    pub fn ensure_fixture(name: &str) -> Result<PathBuf, String> {
        let dir = Self::cache_dir();
        fs::create_dir_all(&dir)
            .map_err(|e| format!("Failed to create fixture dir {}: {e}", dir.display()))?;

        // 1. Check if a real .mp4 already exists in cache
        let mp4_path = dir.join(format!("{name}.mp4"));
        if mp4_path.exists() {
            return Ok(mp4_path);
        }

        // 2. Check if cached .nvvid synthetic fixture already exists
        let nvvid_path = dir.join(format!("{name}.nvvid"));
        if nvvid_path.exists() {
            return Ok(nvvid_path);
        }

        // 3. Check repository sample video
        if name == "sample_video" {
            let repo_sample = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../neoveg_video/tests/assets/sample_video.mp4");
            if repo_sample.exists() {
                return Ok(repo_sample);
            }
        }

        // 4. Generate deterministic synthetic multi-shot video fixture
        Self::generate_synthetic_fixture(&nvvid_path, name)?;
        Ok(nvvid_path)
    }

    /// Generates a lightweight synthetic multi-shot video clip with distinct luminance scenes.
    pub fn generate_synthetic_fixture(dest_path: &Path, name: &str) -> Result<(), String> {
        let (w, h, fps) = (64u32, 36u32, 24u32);
        let frame_bytes = (w * h) as usize;

        // Define shots per movie: (frame_count, luma_value)
        let shots: Vec<(usize, u8)> = match name {
            "sintel" => vec![
                (48, 240), // Shot 1: Snow blizzard (2.0s, high brightness)
                (72, 40),  // Shot 2: Dark mountain cavern (3.0s, low brightness)
            ],
            "tears_of_steel" => vec![
                (60, 210), // Shot 1: High-tech lab (2.5s, bright)
                (60, 50),  // Shot 2: Night alley (2.5s, dark)
            ],
            _ => vec![
                // Default Big Buck Bunny: 3 shots
                (48, 220), // Shot 1: Daylight meadow (2.0s, bright)
                (72, 30),  // Shot 2: Underground burrow (3.0s, dark)
                (60, 150), // Shot 3: Forest canopy (2.5s, mid luma)
            ],
        };

        let total_frames: usize = shots.iter().map(|(count, _)| count).sum();

        let mut file = fs::File::create(dest_path)
            .map_err(|e| format!("Failed to create {}: {e}", dest_path.display()))?;

        // 28-byte NVVID100 header
        file.write_all(b"NVVID100")
            .map_err(|e| format!("Header write failed: {e}"))?;
        file.write_all(&w.to_le_bytes()).map_err(|e| e.to_string())?;
        file.write_all(&h.to_le_bytes()).map_err(|e| e.to_string())?;
        file.write_all(&fps.to_le_bytes()).map_err(|e| e.to_string())?;
        file.write_all(&1u32.to_le_bytes()).map_err(|e| e.to_string())?; // fps_den = 1
        file.write_all(&(total_frames as u32).to_le_bytes())
            .map_err(|e| e.to_string())?;

        // Write pixel data for each frame
        for (count, luma) in shots {
            let buf = vec![luma; frame_bytes];
            for _ in 0..count {
                file.write_all(&buf).map_err(|e| e.to_string())?;
            }
        }

        file.flush().map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Creates a synthetic WAV audio test fixture with two distinct volume segments.
    pub fn ensure_audio_fixture(name: &str) -> Result<PathBuf, String> {
        let dir = Self::cache_dir();
        fs::create_dir_all(&dir)
            .map_err(|e| format!("Failed to create fixture dir {}: {e}", dir.display()))?;

        let wav_path = dir.join(format!("{name}.wav"));
        if wav_path.exists() {
            return Ok(wav_path);
        }

        let sample_rate = 48000u32;
        let channels = 2u16;
        let bits_per_sample = 16u16;
        let byte_rate = sample_rate * u32::from(channels) * u32::from(bits_per_sample / 8);
        let block_align = channels * (bits_per_sample / 8);

        // 4.0 seconds total: 2.0s loud tone + 2.0s silence
        let total_samples = (sample_rate as f64 * 4.0) as usize;
        let mut pcm_bytes = Vec::with_capacity(total_samples * block_align as usize);

        for i in 0..total_samples {
            let sample_val = if i < total_samples / 2 {
                // 440 Hz sine wave in first 2 seconds
                let t = i as f64 / sample_rate as f64;
                (f64::sin(2.0 * std::f64::consts::PI * 440.0 * t) * 16000.0) as i16
            } else {
                0i16 // silence in last 2 seconds
            };
            let b = sample_val.to_le_bytes();
            // Stereo: write L and R
            pcm_bytes.extend_from_slice(&b);
            pcm_bytes.extend_from_slice(&b);
        }

        let data_len = pcm_bytes.len() as u32;
        let riff_len = 36 + data_len;

        let mut file = fs::File::create(&wav_path)
            .map_err(|e| format!("Failed to create {}: {e}", wav_path.display()))?;

        // 44-byte WAV header
        file.write_all(b"RIFF").map_err(|e| e.to_string())?;
        file.write_all(&riff_len.to_le_bytes()).map_err(|e| e.to_string())?;
        file.write_all(b"WAVE").map_err(|e| e.to_string())?;
        file.write_all(b"fmt ").map_err(|e| e.to_string())?;
        file.write_all(&16u32.to_le_bytes()).map_err(|e| e.to_string())?; // Subchunk1Size
        file.write_all(&1u16.to_le_bytes()).map_err(|e| e.to_string())?;  // AudioFormat = PCM
        file.write_all(&channels.to_le_bytes()).map_err(|e| e.to_string())?;
        file.write_all(&sample_rate.to_le_bytes()).map_err(|e| e.to_string())?;
        file.write_all(&byte_rate.to_le_bytes()).map_err(|e| e.to_string())?;
        file.write_all(&block_align.to_le_bytes()).map_err(|e| e.to_string())?;
        file.write_all(&bits_per_sample.to_le_bytes()).map_err(|e| e.to_string())?;
        file.write_all(b"data").map_err(|e| e.to_string())?;
        file.write_all(&data_len.to_le_bytes()).map_err(|e| e.to_string())?;
        file.write_all(&pcm_bytes).map_err(|e| e.to_string())?;

        file.flush().map_err(|e| e.to_string())?;
        Ok(wav_path)
    }
}
