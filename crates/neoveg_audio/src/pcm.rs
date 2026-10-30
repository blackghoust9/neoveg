//! Interleaved stereo `f32` audio that may live on disk.
//!
//! A long recording decoded whole takes about 1.4 GB an hour (48 kHz, stereo), which a small
//! machine cannot spare. [`PcmWriter`] takes the decoded sound block by block and keeps it in memory
//! while it is short; past [`SPILL_FLOATS`] it moves it to an anonymous temporary file. [`PcmStore`]
//! reads either back by frame, through a few cached chunks, so playback, the waveform and the
//! rendering touch only the part of the sound they need.

use std::fs::File;
use std::io::{self, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Frames in a chunk read from the file at once (256 KiB).
pub const CHUNK_FRAMES: usize = 32 * 1024;
/// Chunks a disk store keeps in memory (1.5 MiB).
const CACHE_SLOTS: usize = 6;
/// Samples (two a frame) a writer keeps in memory before it moves to disk: 32 MiB, about 87 s of
/// 48 kHz stereo.
pub const SPILL_FLOATS: usize = 8 << 20;
/// Samples a spilled writer buffers before each write.
const WRITE_FLOATS: usize = 1 << 18;

#[cfg(windows)]
fn read_at(file: &File, buf: &mut [u8], offset: u64) -> io::Result<usize> {
    std::os::windows::fs::FileExt::seek_read(file, buf, offset)
}

#[cfg(unix)]
fn read_at(file: &File, buf: &mut [u8], offset: u64) -> io::Result<usize> {
    std::os::unix::fs::FileExt::read_at(file, buf, offset)
}

fn read_exact_at(file: &File, mut buf: &mut [u8], mut offset: u64) -> io::Result<()> {
    while !buf.is_empty() {
        match read_at(file, buf, offset) {
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(n) => {
                buf = &mut buf[n..];
                offset += n as u64;
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

/// An anonymous file that disappears when closed, even if the program dies: Windows deletes it on
/// close, elsewhere it is unlinked right after it is opened.
fn temp_file() -> io::Result<File> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join("neoveg_pcm");
    std::fs::create_dir_all(&dir)?;
    let name = format!(
        "{}-{}.f32",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let path = dir.join(name);
    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(true).create_new(true);
    #[cfg(windows)]
    {
        // FILE_FLAG_DELETE_ON_CLOSE
        std::os::windows::fs::OpenOptionsExt::custom_flags(&mut options, 0x0400_0000);
    }
    let file = options.open(&path)?;
    #[cfg(unix)]
    std::fs::remove_file(&path)?;
    Ok(file)
}

struct Slot {
    chunk: usize,
    used: u64,
    data: Vec<f32>,
}

#[derive(Default)]
struct Cache {
    slots: Vec<Slot>,
    tick: u64,
}

struct Disk {
    file: File,
    cache: Mutex<Cache>,
}

enum Backing {
    Memory(Vec<f32>),
    Disk(Disk),
}

/// Stereo audio, `frames` frames of `[left, right]`, in memory or in a temporary file.
pub struct PcmStore {
    frames: usize,
    backing: Backing,
}

impl Default for PcmStore {
    fn default() -> Self {
        Self::from_vec(Vec::new())
    }
}

impl std::fmt::Debug for PcmStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PcmStore")
            .field("frames", &self.frames)
            .field("on_disk", &self.is_on_disk())
            .finish()
    }
}

impl PcmStore {
    /// Keeps `samples` (interleaved stereo; a stray last sample is dropped) in memory.
    pub fn from_vec(mut samples: Vec<f32>) -> Self {
        samples.truncate(samples.len() / 2 * 2);
        Self {
            frames: samples.len() / 2,
            backing: Backing::Memory(samples),
        }
    }

    pub fn shared(samples: Vec<f32>) -> Arc<Self> {
        Arc::new(Self::from_vec(samples))
    }

    pub fn frames(&self) -> usize {
        self.frames
    }

    /// Samples held (two a frame).
    pub fn len(&self) -> usize {
        self.frames * 2
    }

    pub fn is_empty(&self) -> bool {
        self.frames == 0
    }

    pub fn is_on_disk(&self) -> bool {
        matches!(self.backing, Backing::Disk(_))
    }

    /// Frame `i`, silence past the end.
    pub fn frame(&self, i: usize) -> (f32, f32) {
        if i >= self.frames {
            return (0.0, 0.0);
        }
        match &self.backing {
            Backing::Memory(v) => (v[i * 2], v[i * 2 + 1]),
            Backing::Disk(d) => d.with_chunk(i / CHUNK_FRAMES, self.frames, |c| {
                let o = (i % CHUNK_FRAMES) * 2;
                (c[o], c[o + 1])
            }),
        }
    }

    /// Frame `i` and the one after it (`None` at the end), for linear interpolation.
    pub fn pair(&self, i: usize) -> ((f32, f32), Option<(f32, f32)>) {
        if i >= self.frames {
            return ((0.0, 0.0), None);
        }
        let has_next = i + 1 < self.frames;
        match &self.backing {
            Backing::Memory(v) => (
                (v[i * 2], v[i * 2 + 1]),
                has_next.then(|| (v[i * 2 + 2], v[i * 2 + 3])),
            ),
            Backing::Disk(d) => {
                let offset = i % CHUNK_FRAMES;
                if has_next && offset + 1 < CHUNK_FRAMES {
                    d.with_chunk(i / CHUNK_FRAMES, self.frames, |c| {
                        let o = offset * 2;
                        ((c[o], c[o + 1]), Some((c[o + 2], c[o + 3])))
                    })
                } else {
                    (self.frame(i), has_next.then(|| self.frame(i + 1)))
                }
            }
        }
    }

    /// Appends the frames `start..start + count` (clamped to the sound) to `out`, interleaved.
    pub fn read(&self, start: usize, count: usize, out: &mut Vec<f32>) {
        let start = start.min(self.frames);
        let end = start.saturating_add(count).min(self.frames);
        if start >= end {
            return;
        }
        match &self.backing {
            Backing::Memory(v) => out.extend_from_slice(&v[start * 2..end * 2]),
            Backing::Disk(d) => {
                out.reserve((end - start) * 2);
                let mut at = start;
                while at < end {
                    let chunk = at / CHUNK_FRAMES;
                    let stop = ((chunk + 1) * CHUNK_FRAMES).min(end);
                    d.with_chunk(chunk, self.frames, |c| {
                        let from = (at % CHUNK_FRAMES) * 2;
                        out.extend_from_slice(&c[from..from + (stop - at) * 2]);
                    });
                    at = stop;
                }
            }
        }
    }

    pub fn to_vec(&self, start: usize, count: usize) -> Vec<f32> {
        let mut out = Vec::new();
        self.read(start, count, &mut out);
        out
    }

    /// Highest absolute sample of the frames `start..end` (clamped); `0.0` for an empty range.
    pub fn peak(&self, start: usize, end: usize) -> f32 {
        let fold = |peak: f32, s: &f32| peak.max(s.abs());
        let start = start.min(self.frames);
        let end = end.clamp(start, self.frames);
        match &self.backing {
            Backing::Memory(v) => v[start * 2..end * 2].iter().fold(0.0, fold),
            Backing::Disk(d) => {
                let mut peak = 0.0_f32;
                let mut at = start;
                while at < end {
                    let chunk = at / CHUNK_FRAMES;
                    let stop = ((chunk + 1) * CHUNK_FRAMES).min(end);
                    peak = peak.max(d.with_chunk(chunk, self.frames, |c| {
                        let from = (at % CHUNK_FRAMES) * 2;
                        c[from..from + (stop - at) * 2].iter().fold(0.0, fold)
                    }));
                    at = stop;
                }
                peak
            }
        }
    }
}

impl Disk {
    /// Runs `f` on the samples of chunk number `chunk`, reading it from the file if it is not cached.
    /// A chunk that cannot be read is silence.
    fn with_chunk<R>(&self, chunk: usize, frames: usize, f: impl FnOnce(&[f32]) -> R) -> R {
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        cache.tick += 1;
        let tick = cache.tick;
        let index = match cache.slots.iter().position(|s| s.chunk == chunk) {
            Some(i) => i,
            None => {
                let first = chunk * CHUNK_FRAMES;
                let count = CHUNK_FRAMES.min(frames.saturating_sub(first));
                let mut bytes = vec![0u8; count * 8];
                if let Err(e) = read_exact_at(&self.file, &mut bytes, first as u64 * 8) {
                    tracing::warn!("cannot read the decoded sound from disk: {e}");
                    bytes.fill(0);
                }
                let data: Vec<f32> = bytes
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|b| f32::from_le_bytes(*b))
                    .collect();
                let slot = Slot {
                    chunk,
                    used: tick,
                    data,
                };
                if cache.slots.len() < CACHE_SLOTS {
                    cache.slots.push(slot);
                    cache.slots.len() - 1
                } else {
                    let oldest = (0..cache.slots.len())
                        .min_by_key(|&i| cache.slots[i].used)
                        .unwrap_or(0);
                    cache.slots[oldest] = slot;
                    oldest
                }
            }
        };
        cache.slots[index].used = tick;
        f(&cache.slots[index].data)
    }
}

/// Collects decoded sound block by block into a [`PcmStore`].
pub struct PcmWriter {
    mem: Vec<f32>,
    spill_at: usize,
    file: Option<File>,
    frames: usize,
}

impl Default for PcmWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl PcmWriter {
    pub fn new() -> Self {
        Self::with_spill(SPILL_FLOATS)
    }

    /// A writer that moves to disk once it holds `spill_at` samples.
    pub fn with_spill(spill_at: usize) -> Self {
        Self {
            mem: Vec::new(),
            spill_at: spill_at.max(2),
            file: None,
            frames: 0,
        }
    }

    /// Room for `frames` frames, so a short sound is not copied while it grows. Never more than the
    /// spill size: a lying header must not reserve gigabytes.
    pub fn reserve_frames(&mut self, frames: usize) {
        self.mem
            .reserve_exact(frames.saturating_mul(2).min(self.spill_at + 2));
    }

    /// Adds interleaved samples of `channels` channels, kept as stereo (a mono sound is mirrored,
    /// channels past the second are dropped).
    pub fn push(&mut self, samples: &[f32], channels: usize) -> io::Result<()> {
        let channels = channels.max(1);
        for frame in samples.chunks_exact(channels) {
            self.mem.push(frame[0]);
            self.mem
                .push(if channels > 1 { frame[1] } else { frame[0] });
            self.frames += 1;
        }
        if self.mem.len() >= self.spill_at {
            self.flush()?;
        }
        Ok(())
    }

    /// Moves what is buffered to the file, opening it the first time. After that the writer only
    /// buffers `WRITE_FLOATS` samples at a time.
    fn flush(&mut self) -> io::Result<()> {
        if self.file.is_none() {
            self.file = Some(temp_file()?);
            self.spill_at = WRITE_FLOATS;
        }
        let Some(file) = self.file.as_mut() else {
            return Ok(());
        };
        let mut bytes = Vec::with_capacity(WRITE_FLOATS.min(self.mem.len()) * 4);
        for piece in self.mem.chunks(WRITE_FLOATS) {
            bytes.clear();
            for v in piece {
                bytes.extend_from_slice(&v.to_le_bytes());
            }
            file.write_all(&bytes)?;
        }
        self.mem.clear();
        self.mem.shrink_to(WRITE_FLOATS);
        Ok(())
    }

    pub fn is_on_disk(&self) -> bool {
        self.file.is_some()
    }

    pub fn frames(&self) -> usize {
        self.frames
    }

    pub fn finish(mut self) -> io::Result<PcmStore> {
        if self.file.is_none() {
            self.mem.shrink_to_fit();
            return Ok(PcmStore::from_vec(self.mem));
        }
        self.flush()?;
        let file = self.file.take().ok_or(io::ErrorKind::Other)?;
        file.sync_data().ok();
        Ok(PcmStore {
            frames: self.frames,
            backing: Backing::Disk(Disk {
                file,
                cache: Mutex::new(Cache::default()),
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `frames` stereo frames where frame `i` is `(i, -i)` scaled to stay small.
    fn ramp(frames: usize) -> Vec<f32> {
        (0..frames)
            .flat_map(|i| [i as f32 * 1e-6, -(i as f32) * 1e-6])
            .collect()
    }

    fn on_disk(samples: &[f32]) -> PcmStore {
        let mut w = PcmWriter::with_spill(1000);
        // Odd block sizes, so a block never lines up with a chunk
        for block in samples.chunks(2 * 777) {
            w.push(block, 2).unwrap();
        }
        assert!(w.is_on_disk());
        w.finish().unwrap()
    }

    #[test]
    fn a_short_sound_stays_in_memory_and_a_long_one_goes_to_disk_with_the_same_content() {
        let frames = CHUNK_FRAMES * 3 + 123;
        let samples = ramp(frames);
        let mut small = PcmWriter::new();
        small.push(&samples, 2).unwrap();
        let small = small.finish().unwrap();
        assert!(!small.is_on_disk());

        let big = on_disk(&samples);
        assert!(big.is_on_disk());
        assert_eq!(big.frames(), frames);
        assert_eq!(small.frames(), frames);
        for i in [
            0,
            1,
            CHUNK_FRAMES - 1,
            CHUNK_FRAMES,
            CHUNK_FRAMES + 1,
            2 * CHUNK_FRAMES - 1,
            frames - 2,
            frames - 1,
        ] {
            assert_eq!(big.frame(i), small.frame(i), "frame {i}");
            assert_eq!(big.pair(i), small.pair(i), "pair {i}");
        }
        // Past the end is silence
        assert_eq!(big.frame(frames), (0.0, 0.0));
        assert_eq!(big.pair(frames - 1).1, None);
    }

    #[test]
    fn ranges_and_peaks_cross_chunk_boundaries_and_clamp_to_the_end() {
        let frames = CHUNK_FRAMES * 2 + 50;
        let samples = ramp(frames);
        let big = on_disk(&samples);
        let small = PcmStore::from_vec(samples.clone());
        for (start, count) in [
            (0, 10),
            (CHUNK_FRAMES - 5, 20),
            (5, CHUNK_FRAMES * 2),
            (frames - 3, 100),
            (frames + 10, 5),
        ] {
            assert_eq!(
                big.to_vec(start, count),
                small.to_vec(start, count),
                "{start}+{count}"
            );
        }
        assert_eq!(big.to_vec(0, frames), samples);
        assert_eq!(big.peak(0, frames), small.peak(0, frames));
        assert_eq!(
            big.peak(CHUNK_FRAMES - 2, CHUNK_FRAMES + 2),
            small.peak(CHUNK_FRAMES - 2, CHUNK_FRAMES + 2)
        );
        assert_eq!(big.peak(10, 10), 0.0);
        assert_eq!(big.peak(frames, frames + 5), 0.0);
    }

    #[test]
    fn mono_is_mirrored_extra_channels_are_dropped_and_an_empty_writer_is_empty() {
        let mut w = PcmWriter::new();
        w.push(&[0.5, -0.25], 1).unwrap();
        w.push(&[0.1, 0.2, 0.9, 0.3, 0.4, 0.9], 3).unwrap();
        let s = w.finish().unwrap();
        assert_eq!(
            s.to_vec(0, 10),
            vec![0.5, 0.5, -0.25, -0.25, 0.1, 0.2, 0.3, 0.4]
        );
        let empty = PcmWriter::new().finish().unwrap();
        assert!(empty.is_empty());
        assert_eq!(empty.frame(0), (0.0, 0.0));
        assert!(PcmStore::default().is_empty());
    }
}
