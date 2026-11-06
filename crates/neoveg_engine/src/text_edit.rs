//! Text-Based Timeline Editing (Feature 20).
//!
//! Synchronizes speech transcription words with timeline audio/video events, allowing
//! deleting, trimming, and reordering media directly by editing transcript text,
//! with automatic ripple gap closure.

use serde::{Deserialize, Serialize};

use crate::command::{EditCommand, TrackChange};
use crate::timeline::{EventId, Timeline, TrackEvent, TrackId, TIME_EPSILON};
use neoveg_common::transcript::TranscriptSegment;

/// A single word token linked to a specific timeline event and time range.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranscriptWord {
    pub word: String,
    pub start_s: f64,
    pub end_s: f64,
    pub track_id: TrackId,
    pub event_id: EventId,
}

/// A time range marked for deletion or rearrangement from text selection.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TextCutRange {
    pub start_s: f64,
    pub end_s: f64,
}

impl TextCutRange {
    pub fn new(start_s: f64, end_s: f64) -> Self {
        Self {
            start_s: start_s.min(end_s),
            end_s: start_s.max(end_s),
        }
    }

    pub fn duration(&self) -> f64 {
        (self.end_s - self.start_s).max(0.0)
    }
}

/// Extracts word-level items mapped to timeline events from Whisper transcript segments.
pub fn parse_words_from_segments(
    segments: &[TranscriptSegment],
    track_id: TrackId,
    event_id: EventId,
) -> Vec<TranscriptWord> {
    let mut words = Vec::new();

    for seg in segments {
        let seg_text = seg.text.trim();
        if seg_text.is_empty() {
            continue;
        }

        let tokens: Vec<&str> = seg_text.split_whitespace().collect();
        if tokens.is_empty() {
            continue;
        }

        let seg_dur = (seg.end_s - seg.start_s).max(1e-4);
        let word_dur = seg_dur / tokens.len() as f64;

        for (idx, &token) in tokens.iter().enumerate() {
            let start = seg.start_s + idx as f64 * word_dur;
            let end = (start + word_dur).min(seg.end_s);

            words.push(TranscriptWord {
                word: token.to_string(),
                start_s: start,
                end_s: end,
                track_id,
                event_id,
            });
        }
    }

    words
}

/// Deletes timeline media corresponding to transcript cut ranges, automatically splitting
/// events at cut boundaries and rippling subsequent events if `auto_ripple` is true.
pub fn delete_transcript_ranges(
    timeline: &mut Timeline,
    cuts: &[TextCutRange],
    auto_ripple: bool,
) -> Option<EditCommand> {
    if cuts.is_empty() || timeline.tracks.is_empty() {
        return None;
    }

    // Merge overlapping or adjacent cut intervals
    let mut sorted_cuts = cuts.to_vec();
    sorted_cuts.sort_by(|a, b| a.start_s.total_cmp(&b.start_s));

    let mut merged: Vec<TextCutRange> = Vec::new();
    for cut in sorted_cuts {
        if cut.duration() <= TIME_EPSILON {
            continue;
        }
        match merged.last_mut() {
            Some(last) if cut.start_s <= last.end_s + TIME_EPSILON => {
                last.end_s = last.end_s.max(cut.end_s);
            }
            _ => merged.push(cut),
        }
    }

    if merged.is_empty() {
        return None;
    }

    let mut changes = Vec::new();

    for track in &mut timeline.tracks {
        let before = track.events.clone();
        let mut new_events: Vec<TrackEvent> = Vec::new();

        for event in &track.events {
            let ev_start = event.start;
            let ev_end = event.end();

            // Check how event interacts with merged cut spans
            let mut surviving_spans = vec![(ev_start, ev_end)];

            for cut in &merged {
                let mut next_spans = Vec::new();
                for (s, e) in surviving_spans {
                    if cut.end_s <= s + TIME_EPSILON || cut.start_s >= e - TIME_EPSILON {
                        // No overlap
                        next_spans.push((s, e));
                    } else {
                        // Overlap exists: retain non-overlapping left and right portions
                        if cut.start_s > s + TIME_EPSILON {
                            next_spans.push((s, cut.start_s));
                        }
                        if cut.end_s < e - TIME_EPSILON {
                            next_spans.push((cut.end_s, e));
                        }
                    }
                }
                surviving_spans = next_spans;
            }

            for (span_start, span_end) in surviving_spans {
                let dur = span_end - span_start;
                if dur <= TIME_EPSILON {
                    continue;
                }
                let mut piece = event.clone();
                let offset_from_orig = span_start - ev_start;
                piece.start = span_start;
                piece.duration = dur;
                piece.trim_in += offset_from_orig * event.playback_rate;
                new_events.push(piece);
            }
        }

        if auto_ripple {
            // Ripple shift all events: for each cut, events starting after cut.end_s move earlier by cut.duration()
            for cut in merged.iter().rev() {
                let cut_dur = cut.duration();
                for ev in &mut new_events {
                    if ev.start >= cut.end_s - TIME_EPSILON {
                        ev.start = (ev.start - cut_dur).max(0.0);
                    }
                }
            }
        }

        new_events.sort_by(|a, b| a.start.total_cmp(&b.start));

        if before != new_events {
            changes.push(TrackChange {
                track_id: track.id,
                before,
                after: new_events.clone(),
            });
            track.events = new_events;
        }
    }

    if changes.is_empty() {
        None
    } else {
        Some(EditCommand::ReplaceEvents { changes })
    }
}

