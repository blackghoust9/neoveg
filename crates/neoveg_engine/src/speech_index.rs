//! Spoken-word search: an inverted index over the transcripts speech recognition produces for the
//! media of a project. It finds where a word or a phrase is said in each media and where that
//! lands on the timeline, so the playhead can jump to it.
//!
//! The transcript gives timed segments; the words of a segment share its time evenly, the same
//! way the text editing does (`text_edit::parse_words_from_segments`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::timeline::Timeline;
use neoveg_common::transcript::TranscriptSegment;

/// One word of a transcript.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexedWord {
    /// The word as spoken, with its punctuation.
    pub text: String,
    /// Lowercase, without accents or punctuation: what the search compares.
    pub key: String,
    pub start_s: f64,
    pub end_s: f64,
}

/// A place where the searched words are said.
#[derive(Debug, Clone, PartialEq)]
pub struct SpeechHit {
    pub media_id: u64,
    /// Position in the media of the first word found.
    pub start_s: f64,
    pub end_s: f64,
    /// Index of the first word in that media's transcript.
    pub word_index: usize,
    /// Some words around the hit, for the results list.
    pub context: String,
}

/// Normalises a word for comparison: lowercase, accents and punctuation removed.
pub fn normalize_word(word: &str) -> String {
    word.chars()
        .filter_map(|c| {
            let c = match c {
                'á' | 'à' | 'â' | 'ã' | 'ä' | 'Á' | 'À' | 'Â' | 'Ã' | 'Ä' => 'a',
                'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
                'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'i',
                'ó' | 'ò' | 'ô' | 'õ' | 'ö' | 'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => 'o',
                'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'u',
                'ç' | 'Ç' => 'c',
                'ñ' | 'Ñ' => 'n',
                other => other,
            };
            if c.is_alphanumeric() {
                Some(c.to_lowercase().next().unwrap_or(c))
            } else {
                None
            }
        })
        .collect()
}

/// Splits a text in normalised words (words with nothing left after normalising are dropped).
fn query_keys(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .map(normalize_word)
        .filter(|k| !k.is_empty())
        .collect()
}

/// The words of `segments`, each with its share of the segment's time.
pub fn words_of_segments(segments: &[TranscriptSegment]) -> Vec<IndexedWord> {
    let mut words = Vec::new();
    for seg in segments {
        let tokens: Vec<&str> = seg.text.split_whitespace().collect();
        if tokens.is_empty() {
            continue;
        }
        let dur = (seg.end_s - seg.start_s).max(1e-4);
        let each = dur / tokens.len() as f64;
        for (i, token) in tokens.iter().enumerate() {
            let key = normalize_word(token);
            if key.is_empty() {
                continue;
            }
            let start = seg.start_s + i as f64 * each;
            words.push(IndexedWord {
                text: (*token).to_string(),
                key,
                start_s: start,
                end_s: (start + each).min(seg.end_s.max(start)),
            });
        }
    }
    words
}

/// The words said in the media of a project, searchable.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(from = "SpeechIndexData")]
pub struct SpeechIndex {
    /// The words of each media, in speech order.
    docs: BTreeMap<u64, Vec<IndexedWord>>,
    /// Normalised word -> where it is (media, index of the word); rebuilt on every change.
    #[serde(skip)]
    postings: BTreeMap<String, Vec<(u64, usize)>>,
}

/// What is stored of an index: the inverted part is rebuilt on loading.
#[derive(Deserialize)]
struct SpeechIndexData {
    #[serde(default)]
    docs: BTreeMap<u64, Vec<IndexedWord>>,
}

impl From<SpeechIndexData> for SpeechIndex {
    fn from(data: SpeechIndexData) -> Self {
        let mut index = SpeechIndex {
            docs: data.docs,
            postings: BTreeMap::new(),
        };
        index.rebuild();
        index
    }
}

