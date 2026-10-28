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
use crate::video_fx_panel::{effects_for, VfxDraft};
use neoveg_common::composite::CompositeMode;
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

/// Opacity of `event` at `time`: its own fade in and fade out, times the fade in over an automatic
/// crossfade of `cross_in` seconds (`0` = none).
pub(super) fn event_opacity(event: &TrackEvent, time: f64, cross_in: f64) -> f32 {
    let local = time - event.start;
    let remaining = event.end() - time;
    (ramp(event.fade_in_curve, event.fade_in, local)
        * ramp(event.fade_out_curve, event.fade_out, remaining.max(0.0))
        * ramp(event.fade_in_curve, cross_in, local)) as f32
}

/// Seconds into the source of the event's media at timeline `time`, honouring speed and Loop.
pub(super) fn source_seconds(event: &TrackEvent, time: f64, source_duration: f64) -> f64 {
    let rate = edit::clamp_playback_rate(event.playback_rate);
    let mut offset = (time - event.start) * rate;
    if event.looping {
        offset = edit::wrap_loop(offset, edit::loop_length(source_duration, event.trim_in));
    }
    offset + event.trim_in
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
                Some(VideoClipRequest {
                    media_id,
                    seconds: source_seconds(e, time, source.duration),
                    source_fps: source.fps,
                    path: source.path,
                    playback_rate: edit::clamp_playback_rate(e.playback_rate),
                    event_opacity: event_opacity(e, time, cross_in),
                    apply_chroma: true,
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
    if nodes.is_empty() {
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
            let envelope_on = track
                .envelopes
                .iter()
                .any(|env| env.enabled && env.target.is_video());
            if on(&e.video_effects)
                || on(&track.video_effects)
                || media_on
                || envelope_on
                || pan_crop_on
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
}
