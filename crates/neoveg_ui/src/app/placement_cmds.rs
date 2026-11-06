//! The commands that place events relative to the cursor and to the end of a track (*append to the end
//! of the track*, *end at the cursor*, *replace edit*), *match frame* and *reverse match*, *dupe
//! detection* and the sequence report. The rules are in `neoveg_engine::{placement, dupes,
//! sequence_report}`; this only reads the state of the window and shows the result.

use neoveg_common::i18n::tr_args;
use neoveg_engine::{dupes, placement, sequence_report};

use super::*;

impl AppState {
    /// The kinds of track the Trimmer media goes on: the picture of a video and its sound.
    fn trimmer_kinds(&self, media_id: u64) -> Vec<TrackKind> {
        let has_video = self.video_info.contains_key(&media_id);
        let has_audio = self.media.contains_key(&media_id);
        let mut kinds = Vec::new();
        if has_video {
            kinds.push(TrackKind::Video);
        }
        if has_audio || !has_video {
            kinds.push(TrackKind::Audio);
        }
        kinds
    }

    /// *Add to the end of the track*: the Trimmer selection goes right after the last event of the
    /// selected track (of the tracks that take it, when it has picture and sound).
    pub(super) fn append_trimmer_to_track_end(&mut self) {
        let Some(trimmer) = &self.trimmer else {
            self.status = "Abra uma mídia no Trimmer para adicioná-la ao fim da faixa".into();
            return;
        };
        let (media_id, name) = (trimmer.media_id, trimmer.name.clone());
        let (from, len) = (trimmer.selection().0, trimmer.selection_len());
        if len <= 0.0 {
            self.status = "Nada para adicionar: marque a seleção no Trimmer".into();
            return;
        }
        let tracks: Vec<TrackId> = self
            .trimmer_kinds(media_id)
            .into_iter()
            .map(|kind| self.track_of_kind(kind))
            .collect();
        let start = placement::append_start(&self.timeline, &tracks);
        let mut commands = Vec::new();
        for track_id in tracks {
            let event = neoveg_engine::trimmer::Placement {
                start,
                duration: len,
                trim_in: from,
                playback_rate: 1.0,
            }
            .into_event(self.timeline.alloc_id(), &name, media_id);
            commands.push(EditCommand::AddEvent { track_id, event });
        }
        self.history
            .execute(&mut self.timeline, EditCommand::Batch(commands));
        self.playhead = start;
        self.last_frame_key = None;
        self.status = tr_args("{} adicionado ao fim da faixa", &[&name]);
    }

    /// *End at the cursor* (backtiming): the selected events move so that the last of them ends at the cursor.
    pub(super) fn end_selected_at_cursor(&mut self) {
        let refs = self.trim_targets();
        let outcome = placement::end_at_cursor(&self.timeline, &refs, self.playhead);
        match self.apply_outcome(outcome) {
            Some(touched) => {
                self.selection.set(touched);
                self.status = "Eventos movidos para terminar no cursor".into();
            }
            None => {
                self.status = "Nada a mover: os eventos já terminam no cursor ou estão bloqueados".into()
            }
        }
    }

    /// *Replace edit*: the selected event plays the Trimmer media instead, with the Trimmer frame
    /// under the timeline cursor.
    pub(super) fn replace_with_trimmer_clip(&mut self) {
        let Some(trimmer) = &self.trimmer else {
            self.status = "Abra uma mídia no Trimmer para substituir o evento".into();
            return;
        };
        let (media_id, name, source) = (trimmer.media_id, trimmer.name.clone(), trimmer.cursor());
        let Some(target) = self.selection.primary() else {
            self.status = "Selecione o evento a substituir".into();
            return;
        };
        let Some((outcome, note)) = placement::replace_with_source(
            &self.timeline,
            target,
            media_id,
            &name,
            source,
            self.playhead,
        ) else {
            self.status =
                "Não foi possível substituir: o cursor precisa estar dentro do evento e o evento não pode estar bloqueado"
                    .into();
            return;
        };
        if self.apply_outcome(Some(outcome)).is_some() {
            self.status = match note {
                placement::ReplaceNote::Aligned => tr_args("Evento substituído por {}", &[&name]),
                placement::ReplaceNote::ClampedToSourceStart => tr_args(
                    "Evento substituído por {}; a fonte começa tarde demais para alinhar os quadros",
                    &[&name],
                ),
            };
        }
    }

    /// *Match frame*: opens the media of the event under the cursor in the Trimmer, at the same frame.
    pub(super) fn match_frame_to_trimmer(&mut self) {
        let target = self.selection.primary().or_else(|| {
            let t = self.selected_track?;
            self.timeline
                .playing_event_at(t, self.playhead)
                .map(|e| (t, e.id))
        });
        let found = target.and_then(|t| placement::match_frame(&self.timeline, t, self.playhead));
        let Some((media_id, source_time)) = found else {
            self.status = "Combinar quadro: ponha o cursor sobre um evento com mídia".into();
            return;
        };
        self.open_in_trimmer(media_id);
        if let Some(trimmer) = &mut self.trimmer {
            trimmer.set_cursor(source_time);
            self.status = tr_args(
                "Quadro combinado no Trimmer: {}",
                &[&seconds_to_timecode(
                    source_time,
                    self.project.settings.fps,
                    self.drop_frame(),
                )],
            );
        }
    }

