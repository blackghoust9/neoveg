//! Pan/Crop of a video event: which part of the picture the event shows, with keyframes.
//!
//! The event owns a **selection box** over its source picture (a centre, a size and a rotation, all
//! relative to the source, so the same box fits a preview at quarter size and the final render). The
//! box can be animated with keyframes whose times are measured from the start of the event and whose
//! `smoothness` bends the move toward the next key from linear to ease-in-out, the "Smoothness"
//! slider of the reference manual.
//!
//! Everything here is a pure function: the engine stores a [`PanCrop`], `neoveg_video::pan_crop`
//! draws the picture for a [`PanCropFrame`], and the interface edits the keys.

use serde::{Deserialize, Serialize};

/// Largest and smallest size of the box, as a fraction of the source (`1.0` is the whole picture).
pub const MIN_SIZE: f32 = 0.02;
pub const MAX_SIZE: f32 = 4.0;
/// Two keyframes closer than this many seconds are the same keyframe.
pub const KEY_TOLERANCE: f64 = 1e-3;

/// The selection box at one instant.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PanCropFrame {
    /// Centre, as a fraction of the source width and height (`0.5, 0.5` is the middle).
    pub x_center: f32,
    pub y_center: f32,
    /// Size, as a fraction of the source width and height.
    pub width: f32,
    pub height: f32,
    /// Clockwise rotation of the box in degrees, around its centre.
    pub rotation_deg: f32,
}

impl PanCropFrame {
    /// The whole picture, not rotated: what an event shows without Pan/Crop.
    pub const FULL: PanCropFrame = PanCropFrame {
        x_center: 0.5,
        y_center: 0.5,
        width: 1.0,
        height: 1.0,
        rotation_deg: 0.0,
    };

    /// `true` for the whole picture.
    pub fn is_full(&self) -> bool {
        (*self).sanitized() == Self::FULL
    }

    /// The box with every number finite and inside its range, the rotation in `-180..=180`.
    pub fn sanitized(self) -> Self {
        let fix = |v: f32, default: f32| if v.is_finite() { v } else { default };
        let mut rotation = fix(self.rotation_deg, 0.0) % 360.0;
        if rotation > 180.0 {
            rotation -= 360.0;
        } else if rotation < -180.0 {
            rotation += 360.0;
        }
        Self {
            x_center: fix(self.x_center, 0.5).clamp(-2.0, 3.0),
            y_center: fix(self.y_center, 0.5).clamp(-2.0, 3.0),
            width: fix(self.width, 1.0).clamp(MIN_SIZE, MAX_SIZE),
            height: fix(self.height, 1.0).clamp(MIN_SIZE, MAX_SIZE),
            rotation_deg: rotation,
        }
    }

    /// Straight interpolation between two boxes, `u` in `0..=1`.
    pub fn lerp(a: Self, b: Self, u: f32) -> Self {
        let m = |x: f32, y: f32| x + (y - x) * u;
        Self {
            x_center: m(a.x_center, b.x_center),
            y_center: m(a.y_center, b.y_center),
            width: m(a.width, b.width),
            height: m(a.height, b.height),
            rotation_deg: m(a.rotation_deg, b.rotation_deg),
        }
    }
}

impl Default for PanCropFrame {
    fn default() -> Self {
        Self::FULL
    }
}

/// A keyframe: the box at `time` seconds from the start of the event.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PanCropKey {
    pub time: f64,
    pub frame: PanCropFrame,
    /// `0.0` moves linearly to the next key, `1.0` eases in and out (smoothstep).
    #[serde(default)]
    pub smoothness: f32,
}

/// The motion of a video **track**: the same keyframed box, read the other way round. Where Pan/Crop
/// says which part of the source fills the frame, a track's motion says where the whole layer of
/// the track sits in the project's frame (a smaller box is a picture-in-picture, a moved one a pan,
/// a rotated one a tilted layer). Key times are timeline seconds, `stretch_to_fill` is not used.
pub type TrackMotion = PanCrop;

fn default_true() -> bool {
    true
}

/// The Pan/Crop of one event. No keys means "no Pan/Crop": the event shows its whole picture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PanCrop {
    /// Keyframes sorted by time. One key is a still box for the whole event.
    #[serde(default)]
    pub keys: Vec<PanCropKey>,
    /// `true`: the box fills the output frame (zoom). `false`: the picture keeps its size and
    /// whatever is outside the box is cropped away.
    #[serde(default = "default_true")]
    pub stretch_to_fill: bool,
}

