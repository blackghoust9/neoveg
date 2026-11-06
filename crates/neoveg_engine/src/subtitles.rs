//! Speech-to-Text Subtitle Generation (Feature 21).
//!
//! Formats raw speech-to-text transcripts into broadcast-compliant subtitle cues (CPL, CPS,
//! line limits, duration constraints) and integrates them onto the timeline either as native
//! project captions or as generated title events on a subtitle video track.

use serde::{Deserialize, Serialize};

use crate::captions::{Caption, Cue};
use crate::command::{EditCommand, TrackChange};
use crate::timeline::{Timeline, TrackEvent, TrackId};
use neoveg_common::transcript::TranscriptSegment;

/// Subtitle cue formatting and broadcast compliance options.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubtitleFormattingOptions {
    /// Maximum characters allowed per subtitle line (typically 32..42).
    pub max_chars_per_line: usize,
    /// Maximum lines per subtitle cue (standard is 1 or 2).
    pub max_lines: usize,
    /// Minimum duration for any subtitle cue in seconds (>= 1.0s).
    pub min_duration_s: f64,
    /// Maximum duration for any subtitle cue in seconds (<= 7.0s).
    pub max_duration_s: f64,
    /// Minimum blank gap between consecutive cues in seconds (typically 2 frames ~0.08s).
    pub gap_between_cues_s: f64,
}

impl Default for SubtitleFormattingOptions {
    fn default() -> Self {
        Self {
            max_chars_per_line: 37,
            max_lines: 2,
            min_duration_s: 1.0,
            max_duration_s: 7.0,
            gap_between_cues_s: 0.08,
        }
    }
}

/// Formats raw Whisper segments into compliant subtitle cues respecting line and character limits.
pub fn format_transcript_to_cues(
    segments: &[TranscriptSegment],
    options: &SubtitleFormattingOptions,
) -> Vec<Cue> {
    let mut raw_cues = Vec::new();

    let max_cpl = options.max_chars_per_line.clamp(20, 80);
    let max_lines = options.max_lines.clamp(1, 2);

    for seg in segments {
        let text = seg.text.trim();
        if text.is_empty() {
            continue;
        }

        let words: Vec<&str> = text.split_whitespace().collect();
        if words.is_empty() {
            continue;
        }

        let seg_dur = (seg.end_s - seg.start_s).max(0.1);

        // Group words into lines and cues
        let mut current_cue_lines: Vec<String> = Vec::new();
        let mut current_line = String::new();
        let mut cue_word_chunks: Vec<Vec<String>> = Vec::new();

        for &word in &words {
            let candidate = if current_line.is_empty() {
                word.to_string()
            } else {
                format!("{current_line} {word}")
            };

            if candidate.chars().count() <= max_cpl {
                current_line = candidate;
            } else {
                // Current line is full
                if !current_line.is_empty() {
                    current_cue_lines.push(current_line);
                }
                if current_cue_lines.len() >= max_lines {
                    cue_word_chunks.push(current_cue_lines);
                    current_cue_lines = Vec::new();
                }
                current_line = word.to_string();
            }
        }

        if !current_line.is_empty() {
            current_cue_lines.push(current_line);
        }
        if !current_cue_lines.is_empty() {
            cue_word_chunks.push(current_cue_lines);
        }

        let chunk_count = cue_word_chunks.len().max(1);
        let chunk_dur = seg_dur / chunk_count as f64;

        for (idx, lines) in cue_word_chunks.into_iter().enumerate() {
            let c_start = seg.start_s + idx as f64 * chunk_dur;
            let c_end = (c_start + chunk_dur).min(seg.end_s);
            raw_cues.push(Cue {
                start: c_start,
                end: c_end,
                text: lines.join("\n"),
                speaker: None,
                id: None,
                settings: String::new(),
            });
        }
    }

    // Enforce min/max duration and inter-cue gaps
    let mut normalized_cues: Vec<Cue> = Vec::new();

    for mut cue in raw_cues {
        let dur = (cue.end - cue.start)
            .max(options.min_duration_s)
            .min(options.max_duration_s);
        cue.end = cue.start + dur;

        if let Some(prev) = normalized_cues.last_mut() {
            let max_prev_end = cue.start - options.gap_between_cues_s;
            if prev.end > max_prev_end {
                prev.end = max_prev_end.max(prev.start + 0.1);
            }
            if cue.start < prev.end + options.gap_between_cues_s {
                cue.start = prev.end + options.gap_between_cues_s;
                cue.end = cue.start + dur;
            }
        }

        normalized_cues.push(cue);
    }

    normalized_cues
}

