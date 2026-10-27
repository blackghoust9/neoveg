//! Sample-accurate waveform source.
//!
//! Audacity-style waveform data for the timeline at any zoom level:
//! - Holds the PCM payload of a RIFF WAVE file (PCM 8/16/24/32-bit, IEEE float 32/64-bit,
//!   WAVE_FORMAT_EXTENSIBLE) or decoded interleaved `f32` samples (Left/Right; mono is mirrored).
//! - Keeps a summary level of `SUMMARY_BLOCK_FRAMES` frames per block (min, max, sum of squares)
//!   so a pixel column covering millions of samples is answered in O(blocks) instead of O(samples).
//! - `query_columns` returns the **exact** min/max/RMS of every pixel column (no interpolation,
//!   no resampled peaks), and `read_samples` returns the raw samples for sample-level zoom.

/// Frames per summary block (Audacity uses 256-sample summaries for its first level).
pub const SUMMARY_BLOCK_FRAMES: usize = 256;

/// Values written per column by [`WaveformSource::query_columns`]:
/// `min_l, max_l, rms_l, min_r, max_r, rms_r`.
pub const COLUMN_STRIDE: usize = 6;

#[derive(Debug, Clone, Copy, Default)]
struct ChannelSummary {
    min: f32,
    max: f32,
    sum_sq: f64,
}

#[derive(Debug, Clone, Copy, Default)]
struct ColumnAccumulator {
    min: f32,
    max: f32,
    sum_sq: f64,
    count: usize,
}

impl ColumnAccumulator {
    fn new() -> Self {
        Self {
            min: f32::MAX,
            max: f32::MIN,
            sum_sq: 0.0,
            count: 0,
        }
    }

    fn add_sample(&mut self, v: f32) {
        self.min = self.min.min(v);
        self.max = self.max.max(v);
        self.sum_sq += (v as f64) * (v as f64);
        self.count += 1;
    }

    fn add_summary(&mut self, s: &ChannelSummary, frames: usize) {
        self.min = self.min.min(s.min);
        self.max = self.max.max(s.max);
        self.sum_sq += s.sum_sq;
        self.count += frames;
    }

    fn finish(&self) -> (f32, f32, f32) {
        if self.count == 0 {
            return (0.0, 0.0, 0.0);
        }
        let rms = (self.sum_sq / self.count as f64).sqrt() as f32;
        (self.min, self.max, rms)
    }
}

/// Audio file kept as its original PCM bytes (memory = file size) plus a summary level for fast
/// column queries; samples are decoded on access.
pub struct WaveformSource {
    pub sample_rate: u32,
    pub channels: u16,
    data: Vec<u8>,
    frames: usize,
    frame_size: usize,
    bytes_per_sample: usize,
    right_offset: usize,
    is_float: bool,
    summary_left: Vec<ChannelSummary>,
    summary_right: Vec<ChannelSummary>,
}

impl WaveformSource {
    pub fn frame_count(&self) -> usize {
        self.frames
    }

    /// Decoded sample of channel 0 (left) or 1 (right; mirrors left on mono files).
    #[inline]
    fn sample(&self, channel: usize, frame: usize) -> f32 {
        let offset = frame * self.frame_size + if channel == 0 { 0 } else { self.right_offset };
        decode_sample(&self.data[offset..], self.bytes_per_sample, self.is_float)
    }

    pub fn from_wav_bytes(mut bytes: Vec<u8>) -> Result<Self, String> {
        if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
            return Err("not a RIFF WAVE file".into());
        }

        let mut format_tag: u16 = 0;
        let mut channels: u16 = 0;
        let mut sample_rate: u32 = 0;
        let mut bits: u16 = 0;
        let mut block_align: u16 = 0;
        let mut data_range: Option<(usize, usize)> = None;

