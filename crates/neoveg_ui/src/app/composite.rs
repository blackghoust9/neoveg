//! The multi-track composite of one frame: which clips of which video tracks are visible at a
//! time, how far into their sources they are and how opaque each one is. The preview and *Make
//! Movie* build their requests here, so what is exported is what was seen.
//!
//! Tracks stack the way the timeline shows them: the first track is the top layer, so the request
//! lists them bottom to top. On one track, events that overlap are ordered by start (the later one
//! on top) and, with automatic crossfades on, the later one fades in over the overlap: blending it
//! over the earlier one at that opacity is the crossfade, so the earlier event keeps its own
//! opacity (fading both would dip through black).

use super::*;
use crate::pan_crop_window::{EditTarget, PanCropDraft};
use crate::velocity_editor::VelocityDraft;
use crate::video_fx_panel::{effects_for, VfxDraft};
use neoveg_common::composite::CompositeMode;
use neoveg_engine::velocity;
use neoveg_video::pan_crop::PanCropRequest;
use neoveg_video::worker::{CompositeFrameRequest, TrackCompositeNode, VideoClipRequest};

/// Where a media id is on disk and how it plays.
#[derive(Debug, Clone)]
pub(super) struct Source {
    pub path: PathBuf,
    /// Frame rate of the source (`0` when unknown: the decoder then assumes 29.97).
    pub fps: f64,
    pub duration: f64,
}

/// What every frame of a request shares: the picture settings of the project.
pub(super) struct FrameSettings {
    pub fps: f64,
    pub canvas: (u32, u32),
    /// Largest decoded width (`0` = full size).
    pub max_width: u32,
    pub playing: bool,
    pub auto_crossfade: bool,
    pub lut: Option<Arc<Lut>>,
    pub grade: Grade,
    pub chroma_key: ChromaKey,
    /// A video effect slider that is being dragged: the preview shows its value live.
    pub fx_draft: Option<VfxDraft>,
    /// A Pan/Crop being edited: the preview shows it instead of the event's own.
    pub pan_crop_draft: Option<PanCropDraft>,
    /// A velocity envelope being edited: the preview shows it instead of the event's own.
    pub velocity_draft: Option<VelocityDraft>,
}

/// Where the layer of `track` sits in the frame at timeline `time` (the motion being edited, when
/// there is one), or `None` for a track that does not move.
fn track_motion_frame(
    track: &neoveg_engine::Track,
    time: f64,
    settings: &FrameSettings,
) -> Option<neoveg_common::pan_crop::PanCropFrame> {
    let stored = match settings
        .pan_crop_draft
        .as_ref()
        .filter(|d| d.target == EditTarget::Track(track.id))
    {
        Some(draft) => &draft.pan_crop,
        None => &track.motion,
    };
    (!stored.is_default()).then(|| stored.frame_at(time))
}

/// The Pan/Crop request of `event` at timeline `time`: its keys (or the draft being edited) at the
/// time since the event started, or `None` for an event without Pan/Crop.
fn pan_crop_request(
    event: &TrackEvent,
    track_id: TrackId,
    time: f64,
    settings: &FrameSettings,
) -> Option<PanCropRequest> {
    let stored = match settings
        .pan_crop_draft
        .as_ref()
        .filter(|d| d.target == EditTarget::Event(track_id, event.id))
    {
        Some(draft) => &draft.pan_crop,
        None => &event.pan_crop,
    };
    if stored.is_default() {
        return None;
    }
    Some(PanCropRequest {
        frame: stored.frame_at((time - event.start).max(0.0)),
        stretch_to_fill: stored.stretch_to_fill,
    })
}

fn ramp(curve: FadeCurve, length: f64, position: f64) -> f64 {
    if length > 0.0 && position < length {
        curve.gain((position / length).clamp(0.0, 1.0))
    } else {
        1.0
    }
}

/// Gain of the fade-in of `event` at `time` (and of the overlap with the event before it).
pub(super) fn in_ramp(event: &TrackEvent, time: f64, cross_in: f64) -> f64 {
    let local = time - event.start;
    ramp(event.fade_in_curve, event.fade_in, local) * ramp(event.fade_in_curve, cross_in, local)
}

/// Gain of the fade-out of `event` at `time`.
pub(super) fn out_ramp(event: &TrackEvent, time: f64) -> f64 {
    ramp(
        event.fade_out_curve,
        event.fade_out,
        (event.end() - time).max(0.0),
    )
}

/// The opacity of a clip and the transitions in progress on it. A fade that has a transition is not
/// an opacity any more: the transition draws it (the gain of the fade is its progress) and the clip
/// stays opaque; a fade with the plain dissolve stays an opacity. `0.0` opacity means not visible.
pub(super) fn clip_look(
    event: &TrackEvent,
    time: f64,
    cross_in: f64,
) -> (f32, Vec<neoveg_video::transitions::TransitionRequest>) {
    let (gain_in, gain_out) = (in_ramp(event, time, cross_in), out_ramp(event, time));
    if gain_in <= 0.0 || gain_out <= 0.0 {
        return (0.0, Vec::new());
    }
    let mut list = Vec::new();
    let mut part =
        |gain: f64, setting: &Option<neoveg_engine::TrackEffect>, entering: bool| -> f32 {
            if gain < 1.0 {
                if let Some(mut request) =
                    crate::transitions_panel::to_request(setting.as_ref(), gain as f32)
                {
                    request.entering = entering;
                    list.push(request);
                    return 1.0;
                }
            }
            gain as f32
        };
    let opacity =
        part(gain_in, &event.transition_in, true) * part(gain_out, &event.transition_out, false);
    (opacity, list)
}

