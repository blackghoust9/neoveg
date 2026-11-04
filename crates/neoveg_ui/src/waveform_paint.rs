//! Sample-accurate waveform drawing.
//!
//! - At 1 or more samples per pixel: the exact min/max (outer) and RMS (inner) of every pixel column.
//!   A single-sample transient is never averaged away.
//! - Below 1 sample per pixel: the individual samples joined by lines, with dots and stems once the
//!   samples are at least 6 px apart (Audacity style).

use eframe::egui::{pos2, Color32, Mesh, Painter, Rect, Shape, Stroke};
use neoveg_audio::waveform::{WaveformSource, COLUMN_STRIDE};

/// Pixels between samples from which dots and stems are drawn.
const SAMPLE_DOT_MIN_SPACING_PX: f64 = 6.0;

pub fn paint_waveform(
    painter: &Painter,
    rect: Rect,
    source: &WaveformSource,
    start_frame: f64,
    frames_per_pixel: f64,
    color: Color32,
) {
    if rect.width() < 1.0
        || rect.height() < 4.0
        || frames_per_pixel <= 0.0
        || !frames_per_pixel.is_finite()
    {
        return;
    }
    let painter = painter.with_clip_rect(rect);
    let lanes = if source.channels > 1 { 2usize } else { 1 };
    let lane_h = rect.height() / lanes as f32;

    // Centre axis of every lane
    for lane in 0..lanes {
        let mid = rect.top() + lane_h * (lane as f32 + 0.5);
        painter.line_segment(
            [pos2(rect.left(), mid), pos2(rect.right(), mid)],
            Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(148, 163, 184, 90)),
        );
    }

    if frames_per_pixel >= 1.0 {
        paint_columns(
            &painter,
            rect,
            source,
            start_frame,
            frames_per_pixel,
            color,
            lanes,
            lane_h,
        );
    } else {
        paint_samples(
            &painter,
            rect,
            source,
            start_frame,
            frames_per_pixel,
            color,
            lanes,
            lane_h,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_columns(
    painter: &Painter,
    rect: Rect,
    source: &WaveformSource,
    start_frame: f64,
    frames_per_pixel: f64,
    color: Color32,
    lanes: usize,
    lane_h: f32,
) {
    let columns = rect.width().ceil() as usize;
    let mut buf = vec![0.0f32; columns * COLUMN_STRIDE];
    source.query_columns(start_frame, frames_per_pixel, &mut buf);

    let outer = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 150);
    let inner = color;
    let half = lane_h * 0.5;
    let mut outer_mesh = Mesh::default();
    let mut inner_mesh = Mesh::default();

    for lane in 0..lanes {
        let mid = rect.top() + lane_h * (lane as f32 + 0.5);
        for col in 0..columns {
            let base = col * COLUMN_STRIDE + lane * 3;
            let (min, max, rms) = (buf[base], buf[base + 1], buf[base + 2]);
            if min == 0.0 && max == 0.0 {
                continue;
            }
            let x = rect.left() + col as f32;
            let mut top = mid - max * half;
            let mut bottom = mid - min * half;
            // Every non-silent column is at least 1 px tall so single-sample peaks stay visible
            if bottom - top < 1.0 {
                let centre = (top + bottom) * 0.5;
                top = centre - 0.5;
                bottom = centre + 0.5;
            }
            outer_mesh.add_colored_rect(
                Rect::from_min_max(pos2(x, top), pos2(x + 1.0, bottom)),
                outer,
            );
            let r = rms * half;
            if r >= 0.5 {
                inner_mesh.add_colored_rect(
                    Rect::from_min_max(pos2(x, mid - r), pos2(x + 1.0, mid + r)),
                    inner,
                );
            }
        }
    }
    painter.add(Shape::mesh(outer_mesh));
    painter.add(Shape::mesh(inner_mesh));
}

#[allow(clippy::too_many_arguments)]
fn paint_samples(
    painter: &Painter,
    rect: Rect,
    source: &WaveformSource,
    start_frame: f64,
    frames_per_pixel: f64,
    color: Color32,
    lanes: usize,
    lane_h: f32,
) {
    let pixels_per_frame = 1.0 / frames_per_pixel;
    let first = start_frame.floor() as i64;
    let count = (rect.width() as f64 * frames_per_pixel).ceil() as usize + 3;
    let mut left = vec![0.0f32; count];
    let mut right = vec![0.0f32; count];
    let read = source.read_samples(first, &mut left, &mut right);
    // read_samples shifts values when `first` is negative, so index by position in the buffer
    let skip = (-first).max(0) as usize;
    if read == 0 {
        return;
    }

    let half = lane_h * 0.5;
    let stem = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 150);
    let draw_dots = pixels_per_frame >= SAMPLE_DOT_MIN_SPACING_PX;

    for lane in 0..lanes {
        let data = if lane == 0 { &left } else { &right };
        let mid = rect.top() + lane_h * (lane as f32 + 0.5);
        let mut points = Vec::with_capacity(read);
        for (i, value) in data.iter().enumerate().skip(skip).take(read) {
            let frame = (first + i as i64) as f64;
            let x = rect.left() + ((frame - start_frame) * pixels_per_frame) as f32;
            points.push(pos2(x, mid - value * half));
        }
        if draw_dots {
            for p in &points {
                painter.line_segment([pos2(p.x, mid), *p], Stroke::new(1.0_f32, stem));
                painter.circle_filled(*p, 2.5, color);
            }
        }
        painter.add(Shape::line(points, Stroke::new(1.0_f32, color)));
    }
}
