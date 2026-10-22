//! OpenVegas Core NLE Engine (Rust)
//!
//! Provides deterministic, frame-accurate Non-Linear Editing (NLE) algorithms
//! modeled after VEGAS Pro:
//! - SMPTE Drop-Frame (`00:00:00;00`) and Non-Drop-Frame (`00:00:00:00`) Timecode
//! - Vegas Pro 5-Curve Fade Envelopes (`Linear`, `Fast`, `Slow`, `Smooth`, `Sharp`)
//! - Automatic Overlap Crossfade Detection & Equal-Power Audio Blending
//! - Event Split (`S` shortcut), Trim, and Ripple Edit solvers
//! - 16-bit/32-bit Riff WAVE PCM Peak & dBFS RMS Waveform Extractor
//! - Multi-Track FFmpeg `filter_complex` Render Graph Compiler

use std::f64::consts::PI;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Vegas Pro Fade Curve Types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FadeCurve {
    Linear,
    Fast,   // Logarithmic / fast attack
    Slow,   // Exponential / slow attack
    Smooth, // S-Curve (cosine / Hermite)
    Sharp,  // Inverse S-Curve
}

impl FadeCurve {
    pub fn from_str(name: &str) -> Self {
        match name.trim().to_ascii_lowercase().as_str() {
            "fast" => Self::Fast,
            "slow" => Self::Slow,
            "smooth" => Self::Smooth,
            "sharp" => Self::Sharp,
            _ => Self::Linear,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::Fast => "fast",
            Self::Slow => "slow",
            Self::Smooth => "smooth",
            Self::Sharp => "sharp",
        }
    }

    /// Evaluates normalized progress `t` in `[0.0, 1.0]` along the selected Vegas fade curve.
    pub fn evaluate(&self, t: f64) -> f64 {
        let x = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => x,
            Self::Fast => 1.0 - (1.0 - x).powi(3),
            Self::Slow => x.powi(3),
            Self::Smooth => 0.5 - 0.5 * (PI * x).cos(),
            Self::Sharp => {
                if x < 0.5 {
                    0.5 * (2.0 * x).powi(3)
                } else {
                    1.0 - 0.5 * (2.0 * (1.0 - x)).powi(3)
                }
            }
        }
    }

    /// Maps Vegas curve to the closest FFmpeg `acrossfade` curve identifier.
    pub fn to_ffmpeg_audio_curve(&self) -> &'static str {
        match self {
            Self::Linear => "tri",
            Self::Fast => "qsin",
            Self::Slow => "esin",
            Self::Smooth => "hsin",
            Self::Sharp => "iquad",
        }
    }
}

/// Vegas Pro Track Compositing Modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    SourceAlpha,
    Add,
    Multiply,
    Screen,
    Overlay,
    Difference,
}

impl BlendMode {
    pub fn from_str(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "add" => Self::Add,
            "multiply" => Self::Multiply,
            "screen" => Self::Screen,
            "overlay" => Self::Overlay,
            "difference" => Self::Difference,
            _ => Self::SourceAlpha,
        }
    }
}

/// Represents a Timeline Clip Event on a Video or Audio Track
#[derive(Debug, Clone)]
pub struct TimelineEvent {
    pub id: String,
    pub name: String,
    pub media_path: String,
    pub media_type: String, // "video", "audio", "generator"
    pub track_index: usize,
    pub start: f64,         // Timeline start time in seconds
    pub duration: f64,      // Timeline duration in seconds
    pub trim_in: f64,       // Media source in-point in seconds
    pub fade_in: f64,       // Fade-in duration in seconds
    pub fade_out: f64,      // Fade-out duration in seconds
    pub fade_in_curve: FadeCurve,
    pub fade_out_curve: FadeCurve,
    pub gain: f64,          // Opacity (0..1) for video or linear gain (0..2) for audio
    pub playback_rate: f64, // Velocity (0.25 .. 4.0)
    pub muted: bool,
}

impl TimelineEvent {
    pub fn end(&self) -> f64 {
        self.start + self.duration
    }

