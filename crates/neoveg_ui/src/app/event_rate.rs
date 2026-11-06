//! `Alt+=` / `Alt+-`: the playback rate of the selected events in steps of 5 %. The rule is
//! `neoveg_engine::event_rate`.

use neoveg_common::i18n::tr;

use super::*;

impl AppState {
    pub(super) fn nudge_selected_rate(&mut self, factor: f64) {
        let refs = self.selection.refs();
        let Some(command) = neoveg_engine::event_rate::nudge(&self.timeline, &refs, factor) else {
            self.status = tr("Selecione eventos desbloqueados; a taxa já está no limite").into();
            return;
        };
        if self.execute_timeline(command) {
            self.restart_audio_if_playing();
            self.last_frame_key = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rate_keys_change_the_selected_event_in_one_undo_step() {
        let mut s = AppState::new();
        let mut track = s.timeline.new_track(TrackKind::Video);
        let event = TrackEvent {
            id: s.timeline.alloc_id(),
            start: 0.0,
            duration: 10.0,
            media_id: Some(1),
            ..TrackEvent::default()
        };
        let key = (track.id, event.id);
        track.events.push(event);
        s.timeline.tracks.push(track);
        s.selection.select_only(key);
        s.nudge_selected_rate(1.05);
        let e = &s.timeline.tracks[0].events[0];
        assert!((e.playback_rate - 1.05).abs() < 1e-9);
        s.history.undo(&mut s.timeline);
        assert_eq!(s.timeline.tracks[0].events[0].playback_rate, 1.0);
    }
}