/// The velocity envelope of `event` as the preview must see it: the one being edited when there
/// is a draft for this event, else the stored one.
fn velocity_of<'a>(
    event: &'a TrackEvent,
    track_id: u64,
    settings: &'a FrameSettings,
) -> Option<&'a neoveg_engine::TrackEnvelope> {
    match settings
        .velocity_draft
        .as_ref()
        .filter(|d| d.track_id == track_id && d.event_id == event.id)
    {
        Some(draft) => draft.envelope.as_ref(),
        None => event.velocity.as_ref(),
    }
}

/// Seconds into the source of the event's media at timeline `time`, honouring speed, the velocity
/// envelope and Loop.
pub(super) fn source_seconds(event: &TrackEvent, time: f64, source_duration: f64) -> f64 {
    source_seconds_with(event, event.velocity.as_ref(), time, source_duration)
}

/// [`source_seconds`] with an explicit velocity envelope (the one being edited, for instance).
///
/// The event keeps its length; the envelope only decides how much of the source plays in it. A speed
/// below zero plays backwards and stops at the start of the media (the first frame is held), and an
/// event that runs past the end of the media holds the last frame.
pub(super) fn source_seconds_with(
    event: &TrackEvent,
    velocity: Option<&neoveg_engine::TrackEnvelope>,
    time: f64,
    source_duration: f64,
) -> f64 {
    let rate = edit::clamp_playback_rate(event.playback_rate);
    let local = time - event.start;
    let shaped = velocity.is_some_and(|v| v.enabled && !v.points.is_empty());
    let mut offset = if shaped {
        rate * velocity::source_offset(velocity, local)
    } else {
        local * rate
    };
    if event.looping {
        offset = edit::wrap_loop(offset, edit::loop_length(source_duration, event.trim_in));
    }
    let seconds = offset + event.trim_in;
    if shaped {
        let end = if source_duration.is_finite() && source_duration > 0.0 {
            source_duration
        } else {
            f64::INFINITY
        };
        seconds.clamp(0.0, end)
    } else {
        seconds
    }
}

/// Number of video clips visible at `time` (muted events and muted tracks do not count).
pub(super) fn visible_clips(timeline: &Timeline, time: f64) -> usize {
    timeline
        .tracks
        .iter()
        .filter(|t| t.kind == TrackKind::Video && timeline.audible(t))
        .map(|t| {
            t.events
                .iter()
                .filter(|e| {
                    !e.muted && time >= e.start - TIME_EPSILON && time < e.end() - TIME_EPSILON
                })
                .count()
        })
        .sum()
}

/// The request for the frame at `time`, or `None` when no video clip is visible there (black).
pub(super) fn composite_request(
    timeline: &Timeline,
    time: f64,
    settings: &FrameSettings,
    resolve: &dyn Fn(u64) -> Option<Source>,
) -> Option<CompositeFrameRequest> {
    let mut nodes = Vec::new();
    // The first track is the top layer, so walk them from the bottom up
    for track in timeline
        .tracks
        .iter()
        .rev()
        .filter(|t| t.kind == TrackKind::Video && timeline.audible(t))
    {
        // The track's level, fade to color and mute envelopes at this instant
        let track_state = envelope::video_track_state(track, time);
        if track_state.muted {
            continue;
        }
        let crossfades = if settings.auto_crossfade {
            find_crossfades(&track.events)
        } else {
            Vec::new()
        };
        let mut active: Vec<&TrackEvent> = track
            .events
            .iter()
            .filter(|e| !e.muted && time >= e.start - TIME_EPSILON && time < e.end() - TIME_EPSILON)
            .collect();
        active.sort_by(|a, b| a.start.total_cmp(&b.start));
        // Adjustment events of this track act on the tracks below it, so they go before its own layer
        for e in active.iter().filter(|e| e.adjustment) {
            let cross_in = crossfades
                .iter()
                .filter(|c| c.later == e.id)
                .map(Crossfade::length)
                .fold(0.0, f64::max);
            let (opacity, _) = clip_look(e, time, cross_in);
            nodes.push(TrackCompositeNode::Adjust {
                effects: effects_for(
                    &e.video_effects,
                    VideoFxTarget::Event {
                        track_id: track.id,
                        event_id: e.id,
                    },
                    settings.fx_draft.as_ref(),
                ),
                opacity: opacity * track_state.level,
            });
        }
        let clips: Vec<VideoClipRequest> = active
            .into_iter()
            .filter_map(|e| {
                let media_id = e.media_id?;
                let source = resolve(media_id)?;
                let cross_in = crossfades
                    .iter()
                    .filter(|c| c.later == e.id)
                    .map(Crossfade::length)
                    .fold(0.0, f64::max);
                let (opacity, transitions) = clip_look(e, time, cross_in);
                let speed = velocity_of(e, track.id, settings);
                Some(VideoClipRequest {
                    media_id,
                    seconds: source_seconds_with(e, speed, time, source.duration),
                    source_fps: source.fps,
                    path: source.path,
                    // The sign says which way the picture runs (the decoder reads ahead with it)
                    playback_rate: edit::clamp_playback_rate(e.playback_rate)
                        * velocity::rate_at(speed, time - e.start),
                    event_opacity: opacity,
                    transitions,
                    apply_chroma: true,
                    blend: e.resample.blends(),
                    pan_crop: pan_crop_request(e, track.id, time, settings),
                    effects: {
                        // The media's chain runs first, then the event's own
                        let stored = timeline
                            .media_video_effects
                            .get(&media_id)
                            .map_or(&[][..], Vec::as_slice);
                        let mut chain = effects_for(
                            stored,
                            VideoFxTarget::Media(media_id),
                            settings.fx_draft.as_ref(),
                        );
                        chain.extend(effects_for(
                            &e.video_effects,
                            VideoFxTarget::Event {
                                track_id: track.id,
                                event_id: e.id,
                            },
                            settings.fx_draft.as_ref(),
                        ));
                        chain
                    },
                })
            })
            .collect();
        if !clips.is_empty() {
            nodes.push(TrackCompositeNode::Track {
                clips,
                mode: CompositeMode::SourceAlpha,
                level: track_state.level,
                effects: effects_for(
                    &track.video_effects,
                    VideoFxTarget::Track(track.id),
                    settings.fx_draft.as_ref(),
                ),
                fade_color: track.fade_color,
                fade_amount: track_state.fade,
                motion: track_motion_frame(track, time, settings),
            });
        }
    }
    // Adjustments with no picture under them show nothing
    if nodes
        .iter()
        .all(|n| matches!(n, TrackCompositeNode::Adjust { .. }))
    {
        return None;
    }
    Some(CompositeFrameRequest {
        timeline_seconds: time,
        fps: settings.fps,
        playing: settings.playing,
        shuttle_speed: 1.0,
        canvas_width: settings.canvas.0,
        canvas_height: settings.canvas.1,
        max_width: settings.max_width,
        nodes,
        lut: settings.lut.clone(),
        grade: settings.grade,
        chroma_key: settings.chroma_key,
        project_effects: effects_for(
            &timeline.video_effects,
            VideoFxTarget::Project,
            settings.fx_draft.as_ref(),
        ),
    })
}