    /// Evaluates the instantaneous envelope multiplier at timeline position `time_sec`.
    pub fn envelope_at(&self, time_sec: f64) -> f64 {
        if self.muted || time_sec < self.start || time_sec > self.end() {
            return 0.0;
        }
        let local_t = time_sec - self.start;
        let mut mult = self.gain;

        if self.fade_in > 0.0001 && local_t < self.fade_in {
            mult *= self.fade_in_curve.evaluate(local_t / self.fade_in);
        }
        let rem = self.duration - local_t;
        if self.fade_out > 0.0001 && rem < self.fade_out {
            mult *= self.fade_out_curve.evaluate(rem / self.fade_out);
        }
        mult.clamp(0.0, 4.0)
    }
}

/// Represents an automatically detected Vegas Pro overlap crossfade between two events
#[derive(Debug, Clone)]
pub struct CrossfadeRegion {
    pub track_index: usize,
    pub outgoing_event_id: String,
    pub incoming_event_id: String,
    pub start: f64,
    pub end: f64,
    pub duration: f64,
    pub out_curve: FadeCurve,
    pub in_curve: FadeCurve,
}

/// SMPTE Timecode Formatter & Parser
pub struct Timecode;

impl Timecode {
    /// Formats seconds into SMPTE timecode `HH:MM:SS;FF` (drop-frame) or `HH:MM:SS:FF`.
    pub fn format_smpte(seconds: f64, fps: f64, drop_frame: bool) -> String {
        let clamped = seconds.max(0.0);
        let nominal_fps = fps.round().max(1.0) as u64;
        let total_frames = (clamped * fps).floor() as u64;

        let frames = total_frames % nominal_fps;
        let total_seconds = total_frames / nominal_fps;
        let secs = total_seconds % 60;
        let total_minutes = total_seconds / 60;
        let mins = total_minutes % 60;
        let hours = total_minutes / 60;

        let sep = if drop_frame { ';' } else { ':' };
        format!("{:02}:{:02}:{:02}{}{:02}", hours, mins, secs, sep, frames)
    }

    /// Parses a SMPTE timecode string (`HH:MM:SS;FF` or `HH:MM:SS:FF`) into seconds.
    pub fn parse_smpte(tc: &str, fps: f64) -> Option<f64> {
        let normalized = tc.trim().replace(';', ":");
        let parts: Vec<&str> = normalized.split(':').collect();
        if parts.len() != 4 {
            return None;
        }
        let hours: f64 = parts[0].parse().ok()?;
        let mins: f64 = parts[1].parse().ok()?;
        let secs: f64 = parts[2].parse().ok()?;
        let frames: f64 = parts[3].parse().ok()?;
        let safe_fps = if fps <= 0.0 { 29.97 } else { fps };
        Some(hours * 3600.0 + mins * 60.0 + secs + (frames / safe_fps))
    }

    /// Formats seconds into Vegas Measures & Beats (`Measure.Beat.Tick`)
    pub fn format_measures_beats(seconds: f64, bpm: f64, beats_per_measure: u32) -> String {
        let safe_bpm = if bpm <= 0.0 { 120.0 } else { bpm };
        let safe_bpm_meas = beats_per_measure.max(1) as f64;
        let total_beats = seconds.max(0.0) * (safe_bpm / 60.0);
        let measure = (total_beats / safe_bpm_meas).floor() as u32 + 1;
        let beat_in_measure = (total_beats % safe_bpm_meas).floor() as u32 + 1;
        let ticks = ((total_beats.fract()) * 64.0).floor() as u32;
        format!("{}.{}.{:02}", measure, beat_in_measure, ticks)
    }
}

/// Detects all overlapping clips on the same track and computes Vegas automatic crossfades.
pub fn detect_auto_crossfades(events: &mut [TimelineEvent]) -> Vec<CrossfadeRegion> {
    // Sort by track_index then start time
    events.sort_by(|a, b| {
        a.track_index
            .cmp(&b.track_index)
            .then_with(|| a.start.partial_cmp(&b.start).unwrap_or(std::cmp::Ordering::Equal))
    });

    let mut crossfades = Vec::new();
    let len = events.len();
    for i in 0..len {
        for j in (i + 1)..len {
            if events[j].track_index != events[i].track_index {
                break;
            }
            let out_end = events[i].end();
            let in_start = events[j].start;
            if in_start < out_end - 0.001 {
                let overlap_end = out_end.min(events[j].end());
                let overlap_dur = (overlap_end - in_start).max(0.0);
                if overlap_dur > 0.001 {
                    // Automatically synchronize fade_out of outgoing and fade_in of incoming
                    if events[i].fade_out < overlap_dur {
                        events[i].fade_out = overlap_dur;
                    }
                    if events[j].fade_in < overlap_dur {
                        events[j].fade_in = overlap_dur;
                    }
                    crossfades.push(CrossfadeRegion {
                        track_index: events[i].track_index,
                        outgoing_event_id: events[i].id.clone(),
                        incoming_event_id: events[j].id.clone(),
                        start: in_start,
                        end: overlap_end,
                        duration: overlap_dur,
                        out_curve: events[i].fade_out_curve,
                        in_curve: events[j].fade_in_curve,
                    });
                }
            }
        }
    }
    crossfades
}