impl Default for PanCrop {
    fn default() -> Self {
        Self {
            keys: Vec::new(),
            stretch_to_fill: true,
        }
    }
}

impl PanCrop {
    /// `true` for an event without Pan/Crop (the value that stays out of the project file).
    pub fn is_default(&self) -> bool {
        self.keys.is_empty() && self.stretch_to_fill
    }

    /// `true` when the event shows its whole picture unchanged.
    pub fn is_identity(&self) -> bool {
        self.stretch_to_fill && self.keys.iter().all(|k| k.frame.is_full())
    }

    /// The box `time` seconds into the event: held before the first key and after the last one,
    /// interpolated in between.
    pub fn frame_at(&self, time: f64) -> PanCropFrame {
        let (Some(first), Some(last)) = (self.keys.first(), self.keys.last()) else {
            return PanCropFrame::FULL;
        };
        let time = if time.is_finite() { time } else { 0.0 };
        if time <= first.time {
            return first.frame.sanitized();
        }
        if time >= last.time {
            return last.frame.sanitized();
        }
        let next = self.keys.partition_point(|k| k.time <= time);
        let (a, b) = (&self.keys[next - 1], &self.keys[next]);
        let span = b.time - a.time;
        if span <= f64::EPSILON {
            return b.frame.sanitized();
        }
        let u = ((time - a.time) / span).clamp(0.0, 1.0) as f32;
        let smooth = u * u * (3.0 - 2.0 * u);
        let s = if a.smoothness.is_finite() {
            a.smoothness.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let eased = u + (smooth - u) * s;
        PanCropFrame::lerp(a.frame.sanitized(), b.frame.sanitized(), eased)
    }

    /// Index of the key within [`KEY_TOLERANCE`] or `tolerance` seconds of `time`.
    pub fn key_near(&self, time: f64, tolerance: f64) -> Option<usize> {
        let tolerance = tolerance.max(KEY_TOLERANCE);
        self.keys
            .iter()
            .position(|k| (k.time - time).abs() <= tolerance)
    }

    /// Puts `frame` at `time`: changes the key that is there or inserts a new one, keeping the keys
    /// sorted. A new key starts with the smoothness of the key before it. Returns its index.
    pub fn upsert_key(&mut self, time: f64, frame: PanCropFrame, tolerance: f64) -> usize {
        let time = if time.is_finite() { time.max(0.0) } else { 0.0 };
        let frame = frame.sanitized();
        if let Some(i) = self.key_near(time, tolerance) {
            self.keys[i].frame = frame;
            return i;
        }
        let at = self.keys.partition_point(|k| k.time < time);
        let smoothness = at.checked_sub(1).map_or(0.0, |i| self.keys[i].smoothness);
        self.keys.insert(
            at,
            PanCropKey {
                time,
                frame,
                smoothness,
            },
        );
        at
    }

    /// Removes the key at `index`. `false` when there is none.
    pub fn remove_key(&mut self, index: usize) -> bool {
        if index < self.keys.len() {
            self.keys.remove(index);
            true
        } else {
            false
        }
    }

    /// The Pan/Crop of the part of the event that starts `offset` seconds in (the second half of a
    /// split): the box at the cut becomes the first key and the later keys move back by `offset`.
    pub fn from_offset(&self, offset: f64) -> PanCrop {
        if self.keys.is_empty() {
            return self.clone();
        }
        let first = self.frame_at(offset);
        let mut keys = vec![PanCropKey {
            time: 0.0,
            frame: first,
            smoothness: self
                .keys
                .iter()
                .rfind(|k| k.time <= offset)
                .map_or(0.0, |k| k.smoothness),
        }];
        keys.extend(
            self.keys
                .iter()
                .filter(|k| k.time > offset + KEY_TOLERANCE)
                .map(|k| PanCropKey {
                    time: k.time - offset,
                    ..*k
                }),
        );
        PanCrop {
            keys,
            stretch_to_fill: self.stretch_to_fill,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(x: f32, w: f32) -> PanCropFrame {
        PanCropFrame {
            x_center: x,
            width: w,
            height: w,
            ..PanCropFrame::FULL
        }
    }

    #[test]
    fn no_keys_means_the_whole_picture_and_stays_out_of_the_file() {
        let none = PanCrop::default();
        assert!(none.is_default() && none.is_identity());
        assert_eq!(none.frame_at(3.0), PanCropFrame::FULL);
        assert_eq!(
            serde_json::to_string(&none).unwrap(),
            r#"{"keys":[],"stretch_to_fill":true}"#
        );
        let old: PanCrop = serde_json::from_str("{}").unwrap();
        assert!(
            old.is_default(),
            "an old file without the field loads as no Pan/Crop"
        );
    }

    #[test]
    fn one_key_is_a_still_box_and_keys_interpolate_with_the_smoothness() {
        let mut p = PanCrop::default();
        p.upsert_key(0.0, frame(0.2, 0.5), 1e-3);
        assert_eq!(p.frame_at(0.0).x_center, 0.2);
        assert_eq!(
            p.frame_at(99.0).x_center,
            0.2,
            "one key holds for the whole event"
        );
        p.upsert_key(4.0, frame(0.8, 1.0), 1e-3);
        // linear: halfway through is halfway between
        let mid = p.frame_at(2.0);
        assert!((mid.x_center - 0.5).abs() < 1e-5 && (mid.width - 0.75).abs() < 1e-5);
        // smooth: a quarter in is less than a quarter of the way
        p.keys[0].smoothness = 1.0;
        let quarter = p.frame_at(1.0);
        assert!(quarter.x_center < 0.2 + 0.6 * 0.25);
        // held before the first key and after the last one
        assert_eq!(p.frame_at(-5.0), p.keys[0].frame);
        assert_eq!(p.frame_at(10.0), p.keys[1].frame);
    }

    #[test]
    fn keys_stay_sorted_and_a_key_at_the_same_time_is_changed_not_added() {
        let mut p = PanCrop::default();
        p.upsert_key(5.0, frame(0.5, 0.5), 1e-3);
        p.upsert_key(1.0, frame(0.1, 0.5), 1e-3);
        p.upsert_key(3.0, frame(0.3, 0.5), 1e-3);
        assert_eq!(
            p.keys.iter().map(|k| k.time).collect::<Vec<_>>(),
            [1.0, 3.0, 5.0]
        );
        let i = p.upsert_key(3.0004, frame(0.9, 0.5), 1e-3);
        assert_eq!((p.keys.len(), i, p.keys[1].frame.x_center), (3, 1, 0.9));
        assert_eq!(p.key_near(5.0, 0.0), Some(2));
        assert!(p.remove_key(0) && !p.remove_key(9));
    }

    #[test]
    fn bad_numbers_are_made_safe() {
        let bad = PanCropFrame {
            x_center: f32::NAN,
            y_center: 99.0,
            width: 0.0,
            height: f32::INFINITY,
            rotation_deg: 725.0,
        }
        .sanitized();
        assert_eq!(bad.x_center, 0.5);
        assert_eq!(bad.y_center, 3.0);
        assert_eq!((bad.width, bad.height), (MIN_SIZE, 1.0));
        assert_eq!(bad.rotation_deg, 5.0);
        let mut p = PanCrop::default();
        p.upsert_key(f64::NAN, PanCropFrame::FULL, 1e-3);
        assert_eq!(p.keys[0].time, 0.0);
        assert_eq!(p.frame_at(f64::NAN), PanCropFrame::FULL);
    }

    #[test]
    fn the_second_half_of_a_split_starts_from_the_box_at_the_cut() {
        let mut p = PanCrop::default();
        p.upsert_key(0.0, frame(0.0, 0.5), 1e-3);
        p.upsert_key(4.0, frame(1.0, 0.5), 1e-3);
        p.upsert_key(8.0, frame(0.0, 0.5), 1e-3);
        let right = p.from_offset(2.0);
        assert_eq!(right.keys[0].time, 0.0);
        assert!(
            (right.keys[0].frame.x_center - 0.5).abs() < 1e-5,
            "the box at the cut"
        );
        assert_eq!(
            right.keys.iter().map(|k| k.time).collect::<Vec<_>>(),
            [0.0, 2.0, 6.0]
        );
        // The picture is the same: the right half at 1 s is the whole event at 3 s
        assert!((right.frame_at(1.0).x_center - p.frame_at(3.0).x_center).abs() < 1e-5);
        assert_eq!(PanCrop::default().from_offset(2.0), PanCrop::default());
    }

    #[test]
    fn a_full_box_is_the_identity_only_when_stretching() {
        let mut p = PanCrop::default();
        p.upsert_key(0.0, PanCropFrame::FULL, 1e-3);
        assert!(p.is_identity() && !p.is_default());
        p.stretch_to_fill = false;
        assert!(!p.is_identity());
    }
}
