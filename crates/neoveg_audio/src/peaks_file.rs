//! Peak files (*arquivos de picos de áudio*): a small summary of the waveform of a sound file, kept
//! beside the file as `<name>.<ext>.nvpk` so the waveform can be drawn without reading the sound
//! again. Two options govern them (Preferências > Geral): whether they are created at all, and
//! whether the file explorer hides them. The options are process-wide switches because the sound is
//! loaded on worker threads that do not see the preferences.
//!
//! Layout (little endian): magic `NVPK1`, `u32` sample rate, `u64` frames, `u64` size of the sound
//! file, `u64` its modification time (seconds), `u32` frames per block, `u32` block count, then per
//! block the lowest and the highest sample as two `f32`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::pcm::PcmStore;

/// The extension of a peak file, after the name of the sound file.
pub const EXTENSION: &str = "nvpk";
/// Frames summarised by one block of the file.
pub const BLOCK_FRAMES: u32 = 256;

const MAGIC: &[u8; 5] = b"NVPK1";

static CREATE: AtomicBool = AtomicBool::new(true);

/// Turns the creation of peak files on or off for the whole program.
pub fn set_create(enabled: bool) {
    CREATE.store(enabled, Ordering::Relaxed);
}

/// Whether peak files are created.
pub fn create_enabled() -> bool {
    CREATE.load(Ordering::Relaxed)
}

/// Whether `name` (a file name) is a peak file: the one of this program or the `.sfk` kind other
/// editors leave beside their media.
pub fn is_peak_file_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with(&format!(".{EXTENSION}")) || lower.ends_with(".sfk")
}

/// `clip.wav` -> `clip.wav.nvpk`.
pub fn peak_file_path(media: &Path) -> PathBuf {
    let mut name = media.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(format!(".{EXTENSION}"));
    media.with_file_name(name)
}

/// What a peak file says about the sound it was made from.
#[derive(Debug, Clone, PartialEq)]
pub struct PeakFile {
    pub sample_rate: u32,
    pub frames: u64,
    /// Size and modification time of the sound file: a file that changed has no valid peaks.
    pub media_size: u64,
    pub media_mtime: u64,
    pub block_frames: u32,
    /// `(lowest, highest)` sample of each block, both channels together.
    pub blocks: Vec<(f32, f32)>,
}

/// Size and modification time (seconds) of `media`.
fn stamp(media: &Path) -> Option<(u64, u64)> {
    let meta = std::fs::metadata(media).ok()?;
    let mtime = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some((meta.len(), mtime))
}