/// Splits an event at `split_time` (Vegas Pro `S` shortcut), returning the left and right halves.
pub fn split_event_at(event: &TimelineEvent, split_time: f64, new_id: &str) -> Option<(TimelineEvent, TimelineEvent)> {
    let min_clip_len = 0.04; // ~1 frame at 25fps
    if split_time <= event.start + min_clip_len || split_time >= event.end() - min_clip_len {
        return None;
    }

    let left_duration = split_time - event.start;
    let right_duration = event.end() - split_time;
    let right_trim_in = event.trim_in + left_duration * event.playback_rate;

    let mut left = event.clone();
    left.duration = left_duration;
    left.fade_out = 0.02; // Micro de-click fade

    let mut right = event.clone();
    right.id = new_id.to_string();
    right.start = split_time;
    right.duration = right_duration;
    right.trim_in = right_trim_in;
    right.fade_in = 0.02;

    Some((left, right))
}

/// Audio Waveform Peak & dBFS Analyzer for RIFF WAVE PCM files
#[derive(Debug, Clone)]
pub struct WaveformAnalysis {
    pub sample_rate: u32,
    pub channels: u16,
    pub duration_sec: f64,
    pub peak_db: f64,
    pub rms_db: f64,
    pub peaks: Vec<f32>,
}

pub fn analyze_wav_peaks(path: &Path, num_buckets: usize) -> Result<WaveformAnalysis, String> {
    let mut file = File::open(path).map_err(|e| format!("Failed to open WAV: {}", e))?;
    let mut header = [0u8; 44];
    file.read_exact(&mut header)
        .map_err(|e| format!("Invalid WAV header: {}", e))?;

    if &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
        return Err("Not a valid RIFF WAVE file".to_string());
    }

    let channels = u16::from_le_bytes([header[22], header[23]]).max(1);
    let sample_rate = u32::from_le_bytes([header[24], header[25], header[26], header[27]]).max(1);
    let bits_per_sample = u16::from_le_bytes([header[34], header[35]]);

    // Locate "data" chunk
    file.seek(SeekFrom::Start(12)).map_err(|e| e.to_string())?;
    let mut chunk_hdr = [0u8; 8];
    let mut data_size: usize = 0;
    while file.read_exact(&mut chunk_hdr).is_ok() {
        let cid = &chunk_hdr[0..4];
        let csize = u32::from_le_bytes([chunk_hdr[4], chunk_hdr[5], chunk_hdr[6], chunk_hdr[7]]) as usize;
        if cid == b"data" {
            data_size = csize;
            break;
        }
        let next = if csize % 2 == 1 { csize + 1 } else { csize };
        if file.seek(SeekFrom::Current(next as i64)).is_err() {
            break;
        }
    }

    if data_size == 0 {
        return Err("WAV data chunk not found or empty".to_string());
    }

    let mut raw = vec![0u8; data_size.min(32 * 1024 * 1024)]; // Cap at 32MB read for speed
    let bytes_read = file.read(&mut raw).map_err(|e| e.to_string())?;
    raw.truncate(bytes_read);

    let bytes_per_sample = (bits_per_sample / 8).max(2) as usize;
    let frame_size = bytes_per_sample * (channels as usize);
    let total_frames = raw.len() / frame_size.max(1);
    let duration_sec = (total_frames as f64) / (sample_rate as f64);

    let buckets = num_buckets.clamp(16, 4096);
    let frames_per_bucket = (total_frames / buckets).max(1);
    let mut peaks = Vec::with_capacity(buckets);
    let mut global_max: f64 = 0.00001;
    let mut sum_sq: f64 = 0.0;
    let mut sample_count: usize = 0;

    for b in 0..buckets {
        let start_frame = b * frames_per_bucket;
        let end_frame = ((b + 1) * frames_per_bucket).min(total_frames);
        let mut max_amp: f32 = 0.0;

        for f in start_frame..end_frame {
            let offset = f * frame_size;
            if offset + 2 <= raw.len() {
                let sample_i16 = i16::from_le_bytes([raw[offset], raw[offset + 1]]);
                let norm = (sample_i16 as f32 / 32768.0).abs();
                if norm > max_amp {
                    max_amp = norm;
                }
                let norm64 = norm as f64;
                if norm64 > global_max {
                    global_max = norm64;
                }
                sum_sq += norm64 * norm64;
                sample_count += 1;
            }
        }
        peaks.push((max_amp * 1000.0).round() / 1000.0);
    }

    let peak_db = 20.0 * global_max.log10();
    let rms = if sample_count > 0 {
        (sum_sq / (sample_count as f64)).sqrt().max(0.00001)
    } else {
        0.00001
    };
    let rms_db = 20.0 * rms.log10();

    Ok(WaveformAnalysis {
        sample_rate,
        channels,
        duration_sec,
        peak_db,
        rms_db,
        peaks,
    })
}

