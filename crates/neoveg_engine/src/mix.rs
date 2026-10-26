//! Per-track mixer controls: mute, solo, fader, pan, phase invert and the project pan law.
//!
//! Every function is pure: it reads the timeline and returns one [`EditCommand`], so a click or a
//! released fader is exactly one undo step.

use crate::command::EditCommand;
use crate::timeline::{Timeline, TrackId, TrackKind, TrackMix};
use neoveg_common::grade::Grade;
use neoveg_common::pan::PanLaw;

fn replace(
    timeline: &Timeline,
    track_id: TrackId,
    change: impl FnOnce(&mut TrackMix),
) -> Option<EditCommand> {
    let before = timeline.track(track_id)?.mix();
    let mut after = before;
    change(&mut after);
    let after = after.sanitized();
    (before != after).then_some(EditCommand::SetTrackMix {
        track_id,
        before,
        after,
    })
}

pub fn set_muted(timeline: &Timeline, track_id: TrackId, muted: bool) -> Option<EditCommand> {
    replace(timeline, track_id, |m| m.muted = muted)
}

/// Mute only this track among tracks of its kind: every other track of the same kind is unmuted,
/// and a track that is already the only one muted is unmuted again.
pub fn mute_exclusive(timeline: &Timeline, track_id: TrackId) -> Option<EditCommand> {
    let target = timeline.track(track_id)?;
    let kind = target.kind;
    let already_alone = target.muted
        && timeline
            .tracks
            .iter()
            .filter(|t| t.kind == kind && t.id != track_id)
            .all(|t| !t.muted);
    let mut commands = Vec::new();
    for t in timeline.tracks.iter().filter(|t| t.kind == kind) {
        let want = t.id == track_id && !already_alone;
        if let Some(c) = set_muted(timeline, t.id, want) {
            commands.push(c);
        }
    }
    match commands.len() {
        0 => None,
        1 => commands.pop(),
        _ => Some(EditCommand::Batch(commands)),
    }
}

pub fn set_solo(timeline: &Timeline, track_id: TrackId, solo: bool) -> Option<EditCommand> {
    if timeline.track(track_id)?.kind != TrackKind::Audio {
        return None;
    }
    replace(timeline, track_id, |m| m.solo = solo)
}

/// Solo only this audio track: every other audio track is cleared, so one click isolates it.
/// A track that is already the only one soloed is un-soloed again.
pub fn solo_exclusive(timeline: &Timeline, track_id: TrackId) -> Option<EditCommand> {
    let target = timeline.track(track_id)?;
    if target.kind != TrackKind::Audio {
        return None;
    }
    let already_alone = target.solo
        && timeline
            .tracks
            .iter()
            .filter(|t| t.kind == TrackKind::Audio && t.id != track_id)
            .all(|t| !t.solo);
    let mut commands = Vec::new();
    for t in timeline
        .tracks
        .iter()
        .filter(|t| t.kind == TrackKind::Audio)
    {
        let want = t.id == track_id && !already_alone;
        if let Some(c) = set_solo(timeline, t.id, want) {
            commands.push(c);
        }
    }
    match commands.len() {
        0 => None,
        1 => commands.pop(),
        _ => Some(EditCommand::Batch(commands)),
    }
}

/// Fader in dB, clamped to [`TrackMix::VOLUME_RANGE_DB`].
pub fn set_volume_db(timeline: &Timeline, track_id: TrackId, db: f32) -> Option<EditCommand> {
    replace(timeline, track_id, |m| m.volume_db = db)
}

/// Pan from `-1.0` to `1.0`.
pub fn set_pan(timeline: &Timeline, track_id: TrackId, pan: f32) -> Option<EditCommand> {
    replace(timeline, track_id, |m| m.pan = pan)
}

pub fn set_invert_phase(
    timeline: &Timeline,
    track_id: TrackId,
    invert: bool,
) -> Option<EditCommand> {
    replace(timeline, track_id, |m| m.invert_phase = invert)
}

pub fn set_pan_law(timeline: &Timeline, law: PanLaw) -> Option<EditCommand> {
    (timeline.pan_law != law).then_some(EditCommand::SetPanLaw {
        before: timeline.pan_law,
        after: law,
    })
}

/// Sets the primary color correction of the video (values are clamped to their ranges).
pub fn set_video_grade(timeline: &Timeline, grade: Grade) -> Option<EditCommand> {
    let after = grade.sanitized();
    (timeline.video_grade != after).then_some(EditCommand::SetVideoGrade {
        before: timeline.video_grade,
        after,
    })
}

