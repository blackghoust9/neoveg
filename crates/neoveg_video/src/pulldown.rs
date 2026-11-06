//! 3:2 pulldown: film at 24 (23.976) frames per second carried in 30 (29.97) interlaced frames
//! per second, and the way back (inverse telecine).
//!
//! Four film frames A B C D become five video frames of two fields each, with the fields taken in
//! the 2:3:2:3 cadence: `AA BB BC CD DD`. The two middle frames mix two film frames (one in the
//! even lines, the other in the odd ones); inverse telecine finds them by their comb pattern and
//! rebuilds the four originals.

use crate::frame::FrameBuffer;

/// Film frames per cadence group.
pub const FILM_GROUP: usize = 4;
/// Video frames per cadence group.
pub const VIDEO_GROUP: usize = 5;

/// Film rate that a video rate carries (`29.97` gives `23.976`).
pub fn film_rate(video_fps: f64) -> f64 {
    video_fps * FILM_GROUP as f64 / VIDEO_GROUP as f64
}

/// Video rate that carries a film rate (`23.976` gives `29.97`).
pub fn video_rate(film_fps: f64) -> f64 {
    film_fps * VIDEO_GROUP as f64 / FILM_GROUP as f64
}

fn row_bytes(f: &FrameBuffer) -> usize {
    f.width as usize * 4
}

/// Applies 3:2 pulldown to `film`. Every frame needs the same size. The output holds 5 frames per
/// 4 film frames; a trailing group of fewer than 4 frames still yields the fields it owns.
pub fn apply_pulldown(film: &[FrameBuffer]) -> Vec<FrameBuffer> {
    if film.is_empty() {
        return Vec::new();
    }
    // Field sequence: film frame index of every field, alternately 2 and 3 fields per frame.
    let mut fields = Vec::new();
    for (i, _) in film.iter().enumerate() {
        let n = if i % 2 == 0 { 2 } else { 3 };
        fields.extend(std::iter::repeat_n(i, n));
    }
    let (w, h) = (film[0].width, film[0].height);
    let rb = w as usize * 4;
    fields
        .chunks(2)
        .map(|pair| {
            let top = &film[pair[0]];
            let bottom = &film[*pair.get(1).unwrap_or(&pair[0])];
            let mut out = top.clone();
            if top.width == w && top.height == h && bottom.width == w && bottom.height == h {
                for y in (1..h as usize).step_by(2) {
                    out.rgba[y * rb..(y + 1) * rb]
                        .copy_from_slice(&bottom.rgba[y * rb..(y + 1) * rb]);
                }
            }
            out
        })
        .collect()
}

/// How much the lines of `f` disagree with their neighbours of the other field: high for a frame
/// whose even and odd lines come from different pictures.
pub fn comb_score(f: &FrameBuffer) -> f64 {
    let rb = row_bytes(f);
    let h = f.height as usize;
    if h < 3 || rb == 0 || f.rgba.len() < rb * h {
        return 0.0;
    }
    let mut sum = 0u64;
    let mut n = 0u64;
    for y in 1..h - 1 {
        let (a, m, b) = (
            &f.rgba[(y - 1) * rb..y * rb],
            &f.rgba[y * rb..(y + 1) * rb],
            &f.rgba[(y + 1) * rb..(y + 2) * rb],
        );
        for x in (0..rb).step_by(4) {
            let up = i32::from(a[x]);
            let mid = i32::from(m[x]);
            let down = i32::from(b[x]);
            // A real edge changes monotonically; a comb zigzags: both neighbours are on the same
            // side of the middle line.
            let d1 = mid - up;
            let d2 = mid - down;
            if d1 * d2 > 0 {
                sum += d1.unsigned_abs().min(d2.unsigned_abs()) as u64;
            }
            n += 1;
        }
    }
    sum as f64 / n.max(1) as f64
}

/// Finds the cadence phase of `video`: the index `p` (`0..5`) such that the frames `p + 2` and
/// `p + 3` of every group are the mixed ones. `None` when no consistent pattern shows (progressive
/// material, or not enough frames).
pub fn detect_phase(video: &[FrameBuffer]) -> Option<usize> {
    if video.len() < VIDEO_GROUP {
        return None;
    }
    let scores: Vec<f64> = video.iter().map(comb_score).collect();
    let mut best: Option<(usize, f64)> = None;
    for p in 0..VIDEO_GROUP {
        let (mut mixed, mut mn, mut clean, mut cn) = (0.0, 0usize, 0.0, 0usize);
        for (i, s) in scores.iter().enumerate() {
            let slot = (i + VIDEO_GROUP - p) % VIDEO_GROUP;
            if slot == 2 || slot == 3 {
                mixed += s;
                mn += 1;
            } else {
                clean += s;
                cn += 1;
            }
        }
        let gap = mixed / mn.max(1) as f64 - clean / cn.max(1) as f64;
        if best.is_none_or(|(_, g)| gap > g) {
            best = Some((p, gap));
        }
    }
    let (p, gap) = best?;
    // The mixed frames must clearly stand out
    let mean_clean = scores.iter().copied().fold(f64::MAX, f64::min);
    (gap > 0.5 && gap > mean_clean * 2.0).then_some(p)
}

