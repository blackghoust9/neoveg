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
use neoveg_common::composite::CompositeMode;
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
                })
            })
            .collect();
        if !clips.is_empty() {
            nodes.push(TrackCompositeNode::Track {
                clips,
                mode: CompositeMode::SourceAlpha,
                level: 1.0,
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
    })
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
}