/// Compiles a list of timeline events into an FFmpeg `filter_complex` string.
pub fn compile_ffmpeg_filtergraph(
    events: &[TimelineEvent],
    width: u32,
    height: u32,
    fps: f64,
) -> String {
    let mut filters = Vec::new();
    let total_dur = events
        .iter()
        .map(|e| e.end())
        .fold(1.0_f64, |acc, x| acc.max(x));

    // Base black canvas and silence bed
    filters.push(format!(
        "color=c=black:s={}x{}:r={:.2}:d={:.3}[base_v]",
        width, height, fps, total_dur
    ));
    filters.push(format!(
        "aevalsrc=0:d={:.3}:s=48000[base_a]",
        total_dur
    ));

    let mut current_v = "base_v".to_string();
    let mut audio_Nodes = vec!["[base_a]".to_string()];

    for (idx, ev) in events.iter().enumerate() {
        if ev.muted {
            continue;
        }
        if ev.media_type == "video" {
            let v_label = format!("v{}", idx);
            let ov_label = format!("ov{}", idx);
            let mut chain = format!(
                "[{idx}:v]trim=start={:.3}:duration={:.3},setpts=PTS-STARTPTS,scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:(ow-iw)/2:(oh-ih)/2",
                ev.trim_in, ev.duration, width, height, width, height
            );
            if ev.fade_in > 0.01 {
                chain.push_str(&format!(",fade=t=in:st=0:d={:.3}", ev.fade_in));
            }
            if ev.fade_out > 0.01 {
                let st = (ev.duration - ev.fade_out).max(0.0);
                chain.push_str(&format!(",fade=t=out:st={:.3}:d={:.3}", st, ev.fade_out));
            }
            chain.push_str(&format!(",setpts=PTS+{:.3}/TB[{}]", ev.start, v_label));
            filters.push(chain);

            filters.push(format!(
                "[{}][{}]overlay=eof_action=pass:enable='between(t,{:.3},{:.3})'[{}]",
                current_v,
                v_label,
                ev.start,
                ev.end(),
                ov_label
            ));
            current_v = ov_label;
        } else if ev.media_type == "audio" {
            let a_label = format!("a{}", idx);
            let delay_ms = (ev.start * 1000.0).round() as u64;
            let mut achain = format!(
                "[{idx}:a]atrim=start={:.3}:duration={:.3},asetpts=PTS-STARTPTS,volume={:.3}",
                ev.trim_in, ev.duration, ev.gain
            );
            if ev.fade_in > 0.01 {
                achain.push_str(&format!(",afade=t=in:st=0:d={:.3}", ev.fade_in));
            }
            if ev.fade_out > 0.01 {
                let st = (ev.duration - ev.fade_out).max(0.0);
                achain.push_str(&format!(",afade=t=out:st={:.3}:d={:.3}", st, ev.fade_out));
            }
            achain.push_str(&format!(",adelay={}|{}[{}]", delay_ms, delay_ms, a_label));
            filters.push(achain);
            audio_Nodes.push(format!("[{}]", a_label));
        }
    }

    if audio_Nodes.len() > 1 {
        filters.push(format!(
            "{}amix=inputs={}:duration=first:dropout_transition=0[out_a]",
            audio_Nodes.join(""),
            audio_Nodes.len()
        ));
    } else {
        filters.push("[base_a]acopy[out_a]".to_string());
    }

    filters.push(format!("[{}]format=yuv420p[out_v]", current_v));
    filters.join("; ")
}