impl PeakFile {
    /// Summarises the stereo `store`, decoded at `sample_rate`, of `media`.
    pub fn from_store(media: &Path, sample_rate: u32, store: &PcmStore) -> Option<Self> {
        let (media_size, media_mtime) = stamp(media)?;
        let frames = store.frames();
        let block = BLOCK_FRAMES as usize;
        let mut blocks = Vec::with_capacity(frames / block + 1);
        let mut at = 0;
        while at < frames {
            let part = store.to_vec(at, block);
            let (lo, hi) = part
                .iter()
                .fold((0.0f32, 0.0f32), |(lo, hi), &v| (lo.min(v), hi.max(v)));
            blocks.push((lo, hi));
            at += block;
        }
        Some(Self {
            sample_rate,
            frames: frames as u64,
            media_size,
            media_mtime,
            block_frames: BLOCK_FRAMES,
            blocks,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(40 + self.blocks.len() * 8);
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&self.sample_rate.to_le_bytes());
        out.extend_from_slice(&self.frames.to_le_bytes());
        out.extend_from_slice(&self.media_size.to_le_bytes());
        out.extend_from_slice(&self.media_mtime.to_le_bytes());
        out.extend_from_slice(&self.block_frames.to_le_bytes());
        out.extend_from_slice(&(self.blocks.len() as u32).to_le_bytes());
        for (lo, hi) in &self.blocks {
            out.extend_from_slice(&lo.to_le_bytes());
            out.extend_from_slice(&hi.to_le_bytes());
        }
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let rest = bytes.strip_prefix(MAGIC.as_slice())?;
        let take = |at: usize, n: usize| rest.get(at..at + n);
        let u32_at = |at: usize| take(at, 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
        let u64_at = |at: usize| take(at, 8).map(|b| u64::from_le_bytes(b.try_into().unwrap()));
        let sample_rate = u32_at(0)?;
        let frames = u64_at(4)?;
        let media_size = u64_at(12)?;
        let media_mtime = u64_at(20)?;
        let block_frames = u32_at(28)?;
        let count = u32_at(32)? as usize;
        let data = rest.get(36..)?;
        if data.len() != count.checked_mul(8)? {
            return None;
        }
        let blocks = data
            .chunks_exact(8)
            .map(|c| {
                (
                    f32::from_le_bytes(c[..4].try_into().unwrap()),
                    f32::from_le_bytes(c[4..].try_into().unwrap()),
                )
            })
            .collect();
        Some(Self {
            sample_rate,
            frames,
            media_size,
            media_mtime,
            block_frames,
            blocks,
        })
    }
}

/// Writes the peak file of `media` when creation is on. Returns the path written, `None` when the
/// option is off or the file could not be written (a peak file is a convenience, never an error).
pub fn write_if_enabled(media: &Path, sample_rate: u32, store: &PcmStore) -> Option<PathBuf> {
    if !create_enabled() {
        return None;
    }
    let file = PeakFile::from_store(media, sample_rate, store)?;
    let path = peak_file_path(media);
    std::fs::write(&path, file.to_bytes()).ok()?;
    Some(path)
}

/// Reads the peak file of `media` when it exists and still matches the sound file.
pub fn read_valid(media: &Path) -> Option<PeakFile> {
    let file = PeakFile::from_bytes(&std::fs::read(peak_file_path(media)).ok()?)?;
    let (size, mtime) = stamp(media)?;
    (file.media_size == size && file.media_mtime == mtime).then_some(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// The creation switch is global: the tests that flip it take turns.
    static SWITCH: Mutex<()> = Mutex::new(());

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("neoveg_peaks_{}_{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn store() -> PcmStore {
        // 1000 stereo frames: a ramp up to 0.5 on both channels, then one -0.25 dip in the last block
        let mut samples: Vec<f32> = (0..1000)
            .flat_map(|i| [i as f32 / 2000.0, i as f32 / 2000.0])
            .collect();
        samples[1998] = -0.25;
        PcmStore::from_vec(samples)
    }

    #[test]
    fn names_say_which_files_are_peak_files() {
        assert!(is_peak_file_name("voz.wav.nvpk"));
        assert!(is_peak_file_name("VOZ.WAV.SFK"));
        assert!(!is_peak_file_name("voz.wav"));
        assert_eq!(
            peak_file_path(Path::new("a/voz.wav")),
            PathBuf::from("a/voz.wav.nvpk")
        );
    }

    #[test]
    fn a_peak_file_round_trips_and_is_valid_only_for_the_sound_it_was_made_from() {
        let _turn = SWITCH.lock().unwrap_or_else(|e| e.into_inner());
        set_create(true);
        let dir = temp("round");
        let media = dir.join("voz.wav");
        std::fs::write(&media, b"not really a wav").unwrap();
        let path = write_if_enabled(&media, 8_000, &store()).expect("written");
        assert_eq!(path, dir.join("voz.wav.nvpk"));
        let file = read_valid(&media).expect("valid");
        assert_eq!((file.sample_rate, file.frames, file.block_frames), (8_000, 1000, 256));
        assert_eq!(file.blocks.len(), 4);
        assert_eq!(file.blocks[0], (0.0, 255.0 / 2000.0));
        assert_eq!(file.blocks[3].0, -0.25);
        assert_eq!(PeakFile::from_bytes(&file.to_bytes()), Some(file));
        // The sound file changed: the peaks no longer fit it
        std::fs::write(&media, b"a different, longer sound file").unwrap();
        assert!(read_valid(&media).is_none());
        // Garbage is not a peak file
        assert!(PeakFile::from_bytes(b"NVPK1\x00").is_none());
        assert!(PeakFile::from_bytes(b"nothing").is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_waveform_read_from_the_peak_file_matches_the_one_computed_directly() {
        use crate::waveform::{WaveformSource, COLUMN_STRIDE};
        use std::sync::Arc;
        let dir = temp("wave");
        let media = dir.join("seno.wav");
        std::fs::write(&media, b"x").unwrap();
        // A mono sine, mirrored into both channels as the decoder does
        let mono: Vec<f32> = (0..4096)
            .map(|i| 0.8 * (i as f32 * 0.05).sin())
            .collect();
        let samples: Vec<f32> = mono.iter().flat_map(|&v| [v, v]).collect();
        let store = Arc::new(PcmStore::from_vec(samples));
        let peaks = PeakFile::from_store(&media, 8_000, &store).unwrap();
        let direct = WaveformSource::from_pcm(8_000, Arc::clone(&store)).unwrap();
        let fast = WaveformSource::from_pcm_with_peaks(8_000, 1, Arc::clone(&store), Some(&peaks)).unwrap();
        // Columns that cover whole blocks come straight from the summaries
        for fpc in [256.0, 512.0, 1024.0] {
            let mut a = vec![0.0f32; 4 * COLUMN_STRIDE];
            let mut b = a.clone();
            direct.query_columns(0.0, fpc, &mut a);
            fast.query_columns(0.0, fpc, &mut b);
            for c in 0..4 {
                for k in [0, 1, 3, 4] {
                    assert_eq!(a[c * COLUMN_STRIDE + k], b[c * COLUMN_STRIDE + k], "fpc {fpc} col {c} field {k}");
                }
                let (ra, rb) = (a[c * COLUMN_STRIDE + 2], b[c * COLUMN_STRIDE + 2]);
                assert!((ra - rb).abs() < 0.05, "rms {ra} vs {rb}");
            }
        }
        // Columns finer than a block read the samples: identical
        let (mut a, mut b) = (vec![0.0f32; 8 * COLUMN_STRIDE], vec![0.0f32; 8 * COLUMN_STRIDE]);
        direct.query_columns(10.0, 7.0, &mut a);
        fast.query_columns(10.0, 7.0, &mut b);
        assert_eq!(a, b);
        // Stereo sound, or peaks of another sound: the sound is scanned, so nothing changes
        let stereo = WaveformSource::from_pcm_with_peaks(8_000, 2, Arc::clone(&store), Some(&peaks)).unwrap();
        let other = WaveformSource::from_pcm_with_peaks(44_100, 1, Arc::clone(&store), Some(&peaks)).unwrap();
        let (mut x, mut y, mut z) = (vec![0.0f32; 4 * COLUMN_STRIDE], vec![0.0f32; 4 * COLUMN_STRIDE], vec![0.0f32; 4 * COLUMN_STRIDE]);
        direct.query_columns(0.0, 512.0, &mut x);
        stereo.query_columns(0.0, 512.0, &mut y);
        other.query_columns(0.0, 512.0, &mut z);
        assert_eq!(x, y);
        assert_eq!(x, z);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn with_the_option_off_no_peak_file_is_created() {
        let _turn = SWITCH.lock().unwrap_or_else(|e| e.into_inner());
        let dir = temp("off");
        let media = dir.join("voz.wav");
        std::fs::write(&media, b"x").unwrap();
        set_create(false);
        let made = write_if_enabled(&media, 8_000, &store());
        set_create(true);
        assert!(made.is_none());
        assert!(!dir.join("voz.wav.nvpk").exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
