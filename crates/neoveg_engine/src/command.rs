//! Undoable edit commands. Each command carries everything needed to apply and revert it.

use crate::captions::Caption;
use crate::envelope::TrackEnvelope;
use crate::markers::Marks;
use crate::timeline::{
    EventGeometry, EventId, Timeline, Track, TrackDucking, TrackEffect, TrackEvent, TrackId,
    TrackMix, TIME_EPSILON,
};
use neoveg_common::chroma::ChromaKey;
use neoveg_common::fade::FadeCurve;
use neoveg_common::grade::Grade;
use neoveg_common::pan::PanLaw;
use neoveg_common::pan_crop::PanCrop;

/// Where an effect chain lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FxTarget {
    Track(TrackId),
    Master,
}

/// Where a video effect chain lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoFxTarget {
    /// One event of a video track.
    Event {
        track_id: TrackId,
        event_id: EventId,
    },
    /// The composited layer of a video track.
    Track(TrackId),
    /// Every event that plays this media pool entry.
    Media(u64),
    /// The whole composited picture.
    Project,
}

/// The whole event list of one track before and after an edit.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackChange {
    pub track_id: TrackId,
    pub before: Vec<TrackEvent>,
    pub after: Vec<TrackEvent>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EditCommand {
    AddTrack {
        track: Track,
    },
    AddEvent {
        track_id: TrackId,
        event: TrackEvent,
    },
    RemoveEvent {
        track_id: TrackId,
        index: usize,
        event: TrackEvent,
    },
    MoveEvent {
        track_id: TrackId,
        event_id: EventId,
        old_start: f64,
        new_start: f64,
    },
    /// Move, trim and/or change of track in one undo step (a mouse drag).
    EditEvent {
        from_track: TrackId,
        to_track: TrackId,
        event_id: EventId,
        old: EventGeometry,
        new: EventGeometry,
    },
    SplitEvent {
        track_id: TrackId,
        event_id: EventId,
        new_event_id: EventId,
        at: f64,
        original_duration: f64,
        original_fade_out: f64,
    },
    /// Compound edit (cut, paste, trim, multi-track split...) stored as the affected tracks'
    /// event lists, so any combination of changes is one exactly reversible undo step.
    ReplaceEvents {
        changes: Vec<TrackChange>,
    },
    /// Any change to the markers and regions (add, move, rename, delete): the whole set before
    /// and after, so it is one exactly reversible undo step.
    ReplaceMarks {
        before: Marks,
        after: Marks,
    },
    /// Any change to an effect chain (add, remove, reorder, bypass, parameter): the whole chain
    /// before and after, so it is one exactly reversible undo step.
    SetEffects {
        target: FxTarget,
        before: Vec<TrackEffect>,
        after: Vec<TrackEffect>,
    },
    /// Any change to the Pan/Crop of a video event (a key added, moved or removed, the stretch
    /// switch): the whole Pan/Crop before and after, so it is one exactly reversible undo step.
    SetPanCrop {
        track_id: TrackId,
        event_id: EventId,
        before: PanCrop,
        after: PanCrop,
    },
    /// Any change to the motion of a video track (a key added, moved or removed): the whole motion
    /// before and after, so it is one exactly reversible undo step.
    SetTrackMotion {
        track_id: TrackId,
        before: PanCrop,
        after: PanCrop,
    },
    /// The video transition that reveals (`outgoing: false`) or hides an event: the transition
    /// before and after, so it is one exactly reversible undo step.
    SetTransition {
        track_id: TrackId,
        event_id: EventId,
        outgoing: bool,
        before: Option<TrackEffect>,
        after: Option<TrackEffect>,
    },
    /// The velocity envelope of a video event (created, edited or removed): the envelope before and
    /// after, so it is one exactly reversible undo step.
    SetVelocity {
        track_id: TrackId,
        event_id: EventId,
        before: Option<TrackEnvelope>,
        after: Option<TrackEnvelope>,
    },
    /// The color a video track fades to.
    SetFadeColor {
        track_id: TrackId,
        before: [u8; 3],
        after: [u8; 3],
    },
    /// Any change to a video effect chain (add, remove, reorder, bypass, parameter): the whole
    /// chain before and after, so it is one exactly reversible undo step.
    SetVideoEffects {
        target: VideoFxTarget,
        before: Vec<TrackEffect>,
        after: Vec<TrackEffect>,
    },
    /// Several commands that are one undo step (an import that adds tracks and events): applied in
    /// order, reverted in reverse order; if one does not apply, the ones before it are reverted.
    Batch(Vec<EditCommand>),
    /// Any change to the subtitles (import, add, edit, remove): the whole list before and after, so
    /// it is one exactly reversible undo step.
    ReplaceCaptions {
        before: Vec<Caption>,
        after: Vec<Caption>,
    },
    /// Any change to a track's mixer controls (mute, solo, fader, pan, phase): one undo step.
    SetTrackMix {
        track_id: TrackId,
        before: TrackMix,
        after: TrackMix,
    },
    /// The primary color correction of the video.
    SetVideoGrade {
        before: Grade,
        after: Grade,
    },
    /// Chroma key (green/blue screen) of the video.
    SetChromaKey {
        before: ChromaKey,
        after: ChromaKey,
    },
    /// The color table of the video (a `.cube` path), or none.
    SetVideoLut {
        before: Option<String>,
        after: Option<String>,
    },
    /// The pan law of the project.
    SetPanLaw {
        before: PanLaw,
        after: PanLaw,
    },
    /// Any change to a track's auto-ducking (on / off, controllers, amounts): one undo step.
    SetDucking {
        track_id: TrackId,
        before: Option<TrackDucking>,
        after: Option<TrackDucking>,
    },
    /// Any change to automation envelopes on a track or on the master bus: one undo step.
    SetEnvelopes {
        target: FxTarget,
        before: Vec<TrackEnvelope>,
        after: Vec<TrackEnvelope>,
    },
}