/// `true` when a clip visible at `time` has a video effect that is on (media, event, track) or
/// the project has one: the preview then goes through the composite path, the one that runs them.
pub(super) fn has_effects(timeline: &Timeline, time: f64) -> bool {
    let on = |chain: &[neoveg_engine::TrackEffect]| chain.iter().any(|e| !e.bypass);
    let mut visible = false;
    for track in timeline
        .tracks
        .iter()
        .filter(|t| t.kind == TrackKind::Video && timeline.audible(t))
    {
        for e in track
            .events
            .iter()
            .filter(|e| !e.muted && time >= e.start - TIME_EPSILON && time < e.end() - TIME_EPSILON)
        {
            visible = true;
            let media_on = e
                .media_id
                .and_then(|m| timeline.media_video_effects.get(&m))
                .is_some_and(|chain| on(chain));
            let pan_crop_on = !e.pan_crop.is_default() || !track.motion.is_default();
            let transition_on = e.transition_in.is_some() || e.transition_out.is_some();
            let velocity_on = e.velocity.as_ref().is_some_and(|v| v.enabled)
                || e.adjustment
                || e.resample.blends();
            let envelope_on = track
                .envelopes
                .iter()
                .any(|env| env.enabled && env.target.is_video());
            if on(&e.video_effects)
                || on(&track.video_effects)
                || media_on
                || envelope_on
                || pan_crop_on
                || transition_on
                || velocity_on
            {
                return true;
            }
        }
    }
    visible && on(&timeline.video_effects)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> FrameSettings {
        FrameSettings {
            fps: 25.0,
            canvas: (640, 360),
            max_width: 0,
            playing: false,
            auto_crossfade: true,
            lut: None,
            grade: Grade::default(),
            chroma_key: ChromaKey::default(),
            fx_draft: None,
            pan_crop_draft: None,
            velocity_draft: None,
        }
    }

    fn resolve(id: u64) -> Option<Source> {
        Some(Source {
            path: PathBuf::from(format!("m{id}.mp4")),
            fps: 25.0,
            duration: 10.0,
        })
    }

    fn event(tl: &mut Timeline, media: u64, start: f64, duration: f64) -> TrackEvent {
        TrackEvent {
            id: tl.alloc_id(),
            media_id: Some(media),
            start,
            duration,
            ..TrackEvent::default()
        }
    }

    fn clips_of(request: &CompositeFrameRequest) -> Vec<Vec<(u64, f32)>> {
        request
            .nodes
            .iter()
            .map(|n| match n {
                TrackCompositeNode::Track { clips, .. } => clips
                    .iter()
                    .map(|c| (c.media_id, c.event_opacity))
                    .collect(),
                TrackCompositeNode::Group { .. } => panic!("no groups here"),
                TrackCompositeNode::Adjust { .. } => panic!("no adjustments here"),
            })
            .collect()
    }

    #[test]
    fn nothing_visible_is_black() {
        let mut tl = Timeline::new();
        let mut track = tl.new_track(TrackKind::Video);
        let e = event(&mut tl, 1, 5.0, 2.0);
        track.events.push(e);
        tl.tracks.push(track);
        assert!(composite_request(&tl, 1.0, &settings(), &resolve).is_none());
        assert_eq!(visible_clips(&tl, 1.0), 0);
        assert!(composite_request(&tl, 6.0, &settings(), &resolve).is_some());
    }

    #[test]
    fn the_first_track_is_the_top_layer_so_it_is_listed_last() {
        let mut tl = Timeline::new();
        let (mut top, mut bottom) = (
            tl.new_track(TrackKind::Video),
            tl.new_track(TrackKind::Video),
        );
        let (a, b) = (event(&mut tl, 1, 0.0, 4.0), event(&mut tl, 2, 0.0, 4.0));
        top.events.push(a);
        bottom.events.push(b);
        tl.tracks.extend([top, bottom]);
        let request = composite_request(&tl, 1.0, &settings(), &resolve).unwrap();
        assert_eq!(
            clips_of(&request),
            vec![vec![(2, 1.0)], vec![(1, 1.0)]],
            "bottom track first, top track last"
        );
        assert_eq!(visible_clips(&tl, 1.0), 2);
    }

    #[test]
    fn an_overlap_fades_the_later_event_in_and_leaves_the_earlier_one_alone() {
        let mut tl = Timeline::new();
        let mut track = tl.new_track(TrackKind::Video);
        let (a, b) = (event(&mut tl, 1, 0.0, 6.0), event(&mut tl, 2, 4.0, 6.0));
        track.events.extend([b, a]);
        tl.tracks.push(track);
        // The overlap is 4..6 s: halfway through it the later event is half opaque
        let request = composite_request(&tl, 5.0, &settings(), &resolve).unwrap();
        let clips = clips_of(&request);
        assert_eq!(clips[0].len(), 2);
        assert_eq!(
            clips[0][0],
            (1, 1.0),
            "the earlier event stays solid, underneath"
        );
        assert!((clips[0][1].1 - 0.5).abs() < 1e-6, "{:?}", clips[0][1]);
        assert_eq!(clips[0][1].0, 2);

        let mut off = settings();
        off.auto_crossfade = false;
        let request = composite_request(&tl, 5.0, &off, &resolve).unwrap();
        assert_eq!(
            clips_of(&request)[0][1],
            (2, 1.0),
            "without crossfades it simply covers"
        );
    }

    #[test]
    fn muted_events_and_tracks_are_left_out() {
        let mut tl = Timeline::new();
        let mut track = tl.new_track(TrackKind::Video);
        let mut e = event(&mut tl, 1, 0.0, 4.0);
        e.muted = true;
        track.events.push(e);
        let mut hidden = tl.new_track(TrackKind::Video);
        let e = event(&mut tl, 2, 0.0, 4.0);
        hidden.events.push(e);
        hidden.muted = true;
        tl.tracks.extend([track, hidden]);
        assert!(composite_request(&tl, 1.0, &settings(), &resolve).is_none());
    }

    #[test]
    fn a_fade_out_dims_the_last_part_of_the_event() {
        let mut tl = Timeline::new();
        let mut track = tl.new_track(TrackKind::Video);
        let mut e = event(&mut tl, 1, 0.0, 4.0);
        e.fade_out = 2.0;
        e.fade_out_curve = FadeCurve::Linear;
        track.events.push(e);
        tl.tracks.push(track);
        let request = composite_request(&tl, 3.0, &settings(), &resolve).unwrap();
        assert!((clips_of(&request)[0][0].1 - 0.5).abs() < 1e-6);
    }

    #[test]
    fn source_time_follows_trim_speed_and_loop() {
        let mut e = TrackEvent {
            start: 2.0,
            duration: 30.0,
            trim_in: 1.0,
            ..TrackEvent::default()
        };
        assert_eq!(source_seconds(&e, 5.0, 10.0), 4.0);
        e.playback_rate = 2.0;
        assert_eq!(source_seconds(&e, 5.0, 10.0), 7.0);
        e.playback_rate = 1.0;
        e.looping = true;
        // The source runs 1..10 s, so it repeats every 9 s: 14 s into the event is 5 s in
        assert_eq!(source_seconds(&e, 16.0, 10.0), 6.0);
    }

    #[test]
    fn event_and_track_effects_reach_the_request_without_the_bypassed_ones() {
        use neoveg_engine::TrackEffect;
        let mut tl = Timeline::new();
        let mut track = tl.new_track(TrackKind::Video);
        let mut e = event(&mut tl, 1, 0.0, 4.0);
        let mut off = TrackEffect::new("sharpen");
        off.bypass = true;
        e.video_effects = vec![TrackEffect::new("gaussian_blur"), off];
        track.events.push(e);
        track.video_effects = vec![TrackEffect::new("vignette")];
        tl.tracks.push(track);

        let request = composite_request(&tl, 1.0, &settings(), &resolve).unwrap();
        let TrackCompositeNode::Track { clips, effects, .. } = &request.nodes[0] else {
            panic!("a track node");
        };
        assert_eq!(
            clips[0]
                .effects
                .iter()
                .map(|f| f.kind.as_str())
                .collect::<Vec<_>>(),
            ["gaussian_blur"]
        );
        assert_eq!(
            effects.iter().map(|f| f.kind.as_str()).collect::<Vec<_>>(),
            ["vignette"]
        );
    }

    #[test]
    fn media_chains_run_before_event_chains_and_the_project_chain_rides_the_request() {
        use neoveg_engine::TrackEffect;
        let mut tl = Timeline::new();
        let mut track = tl.new_track(TrackKind::Video);
        let mut e = event(&mut tl, 1, 0.0, 4.0);
        e.video_effects = vec![TrackEffect::new("vignette")];
        let eid = e.id;
        let tid = track.id;
        track.events.push(e);
        tl.tracks.push(track);
        tl.media_video_effects
            .insert(1, vec![TrackEffect::new("gaussian_blur")]);
        tl.video_effects = vec![TrackEffect::new("sharpen")];

        let request = composite_request(&tl, 1.0, &settings(), &resolve).unwrap();
        let TrackCompositeNode::Track { clips, .. } = &request.nodes[0] else {
            panic!("a track node");
        };
        let kinds: Vec<&str> = clips[0].effects.iter().map(|f| f.kind.as_str()).collect();
        assert_eq!(kinds, ["gaussian_blur", "vignette"]);
        assert_eq!(request.project_effects[0].kind, "sharpen");
        assert!(has_effects(&tl, 1.0));
        assert!(!has_effects(&tl, 9.0), "nothing is visible then");

        // A dragged slider shows its value in the request and nowhere else
        let mut live = settings();
        live.fx_draft = Some(VfxDraft {
            target: VideoFxTarget::Event {
                track_id: tid,
                event_id: eid,
            },
            index: 0,
            param: "outer_radius".into(),
            value: 0.7,
        });
        let request = composite_request(&tl, 1.0, &live, &resolve).unwrap();
        let TrackCompositeNode::Track { clips, .. } = &request.nodes[0] else {
            panic!("a track node");
        };
        assert_eq!(clips[0].effects[1].params["outer_radius"], 0.7);
        assert!(clips[0].effects[0].params.is_empty());
        assert!(tl.track(tid).unwrap().events[0].video_effects[0]
            .params
            .is_empty());
    }

    #[test]
    fn a_bypassed_chain_does_not_send_the_preview_through_the_effects_path() {
        use neoveg_engine::TrackEffect;
        let mut tl = Timeline::new();
        let mut track = tl.new_track(TrackKind::Video);
        let mut e = event(&mut tl, 1, 0.0, 4.0);
        let mut off = TrackEffect::new("vignette");
        off.bypass = true;
        e.video_effects = vec![off];
        track.events.push(e);
        tl.tracks.push(track);
        assert!(!has_effects(&tl, 1.0));
    }

    #[test]
    fn the_track_envelopes_set_the_level_the_fade_and_the_mute_of_each_request() {
        use neoveg_engine::envelope::{AutomationTarget, EnvelopeShape, TrackEnvelope};
        let mut tl = Timeline::new();
        let mut track = tl.new_track(TrackKind::Video);
        let e = event(&mut tl, 1, 0.0, 10.0);
        track.events.push(e);
        track.fade_color = [10, 20, 30];
        let mut level = TrackEnvelope::new(AutomationTarget::VideoLevel, 1.0);
        level.upsert_point(4.0, 0.0, EnvelopeShape::Linear, 0.0, 1.0, 1e-4);
        let mut fade = TrackEnvelope::new(AutomationTarget::VideoFade, 0.0);
        fade.upsert_point(4.0, 1.0, EnvelopeShape::Linear, 0.0, 1.0, 1e-4);
        let mut mute = TrackEnvelope::new(AutomationTarget::VideoMute, 0.0);
        mute.points[0].shape = EnvelopeShape::Hold;
        mute.upsert_point(8.0, 1.0, EnvelopeShape::Hold, 0.0, 1.0, 1e-4);
        track.envelopes = vec![level, fade, mute];
        tl.tracks.push(track);

        let node = |tl: &Timeline, time: f64| {
            composite_request(tl, time, &settings(), &resolve).map(|r| match &r.nodes[0] {
                TrackCompositeNode::Track {
                    level,
                    fade_color,
                    fade_amount,
                    ..
                } => (*level, *fade_color, *fade_amount),
                TrackCompositeNode::Group { .. } => panic!("no groups here"),
                TrackCompositeNode::Adjust { .. } => panic!("no adjustments here"),
            })
        };
        let (level, color, amount) = node(&tl, 2.0).unwrap();
        assert!((level - 0.5).abs() < 1e-6 && (amount - 0.5).abs() < 1e-6);
        assert_eq!(color, [10, 20, 30]);
        assert!(
            has_effects(&tl, 2.0),
            "an envelope sends the preview through the composite path"
        );
        assert!(
            node(&tl, 9.0).is_none(),
            "the mute envelope hides the only track: black"
        );
        // A disabled envelope leaves the track alone
        for env in &mut tl.tracks[0].envelopes {
            env.enabled = false;
        }
        assert_eq!(node(&tl, 2.0).unwrap().0, 1.0);
        assert!(!has_effects(&tl, 2.0));
    }

    #[test]
    fn the_pan_crop_of_an_event_reaches_the_request_at_the_time_since_its_start() {
        use neoveg_common::pan_crop::{PanCrop, PanCropFrame};
        let mut tl = Timeline::new();
        let mut track = tl.new_track(TrackKind::Video);
        let mut e = event(&mut tl, 1, 10.0, 8.0);
        let mut pc = PanCrop::default();
        let at = |x: f32| PanCropFrame {
            x_center: x,
            width: 0.5,
            height: 0.5,
            rotation_deg: 0.0,
            y_center: 0.5,
        };
        pc.upsert_key(0.0, at(0.25), 1e-3);
        pc.upsert_key(4.0, at(0.75), 1e-3);
        e.pan_crop = pc;
        let (tid, eid) = (track.id, e.id);
        track.events.push(e);
        tl.tracks.push(track);
        assert!(
            has_effects(&tl, 12.0),
            "a Pan/Crop sends the preview through the composite path"
        );

        let clip = |tl: &Timeline, settings: &FrameSettings, time: f64| {
            let r = composite_request(tl, time, settings, &resolve).unwrap();
            let TrackCompositeNode::Track { clips, .. } = &r.nodes[0] else {
                panic!("a track node");
            };
            clips[0].pan_crop
        };
        let mid = clip(&tl, &settings(), 12.0).unwrap();
        assert!(
            (mid.frame.x_center - 0.5).abs() < 1e-5,
            "2 s into a 0 to 4 s move"
        );
        assert!(mid.stretch_to_fill);
        assert_eq!(clip(&tl, &settings(), 10.0).unwrap().frame.x_center, 0.25);

        // The draft being edited replaces the stored one for that event only
        let mut live = settings();
        let mut draft = PanCrop::default();
        draft.upsert_key(0.0, at(0.9), 1e-3);
        live.pan_crop_draft = Some(PanCropDraft {
            target: EditTarget::Event(tid, eid),
            pan_crop: draft,
        });
        assert_eq!(clip(&tl, &live, 12.0).unwrap().frame.x_center, 0.9);
        assert_eq!(
            tl.tracks[0].events[0].pan_crop.keys.len(),
            2,
            "the project is untouched"
        );

        // An event without Pan/Crop sends none
        tl.tracks[0].events[0].pan_crop = PanCrop::default();
        assert!(clip(&tl, &settings(), 12.0).is_none());
    }

    #[test]
    fn the_motion_of_a_track_reaches_the_request_at_the_timeline_time() {
        use neoveg_common::pan_crop::{PanCrop, PanCropFrame};
        let mut tl = Timeline::new();
        let mut track = tl.new_track(TrackKind::Video);
        let e = event(&mut tl, 1, 0.0, 10.0);
        track.events.push(e);
        let at = |x: f32| PanCropFrame {
            x_center: x,
            y_center: 0.5,
            width: 0.3,
            height: 0.3,
            rotation_deg: 0.0,
        };
        let mut motion = PanCrop::default();
        motion.upsert_key(2.0, at(0.2), 1e-3);
        motion.upsert_key(6.0, at(0.8), 1e-3);
        let tid = track.id;
        track.motion = motion;
        tl.tracks.push(track);
        assert!(
            has_effects(&tl, 4.0),
            "a moving track goes through the composite path"
        );

        let frame = |tl: &Timeline, settings: &FrameSettings, time: f64| {
            let r = composite_request(tl, time, settings, &resolve).unwrap();
            let TrackCompositeNode::Track { motion, .. } = &r.nodes[0] else {
                panic!("a track node");
            };
            *motion
        };
        assert!(
            (frame(&tl, &settings(), 4.0).unwrap().x_center - 0.5).abs() < 1e-5,
            "halfway"
        );
        assert_eq!(
            frame(&tl, &settings(), 1.0).unwrap().x_center,
            0.2,
            "held before the first key"
        );

        // The motion being edited replaces the stored one for that track only
        let mut live = settings();
        let mut draft = PanCrop::default();
        draft.upsert_key(0.0, at(0.9), 1e-3);
        live.pan_crop_draft = Some(PanCropDraft {
            target: EditTarget::Track(tid),
            pan_crop: draft,
        });
        assert_eq!(frame(&tl, &live, 4.0).unwrap().x_center, 0.9);
        assert_eq!(
            tl.tracks[0].motion.keys.len(),
            2,
            "the project is untouched"
        );

        tl.tracks[0].motion = PanCrop::default();
        assert!(
            frame(&tl, &settings(), 4.0).is_none(),
            "a track without motion sends none"
        );
        assert!(!has_effects(&tl, 4.0));
    }

    #[test]
    fn a_fade_with_a_transition_becomes_a_progress_and_a_plain_fade_stays_an_opacity() {
        use neoveg_engine::TrackEffect;
        let mut tl = Timeline::new();
        let mut e = event(&mut tl, 1, 10.0, 8.0);
        e.fade_in = 2.0;
        e.fade_out = 2.0;
        // No transition: the fades are opacities
        let (opacity, list) = clip_look(&e, 11.0, 0.0);
        assert!((opacity - 0.5).abs() < 1e-5 && list.is_empty());
        assert_eq!(clip_look(&e, 14.0, 0.0), (1.0, Vec::new()));

        e.transition_in = Some(TrackEffect::new("iris"));
        e.transition_out = Some(TrackEffect::new("clock_wipe"));
        // In the middle of the fade-in the clip is opaque and the transition is half way
        let (opacity, list) = clip_look(&e, 11.0, 0.0);
        assert_eq!(opacity, 1.0);
        assert_eq!((list.len(), list[0].kind.as_str()), (1, "iris"));
        assert!((list[0].progress - 0.5).abs() < 1e-5);
        // The fade-out has its own, going down
        let (opacity, list) = clip_look(&e, 17.0, 0.0);
        assert_eq!(opacity, 1.0);
        assert_eq!(
            (list[0].kind.as_str(), list[0].progress),
            ("clock_wipe", 0.5)
        );
        // Once the fade is over the transition is gone and the clip is a plain full clip
        assert_eq!(clip_look(&e, 14.0, 0.0), (1.0, Vec::new()));
        // At the very start nothing is visible: no request, opacity zero
        assert_eq!(clip_look(&e, 10.0, 0.0), (0.0, Vec::new()));
        // The overlap with the event before drives the entrance too, with the fade-in curve
        e.fade_in = 0.0;
        let (opacity, list) = clip_look(&e, 10.5, 1.0);
        assert_eq!(opacity, 1.0);
        assert!((list[0].progress - 0.5).abs() < 1e-5);
        // The plain dissolve chosen by name is not a transition
        e.transition_in = Some(TrackEffect::new("dissolve"));
        let (opacity, list) = clip_look(&e, 10.5, 1.0);
        assert!((opacity - 0.5).abs() < 1e-5 && list.is_empty());
    }

    #[test]
    fn a_transition_reaches_the_request_of_its_clip_and_sends_the_preview_through_the_composite_path(
    ) {
        use neoveg_engine::TrackEffect;
        let mut tl = Timeline::new();
        let mut track = tl.new_track(TrackKind::Video);
        let mut e = event(&mut tl, 1, 0.0, 6.0);
        e.fade_in = 2.0;
        e.transition_in = Some(TrackEffect::new("linear_wipe"));
        track.events.push(e);
        tl.tracks.push(track);
        assert!(has_effects(&tl, 4.0));
        let request = composite_request(&tl, 1.0, &settings(), &resolve).unwrap();
        let TrackCompositeNode::Track { clips, .. } = &request.nodes[0] else {
            panic!("a track node");
        };
        assert_eq!(clips[0].event_opacity, 1.0);
        assert_eq!(clips[0].transitions[0].kind, "linear_wipe");
        // After the fade there is nothing left to draw
        let later = composite_request(&tl, 3.0, &settings(), &resolve).unwrap();
        let TrackCompositeNode::Track { clips, .. } = &later.nodes[0] else {
            panic!("a track node");
        };
        assert!(clips[0].transitions.is_empty());
    }

    /// An envelope with these `(time, speed)` keys, each held to the next one.
    fn speeds(points: &[(f64, f32)]) -> neoveg_engine::TrackEnvelope {
        let mut env = velocity::new_envelope(1.0);
        env.points = points
            .iter()
            .map(|&(time, value)| neoveg_engine::EnvelopePoint {
                time,
                value,
                shape: neoveg_engine::EnvelopeShape::Hold,
            })
            .collect();
        env
    }

    #[test]
    fn the_velocity_envelope_decides_how_much_of_the_source_plays_and_not_how_long_the_event_is() {
        let mut e = TrackEvent {
            start: 2.0,
            duration: 8.0,
            trim_in: 1.0,
            ..TrackEvent::default()
        };
        // Double speed all along: 3 s into the event is 6 s of source
        e.velocity = Some(speeds(&[(0.0, 2.0)]));
        assert!((source_seconds(&e, 5.0, 20.0) - 7.0).abs() < 1e-6);
        // Half speed for 2 s, then normal: 1 s + 2 s of the source after 4 s of the event
        e.velocity = Some(speeds(&[(0.0, 0.5), (2.0, 1.0)]));
        assert!((source_seconds(&e, 6.0, 20.0) - (1.0 + 1.0 + 2.0)).abs() < 1e-3);
        // The event's own speed multiplies the envelope: 3 s of envelope make 6 s of source
        e.playback_rate = 2.0;
        assert!((source_seconds(&e, 6.0, 20.0) - (1.0 + 2.0 * 3.0)).abs() < 1e-3);
        // A disabled envelope is plain speed again
        e.playback_rate = 1.0;
        let mut off = speeds(&[(0.0, 3.0)]);
        off.enabled = false;
        e.velocity = Some(off);
        assert_eq!(source_seconds(&e, 5.0, 20.0), 4.0);
    }

    #[test]
    fn a_freeze_holds_one_frame_and_a_reverse_stops_at_the_first_frame() {
        let mut e = TrackEvent {
            start: 0.0,
            duration: 10.0,
            trim_in: 0.0,
            ..TrackEvent::default()
        };
        // Normal for 3 s, then frozen
        e.velocity = Some(velocity::freeze_at(None, 3.0));
        let held = source_seconds(&e, 3.5, 20.0);
        assert!((held - 3.0).abs() < 0.01, "got {held}");
        assert_eq!(
            held,
            source_seconds(&e, 9.0, 20.0),
            "the same frame to the end"
        );
        // Backwards from 2 s: it passes through 1 s and then holds the start of the media
        e.velocity = Some(speeds(&[(0.0, 1.0), (2.0, -1.0)]));
        assert!((source_seconds(&e, 3.0, 20.0) - 1.0).abs() < 1e-3);
        assert_eq!(source_seconds(&e, 6.0, 20.0), 0.0);
        // Running past the end of the media holds the last frame
        e.velocity = Some(speeds(&[(0.0, 3.0)]));
        assert_eq!(source_seconds(&e, 9.0, 10.0), 10.0);
    }

    #[test]
    fn the_velocity_reaches_the_request_with_its_sign_and_sends_the_preview_through_the_composite_path(
    ) {
        let mut tl = Timeline::new();
        let mut track = tl.new_track(TrackKind::Video);
        let mut e = event(&mut tl, 1, 0.0, 8.0);
        e.trim_in = 0.0;
        track.events.push(e);
        let (track_id, event_id) = (track.id, track.events[0].id);
        tl.tracks.push(track);
        assert!(!has_effects(&tl, 2.0), "no envelope, the plain path");
        tl.tracks[0].events[0].velocity = Some(speeds(&[(0.0, 2.0), (4.0, -1.0)]));
        assert!(has_effects(&tl, 2.0));
        let rate_of = |tl: &Timeline, settings: &FrameSettings, time: f64| {
            let request = composite_request(tl, time, settings, &resolve).unwrap();
            let TrackCompositeNode::Track { clips, .. } = &request.nodes[0] else {
                panic!("a track node");
            };
            (clips[0].playback_rate, clips[0].seconds)
        };
        let (rate, seconds) = rate_of(&tl, &settings(), 1.0);
        assert_eq!(rate, 2.0);
        assert!((seconds - 2.0).abs() < 1e-6);
        let (rate, _) = rate_of(&tl, &settings(), 5.0);
        assert_eq!(
            rate, -1.0,
            "backwards: the decoder reads ahead the other way"
        );
        // The envelope being dragged replaces the stored one for that event only
        let mut live = settings();
        live.velocity_draft = Some(VelocityDraft {
            track_id,
            event_id,
            envelope: Some(speeds(&[(0.0, 0.5)])),
        });
        let (rate, seconds) = rate_of(&tl, &live, 1.0);
        assert_eq!(rate, 0.5);
        assert!((seconds - 0.5).abs() < 1e-6);
        live.velocity_draft = Some(VelocityDraft {
            track_id,
            event_id: event_id + 1,
            envelope: Some(speeds(&[(0.0, 0.5)])),
        });
        assert_eq!(
            rate_of(&tl, &live, 1.0).0,
            2.0,
            "another event's draft is not ours"
        );
    }

    #[test]
    fn an_adjustment_event_becomes_a_node_before_its_own_track_and_acts_on_the_tracks_below() {
        use neoveg_engine::TrackEffect;
        let mut tl = Timeline::new();
        // Track order: the first is the top layer. Top: an adjustment and a clip; bottom: a clip.
        let mut top = tl.new_track(TrackKind::Video);
        let mut bottom = tl.new_track(TrackKind::Video);
        let adjust_id = tl.alloc_id();
        top.events.push(TrackEvent {
            id: adjust_id,
            name: "Evento de ajuste".into(),
            start: 1.0,
            duration: 4.0,
            fade_in: 2.0,
            adjustment: true,
            video_effects: vec![TrackEffect::new("vignette")],
            ..TrackEvent::default()
        });
        let overlay = event(&mut tl, 2, 0.0, 10.0);
        top.events.push(overlay);
        let base = event(&mut tl, 1, 0.0, 10.0);
        bottom.events.push(base);
        tl.tracks.push(top);
        tl.tracks.push(bottom);
        assert!(
            has_effects(&tl, 2.0),
            "an adjustment sends the preview through the composite path"
        );
        let request = composite_request(&tl, 2.0, &settings(), &resolve).unwrap();
        let kinds: Vec<&str> = request
            .nodes
            .iter()
            .map(|n| match n {
                TrackCompositeNode::Track { .. } => "track",
                TrackCompositeNode::Adjust { .. } => "adjust",
                TrackCompositeNode::Group { .. } => "group",
            })
            .collect();
        // Bottom up: the lower track, then the adjustment, then the track that holds it
        assert_eq!(kinds, ["track", "adjust", "track"]);
        let TrackCompositeNode::Adjust { effects, opacity } = &request.nodes[1] else {
            panic!("an adjustment node");
        };
        assert_eq!(effects.len(), 1);
        assert!(
            (opacity - 0.5).abs() < 1e-5,
            "half way through its 2 s fade-in"
        );
        // The adjustment is the only thing there and nothing is under it: black
        let mut alone = Timeline::new();
        let mut t = alone.new_track(TrackKind::Video);
        t.events.push(TrackEvent {
            id: 9,
            start: 0.0,
            duration: 4.0,
            adjustment: true,
            ..TrackEvent::default()
        });
        alone.tracks.push(t);
        assert!(composite_request(&alone, 1.0, &settings(), &resolve).is_none());
        // Mute switches it off
        tl.tracks[0].events[0].muted = true;
        let muted = composite_request(&tl, 2.0, &settings(), &resolve).unwrap();
        assert_eq!(muted.nodes.len(), 2);
    }
}