        let mut pos = 12usize;
        while pos + 8 <= bytes.len() {
            let id = &bytes[pos..pos + 4];
            let size = u32::from_le_bytes([
                bytes[pos + 4],
                bytes[pos + 5],
                bytes[pos + 6],
                bytes[pos + 7],
            ]) as usize;
            let body_start = pos + 8;
            // Tolerate truncated files / streaming headers (data size 0 or 0xFFFFFFFF)
            let body_end = if size == 0 || size == u32::MAX as usize {
                bytes.len()
            } else {
                body_start.saturating_add(size).min(bytes.len())
            };
            let body = &bytes[body_start..body_end];
            let body_range = (body_start, body_end);

            if id == b"fmt " && body.len() >= 16 {
                format_tag = u16::from_le_bytes([body[0], body[1]]);
                channels = u16::from_le_bytes([body[2], body[3]]);
                sample_rate = u32::from_le_bytes([body[4], body[5], body[6], body[7]]);
                block_align = u16::from_le_bytes([body[12], body[13]]);
                bits = u16::from_le_bytes([body[14], body[15]]);
                // WAVE_FORMAT_EXTENSIBLE: the real format is the first 2 bytes of the SubFormat GUID
                if format_tag == 0xFFFE && body.len() >= 26 {
                    format_tag = u16::from_le_bytes([body[24], body[25]]);
                }
            } else if id == b"data" {
                data_range = Some(body_range);
                if channels != 0 {
                    break;
                }
            }

            pos = body_start.saturating_add(size + (size & 1));
            if size == 0 || size == u32::MAX as usize {
                break;
            }
        }

        let (data_start, data_end) = data_range.ok_or("WAV data chunk not found")?;
        if channels == 0 || sample_rate == 0 || bits == 0 {
            return Err("WAV fmt chunk missing or invalid".into());
        }
        let bytes_per_sample = (bits as usize).div_ceil(8);
        let frame_size = if block_align as usize >= bytes_per_sample * channels as usize {
            block_align as usize
        } else {
            bytes_per_sample * channels as usize
        };
        let is_float = format_tag == 3;
        if !(format_tag == 1 || is_float) {
            return Err(format!("unsupported WAV format tag {format_tag}"));
        }

        // Keep only the PCM payload (drop the header and trailing chunks without copying)
        bytes.truncate(data_end);
        bytes.drain(..data_start);
        bytes.shrink_to_fit();
        let frames = bytes.len() / frame_size;
        let right_offset = if channels > 1 { bytes_per_sample } else { 0 };