impl EditCommand {
    /// Builds a split at `at` for the event `event_id`, or `None` when `at` is not strictly inside it.
    pub fn split(
        timeline: &mut Timeline,
        track_id: TrackId,
        event_id: EventId,
        at: f64,
    ) -> Option<Self> {
        let event = timeline
            .track(track_id)?
            .events
            .iter()
            .find(|e| e.id == event_id)?;
        if at <= event.start + TIME_EPSILON || at >= event.end() - TIME_EPSILON {
            return None;
        }
        let (original_duration, original_fade_out) = (event.duration, event.fade_out);
        let new_event_id = timeline.alloc_id();
        Some(Self::SplitEvent {
            track_id,
            event_id,
            new_event_id,
            at,
            original_duration,
            original_fade_out,
        })
    }

    fn apply(&self, tl: &mut Timeline) -> bool {
        match self {
            Self::AddTrack { track } => {
                tl.tracks.push(track.clone());
                true
            }
            Self::AddEvent { track_id, event } => match tl.track_mut(*track_id) {
                Some(t) => {
                    t.events.push(event.clone());
                    t.events.sort_by(|a, b| a.start.total_cmp(&b.start));
                    true
                }
                None => false,
            },
            Self::RemoveEvent {
                track_id, event, ..
            } => match tl.track_mut(*track_id) {
                Some(t) => {
                    t.events.retain(|e| e.id != event.id);
                    true
                }
                None => false,
            },
            Self::MoveEvent {
                track_id,
                event_id,
                new_start,
                ..
            } => set_start(tl, *track_id, *event_id, *new_start),
            Self::EditEvent {
                from_track,
                to_track,
                event_id,
                new,
                ..
            } => relocate(tl, *from_track, *to_track, *event_id, *new),
            Self::ReplaceEvents { changes } => {
                if changes.iter().any(|c| tl.track(c.track_id).is_none()) {
                    return false;
                }
                for c in changes {
                    if let Some(t) = tl.track_mut(c.track_id) {
                        t.events = c.after.clone();
                    }
                }
                true
            }
            Self::ReplaceMarks { after, .. } => {
                tl.set_marks(after);
                true
            }
            Self::Batch(commands) => {
                for (i, command) in commands.iter().enumerate() {
                    if !command.apply(tl) {
                        for done in commands[..i].iter().rev() {
                            done.revert(tl);
                        }
                        return false;
                    }
                }
                true
            }
            Self::ReplaceCaptions { after, .. } => {
                tl.captions = after.clone();
                true
            }
            Self::SetTrackMix {
                track_id, after, ..
            } => set_track_mix(tl, *track_id, *after),
            Self::SetPanLaw { after, .. } => {
                tl.pan_law = *after;
                true
            }
            Self::SetVideoGrade { after, .. } => {
                tl.video_grade = after.sanitized();
                true
            }
            Self::SetChromaKey { after, .. } => {
                tl.chroma_key = after.sanitized();
                true
            }
            Self::SetVideoLut { after, .. } => {
                tl.video_lut = after.clone();
                true
            }
            Self::SetEffects { target, after, .. } => set_effects(tl, *target, after),
            Self::SetVideoEffects { target, after, .. } => set_video_effects(tl, *target, after),
            Self::SetPanCrop {
                track_id,
                event_id,
                after,
                ..
            } => set_pan_crop(tl, *track_id, *event_id, after),
            Self::SetTrackMotion {
                track_id, after, ..
            } => match tl.track_mut(*track_id) {
                Some(t) => {
                    t.motion = after.clone();
                    true
                }
                None => false,
            },
            Self::SetTransition {
                track_id,
                event_id,
                outgoing,
                after,
                ..
            } => set_transition(tl, *track_id, *event_id, *outgoing, after),
            Self::SetVelocity {
                track_id,
                event_id,
                after,
                ..
            } => set_velocity(tl, *track_id, *event_id, after),
            Self::SetFadeColor {
                track_id, after, ..
            } => match tl.track_mut(*track_id) {
                Some(t) => {
                    t.fade_color = *after;
                    true
                }
                None => false,
            },
            Self::SetDucking {
                track_id, after, ..
            } => set_ducking(tl, *track_id, after),
            Self::SetEnvelopes { target, after, .. } => set_envelopes(tl, *target, after),
            Self::SplitEvent {
                track_id,
                event_id,
                new_event_id,
                at,
                ..
            } => {
                let Some(t) = tl.track_mut(*track_id) else {
                    return false;
                };
                let Some(i) = t.events.iter().position(|e| e.id == *event_id) else {
                    return false;
                };
                let left = &mut t.events[i];
                let left_duration = at - left.start;
                let right = TrackEvent {
                    group: None,
                    id: *new_event_id,
                    name: left.name.clone(),
                    media_id: left.media_id,
                    start: *at,
                    duration: left.duration - left_duration,
                    trim_in: left.trim_in
                        + left.playback_rate
                            * crate::velocity::source_offset(left.velocity.as_ref(), left_duration),
                    fade_in: 0.0,
                    fade_out: left.fade_out,
                    fade_in_curve: FadeCurve::default(),
                    fade_out_curve: left.fade_out_curve,
                    playback_rate: left.playback_rate,
                    preserve_pitch: left.preserve_pitch,
                    muted: left.muted,
                    locked: left.locked,
                    invert_phase: left.invert_phase,
                    looping: left.looping,
                    normalize: left.normalize,
                    normalize_gain: left.normalize_gain,
                    channels: left.channels,
                    video_effects: left.video_effects.clone(),
                    // The left half keeps its keys (so undoing the split restores it as it was)
                    pan_crop: left.pan_crop.from_offset(left_duration),
                    // The new event starts at the cut without a transition; it ends where the
                    // old one did (the left half's fade-out is cleared, so its copy is inert)
                    transition_in: None,
                    transition_out: left.transition_out.clone(),
                    velocity: crate::velocity::from_offset(left.velocity.as_ref(), left_duration),
                    adjustment: left.adjustment,
                };
                left.duration = left_duration;
                left.fade_out = 0.0;
                t.events.insert(i + 1, right);
                true
            }
        }
    }

