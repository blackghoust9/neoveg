//! *Reenquadrar para vertical*: a copy of the project in 9:16 whose video events are reframed
//! with Pan/Crop.
//!
//! The copy gets a portrait frame (the long side of the original), and every video event gets a
//! Pan/Crop box of the portrait shape over its source picture. Where the box sits comes from the
//! subject positions the caller gives for the event (the detector of `neoveg_video::auto_reframe`,
//! or a value the user typed for that event); without any, the box is centered. The functions are
//! pure: the sources are never read here.

use neoveg_common::pan_crop::{PanCrop, PanCropFrame, PanCropKey};
use neoveg_common::project::ProjectInfo;

use crate::timeline::{EventId, Timeline, TrackKind};

/// Aspect of the vertical frame, width over height.
pub const VERTICAL_ASPECT: f32 = 9.0 / 16.0;

/// Portrait frame size (width, height) with the long side of the project's frame, both even.
pub fn vertical_size(width: u32, height: u32) -> (u32, u32) {
    let long = width.max(height).max(2);
    let h = long & !1;
    let w = ((h as f32 * VERTICAL_ASPECT).round() as u32).max(2) & !1;
    (w, h)
}

/// Where the subject is in one event: seconds from the start of the event, and the horizontal
/// center as a fraction of the source width.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SubjectPoint {
    pub time: f64,
    pub center_x: f32,
}

/// The size of the portrait box over a source of `source_aspect` (width over height), as
/// fractions of the source.
pub fn crop_box_size(source_aspect: f32) -> (f32, f32) {
    let src = if source_aspect.is_finite() && source_aspect > 1e-3 {
        source_aspect
    } else {
        16.0 / 9.0
    };
    if VERTICAL_ASPECT <= src {
        ((VERTICAL_ASPECT / src).clamp(0.05, 1.0), 1.0)
    } else {
        (1.0, (src / VERTICAL_ASPECT).clamp(0.05, 1.0))
    }
}

/// The Pan/Crop of a vertical event: a box of the right shape that follows `subject` (kept
/// inside the picture). No points: a still box in the middle.
pub fn vertical_pan_crop(source_aspect: f32, subject: &[SubjectPoint], smoothness: f32) -> PanCrop {
    let (w, h) = crop_box_size(source_aspect);
    let mut points: Vec<SubjectPoint> = subject
        .iter()
        .copied()
        .filter(|p| p.time.is_finite() && p.center_x.is_finite())
        .collect();
    points.sort_by(|a, b| a.time.total_cmp(&b.time));
    if points.is_empty() {
        points.push(SubjectPoint {
            time: 0.0,
            center_x: 0.5,
        });
    }
    let half = w * 0.5;
    let keys = points
        .iter()
        .map(|p| PanCropKey {
            time: p.time.max(0.0),
            frame: PanCropFrame {
                x_center: p.center_x.clamp(half, 1.0 - half),
                y_center: 0.5,
                width: w,
                height: h,
                rotation_deg: 0.0,
            },
            smoothness,
        })
        .collect();
    PanCrop {
        keys,
        stretch_to_fill: true,
    }
}