        let mut src = Self {
            sample_rate,
            channels,
            data: bytes,
            frames,
            frame_size,
            bytes_per_sample,
            right_offset,
            is_float,
            summary_left: Vec::new(),
            summary_right: Vec::new(),
        };
        src.summary_left = src.build_summary(0);
        src.summary_right = src.build_summary(1);
        Ok(src)
    }

    fn build_summary(&self, channel: usize) -> Vec<ChannelSummary> {
        (0..self.frames.div_ceil(SUMMARY_BLOCK_FRAMES))
            .map(|block| {
                let start = block * SUMMARY_BLOCK_FRAMES;
                let end = (start + SUMMARY_BLOCK_FRAMES).min(self.frames);
                let mut s = ChannelSummary {
                    min: f32::MAX,
                    max: f32::MIN,
                    sum_sq: 0.0,
                };
                for f in start..end {
                    let v = self.sample(channel, f);
                    s.min = s.min.min(v);
                    s.max = s.max.max(v);
                    s.sum_sq += (v as f64) * (v as f64);
                }
                s
            })
            .collect()
    }

    /// Exact min/max/RMS per pixel column. Column `c` covers source frames
    /// `[start_frame + c * frames_per_column, start_frame + (c + 1) * frames_per_column)`.
    /// Writes `columns * COLUMN_STRIDE` floats into `out`; columns outside the file are zero.
    pub fn query_columns(&self, start_frame: f64, frames_per_column: f64, out: &mut [f32]) {
        let columns = out.len() / COLUMN_STRIDE;
        let total = self.frame_count();
        let fpc = frames_per_column.max(1e-9);
        for c in 0..columns {
            let a = start_frame + c as f64 * fpc;
            let b = a + fpc;
            let first = a.floor().max(0.0) as usize;
            // Every column covers at least one sample so narrow columns never show gaps
            let last = (b.ceil().max(a.floor() + 1.0)).max(0.0) as usize;
            let (l, r) = if first >= total || b <= 0.0 {
                ((0.0, 0.0, 0.0), (0.0, 0.0, 0.0))
            } else {
                let last = last.min(total);
                (
                    self.scan_range(0, &self.summary_left, first, last),
                    self.scan_range(1, &self.summary_right, first, last),
                )
            };
            let o = &mut out[c * COLUMN_STRIDE..(c + 1) * COLUMN_STRIDE];
            o[0] = l.0;
            o[1] = l.1;
            o[2] = l.2;
            o[3] = r.0;
            o[4] = r.1;
            o[5] = r.2;
        }
    }

    fn scan_range(
        &self,
        channel: usize,
        summary: &[ChannelSummary],
        first: usize,
        last: usize,
    ) -> (f32, f32, f32) {
        let mut acc = ColumnAccumulator::new();
        let mut f = first;
        while f < last {
            let block = f / SUMMARY_BLOCK_FRAMES;
            let block_start = block * SUMMARY_BLOCK_FRAMES;
            let block_end = (block_start + SUMMARY_BLOCK_FRAMES).min(self.frames);
            if f == block_start && block_end <= last && block < summary.len() {
                acc.add_summary(&summary[block], block_end - block_start);
                f = block_end;
            } else {
                let end = block_end.min(last);
                for frame in f..end {
                    acc.add_sample(self.sample(channel, frame));
                }
                f = end;
            }
        }
        acc.finish()
    }

    /// Copies raw samples `[start_frame, start_frame + count)` (clamped to the file) into `out_l` / `out_r`.
    /// Returns the number of frames copied.
    pub fn read_samples(&self, start_frame: i64, out_l: &mut [f32], out_r: &mut [f32]) -> usize {
        let total = self.frame_count() as i64;
        if start_frame >= total {
            return 0;
        }
        let first = start_frame.max(0) as usize;
        let skip = (first as i64 - start_frame) as usize;
        let count = out_l.len().min(out_r.len());
        if skip >= count {
            return 0;
        }
        let n = (count - skip).min(self.frame_count() - first);
        for i in 0..n {
            out_l[skip + i] = self.sample(0, first + i);
            out_r[skip + i] = self.sample(1, first + i);
        }
        n
    }
}

fn decode_sample(bytes: &[u8], bytes_per_sample: usize, is_float: bool) -> f32 {
    if bytes.len() < bytes_per_sample {
        return 0.0;
    }
    let v = match (bytes_per_sample, is_float) {
        (4, true) => f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        (8, true) => f64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]) as f32,
        (1, false) => (bytes[0] as f32 - 128.0) / 128.0,
        (2, false) => i16::from_le_bytes([bytes[0], bytes[1]]) as f32 / 32768.0,
        (3, false) => {
            let raw = (bytes[0] as i32) | ((bytes[1] as i32) << 8) | ((bytes[2] as i32) << 16);
            ((raw << 8) >> 8) as f32 / 8_388_608.0
        }
        (4, false) => {
            i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as f32 / 2_147_483_648.0
        }
        _ => 0.0,
    };
    if v.is_finite() {
        v.clamp(-1.0, 1.0)
    } else {
        0.0
    }
}

impl WaveformSource {
    /// Builds a source from decoded interleaved `f32` samples (1 or 2+ channels; only the first two are kept).
    pub fn from_interleaved_f32(
        sample_rate: u32,
        channels: u16,
        interleaved: &[f32],
    ) -> Result<Self, String> {
        if channels == 0 || sample_rate == 0 {
            return Err("invalid channel count or sample rate".into());
        }
        let ch = channels as usize;
        let frames = interleaved.len() / ch;
        let kept = ch.min(2);
        let mut data = Vec::with_capacity(frames * kept * 4);
        for frame in interleaved.chunks_exact(ch) {
            for &v in &frame[..kept] {
                data.extend_from_slice(&v.to_le_bytes());
            }
        }
        let mut src = Self {
            sample_rate,
            channels: kept as u16,
            data,
            frames,
            frame_size: kept * 4,
            bytes_per_sample: 4,
            right_offset: if kept > 1 { 4 } else { 0 },
            is_float: true,
            summary_left: Vec::new(),
            summary_right: Vec::new(),
        };
        src.summary_left = src.build_summary(0);
        src.summary_right = src.build_summary(1);
        Ok(src)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav_16bit_stereo(samples: &[(i16, i16)], rate: u32) -> Vec<u8> {
        let data_len = samples.len() * 4;
        let mut v = Vec::new();
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&((36 + data_len) as u32).to_le_bytes());
        v.extend_from_slice(b"WAVEfmt ");
        v.extend_from_slice(&16u32.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&2u16.to_le_bytes());
        v.extend_from_slice(&rate.to_le_bytes());
        v.extend_from_slice(&(rate * 4).to_le_bytes());
        v.extend_from_slice(&4u16.to_le_bytes());
        v.extend_from_slice(&16u16.to_le_bytes());
        v.extend_from_slice(b"data");
        v.extend_from_slice(&(data_len as u32).to_le_bytes());
        for &(l, r) in samples {
            v.extend_from_slice(&l.to_le_bytes());
            v.extend_from_slice(&r.to_le_bytes());
        }
        v
    }