    /// *Reverse match*: selects the events that show the Trimmer frame and takes the cursor to the first.
    pub(super) fn reverse_match_from_trimmer(&mut self) {
        let Some(trimmer) = &self.trimmer else {
            self.status = "Abra uma mídia no Trimmer para combinar de volta".into();
            return;
        };
        let hits = placement::reverse_match(&self.timeline, trimmer.media_id, trimmer.cursor());
        let Some(&(_, time)) = hits.first() else {
            self.status = "Nenhum evento da linha do tempo usa esse quadro".into();
            return;
        };
        self.playhead = time;
        self.last_frame_key = None;
        let refs: Vec<EventRef> = hits.iter().map(|h| h.0).collect();
        self.selection.set(refs);
        self.status = tr_args("{} evento(s) usam esse quadro", &[&hits.len()]);
    }

    /// *Dupe detection*: selects the events that repeat a part of one media file.
    pub(super) fn select_dupes(&mut self) {
        let groups = dupes::find_dupes(&self.timeline, 0.0);
        let events = dupes::all_events(&groups);
        if events.is_empty() {
            self.status = "Nenhum trecho de mídia se repete na linha do tempo".into();
            return;
        }
        self.status = tr_args(
            "{} evento(s) em {} grupo(s) repetem o mesmo trecho",
            &[&events.len(), &groups.len()],
        );
        self.selection.set(events);
    }

    /// File > Export sequence report: the media in use as CSV.
    pub(super) fn export_sequence_report(&mut self) {
        let rows = sequence_report::rows(&self.timeline, &self.project.media_pool, &|p| {
            std::path::Path::new(p).exists()
        });
        let text = sequence_report::to_csv(&rows, self.project.settings.fps, self.drop_frame());
        let Some(path) = rfd::FileDialog::new()
            .add_filter("CSV", &["csv"])
            .set_file_name(format!("{} - relatorio.csv", self.project.name))
            .save_file()
        else {
            return;
        };
        match std::fs::write(&path, text) {
            Ok(()) => {
                let missing = rows.iter().filter(|r| r.missing).count();
                self.status = tr_args(
                    "Relatório exportado: {} ({} mídia(s) ausente(s))",
                    &[&path.display(), &missing],
                );
            }
            Err(e) => {
                tracing::error!("[ERROR] cannot write {}: {e}", path.display());
                self.status = tr_args("Falha ao exportar o relatório: {}", &[&e]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_event(s: &mut AppState, media: u64, start: f64, duration: f64, trim: f64) -> EventRef {
        let track = s.timeline.new_track(TrackKind::Audio);
        let track_id = track.id;
        s.timeline.tracks.push(track);
        let event = TrackEvent {
            id: s.timeline.alloc_id(),
            name: "e".into(),
            media_id: Some(media),
            start,
            duration,
            trim_in: trim,
            ..TrackEvent::default()
        };
        let id = event.id;
        s.timeline.track_mut(track_id).unwrap().events.push(event);
        (track_id, id)
    }

    #[test]
    fn the_selected_event_ends_at_the_cursor_in_one_undo_step() {
        let mut s = AppState::new();
        let ev = with_event(&mut s, 1, 0.0, 4.0, 0.0);
        s.selection.set(vec![ev]);
        s.playhead = 10.0;
        s.run_command(Command::EditEndAtCursor, &egui::Context::default(), 0.0);
        assert_eq!(s.timeline.track(ev.0).unwrap().events[0].end(), 10.0);
        assert!(s.history.undo(&mut s.timeline));
        assert_eq!(s.timeline.track(ev.0).unwrap().events[0].start, 0.0);
    }

    #[test]
    fn the_trimmer_media_goes_after_the_last_event_of_the_track() {
        let mut s = AppState::new();
        let ev = with_event(&mut s, 1, 2.0, 3.0, 0.0);
        s.selected_track = Some(ev.0);
        s.trimmer = Some(neoveg_engine::trimmer::TrimmerState::new(9, "clip", 8.0, 25.0));
        s.run_command(Command::EditAppendToTrack, &egui::Context::default(), 0.0);
        let events = &s.timeline.track(ev.0).unwrap().events;
        assert_eq!(events.len(), 2);
        assert_eq!((events[1].start, events[1].duration), (5.0, 8.0));
        assert_eq!(events[1].media_id, Some(9));
    }

    #[test]
    fn replace_edit_swaps_the_media_and_keeps_place_and_length() {
        let mut s = AppState::new();
        let ev = with_event(&mut s, 1, 5.0, 4.0, 0.0);
        s.selection.set(vec![ev]);
        s.playhead = 6.0;
        let mut trimmer = neoveg_engine::trimmer::TrimmerState::new(9, "clip", 20.0, 25.0);
        trimmer.set_cursor(10.0);
        s.trimmer = Some(trimmer);
        s.run_command(Command::EditReplaceFromSource, &egui::Context::default(), 0.0);
        let e = &s.timeline.track(ev.0).unwrap().events[0];
        assert_eq!((e.media_id, e.start, e.duration), (Some(9), 5.0, 4.0));
        assert_eq!(e.trim_in, 9.0);
    }

    #[test]
    fn dupes_are_selected_and_reverse_match_finds_the_event() {
        let mut s = AppState::new();
        let a = with_event(&mut s, 1, 0.0, 4.0, 0.0);
        let b = with_event(&mut s, 1, 10.0, 4.0, 2.0);
        let _other = with_event(&mut s, 2, 20.0, 4.0, 0.0);
        s.run_command(Command::EditSelectDupes, &egui::Context::default(), 0.0);
        let mut got = s.selection.refs();
        got.sort();
        let mut want = vec![a, b];
        want.sort();
        assert_eq!(got, want);
        let mut trimmer = neoveg_engine::trimmer::TrimmerState::new(1, "clip", 20.0, 25.0);
        trimmer.set_cursor(5.0);
        s.trimmer = Some(trimmer);
        s.run_command(Command::CursorReverseMatch, &egui::Context::default(), 0.0);
        assert_eq!(s.selection.refs(), vec![b]);
        assert_eq!(s.playhead, 13.0);
    }
}