/// The vertical copy of a project: a new frame size and name, and a Pan/Crop on every video event
/// (adjustment events are left alone). `subject(event)` gives the subject positions of an event,
/// `source_aspect(media_id)` the aspect of its picture (`None`: the project's).
pub fn reframe_project(
    info: &ProjectInfo,
    timeline: &Timeline,
    subject: impl Fn(EventId) -> Vec<SubjectPoint>,
    source_aspect: impl Fn(u64) -> Option<f32>,
) -> (ProjectInfo, Timeline) {
    let mut info = info.clone();
    let mut timeline = timeline.clone();
    let project_aspect = info.settings.width as f32 / info.settings.height.max(1) as f32;
    let (w, h) = vertical_size(info.settings.width, info.settings.height);
    info.settings.width = w;
    info.settings.height = h;
    info.name = format!("{} (9x16)", info.name);
    for track in timeline
        .tracks
        .iter_mut()
        .filter(|t| t.kind == TrackKind::Video)
    {
        for event in track.events.iter_mut().filter(|e| !e.adjustment) {
            let aspect = event
                .media_id
                .and_then(&source_aspect)
                .unwrap_or(project_aspect);
            event.pan_crop = vertical_pan_crop(aspect, &subject(event.id), 0.5);
        }
    }
    (info, timeline)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::TrackEvent;
    use neoveg_video::auto_reframe::{AutoReframeEngine, SubjectDetection, TargetAspectRatio};

    fn project() -> (ProjectInfo, Timeline, EventId, EventId) {
        let mut info = ProjectInfo::create_empty();
        info.name = "Filme".into();
        info.settings.width = 1920;
        info.settings.height = 1080;
        let mut tl = Timeline::new();
        let mut video = tl.new_track(TrackKind::Video);
        let a = tl.alloc_id();
        let b = tl.alloc_id();
        for id in [a, b] {
            video.events.push(TrackEvent {
                id,
                media_id: Some(1),
                duration: 4.0,
                ..TrackEvent::default()
            });
        }
        let mut audio = tl.new_track(TrackKind::Audio);
        let c = tl.alloc_id();
        audio.events.push(TrackEvent {
            id: c,
            media_id: Some(2),
            duration: 4.0,
            ..TrackEvent::default()
        });
        tl.tracks.push(video);
        tl.tracks.push(audio);
        (info, tl, a, b)
    }

    #[test]
    fn the_copy_is_portrait_and_every_video_event_gets_a_box() {
        let (info, tl, a, b) = project();
        let subject = |id: EventId| {
            if id == a {
                vec![SubjectPoint {
                    time: 0.0,
                    center_x: 0.8,
                }]
            } else {
                Vec::new()
            }
        };
        let (info2, tl2) = reframe_project(&info, &tl, subject, |_| Some(16.0 / 9.0));
        assert_eq!((info2.settings.width, info2.settings.height), (1080, 1920));
        assert_eq!(info2.name, "Filme (9x16)");
        assert_eq!((info.settings.width, info.name.as_str()), (1920, "Filme"));
        let events = &tl2.tracks[0].events;
        let ea = events.iter().find(|e| e.id == a).unwrap();
        let eb = events.iter().find(|e| e.id == b).unwrap();
        let fa = ea.pan_crop.keys[0].frame;
        // 9:16 over 16:9 keeps 0.3164 of the width and the whole height
        assert!((fa.width - 0.31640625).abs() < 1e-4 && fa.height == 1.0);
        assert!((fa.x_center - 0.8).abs() < 1e-6);
        assert_eq!(
            eb.pan_crop.keys[0].frame.x_center, 0.5,
            "no subject: centered"
        );
        assert!(
            tl2.tracks[1].events[0].pan_crop.is_default(),
            "audio events are untouched"
        );
    }

    #[test]
    fn the_box_never_leaves_the_picture() {
        let pc = vertical_pan_crop(
            16.0 / 9.0,
            &[
                SubjectPoint {
                    time: 2.0,
                    center_x: 1.0,
                },
                SubjectPoint {
                    time: 0.0,
                    center_x: 0.0,
                },
            ],
            0.0,
        );
        assert_eq!(pc.keys.len(), 2);
        assert!(pc.keys[0].time < pc.keys[1].time, "sorted by time");
        for k in &pc.keys {
            let f = k.frame;
            assert!(f.x_center - f.width / 2.0 >= -1e-6);
            assert!(f.x_center + f.width / 2.0 <= 1.0 + 1e-6);
        }
    }

    #[test]
    fn the_box_matches_the_auto_reframe_detector_plan() {
        let planned = AutoReframeEngine::plan_reframe_trajectory(
            &[SubjectDetection {
                time_s: 0.0,
                center_x: 0.5,
                center_y: 0.5,
                confidence: 1.0,
            }],
            16.0 / 9.0,
            TargetAspectRatio::Ratio9_16,
            0.15,
            0.5,
        );
        let mine = vertical_pan_crop(16.0 / 9.0, &[], 0.5);
        let (p, m) = (planned.keys[0].frame, mine.keys[0].frame);
        assert!((p.width - m.width).abs() < 1e-5 && (p.height - m.height).abs() < 1e-5);
    }

    #[test]
    fn vertical_size_uses_the_long_side_and_stays_even() {
        assert_eq!(vertical_size(1920, 1080), (1080, 1920));
        assert_eq!(vertical_size(1080, 1920), (1080, 1920));
        assert_eq!(vertical_size(1280, 720), (720, 1280));
    }
}