/// Generates timeline caption elements from speech segments.
pub fn generate_timeline_captions(
    timeline: &mut Timeline,
    segments: &[TranscriptSegment],
    options: &SubtitleFormattingOptions,
    replace_all: bool,
) -> Option<EditCommand> {
    let cues = format_transcript_to_cues(segments, options);
    if cues.is_empty() {
        return None;
    }

    let mut after = if replace_all {
        Vec::new()
    } else {
        timeline.captions.clone()
    };

    for cue in cues {
        after.push(Caption {
            id: timeline.alloc_id(),
            start: cue.start.max(0.0),
            end: cue.end,
            text: cue.text,
            speaker: cue.speaker,
        });
    }

    after.sort_by(|a, b| a.start.total_cmp(&b.start));

    let before = std::mem::replace(&mut timeline.captions, after.clone());
    Some(EditCommand::ReplaceCaptions { before, after })
}

/// Inserts video title events on a dedicated video track representing subtitle cues.
pub fn generate_title_events_on_track(
    timeline: &mut Timeline,
    target_track_id: TrackId,
    segments: &[TranscriptSegment],
    options: &SubtitleFormattingOptions,
) -> Option<EditCommand> {
    let cues = format_transcript_to_cues(segments, options);
    if cues.is_empty() {
        return None;
    }

    let before = timeline.track(target_track_id)?.events.clone();
    let mut after = before.clone();

    for cue in cues {
        let dur = (cue.end - cue.start).max(0.1);
        let ev = TrackEvent {
            id: timeline.alloc_id(),
            name: cue.text.clone(),
            start: cue.start,
            duration: dur,
            ..Default::default()
        };
        after.push(ev);
    }

    after.sort_by(|a, b| a.start.total_cmp(&b.start));
    let track = timeline.track_mut(target_track_id)?;
    track.events = after.clone();

    Some(EditCommand::ReplaceEvents {
        changes: vec![TrackChange {
            track_id: target_track_id,
            before,
            after,
        }],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::TrackKind;

    #[test]
    fn test_format_transcript_to_cues_line_wrapping() {
        let segs = vec![TranscriptSegment {
            start_s: 0.0,
            end_s: 4.0,
            text: "This is a long sentence that should wrap into multiple lines for broadcast subtitles.".into(),
        }];

        let opts = SubtitleFormattingOptions {
            max_chars_per_line: 30,
            max_lines: 2,
            min_duration_s: 1.0,
            max_duration_s: 7.0,
            gap_between_cues_s: 0.08,
        };

        let cues = format_transcript_to_cues(&segs, &opts);
        assert!(!cues.is_empty());
        for c in &cues {
            for line in c.text.lines() {
                assert!(line.chars().count() <= 30);
            }
            assert!(c.text.lines().count() <= 2);
        }
    }

    #[test]
    fn test_generate_timeline_captions() {
        let mut tl = Timeline::new();
        let segs = vec![
            TranscriptSegment {
                start_s: 1.0,
                end_s: 3.0,
                text: "Caption 1".into(),
            },
            TranscriptSegment {
                start_s: 4.0,
                end_s: 6.0,
                text: "Caption 2".into(),
            },
        ];

        let cmd = generate_timeline_captions(
            &mut tl,
            &segs,
            &SubtitleFormattingOptions::default(),
            true,
        );
        assert!(cmd.is_some());
        assert_eq!(tl.captions.len(), 2);
        assert_eq!(tl.captions[0].text, "Caption 1");
    }

    #[test]
    fn test_generate_title_events_on_track() {
        let mut tl = Timeline::new();
        let track = tl.new_track(TrackKind::Video);
        let tid = track.id;
        tl.tracks.push(track);

        let segs = vec![TranscriptSegment {
            start_s: 0.0,
            end_s: 2.0,
            text: "Subtitle Overlay".into(),
        }];

        let cmd = generate_title_events_on_track(
            &mut tl,
            tid,
            &segs,
            &SubtitleFormattingOptions::default(),
        );
        assert!(cmd.is_some());
        assert_eq!(tl.tracks[0].events.len(), 1);
        assert_eq!(tl.tracks[0].events[0].name, "Subtitle Overlay");
    }
}