    fn revert(&self, tl: &mut Timeline) {
        match self {
            Self::AddTrack { track } => tl.tracks.retain(|t| t.id != track.id),
            Self::AddEvent { track_id, event } => {
                if let Some(t) = tl.track_mut(*track_id) {
                    t.events.retain(|e| e.id != event.id);
                }
            }
            Self::RemoveEvent {
                track_id,
                index,
                event,
            } => {
                if let Some(t) = tl.track_mut(*track_id) {
                    t.events.insert((*index).min(t.events.len()), event.clone());
                }
            }
            Self::MoveEvent {
                track_id,
                event_id,
                old_start,
                ..
            } => {
                set_start(tl, *track_id, *event_id, *old_start);
            }
            Self::EditEvent {
                from_track,
                to_track,
                event_id,
                old,
                ..
            } => {
                relocate(tl, *to_track, *from_track, *event_id, *old);
            }
            Self::ReplaceEvents { changes } => {
                for c in changes {
                    if let Some(t) = tl.track_mut(c.track_id) {
                        t.events = c.before.clone();
                    }
                }
            }
            Self::ReplaceMarks { before, .. } => tl.set_marks(before),
            Self::Batch(commands) => {
                for command in commands.iter().rev() {
                    command.revert(tl);
                }
            }
            Self::ReplaceCaptions { before, .. } => tl.captions = before.clone(),
            Self::SetTrackMix {
                track_id, before, ..
            } => {
                set_track_mix(tl, *track_id, *before);
            }
            Self::SetPanLaw { before, .. } => tl.pan_law = *before,
            Self::SetVideoGrade { before, .. } => tl.video_grade = before.sanitized(),
            Self::SetChromaKey { before, .. } => tl.chroma_key = before.sanitized(),
            Self::SetVideoLut { before, .. } => tl.video_lut = before.clone(),
            Self::SetEffects { target, before, .. } => {
                set_effects(tl, *target, before);
            }
            Self::SetVideoEffects { target, before, .. } => {
                set_video_effects(tl, *target, before);
            }
            Self::SetPanCrop {
                track_id,
                event_id,
                before,
                ..
            } => {
                set_pan_crop(tl, *track_id, *event_id, before);
            }
            Self::SetTrackMotion {
                track_id, before, ..
            } => {
                if let Some(t) = tl.track_mut(*track_id) {
                    t.motion = before.clone();
                }
            }
            Self::SetTransition {
                track_id,
                event_id,
                outgoing,
                before,
                ..
            } => {
                set_transition(tl, *track_id, *event_id, *outgoing, before);
            }
            Self::SetVelocity {
                track_id,
                event_id,
                before,
                ..
            } => {
                set_velocity(tl, *track_id, *event_id, before);
            }
            Self::SetFadeColor {
                track_id, before, ..
            } => {
                if let Some(t) = tl.track_mut(*track_id) {
                    t.fade_color = *before;
                }
            }
            Self::SetDucking {
                track_id, before, ..
            } => {
                set_ducking(tl, *track_id, before);
            }
            Self::SetEnvelopes { target, before, .. } => {
                set_envelopes(tl, *target, before);
            }
            Self::SplitEvent {
                track_id,
                event_id,
                new_event_id,
                original_duration,
                original_fade_out,
                ..
            } => {
                if let Some(t) = tl.track_mut(*track_id) {
                    t.events.retain(|e| e.id != *new_event_id);
                    if let Some(e) = t.events.iter_mut().find(|e| e.id == *event_id) {
                        e.duration = *original_duration;
                        e.fade_out = *original_fade_out;
                    }
                }
            }
        }
    }
}