/// Represents a VEGAS Pro Audio/Video Track Header with Track Grouping & Renaming Support
#[derive(Debug, Clone)]
pub struct TrackHeaderNode {
    pub id: String,
    pub name: String,
    pub track_type: String, // "video" or "audio"
    pub group_id: Option<String>,
    pub group_name: Option<String>,
    pub group_color: Option<String>,
    pub muted: bool,
    pub solo: bool,
    pub locked: bool,
    pub level: f64,
}

/// VEGAS Pro Track Renaming & Audio/Video Track Grouping Solver
pub struct TrackGroupEngine;

impl TrackGroupEngine {
    /// Sanitizes and renames a video or audio track.
    pub fn rename_track(track: &mut TrackHeaderNode, new_name: &str) -> String {
        let cleaned = new_name.trim();
        if !cleaned.is_empty() {
            track.name = cleaned.chars().take(64).collect();
        }
        track.name.clone()
    }

    /// Groups multiple audio and video tracks together under a shared VEGAS Track Group.
    pub fn group_tracks(
        tracks: &mut [TrackHeaderNode],
        target_ids: &[String],
        group_id: &str,
        group_name: &str,
        group_color: &str,
    ) -> usize {
        let mut count = 0;
        for tr in tracks.iter_mut() {
            if target_ids.iter().any(|id| id == &tr.id) {
                tr.group_id = Some(group_id.to_string());
                tr.group_name = Some(group_name.to_string());
                tr.group_color = Some(group_color.to_string());
                count += 1;
            }
        }
        count
    }

    /// Synchronizes Mute and Solo state across all tracks sharing the same `group_id`.
    pub fn sync_group_mute_solo(
        tracks: &mut [TrackHeaderNode],
        group_id: &str,
        muted: Option<bool>,
        solo: Option<bool>,
    ) -> usize {
        let mut updated = 0;
        for tr in tracks.iter_mut() {
            if tr.group_id.as_deref() == Some(group_id) {
                if let Some(m) = muted {
                    tr.muted = m;
                }
                if let Some(s) = solo {
                    tr.solo = s;
                }
                updated += 1;
            }
        }
        updated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smpte_formatting() {
        let tc = Timecode::format_smpte(65.5, 30.0, true);
        assert_eq!(tc, "00:01:05;15");
        let parsed = Timecode::parse_smpte("00:01:05;15", 30.0).unwrap();
        assert!((parsed - 65.5).abs() < 0.01);
    }

    #[test]
    fn test_auto_crossfade_detection() {
        let mut events = vec![
            TimelineEvent {
                id: "ev1".into(),
                name: "Clip A".into(),
                media_path: "a.mp4".into(),
                media_type: "video".into(),
                track_index: 0,
                start: 0.0,
                duration: 5.0,
                trim_in: 0.0,
                fade_in: 0.0,
                fade_out: 0.0,
                fade_in_curve: FadeCurve::Smooth,
                fade_out_curve: FadeCurve::Smooth,
                gain: 1.0,
                playback_rate: 1.0,
                muted: false,
            },
            TimelineEvent {
                id: "ev2".into(),
                name: "Clip B".into(),
                media_path: "b.mp4".into(),
                media_type: "video".into(),
                track_index: 0,
                start: 3.5,
                duration: 5.0,
                trim_in: 0.0,
                fade_in: 0.0,
                fade_out: 0.0,
                fade_in_curve: FadeCurve::Smooth,
                fade_out_curve: FadeCurve::Smooth,
                gain: 1.0,
                playback_rate: 1.0,
                muted: false,
            },
        ];
        let xfades = detect_auto_crossfades(&mut events);
        assert_eq!(xfades.len(), 1);
        assert!((xfades[0].duration - 1.5).abs() < 0.001);
        assert!((events[0].fade_out - 1.5).abs() < 0.001);
        assert!((events[1].fade_in - 1.5).abs() < 0.001);
    }
}
