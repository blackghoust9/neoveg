//! Two sound edits of the selected event: *Alinhar ao transiente* (the first strong peak of the sound
//! is moved onto the cursor, or onto the marker nearest to it) and *Aplicar efeitos ao áudio do
//! evento* (the effects of the event are written into its sound off line, in a file of its own, and
//! the event moves to that file: one undo step, like *Reverter evento*). What is pure is in
//! `neoveg_engine::transient` and `neoveg_engine::subclip`; the file is written by
//! `neoveg_audio::subclip::write_wav_with_effects`.

use neoveg_audio::fxmixer::EffectSpec;
use neoveg_engine::{subclip as sub, transient};

use super::*;

impl AppState {
    /// The primary selected event when it is an unlocked audio event whose sound is loaded.
    fn transient_source(&self) -> Option<(EventRef, TrackEvent)> {
        let (track_id, event_id) = self.selection.primary()?;
        let track = self.timeline.track(track_id)?;
        let event = track.events.iter().find(|e| e.id == event_id)?;
        if track.kind != TrackKind::Audio || event.locked || track.locked {
            return None;
        }
        self.media.contains_key(&event.media_id?).then_some(())?;
        sub::section_of(event)?;
        Some(((track_id, event_id), event.clone()))
    }

    pub(super) fn can_align_to_transient(&self) -> bool {
        self.transient_source().is_some()
    }

    /// *Alinhar ao transiente*: moves the primary selected audio event so that the first strong peak
    /// of the part it plays falls on the nearest marker (within reach) or on the cursor.
    pub(super) fn align_to_transient(&mut self) {
        let Some((event_ref, event)) = self.transient_source() else {
            self.status = tr("Selecione um evento de áudio desbloqueado para alinhar").into();
            return;
        };
        let Some(media) = event.media_id.and_then(|id| self.media.get(&id)) else {
            return;
        };
        let Some(section) = sub::section_of(&event) else {
            return;
        };
        let rate = f64::from(media.sample_rate.max(1));
        let from = (section.start * rate).round() as usize;
        let count = (section.length * rate).round() as usize;
        let samples = media.stereo.to_vec(from, count);
        let offset = transient::first_transient(&samples, 2, media.sample_rate)
            .map(|seconds| seconds / neoveg_engine::edit::clamp_playback_rate(event.playback_rate));
        let Some(offset) = offset else {
            self.status = tr("O evento não tem um pico forte para alinhar (silêncio)").into();
            return;
        };
        let target = transient::target_time(&self.timeline, self.playhead);
        let Some(command) = transient::align(&self.timeline, event_ref, offset, target) else {
            self.status =
                tr("O evento já está alinhado, ou o pico cairia antes do início do projeto").into();
            return;
        };
        if self.history.execute(&mut self.timeline, command) {
            self.last_frame_key = None;
            self.restart_audio_if_playing();
            self.status = tr_args(
                "Primeiro pico do evento alinhado em {} s",
                &[&format!("{target:.3}")],
            );
        }
    }

    /// The effects of the event that sound (the bypassed ones are left out), as the mixer builds them.
    fn event_effect_chain(event: &TrackEvent) -> Vec<EffectSpec> {
        event
            .audio_effects
            .iter()
            .filter(|e| !e.bypass)
            .map(|e| EffectSpec {
                kind: e.kind.clone(),
                params: e.params.iter().map(|(k, v)| (k.clone(), *v)).collect(),
            })
            .collect()
    }

    /// Whether *Aplicar efeitos ao áudio do evento* can run: one at a time, on an unlocked audio
    /// event of a sound file with effects on.
    pub(super) fn can_bake_event_effects(&self) -> bool {
        if !self.pending_subclips.is_empty() {
            return false;
        }
        self.transient_source()
            .is_some_and(|(_, event)| !Self::event_effect_chain(&event).is_empty())
            && self
                .subclip_source()
                .is_some_and(|(_, _, input)| !is_video_path(&input))
    }

    /// *Aplicar efeitos ao áudio do evento*: writes the part the event plays through its effects, off
    /// line, to a WAV in the `subclipes` folder; when it is read the event moves to it and its effects
    /// come off (see `place_subclip`).
    pub(super) fn begin_bake_event_effects(&mut self, ctx: &egui::Context) {
        if !self.can_bake_event_effects() {
            self.status =
                tr("Selecione um evento de áudio com efeitos ligados (e desbloqueado)").into();
            return;
        }
        let Some((event_ref, section, input)) = self.subclip_source() else {
            return;
        };
        let chain = self
            .timeline
            .track(event_ref.0)
            .and_then(|t| t.events.iter().find(|e| e.id == event_ref.1))
            .map(Self::event_effect_chain)
            .unwrap_or_default();
        // Hosted plug-ins cannot run here; say so before writing anything
        if let Some(spec) = chain
            .iter()
            .find(|s| neoveg_audio::dsp::effects::effect_info(&s.kind).is_none())
        {
            self.status = tr_args(
                "O efeito {} não roda fora de tempo real: desligue-o antes",
                &[&spec.kind],
            );
            return;
        }
        let Some(dir) = self.subclip_dir() else {
            self.status = tr("Não há pasta para guardar o áudio").into();
            return;
        };
        let taken =
            |p: &std::path::Path| p.exists() || self.pending_subclips.iter().any(|s| s.path == p);
        let named = sub::output_path(&input, &dir, section, false, "wav", |_| false);
        let stem = named
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let output = (1..)
            .map(|n| {
                dir.join(if n == 1 {
                    format!("{stem}_fx.wav")
                } else {
                    format!("{stem}_fx_{n}.wav")
                })
            })
            .find(|p| !taken(p))
            .expect("an unbounded range finds a free name");
        self.status = tr("Aplicando os efeitos ao áudio do evento…").into();
        self.pending_subclips.push(subclip::PendingSubclip {
            path: output.clone(),
            replace: vec![event_ref],
            reverse: false,
            bake: true,
        });
        let (tx, ctx) = (self.loader_tx.clone(), ctx.clone());
        std::thread::spawn(move || {
            let result = neoveg_audio::subclip::write_wav_with_effects(
                &input,
                &output,
                section.start,
                section.length,
                &chain,
            );
            let msg = match result {
                Ok(_) => load_message(output, None, true),
                Err(error) => LoadMsg::Failed {
                    path: output,
                    error,
                    existing_id: None,
                    converted: true,
                },
            };
            let _ = tx.send(msg);
            ctx.request_repaint();
        });
    }
}