fn set_track_mix(tl: &mut Timeline, track_id: TrackId, mix: TrackMix) -> bool {
    match tl.track_mut(track_id) {
        Some(t) => {
            t.set_mix(mix);
            true
        }
        None => false,
    }
}

fn set_ducking(tl: &mut Timeline, track_id: TrackId, ducking: &Option<TrackDucking>) -> bool {
    match tl.track_mut(track_id) {
        Some(t) => {
            t.ducking = ducking.clone();
            true
        }
        None => false,
    }
}

fn set_envelopes(tl: &mut Timeline, target: FxTarget, envelopes: &[TrackEnvelope]) -> bool {
    match target {
        FxTarget::Master => {
            tl.master_envelopes = envelopes.to_vec();
            true
        }
        FxTarget::Track(id) => match tl.track_mut(id) {
            Some(t) => {
                t.envelopes = envelopes.to_vec();
                true
            }
            None => false,
        },
    }
}

fn set_velocity(
    tl: &mut Timeline,
    track_id: TrackId,
    event_id: EventId,
    value: &Option<TrackEnvelope>,
) -> bool {
    let event = tl
        .track_mut(track_id)
        .and_then(|t| t.events.iter_mut().find(|e| e.id == event_id));
    match event {
        Some(e) => {
            e.velocity = value.clone();
            true
        }
        None => false,
    }
}