/// Sets (or clears) the `.cube` color table of the video.
pub fn set_video_lut(timeline: &Timeline, lut: Option<String>) -> Option<EditCommand> {
    (timeline.video_lut != lut).then(|| EditCommand::SetVideoLut {
        before: timeline.video_lut.clone(),
        after: lut,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;

    fn go(command: EditCommand, history: &mut History, tl: &mut Timeline) {
        assert!(history.execute(tl, command));
    }

    fn project() -> (Timeline, TrackId, TrackId) {
        let mut tl = Timeline::new();
        let a = tl.new_track(TrackKind::Audio);
        let b = tl.new_track(TrackKind::Audio);
        let (ia, ib) = (a.id, b.id);
        tl.tracks.push(a);
        tl.tracks.push(b);
        (tl, ia, ib)
    }

    #[test]
    fn every_control_is_one_undo_step() {
        let (mut tl, a, _) = project();
        let mut history = History::new();
        go(set_muted(&tl, a, true).unwrap(), &mut history, &mut tl);
        go(set_volume_db(&tl, a, -6.0).unwrap(), &mut history, &mut tl);
        go(set_pan(&tl, a, 0.5).unwrap(), &mut history, &mut tl);
        go(
            set_invert_phase(&tl, a, true).unwrap(),
            &mut history,
            &mut tl,
        );
        go(
            set_pan_law(&tl, PanLaw::ConstantPower).unwrap(),
            &mut history,
            &mut tl,
        );
        let t = tl.track(a).unwrap();
        assert!(t.muted && t.invert_phase);
        assert_eq!((t.volume_db, t.pan), (-6.0, 0.5));
        assert_eq!(tl.pan_law, PanLaw::ConstantPower);
        for _ in 0..5 {
            assert!(history.undo(&mut tl));
        }
        assert_eq!(tl.track(a).unwrap().mix(), TrackMix::default());
        assert_eq!(tl.pan_law, PanLaw::Balance);
        assert!(history.redo(&mut tl));
        assert!(tl.track(a).unwrap().muted);
    }

    #[test]
    fn no_change_means_no_command_and_values_are_clamped() {
        let (tl, a, _) = project();
        assert!(set_muted(&tl, a, false).is_none());
        assert!(set_volume_db(&tl, a, 0.0).is_none());
        assert!(set_pan_law(&tl, PanLaw::Balance).is_none());
        assert!(set_pan(&tl, 9999, 0.1).is_none());
        let mut tl = tl;
        let mut history = History::new();
        go(set_volume_db(&tl, a, 500.0).unwrap(), &mut history, &mut tl);
        go(set_pan(&tl, a, -4.0).unwrap(), &mut history, &mut tl);
        let t = tl.track(a).unwrap();
        assert_eq!(t.volume_db, TrackMix::VOLUME_RANGE_DB.1);
        assert_eq!(t.pan, -1.0);
        assert!(set_volume_db(&tl, a, f32::NAN).is_some_and(|_| true));
    }

    #[test]
    fn solo_silences_the_other_audio_tracks() {
        let (mut tl, a, b) = project();
        assert!(tl.audible(tl.track(a).unwrap()) && tl.audible(tl.track(b).unwrap()));
        let mut history = History::new();
        go(set_solo(&tl, a, true).unwrap(), &mut history, &mut tl);
        assert!(tl.audible(tl.track(a).unwrap()));
        assert!(!tl.audible(tl.track(b).unwrap()));
        // A muted track stays silent even when soloed
        go(set_muted(&tl, a, true).unwrap(), &mut history, &mut tl);
        assert!(!tl.audible(tl.track(a).unwrap()));
    }

    #[test]
    fn exclusive_solo_isolates_one_track_and_toggles_off() {
        let (mut tl, a, b) = project();
        let mut history = History::new();
        go(set_solo(&tl, a, true).unwrap(), &mut history, &mut tl);
        go(solo_exclusive(&tl, b).unwrap(), &mut history, &mut tl);
        assert!(!tl.track(a).unwrap().solo && tl.track(b).unwrap().solo);
        go(solo_exclusive(&tl, b).unwrap(), &mut history, &mut tl);
        assert!(!tl.track(a).unwrap().solo && !tl.track(b).unwrap().solo);
        // The swap above was a single step
        assert!(history.undo(&mut tl));
        assert!(tl.track(b).unwrap().solo);
    }

    #[test]
    fn exclusive_mute_isolates_one_track_within_its_kind_and_toggles_off() {
        let (mut tl, a, b) = project();
        let v = tl.new_track(TrackKind::Video);
        let vid = v.id;
        tl.tracks.push(v);
        let mut history = History::new();
        go(set_muted(&tl, vid, true).unwrap(), &mut history, &mut tl);
        go(set_muted(&tl, a, true).unwrap(), &mut history, &mut tl);
        go(mute_exclusive(&tl, b).unwrap(), &mut history, &mut tl);
        assert!(!tl.track(a).unwrap().muted && tl.track(b).unwrap().muted);
        assert!(
            tl.track(vid).unwrap().muted,
            "video track mute is untouched"
        );
        go(mute_exclusive(&tl, b).unwrap(), &mut history, &mut tl);
        assert!(!tl.track(a).unwrap().muted && !tl.track(b).unwrap().muted);
        assert!(history.undo(&mut tl));
        assert!(tl.track(b).unwrap().muted);
    }

    #[test]
    fn solo_applies_to_audio_only_and_never_hides_video_tracks() {
        let (mut tl, a, _) = project();
        let v = tl.new_track(TrackKind::Video);
        let vid = v.id;
        tl.tracks.push(v);
        assert!(set_solo(&tl, vid, true).is_none());
        assert!(solo_exclusive(&tl, vid).is_none());
        let mut history = History::new();
        go(set_solo(&tl, a, true).unwrap(), &mut history, &mut tl);
        assert!(tl.audible(tl.track(vid).unwrap()));
        go(set_muted(&tl, vid, true).unwrap(), &mut history, &mut tl);
        assert!(!tl.audible(tl.track(vid).unwrap()));
    }

    #[test]
    fn the_color_grade_is_one_undo_step_clamped_and_saved_only_when_set() {
        let (mut tl, _, _) = project();
        let mut history = History::new();
        assert!(set_video_grade(&tl, Grade::default()).is_none());
        let json = serde_json::to_string(&tl).unwrap();
        assert!(
            !json.contains("video_grade"),
            "a neutral grade is not written"
        );
        let grade = Grade {
            exposure: 99.0,
            saturation: -0.5,
            ..Grade::default()
        };
        go(set_video_grade(&tl, grade).unwrap(), &mut history, &mut tl);
        assert_eq!(tl.video_grade.exposure, 4.0);
        let back: Timeline = serde_json::from_str(&serde_json::to_string(&tl).unwrap()).unwrap();
        assert_eq!(back.video_grade, tl.video_grade);
        assert!(history.undo(&mut tl));
        assert!(tl.video_grade.is_neutral());
        assert!(history.redo(&mut tl));
        assert_eq!(tl.video_grade.saturation, -0.5);
    }

    #[test]
    fn the_video_lut_is_one_undo_step_and_survives_saving() {
        let (mut tl, _, _) = project();
        let mut history = History::new();
        assert!(set_video_lut(&tl, None).is_none());
        go(
            set_video_lut(&tl, Some("CINEMATIC.cube".into())).unwrap(),
            &mut history,
            &mut tl,
        );
        let json = serde_json::to_string(&tl).unwrap();
        let back: Timeline = serde_json::from_str(&json).unwrap();
        assert_eq!(back.video_lut.as_deref(), Some("CINEMATIC.cube"));
        assert!(history.undo(&mut tl));
        assert!(tl.video_lut.is_none());
        let old: Timeline = serde_json::from_str(r#"{"tracks":[],"next_id":0}"#).unwrap();
        assert!(old.video_lut.is_none());
    }

    #[test]
    fn the_fader_gain_follows_decibels() {
        let mut m = TrackMix::default();
        assert!((m.gain() - 1.0).abs() < 1e-6);
        m.volume_db = -6.0206;
        assert!((m.gain() - 0.5).abs() < 1e-3);
        m.volume_db = TrackMix::VOLUME_RANGE_DB.0;
        assert_eq!(m.gain(), 0.0);
        assert!(!TrackMix {
            pan: 0.1,
            ..TrackMix::default()
        }
        .is_neutral());
    }

    #[test]
    fn older_project_files_load_with_a_neutral_mixer() {
        let t: crate::timeline::Track =
            serde_json::from_str(r#"{"id":1,"kind":"Audio","name":"A","muted":false,"events":[]}"#)
                .unwrap();
        assert_eq!(t.mix(), TrackMix::default());
        let tl: Timeline = serde_json::from_str(r#"{"tracks":[],"next_id":0}"#).unwrap();
        assert_eq!(tl.pan_law, PanLaw::Balance);
        // A neutral mixer is not written, so files stay small and compatible
        let json = serde_json::to_string(&t).unwrap();
        assert!(!json.contains("volume_db") && !json.contains("pan"));
    }
}