impl SpeechIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.docs.is_empty()
    }

    /// Indexes (or re-indexes) the transcript of a media.
    pub fn add_media(&mut self, media_id: u64, segments: &[TranscriptSegment]) {
        self.docs.insert(media_id, words_of_segments(segments));
        self.rebuild();
    }

    pub fn remove_media(&mut self, media_id: u64) {
        if self.docs.remove(&media_id).is_some() {
            self.rebuild();
        }
    }

    pub fn media_count(&self) -> usize {
        self.docs.len()
    }

    pub fn word_count(&self) -> usize {
        self.docs.values().map(Vec::len).sum()
    }

    /// Rebuilds the inverted index (needed after loading a saved index).
    pub fn rebuild(&mut self) {
        self.postings.clear();
        for (&media, words) in &self.docs {
            for (i, w) in words.iter().enumerate() {
                self.postings
                    .entry(w.key.clone())
                    .or_default()
                    .push((media, i));
            }
        }
    }

    /// Where `query` is said: one word, or several words one after the other. Hits come in order
    /// of media and then of time. With `prefix`, the last word of the query also matches the words
    /// that start with it.
    pub fn search(&self, query: &str, prefix: bool) -> Vec<SpeechHit> {
        let keys = query_keys(query);
        let Some(first) = keys.first() else {
            return Vec::new();
        };
        // Candidate starts: the postings of the first word (or of everything starting with it
        // when it is the only word and a prefix is allowed)
        let mut starts: Vec<(u64, usize)> = if prefix && keys.len() == 1 {
            self.postings
                .range(first.clone()..)
                .take_while(|(k, _)| k.starts_with(first.as_str()))
                .flat_map(|(_, v)| v.iter().copied())
                .collect()
        } else {
            self.postings.get(first).cloned().unwrap_or_default()
        };
        starts.sort_unstable();
        let mut hits = Vec::new();
        for (media, i) in starts {
            let Some(words) = self.docs.get(&media) else {
                continue;
            };
            let matches = keys.iter().enumerate().all(|(n, key)| {
                words.get(i + n).is_some_and(|w| {
                    if prefix && n + 1 == keys.len() {
                        w.key.starts_with(key.as_str())
                    } else {
                        w.key == *key
                    }
                })
            });
            if !matches {
                continue;
            }
            let last = i + keys.len() - 1;
            let from = i.saturating_sub(3);
            let to = (last + 4).min(words.len());
            let context = words[from..to]
                .iter()
                .map(|w| w.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            hits.push(SpeechHit {
                media_id: media,
                start_s: words[i].start_s,
                end_s: words[last].end_s,
                word_index: i,
                context,
            });
        }
        hits
    }
}

/// Where the moment `media_time_s` of the media `media_id` falls on the timeline: the start of
/// each event that plays that part of the media, in timeline order.
pub fn timeline_positions(timeline: &Timeline, media_id: u64, media_time_s: f64) -> Vec<f64> {
    let mut found = Vec::new();
    for track in &timeline.tracks {
        for event in &track.events {
            if event.media_id != Some(media_id) {
                continue;
            }
            let rate = if event.playback_rate > 0.0 {
                event.playback_rate
            } else {
                1.0
            };
            let source_len = event.duration * rate;
            if media_time_s >= event.trim_in && media_time_s <= event.trim_in + source_len {
                found.push(event.start + (media_time_s - event.trim_in) / rate);
            }
        }
    }
    found.sort_by(f64::total_cmp);
    found.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    found
}

/// The words of a transcript made from the mix of the timeline, given to the media that play
/// them: a segment goes to every event whose span holds its middle, converted to the time of the
/// media (trim and speed). Returns the segments by media id.
pub fn split_timeline_transcript(
    timeline: &Timeline,
    segments: &[TranscriptSegment],
) -> BTreeMap<u64, Vec<TranscriptSegment>> {
    let mut by_media: BTreeMap<u64, Vec<TranscriptSegment>> = BTreeMap::new();
    for track in &timeline.tracks {
        for event in &track.events {
            let Some(media) = event.media_id else {
                continue;
            };
            let rate = if event.playback_rate > 0.0 {
                event.playback_rate
            } else {
                1.0
            };
            let end = event.start + event.duration;
            for seg in segments {
                let mid = (seg.start_s + seg.end_s) / 2.0;
                if mid < event.start || mid >= end {
                    continue;
                }
                let to_media = |t: f64| (event.trim_in + (t - event.start) * rate).max(0.0);
                by_media.entry(media).or_default().push(TranscriptSegment {
                    start_s: to_media(seg.start_s),
                    end_s: to_media(seg.end_s),
                    text: seg.text.clone(),
                });
            }
        }
    }
    for list in by_media.values_mut() {
        list.sort_by(|a, b| a.start_s.total_cmp(&b.start_s));
        list.dedup_by(|a, b| (a.start_s - b.start_s).abs() < 1e-6 && a.text == b.text);
    }
    by_media
}

