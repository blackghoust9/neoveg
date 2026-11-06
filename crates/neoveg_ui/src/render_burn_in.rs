//! Render option "Gravar timecode e nome do evento na imagem" (burn-in for review copies): the
//! settings kept by the render dialog, their controls, and the pure step that draws them into a
//! rendered frame.

use eframe::egui;
use neoveg_common::i18n::tr;
use neoveg_engine::timeline::{Timeline, TrackKind};
use neoveg_video::burn::BurnIn;
use neoveg_video::frame::FrameBuffer;

/// What the render dialog keeps about the burn-in.
#[derive(Debug, Clone, PartialEq)]
pub struct BurnInSettings {
    pub enabled: bool,
    pub timecode: bool,
    pub event_name: bool,
}

impl Default for BurnInSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            timecode: true,
            event_name: true,
        }
    }
}

impl BurnInSettings {
    /// The burn-in to render with; `None` when off or when nothing would be drawn.
    pub fn to_burn_in(&self, drop_frame: bool) -> Option<BurnIn> {
        (self.enabled && (self.timecode || self.event_name)).then(|| BurnIn {
            timecode: self.timecode,
            event_name: self.event_name,
            drop_frame,
            ..BurnIn::default()
        })
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(
            &mut self.enabled,
            tr("Gravar timecode e nome do evento na imagem"),
        );
        if self.enabled {
            ui.indent("burn_in_options", |ui| {
                ui.checkbox(&mut self.timecode, tr("Timecode do projeto"));
                ui.checkbox(&mut self.event_name, tr("Nome do evento"));
            });
        }
    }
}

/// Name of the event shown at `time`: the first visible video track's event under it.
pub fn event_name_at(timeline: &Timeline, time: f64) -> Option<String> {
    timeline
        .tracks
        .iter()
        .filter(|t| t.kind == TrackKind::Video && timeline.audible(t))
        .find_map(|t| timeline.playing_event_at(t.id, time))
        .map(|e| e.name.clone())
}

/// Draws the burn-in of the frame at `time` (the timeline time, which the timecode shows).
pub fn burn_in_frame(
    frame: &mut FrameBuffer,
    burn: &BurnIn,
    timeline: &Timeline,
    time: f64,
    fps: f64,
) {
    let name = event_name_at(timeline, time);
    burn.apply(frame, time, fps, name.as_deref());
}

#[cfg(test)]
mod tests {
    use super::*;
    use neoveg_engine::timeline::TrackEvent;

    fn timeline() -> Timeline {
        let mut tl = Timeline::new();
        let mut track = tl.new_track(TrackKind::Video);
        track.events.push(TrackEvent {
            id: 1,
            name: "Entrevista".into(),
            media_id: Some(1),
            start: 1.0,
            duration: 2.0,
            ..TrackEvent::default()
        });
        tl.tracks.push(track);
        tl
    }

    #[test]
    fn settings_become_a_burn_in_only_when_on_and_something_is_chosen() {
        let mut s = BurnInSettings::default();
        assert!(s.to_burn_in(false).is_none());
        s.enabled = true;
        let b = s.to_burn_in(true).unwrap();
        assert!(b.timecode && b.event_name && b.drop_frame);
        s.timecode = false;
        s.event_name = false;
        assert!(s.to_burn_in(false).is_none());
    }

    #[test]
    fn the_frame_changes_with_the_time_and_the_event_name_comes_from_the_timeline() {
        let tl = timeline();
        assert_eq!(event_name_at(&tl, 1.5).as_deref(), Some("Entrevista"));
        assert_eq!(event_name_at(&tl, 5.0), None);
        let burn = BurnInSettings {
            enabled: true,
            ..Default::default()
        }
        .to_burn_in(false)
        .unwrap();
        let blank = FrameBuffer::solid(640, 360, [0, 0, 0, 255]);
        let mut a = blank.clone();
        burn_in_frame(&mut a, &burn, &tl, 1.5, 30.0);
        let mut b = blank.clone();
        burn_in_frame(&mut b, &burn, &tl, 1.6, 30.0);
        assert_ne!(a.rgba, blank.rgba);
        assert_ne!(a.rgba, b.rgba);
        // Outside the event only the timecode is drawn
        let mut c = blank.clone();
        burn_in_frame(&mut c, &burn, &tl, 5.0, 30.0);
        assert_ne!(c.rgba, blank.rgba);
    }
}