fn set_transition(
    tl: &mut Timeline,
    track_id: TrackId,
    event_id: EventId,
    outgoing: bool,
    value: &Option<TrackEffect>,
) -> bool {
    let event = tl
        .track_mut(track_id)
        .and_then(|t| t.events.iter_mut().find(|e| e.id == event_id));
    match event {
        Some(e) => {
            if outgoing {
                e.transition_out = value.clone();
            } else {
                e.transition_in = value.clone();
            }
            true
        }
        None => false,
    }
}

fn set_pan_crop(tl: &mut Timeline, track_id: TrackId, event_id: EventId, value: &PanCrop) -> bool {
    let event = tl
        .track_mut(track_id)
        .and_then(|t| t.events.iter_mut().find(|e| e.id == event_id));
    match event {
        Some(e) => {
            e.pan_crop = value.clone();
            true
        }
        None => false,
    }
}

fn set_video_effects(tl: &mut Timeline, target: VideoFxTarget, chain: &[TrackEffect]) -> bool {
    match target {
        VideoFxTarget::Project => {
            tl.video_effects = chain.to_vec();
            true
        }
        VideoFxTarget::Media(id) => {
            if chain.is_empty() {
                tl.media_video_effects.remove(&id);
            } else {
                tl.media_video_effects.insert(id, chain.to_vec());
            }
            true
        }
        VideoFxTarget::Track(id) => match tl.track_mut(id) {
            Some(t) => {
                t.video_effects = chain.to_vec();
                true
            }
            None => false,
        },
        VideoFxTarget::Event { track_id, event_id } => {
            let event = tl
                .track_mut(track_id)
                .and_then(|t| t.events.iter_mut().find(|e| e.id == event_id));
            match event {
                Some(e) => {
                    e.video_effects = chain.to_vec();
                    true
                }
                None => false,
            }
        }
    }
}

fn set_effects(tl: &mut Timeline, target: FxTarget, chain: &[TrackEffect]) -> bool {
    match target {
        FxTarget::Master => {
            tl.master_effects = chain.to_vec();
            true
        }
        FxTarget::Track(id) => match tl.track_mut(id) {
            Some(t) => {
                t.effects = chain.to_vec();
                true
            }
            None => false,
        },
    }
}

/// Moves `event_id` from one track to another (or within a track) and sets its geometry.
fn relocate(
    tl: &mut Timeline,
    from: TrackId,
    to: TrackId,
    event_id: EventId,
    geometry: EventGeometry,
) -> bool {
    if tl.track(to).is_none() {
        return false;
    }
    let Some(source) = tl.track_mut(from) else {
        return false;
    };
    let Some(index) = source.events.iter().position(|e| e.id == event_id) else {
        return false;
    };
    let mut event = source.events.remove(index);
    event.set_geometry(geometry);
    if let Some(target) = tl.track_mut(to) {
        target.events.push(event);
        target.events.sort_by(|a, b| a.start.total_cmp(&b.start));
    }
    true
}