    #[test]
    fn column_query_matches_brute_force_exactly() {
        // 10_000 frames of a deterministic signal with a single spike at frame 4321
        let samples: Vec<(i16, i16)> = (0..10_000)
            .map(|i| {
                let l = if i == 4321 {
                    32767
                } else {
                    ((i * 37) % 2000) as i16 - 1000
                };
                (l, -l / 2)
            })
            .collect();
        let src = WaveformSource::from_wav_bytes(wav_16bit_stereo(&samples, 48_000)).unwrap();
        assert_eq!(src.frame_count(), 10_000);

        let fpc = 777.3;
        let columns = 12;
        let mut out = vec![0f32; columns * COLUMN_STRIDE];
        src.query_columns(0.0, fpc, &mut out);

        for c in 0..columns {
            let a = (c as f64 * fpc).floor() as usize;
            let b = (((c + 1) as f64 * fpc).ceil() as usize).min(10_000);
            if a >= 10_000 {
                continue;
            }
            let slice = &samples[a..b];
            let min = slice
                .iter()
                .map(|s| s.0 as f32 / 32768.0)
                .fold(f32::MAX, f32::min);
            let max = slice
                .iter()
                .map(|s| s.0 as f32 / 32768.0)
                .fold(f32::MIN, f32::max);
            assert_eq!(out[c * COLUMN_STRIDE], min, "min of column {c}");
            assert_eq!(out[c * COLUMN_STRIDE + 1], max, "max of column {c}");
        }
        // The single-sample spike must survive any zoom level (never averaged away)
        let mut one = vec![0f32; COLUMN_STRIDE];
        src.query_columns(0.0, 10_000.0, &mut one);
        assert_eq!(one[1], 32767.0 / 32768.0);
    }

    #[test]
    fn read_samples_returns_raw_values_and_clamps_edges() {
        let samples: Vec<(i16, i16)> = (0..100)
            .map(|i| (i as i16 * 100, -(i as i16) * 100))
            .collect();
        let src = WaveformSource::from_wav_bytes(wav_16bit_stereo(&samples, 44_100)).unwrap();
        let mut l = vec![9f32; 5];
        let mut r = vec![9f32; 5];
        l.fill(0.0);
        r.fill(0.0);
        let n = src.read_samples(-2, &mut l, &mut r);
        assert_eq!(n, 3);
        assert_eq!(l, vec![0.0, 0.0, 0.0, 100.0 / 32768.0, 200.0 / 32768.0]);
        assert_eq!(r[4], -200.0 / 32768.0);
    }

    #[test]
    fn interleaved_f32_source_matches_its_input() {
        let samples: Vec<f32> = (0..1000)
            .flat_map(|i| [i as f32 / 1000.0, -(i as f32) / 1000.0])
            .collect();
        let src = WaveformSource::from_interleaved_f32(48_000, 2, &samples).unwrap();
        assert_eq!(src.frame_count(), 1000);
        let mut out = vec![0f32; COLUMN_STRIDE];
        src.query_columns(0.0, 1000.0, &mut out);
        assert_eq!(out[0], 0.0);
        assert_eq!(out[1], 0.999);
        assert_eq!(out[3], -0.999);
    }
}