/// Removes the pulldown from `video` whose cadence starts at `phase` (see [`detect_phase`]),
/// rebuilding four film frames from every complete group of five. Frames before the first
/// complete group and an incomplete last group are dropped.
pub fn remove_pulldown(video: &[FrameBuffer], phase: usize) -> Vec<FrameBuffer> {
    let phase = phase % VIDEO_GROUP;
    let mut out = Vec::new();
    let mut at = phase;
    while at + VIDEO_GROUP <= video.len() {
        let g = &video[at..at + VIDEO_GROUP];
        let rb = row_bytes(&g[2]);
        let h = g[2].height as usize;
        // C: the odd lines of BC and the even lines of CD
        let mut c = g[3].clone();
        if g[2].width == g[3].width && g[2].height == g[3].height {
            for y in (1..h).step_by(2) {
                c.rgba[y * rb..(y + 1) * rb].copy_from_slice(&g[2].rgba[y * rb..(y + 1) * rb]);
            }
        }
        out.extend([g[0].clone(), g[1].clone(), c, g[4].clone()]);
        at += VIDEO_GROUP;
    }
    out
}

/// Detects the cadence and removes the pulldown; `None` when `video` shows no 3:2 cadence.
pub fn remove_pulldown_auto(video: &[FrameBuffer]) -> Option<Vec<FrameBuffer>> {
    let p = detect_phase(video)?;
    Some(remove_pulldown(video, p))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A film frame with a vertical-stripe pattern per index and a gradient in y, so that every
    /// frame differs everywhere and neighbouring lines are alike.
    fn film_frame(i: usize) -> FrameBuffer {
        let (w, h) = (32u32, 24u32);
        let mut f = FrameBuffer::solid(w, h, [0, 0, 0, 255]);
        for y in 0..h {
            for x in 0..w {
                let base = (i as u32 * 50 + 20) as u8;
                let v = base.wrapping_add((x * 3) as u8 / 2);
                let p = ((y * w + x) * 4) as usize;
                f.rgba[p] = v;
                f.rgba[p + 1] = v / 2 + y as u8;
                f.rgba[p + 2] = 200 - base / 2;
            }
        }
        f
    }

    #[test]
    fn rates_convert_between_film_and_video() {
        assert!((film_rate(30.0) - 24.0).abs() < 1e-9);
        assert!((video_rate(24.0) - 30.0).abs() < 1e-9);
        assert!((film_rate(30000.0 / 1001.0) - 24000.0 / 1001.0).abs() < 1e-9);
    }

    #[test]
    fn pulldown_makes_five_video_frames_of_four_with_two_mixed_ones() {
        let film: Vec<_> = (0..4).map(film_frame).collect();
        let video = apply_pulldown(&film);
        assert_eq!(video.len(), 5);
        assert_eq!(video[0], film[0]);
        assert_eq!(video[1], film[1]);
        assert_eq!(video[4], film[3]);
        assert_ne!(video[2], film[1]);
        assert_ne!(video[2], film[2]);
        // BC: B in the even lines, C in the odd ones
        let rb = 32 * 4;
        assert_eq!(&video[2].rgba[0..rb], &film[1].rgba[0..rb]);
        assert_eq!(&video[2].rgba[rb..2 * rb], &film[2].rgba[rb..2 * rb]);
        // CD: C even, D odd
        assert_eq!(&video[3].rgba[0..rb], &film[2].rgba[0..rb]);
        assert_eq!(&video[3].rgba[rb..2 * rb], &film[3].rgba[rb..2 * rb]);
        assert!(comb_score(&video[2]) > comb_score(&video[1]) + 1.0);
    }

    #[test]
    fn removing_the_pulldown_gives_the_original_film_back_whatever_the_phase() {
        let film: Vec<_> = (0..12).map(film_frame).collect();
        let video = apply_pulldown(&film);
        assert_eq!(video.len(), 15);
        assert_eq!(detect_phase(&video), Some(0));
        let back = remove_pulldown_auto(&video).unwrap();
        assert_eq!(back, film);
        // Cut two frames off the front: the cadence starts mid-group
        let cut = &video[2..];
        let p = detect_phase(cut).expect("phase found");
        assert_eq!(p, 3);
        let rebuilt = remove_pulldown(cut, p);
        // The group at video[5..10] is film 4..8, and so on
        assert_eq!(rebuilt, film[4..12].to_vec());
    }

    #[test]
    fn progressive_material_has_no_cadence() {
        let film: Vec<_> = (0..10).map(film_frame).collect();
        assert_eq!(detect_phase(&film), None);
        assert!(remove_pulldown_auto(&film).is_none());
        assert!(remove_pulldown_auto(&film[..3]).is_none());
        assert!(apply_pulldown(&[]).is_empty());
    }
}