/// Reorders a timeline section identified by `from` range to a new timestamp `to_time`.
pub fn reorder_transcript_range(
    timeline: &mut Timeline,
    from: TextCutRange,
    to_time: f64,
) -> Option<EditCommand> {
    if from.duration() <= TIME_EPSILON || timeline.tracks.is_empty() {
        return None;
    }

    let cut_dur = from.duration();
    let mut changes = Vec::new();

    for track in &mut timeline.tracks {
        let before = track.events.clone();
        let mut extracted = Vec::new();
        let mut remaining = Vec::new();

        for ev in &track.events {
            if ev.start >= from.start_s - TIME_EPSILON && ev.end() <= from.end_s + TIME_EPSILON {
                let mut moved = ev.clone();
                let rel = ev.start - from.start_s;
                moved.start = to_time + rel;
                extracted.push(moved);
            } else {
                let mut rem = ev.clone();
                // Ripple close original gap
                if rem.start >= from.end_s - TIME_EPSILON {
                    rem.start = (rem.start - cut_dur).max(0.0);
                }
                // Ripple open space at destination
                if rem.start >= to_time - TIME_EPSILON {
                    rem.start += cut_dur;
                }
                remaining.push(rem);
            }
        }

        remaining.extend(extracted);
        remaining.sort_by(|a, b| a.start.total_cmp(&b.start));

        if before != remaining {
            changes.push(TrackChange {
                track_id: track.id,
                before,
                after: remaining.clone(),
            });
            track.events = remaining;
        }
    }

    if changes.is_empty() {
        None
    } else {
        Some(EditCommand::ReplaceEvents { changes })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::TrackKind;

    #[test]
    fn test_parse_words_from_segments() {
        let segments = vec![TranscriptSegment {
            start_s: 0.0,
            end_s: 2.0,
            text: "Hello Neoveg Editor".into(),
        }];
        let words = parse_words_from_segments(&segments, 1, 10);
        assert_eq!(words.len(), 3);
        assert_eq!(words[0].word, "Hello");
        assert_eq!(words[1].word, "Neoveg");
        assert_eq!(words[2].word, "Editor");
        assert!((words[0].start_s - 0.0).abs() < 1e-4);
        assert!((words[2].end_s - 2.0).abs() < 1e-4);
    }

    #[test]
    fn test_delete_transcript_ranges_with_ripple() {
        let mut tl = Timeline::new();
        let mut track = tl.new_track(TrackKind::Video);
        let ev = TrackEvent {
            id: 1,
            start: 0.0,
            duration: 10.0,
            ..TrackEvent::default()
        };
        track.events.push(ev);
        tl.tracks.push(track);

        // Cut out [2.0, 5.0] (3.0s cut) with auto ripple
        let cuts = vec![TextCutRange::new(2.0, 5.0)];
        let cmd = delete_transcript_ranges(&mut tl, &cuts, true);
        assert!(cmd.is_some());

        let evs = &tl.tracks[0].events;
        assert_eq!(evs.len(), 2);
        // First piece: [0.0..2.0]
        assert_eq!(evs[0].start, 0.0);
        assert_eq!(evs[0].duration, 2.0);
        // Second piece (rippled): was [5.0..10.0], now shifted earlier by 3.0s -> [2.0..7.0]
        assert_eq!(evs[1].start, 2.0);
        assert_eq!(evs[1].duration, 5.0);
    }
}