impl SpeechIndex {
    /// Indexes a transcript of the timeline mix for the media it came from. Returns how many
    /// media got words.
    pub fn add_timeline_transcript(
        &mut self,
        timeline: &Timeline,
        segments: &[TranscriptSegment],
    ) -> usize {
        let split = split_timeline_transcript(timeline, segments);
        let count = split.len();
        for (media, segs) in split {
            self.add_media(media, &segs);
        }
        count
    }
}

/// The timeline positions of every hit, in time order, for jumping from one to the next.
pub fn jump_points(timeline: &Timeline, hits: &[SpeechHit]) -> Vec<f64> {
    let mut points: Vec<f64> = hits
        .iter()
        .flat_map(|h| timeline_positions(timeline, h.media_id, h.start_s))
        .collect();
    points.sort_by(f64::total_cmp);
    points.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    points
}

/// The jump point after (or, going backwards, before) `current_s`; the search wraps around the
/// ends. `None` when there is no point.
pub fn next_jump(points: &[f64], current_s: f64, forward: bool) -> Option<f64> {
    const EPS: f64 = 1e-6;
    if forward {
        points
            .iter()
            .copied()
            .find(|&p| p > current_s + EPS)
            .or_else(|| points.first().copied())
    } else {
        points
            .iter()
            .rev()
            .copied()
            .find(|&p| p < current_s - EPS)
            .or_else(|| points.last().copied())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::{TrackEvent, TrackKind};

    fn seg(start: f64, end: f64, text: &str) -> TranscriptSegment {
        TranscriptSegment {
            start_s: start,
            end_s: end,
            text: text.into(),
        }
    }

    fn index() -> SpeechIndex {
        let mut idx = SpeechIndex::new();
        idx.add_media(
            1,
            &[
                seg(0.0, 4.0, "Olá, pessoal! Hoje vamos falar de edição."),
                seg(4.0, 8.0, "A edição de vídeo é divertida."),
            ],
        );
        idx.add_media(2, &[seg(0.0, 2.0, "Outra edição aqui")]);
        idx
    }

    #[test]
    fn words_are_compared_without_case_accents_or_punctuation() {
        assert_eq!(normalize_word("Edição,"), "edicao");
        assert_eq!(normalize_word("OLÁ!"), "ola");
        assert_eq!(normalize_word("..."), "");
        let idx = index();
        let hits = idx.search("EDICAO", false);
        assert_eq!(hits.len(), 3, "two in media 1 and one in media 2");
        assert_eq!(hits[0].media_id, 1);
        assert_eq!(hits[2].media_id, 2);
        assert!(hits[0].start_s < hits[1].start_s);
        assert!(hits[0].context.contains("falar"));
        assert!(idx.search("inexistente", false).is_empty());
        assert!(idx.search("   ", false).is_empty());
    }

    #[test]
    fn a_word_is_found_at_the_time_it_is_said() {
        let idx = index();
        // "Olá, pessoal! Hoje vamos falar de edição." has 7 words over 4 s
        let hit = &idx.search("edição", false)[0];
        assert_eq!(hit.word_index, 6);
        assert!(
            (hit.start_s - 4.0 * 6.0 / 7.0).abs() < 1e-9,
            "{}",
            hit.start_s
        );
        let first = &idx.search("ola", false)[0];
        assert_eq!(first.start_s, 0.0);
    }

    #[test]
    fn phrases_need_consecutive_words_and_prefixes_match_the_end() {
        let idx = index();
        assert_eq!(idx.search("edição de vídeo", false).len(), 1);
        assert!(idx.search("vídeo de edição", false).is_empty());
        // "vamos de" are not neighbours
        assert!(idx.search("vamos de", false).is_empty());
        assert_eq!(idx.search("edi", true).len(), 3);
        assert!(idx.search("edi", false).is_empty());
        assert_eq!(idx.search("falar de ed", true).len(), 1);
    }

    #[test]
    fn re_indexing_replaces_and_removing_forgets_a_media() {
        let mut idx = index();
        assert_eq!((idx.media_count(), idx.word_count()), (2, 7 + 6 + 3));
        idx.add_media(2, &[seg(0.0, 1.0, "nada")]);
        assert_eq!(idx.search("edição", false).len(), 2);
        idx.remove_media(1);
        assert!(idx.search("edição", false).is_empty());
        assert_eq!(idx.search("nada", false).len(), 1);
        // A saved index finds again after the postings are rebuilt
        let json = serde_json::to_string(&index()).unwrap();
        let back: SpeechIndex = serde_json::from_str(&json).unwrap();
        assert_eq!(back.search("edição", false).len(), 3, "rebuilt on loading");
        // A project saved before the index existed has none
        let old: Timeline = serde_json::from_str(r#"{"tracks":[],"next_id":1}"#).unwrap();
        assert!(old.speech_index.is_empty());
        let mut tl = Timeline::new();
        tl.speech_index = index();
        let loaded: Timeline = serde_json::from_str(&serde_json::to_string(&tl).unwrap()).unwrap();
        assert_eq!(loaded.speech_index.search("edição", false).len(), 3);
    }

    #[test]
    fn a_mix_transcript_is_given_back_to_the_media_that_played_it() {
        let mut tl = Timeline::new();
        let mut track = tl.new_track(TrackKind::Audio);
        // Media 7 from its 5 s on, placed at 10 s for 4 s
        track.events.push(TrackEvent {
            id: tl.alloc_id(),
            media_id: Some(7),
            start: 10.0,
            duration: 4.0,
            trim_in: 5.0,
            ..TrackEvent::default()
        });
        tl.tracks.push(track);
        let mut idx = SpeechIndex::new();
        let n = idx.add_timeline_transcript(
            &tl,
            &[
                seg(0.0, 2.0, "antes do corte"),
                seg(11.0, 12.0, "palavra certa"),
            ],
        );
        assert_eq!(n, 1);
        let hits = idx.search("certa", false);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].media_id, 7);
        assert!((hits[0].start_s - 6.5).abs() < 1e-9, "{}", hits[0].start_s);
        assert!(
            idx.search("corte", false).is_empty(),
            "not played by any event"
        );
        // And the hit leads back to the timeline position
        let points = jump_points(&tl, &hits);
        assert!((points[0] - 11.5).abs() < 1e-9);
    }

    #[test]
    fn hits_map_to_the_timeline_and_the_jump_goes_to_the_next_one() {
        let mut tl = Timeline::new();
        let mut track = tl.new_track(TrackKind::Audio);
        // Media 1 from 2 s on, placed at 10 s for 2 s: media 2.0..4.0 -> timeline 10..12
        track.events.push(TrackEvent {
            id: tl.alloc_id(),
            media_id: Some(1),
            start: 10.0,
            duration: 2.0,
            trim_in: 2.0,
            ..TrackEvent::default()
        });
        tl.tracks.push(track);
        assert_eq!(timeline_positions(&tl, 1, 3.0), vec![11.0]);
        assert!(
            timeline_positions(&tl, 1, 1.0).is_empty(),
            "before the trim"
        );
        assert!(timeline_positions(&tl, 1, 4.5).is_empty(), "after the cut");
        assert!(timeline_positions(&tl, 9, 3.0).is_empty(), "another media");
        let hits = index().search("edição", false);
        // Hits of media 1 at 3.43 s and at 4.8 s: only the first plays
        let points = jump_points(&tl, &hits);
        assert_eq!(points.len(), 1);
        assert!((points[0] - (10.0 + 4.0 * 6.0 / 7.0 - 2.0)).abs() < 1e-9);
        // Jumping wraps around
        let p = [1.0, 5.0, 9.0];
        assert_eq!(next_jump(&p, 0.0, true), Some(1.0));
        assert_eq!(next_jump(&p, 1.0, true), Some(5.0));
        assert_eq!(next_jump(&p, 9.0, true), Some(1.0));
        assert_eq!(next_jump(&p, 5.0, false), Some(1.0));
        assert_eq!(next_jump(&p, 1.0, false), Some(9.0));
        assert_eq!(next_jump(&[], 1.0, true), None);
    }
}
