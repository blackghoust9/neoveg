//! Spoken-word search wired to the project: the index is filled when the speech is transcribed
//! and the window moves the cursor to a hit.

use eframe::egui;
use neoveg_common::i18n::tr_args;
use neoveg_common::transcript::TranscriptSegment;

use super::AppState;

impl AppState {
    /// Adds a transcript of the timeline mix to the index kept in the project (saved with it).
    pub(super) fn index_whisper_transcript(&mut self, segments: &[TranscriptSegment]) {
        let split = neoveg_engine::speech_index::split_timeline_transcript(&self.timeline, segments);
        for (media, segs) in split {
            self.timeline.speech_index.add_media(media, &segs);
        }
    }

    /// The spoken-word search window: a hit moves the cursor.
    pub(super) fn run_speech_search(&mut self, ctx: &egui::Context) {
        if !self.speech_search.open {
            return;
        }
        let jump = self.speech_search.show(
            ctx,
            &self.timeline.speech_index,
            &self.timeline,
            &self.project.media_pool,
            self.playhead,
        );
        if let Some(time) = jump {
            self.playhead = time;
            self.blink_origin = ctx.input(|i| i.time);
            self.status = tr_args("Cursor em {} s (busca de palavra falada)", &[&format!("{time:.2}")]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neoveg_engine::{TrackEvent, TrackKind};

    #[test]
    fn a_transcript_is_indexed_for_the_media_and_the_command_opens_the_window() {
        let mut s = AppState::new();
        let track = s.timeline.new_track(TrackKind::Audio);
        let track_id = track.id;
        s.timeline.tracks.push(track);
        let event = TrackEvent {
            id: s.timeline.alloc_id(),
            media_id: Some(3),
            start: 0.0,
            duration: 10.0,
            ..TrackEvent::default()
        };
        s.timeline.track_mut(track_id).unwrap().events.push(event);
        s.index_whisper_transcript(&[TranscriptSegment {
            start_s: 2.0,
            end_s: 4.0,
            text: "olá mundo".into(),
        }]);
        assert_eq!(s.timeline.speech_index.search("mundo", false).len(), 1);
        s.speech_search.query = "mundo".into();
        s.speech_search.refresh(&s.timeline.speech_index);
        assert_eq!(s.speech_search.hits().len(), 1);
        assert!(!s.speech_search.open);
        s.speech_search.open = true;
    }
}