fn set_start(tl: &mut Timeline, track_id: TrackId, event_id: EventId, start: f64) -> bool {
    let Some(t) = tl.track_mut(track_id) else {
        return false;
    };
    let Some(e) = t.events.iter_mut().find(|e| e.id == event_id) else {
        return false;
    };
    e.start = start.max(0.0);
    t.events.sort_by(|a, b| a.start.total_cmp(&b.start));
    true
}

/// Undo/redo stacks. Executing a new command clears the redo stack.
#[derive(Debug, Default)]
pub struct History {
    undo: Vec<EditCommand>,
    redo: Vec<EditCommand>,
    /// Bumped by every execute/undo/redo; lets callers detect unsaved changes cheaply.
    revision: u64,
}

impl History {
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies `command`; returns `false` (and records nothing) when it does not apply.
    pub fn execute(&mut self, timeline: &mut Timeline, command: EditCommand) -> bool {
        // A trim or slide of an event with a velocity envelope keeps the picture in place
        let command = crate::velocity::keep_picture(timeline, command);
        if !command.apply(timeline) {
            return false;
        }
        self.undo.push(command);
        self.redo.clear();
        self.revision += 1;
        true
    }

    pub fn undo(&mut self, timeline: &mut Timeline) -> bool {
        match self.undo.pop() {
            Some(cmd) => {
                cmd.revert(timeline);
                self.redo.push(cmd);
                self.revision += 1;
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self, timeline: &mut Timeline) -> bool {
        match self.redo.pop() {
            Some(cmd) => {
                cmd.apply(timeline);
                self.undo.push(cmd);
                self.revision += 1;
                true
            }
            None => false,
        }
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::TrackKind;

    fn timeline_with_event() -> (Timeline, History, TrackId, EventId) {
        let mut tl = Timeline::new();
        let mut history = History::new();
        let track = tl.new_track(TrackKind::Audio);
        let track_id = track.id;
        assert!(history.execute(&mut tl, EditCommand::AddTrack { track }));
        let event_id = tl.alloc_id();
        let event = TrackEvent {
            group: None,
            id: event_id,
            name: "clip".into(),
            media_id: Some(1),
            start: 2.0,
            duration: 10.0,
            trim_in: 1.0,
            fade_in: 0.0,
            fade_out: 0.5,
            ..Default::default()
        };
        assert!(history.execute(&mut tl, EditCommand::AddEvent { track_id, event }));
        (tl, history, track_id, event_id)
    }

    #[test]
    fn new_timeline_is_empty() {
        let tl = Timeline::new();
        assert!(tl.tracks.is_empty());
        assert_eq!(tl.duration(), 0.0);
    }

    #[test]
    fn split_divides_the_event_and_keeps_source_alignment() {
        let (mut tl, mut history, track_id, event_id) = timeline_with_event();
        let cmd = EditCommand::split(&mut tl, track_id, event_id, 5.0).unwrap();
        assert!(history.execute(&mut tl, cmd));

        let events = &tl.track(track_id).unwrap().events;
        assert_eq!(events.len(), 2);
        assert_eq!(
            (events[0].start, events[0].duration, events[0].fade_out),
            (2.0, 3.0, 0.0)
        );
        assert_eq!(
            (events[1].start, events[1].duration, events[1].trim_in),
            (5.0, 7.0, 4.0)
        );
        assert_eq!(events[1].fade_out, 0.5);
    }

    #[test]
    fn split_outside_the_event_is_rejected() {
        let (mut tl, _, track_id, event_id) = timeline_with_event();
        assert!(EditCommand::split(&mut tl, track_id, event_id, 2.0).is_none());
        assert!(EditCommand::split(&mut tl, track_id, event_id, 12.0).is_none());
        assert!(EditCommand::split(&mut tl, track_id, event_id, 20.0).is_none());
    }

    #[test]
    fn undo_and_redo_restore_the_exact_state() {
        let (mut tl, mut history, track_id, event_id) = timeline_with_event();
        let before = tl.clone();

        let cmd = EditCommand::split(&mut tl, track_id, event_id, 5.0).unwrap();
        history.execute(&mut tl, cmd);
        let after = tl.clone();

        assert!(history.undo(&mut tl));
        assert_eq!(tl.tracks, before.tracks);
        assert!(history.redo(&mut tl));
        assert_eq!(tl.tracks, after.tracks);
    }

    #[test]
    fn move_and_remove_are_undoable() {
        let (mut tl, mut history, track_id, event_id) = timeline_with_event();
        history.execute(
            &mut tl,
            EditCommand::MoveEvent {
                track_id,
                event_id,
                old_start: 2.0,
                new_start: 8.0,
            },
        );
        assert_eq!(tl.track(track_id).unwrap().events[0].start, 8.0);
        history.undo(&mut tl);
        assert_eq!(tl.track(track_id).unwrap().events[0].start, 2.0);

        let event = tl.track(track_id).unwrap().events[0].clone();
        history.execute(
            &mut tl,
            EditCommand::RemoveEvent {
                track_id,
                index: 0,
                event,
            },
        );
        assert!(tl.track(track_id).unwrap().events.is_empty());
        history.undo(&mut tl);
        assert_eq!(tl.track(track_id).unwrap().events.len(), 1);
    }

    #[test]
    fn revision_changes_with_every_edit() {
        let (mut tl, mut history, track_id, event_id) = timeline_with_event();
        let r0 = history.revision();
        history.execute(
            &mut tl,
            EditCommand::MoveEvent {
                track_id,
                event_id,
                old_start: 2.0,
                new_start: 3.0,
            },
        );
        assert_ne!(history.revision(), r0);
        let r1 = history.revision();
        history.undo(&mut tl);
        assert_ne!(history.revision(), r1);
        // a rejected command is not a change
        let r2 = history.revision();
        assert!(!history.undo(&mut Timeline::new()) || history.revision() != r2);
    }

    #[test]
    fn edit_event_moves_trims_and_changes_track_in_one_undo_step() {
        let (mut tl, mut history, from_track, event_id) = timeline_with_event();
        let other = tl.new_track(TrackKind::Audio);
        let to_track = other.id;
        history.execute(&mut tl, EditCommand::AddTrack { track: other });
        let before = tl.clone();
        let old = tl.track(from_track).unwrap().events[0].geometry();
        let new = EventGeometry {
            start: 20.0,
            duration: 4.0,
            trim_in: 3.0,
            fade_in: 0.0,
            fade_out: 0.5,
            ..Default::default()
        };

        assert!(history.execute(
            &mut tl,
            EditCommand::EditEvent {
                from_track,
                to_track,
                event_id,
                old,
                new
            },
        ));
        assert!(tl.track(from_track).unwrap().events.is_empty());
        assert_eq!(tl.track(to_track).unwrap().events[0].geometry(), new);

        assert!(history.undo(&mut tl));
        assert_eq!(tl.tracks, before.tracks);
        assert!(history.redo(&mut tl));
        assert_eq!(tl.track(to_track).unwrap().events[0].geometry(), new);
    }

    #[test]
    fn edit_event_on_a_missing_track_is_rejected() {
        let (mut tl, mut history, from_track, event_id) = timeline_with_event();
        let old = tl.track(from_track).unwrap().events[0].geometry();
        let applied = history.execute(
            &mut tl,
            EditCommand::EditEvent {
                from_track,
                to_track: 999,
                event_id,
                old,
                new: old,
            },
        );
        assert!(!applied);
        assert_eq!(tl.track(from_track).unwrap().events.len(), 1);
    }

    #[test]
    fn a_new_edit_clears_redo() {
        let (mut tl, mut history, track_id, event_id) = timeline_with_event();
        history.execute(
            &mut tl,
            EditCommand::MoveEvent {
                track_id,
                event_id,
                old_start: 2.0,
                new_start: 3.0,
            },
        );
        history.undo(&mut tl);
        assert!(history.can_redo());
        history.execute(
            &mut tl,
            EditCommand::MoveEvent {
                track_id,
                event_id,
                old_start: 2.0,
                new_start: 4.0,
            },
        );
        assert!(!history.can_redo());
    }
}
