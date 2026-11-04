//! Geometry of the timeline scrollbars: the thumb of a bar and the scroll offset a drag asks for.
//!
//! Pure functions, so the rules (what a bar spans, how long the thumb is, where a drag lands) are
//! tested without a window. The widget that draws and reads the bar lives in `app.rs`.

/// Thickness of a scrollbar, in points.
pub const BAR: f32 = 12.0;
/// The thumb never gets shorter than this, so it stays easy to grab on a long project.
const MIN_THUMB: f32 = 24.0;

/// What the mouse wheel does over the timeline (the reference's *Mouse wheel shortcuts*): the wheel
/// alone zooms, `Shift` scrolls in time, `Ctrl` scrolls the tracks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Wheel {
    /// Multiply the zoom by this factor about the pointer.
    Zoom(f64),
    /// Scroll in time by this many points (positive: later).
    ScrollTime(f64),
    /// Scroll the tracks by this many points (positive: down).
    ScrollTracks(f64),
    Nothing,
}

/// Points of wheel that make the zoom change by a factor of `e` (about 1.28 for a notch of 50).
const WHEEL_ZOOM_SCALE: f64 = 200.0;

/// Reads what the wheel asked for. `scroll` is the scroll delta of the frame in points (the wheel
/// forward is positive `y`), `zoom_delta` what the input made of the pinch gesture and of a wheel
/// with `Ctrl`, which egui reports as a zoom.
pub fn wheel(scroll: (f32, f32), zoom_delta: f64, ctrl: bool, shift: bool) -> Wheel {
    let (sx, sy) = (f64::from(scroll.0), f64::from(scroll.1));
    if ctrl {
        // Ctrl+wheel arrives as a zoom: turn it back into the points of the wheel
        let points = zoom_delta.max(1e-6).ln() * WHEEL_ZOOM_SCALE;
        return if points.abs() > 1e-6 {
            Wheel::ScrollTracks(-points)
        } else if sy.abs() > 1e-6 {
            Wheel::ScrollTracks(-sy)
        } else {
            Wheel::Nothing
        };
    }
    if (zoom_delta - 1.0).abs() > 1e-6 {
        return Wheel::Zoom(zoom_delta); // a pinch on a touchpad
    }
    if shift {
        let points = sx + sy;
        return if points.abs() > 1e-6 {
            Wheel::ScrollTime(-points)
        } else {
            Wheel::Nothing
        };
    }
    if sy.abs() > 1e-6 {
        return Wheel::Zoom((sy / WHEEL_ZOOM_SCALE).exp());
    }
    if sx.abs() > 1e-6 {
        return Wheel::ScrollTime(-sx);
    }
    Wheel::Nothing
}

/// The zoom (`pixels_per_second`) and scroll offset after the zoom is multiplied by `factor` with the
/// time `anchor` (the cursor) kept at the same place on the screen. The zoom stays between `min` and
/// `max`.
pub fn zoom_about(
    pixels_per_second: f64,
    scroll: f64,
    anchor: f64,
    factor: f64,
    min: f64,
    max: f64,
) -> (f64, f64) {
    let zoom = (pixels_per_second * factor).clamp(min, max);
    let scroll = anchor - (anchor - scroll) * pixels_per_second / zoom;
    (zoom, scroll.max(0.0))
}

/// The zoom and scroll offset that show `start..end` across `width_px` pixels with a margin of a
/// twentieth on each side; `None` for an empty range or no room.
pub fn fit_range(start: f64, end: f64, width_px: f64, min: f64, max: f64) -> Option<(f64, f64)> {
    let span = end - start;
    if !(span.is_finite() && span > 1e-9 && width_px > 1.0) {
        return None;
    }
    let zoom = (width_px / (span * 1.1)).clamp(min, max);
    let shown = width_px / zoom;
    // The range sits in the middle when the zoom limit leaves room
    Some((zoom, (start - (shown - span) * 0.5).max(0.0)))
}

/// The zoom at which a frame is `frame_px` pixels wide (a video thumbnail of that width then shows
/// one frame).
pub fn pixels_for_frame(fps: f64, frame_px: f64, min: f64, max: f64) -> f64 {
    let fps = if fps.is_finite() && fps > 0.0 {
        fps
    } else {
        30.0
    };
    (fps * frame_px).clamp(min, max)
}

/// Seconds the horizontal bar spans: the end of the project or of the cursor plus a quarter of the
/// view for room to work, and never less than what is on screen now.
pub fn content_seconds(project_end: f64, playhead: f64, view: f64, offset: f64) -> f64 {
    let end = project_end.max(playhead).max(0.0);
    (end + view * 0.25).max(offset + view).max(view)
}

/// Largest scroll offset: the content minus one view (zero when everything fits).
pub fn max_offset(content: f64, view: f64) -> f64 {
    (content - view).max(0.0)
}

/// Start and length, along a bar `track_len` long, of the thumb for `offset`. The thumb fills the
/// bar when everything fits.
pub fn thumb(track_len: f32, content: f64, view: f64, offset: f64) -> (f32, f32) {
    let track_len = track_len.max(0.0);
    let room = max_offset(content, view);
    if room <= 1e-9 || content <= 0.0 {
        return (0.0, track_len);
    }
    let len = ((view / content) as f32 * track_len).clamp(MIN_THUMB.min(track_len), track_len);
    let start = (offset.clamp(0.0, room) / room) as f32 * (track_len - len);
    (start, len)
}

/// The scroll offset for a thumb whose start sits `start` along the bar.
pub fn offset_for_thumb_start(
    track_len: f32,
    content: f64,
    view: f64,
    thumb_len: f32,
    start: f32,
) -> f64 {
    let travel = (track_len - thumb_len).max(0.0);
    if travel <= 0.0 {
        return 0.0;
    }
    f64::from((start / travel).clamp(0.0, 1.0)) * max_offset(content, view)
}

/// How far the track rows can scroll: their height minus the body of the canvas (never negative).
pub fn max_scroll_y(rows: usize, row_height: f32, body_height: f32) -> f32 {
    (rows as f32 * row_height - body_height).max(0.0)
}

/// The scroll offset that shows row number `row` completely, moving as little as possible: the
/// same offset when the row is already in view, and the top of the row when it is taller than the
/// body.
pub fn reveal_row(scroll: f32, row: usize, row_height: f32, body_height: f32) -> f32 {
    let top = row as f32 * row_height;
    let bottom = top + row_height;
    if row_height >= body_height || top < scroll {
        top
    } else if bottom > scroll + body_height {
        bottom - body_height
    } else {
        scroll
    }
}

/// `scroll` moved by `delta` and kept between zero and `max`.
pub fn scrolled(scroll: f32, delta: f32, max: f32) -> f32 {
    (scroll + delta).clamp(0.0, max.max(0.0))
}

/// How far a page of rows scrolls: the body less one row, so the row at the edge stays in view as
/// a landmark (never less than one row).
pub fn page_delta(row_height: f32, body_height: f32) -> f32 {
    (body_height - row_height).max(row_height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_out_of_view_scrolls_in_by_the_least_and_one_in_view_stays_put() {
        // 64 px rows in a 300 px body
        assert_eq!(reveal_row(0.0, 2, 64.0, 300.0), 0.0, "inside: no move");
        assert_eq!(
            reveal_row(0.0, 4, 64.0, 300.0),
            20.0,
            "ends at 320, 20 px below the body"
        );
        assert_eq!(
            reveal_row(0.0, 5, 64.0, 300.0),
            84.0,
            "below: its bottom meets the bottom edge"
        );
        assert_eq!(
            reveal_row(500.0, 3, 64.0, 300.0),
            192.0,
            "above: its top meets the top edge"
        );
        assert_eq!(
            reveal_row(100.0, 3, 64.0, 300.0),
            100.0,
            "192 to 256 is inside 100 to 400"
        );
        // A body shorter than a row shows the top of the row
        assert_eq!(reveal_row(0.0, 3, 64.0, 40.0), 192.0);
    }

    #[test]
    fn steps_and_pages_stay_inside_the_scrollable_range() {
        assert_eq!(scrolled(10.0, -64.0, 200.0), 0.0);
        assert_eq!(scrolled(190.0, 64.0, 200.0), 200.0);
        assert_eq!(
            scrolled(50.0, 64.0, 0.0),
            0.0,
            "everything fits: nowhere to go"
        );
        assert_eq!(page_delta(64.0, 300.0), 236.0);
        assert_eq!(page_delta(64.0, 30.0), 64.0, "never less than a row");
    }

    #[test]
    fn the_bar_spans_the_project_with_room_and_never_less_than_the_view() {
        // 100 s project, 20 s on screen: the project plus a quarter of the view (5 s)
        assert_eq!(content_seconds(100.0, 0.0, 20.0, 0.0), 105.0);
        // The cursor past the end extends it
        assert_eq!(content_seconds(10.0, 200.0, 20.0, 0.0), 205.0);
        // An empty project is one view long
        assert_eq!(content_seconds(0.0, 0.0, 20.0, 0.0), 20.0);
        // Scrolled beyond the content: the bar grows to still show what is on screen
        assert_eq!(content_seconds(5.0, 0.0, 20.0, 90.0), 110.0);
        assert_eq!(max_offset(105.0, 20.0), 85.0);
        assert_eq!(max_offset(10.0, 20.0), 0.0);
    }

    #[test]
    fn the_thumb_fills_the_bar_when_everything_fits_and_shrinks_with_the_content() {
        assert_eq!(thumb(300.0, 20.0, 20.0, 0.0), (0.0, 300.0));
        let (start, len) = thumb(300.0, 100.0, 25.0, 0.0);
        assert_eq!((start, len), (0.0, 75.0));
        let (start, len) = thumb(300.0, 100.0, 25.0, 75.0);
        assert!(
            (start - 225.0).abs() < 1e-3 && (len - 75.0).abs() < 1e-3,
            "at the end the thumb touches the end"
        );
        // A huge project still leaves a thumb that can be grabbed
        assert_eq!(thumb(300.0, 100_000.0, 20.0, 0.0).1, MIN_THUMB);
        // An offset outside the range is clamped
        assert_eq!(thumb(300.0, 100.0, 25.0, -5.0).0, 0.0);
        assert!((thumb(300.0, 100.0, 25.0, 500.0).0 - 225.0).abs() < 1e-3);
    }

    #[test]
    fn dragging_the_thumb_maps_back_to_the_offset_and_round_trips() {
        let (content, view, track) = (100.0, 25.0, 300.0);
        for offset in [0.0, 10.0, 37.5, 75.0] {
            let (start, len) = thumb(track, content, view, offset);
            let back = offset_for_thumb_start(track, content, view, len, start);
            assert!(
                (back - offset).abs() < 1e-3,
                "offset {offset} came back as {back}"
            );
        }
        // Past either end of the bar the offset stops at its limits
        assert_eq!(
            offset_for_thumb_start(track, content, view, 75.0, -50.0),
            0.0
        );
        assert_eq!(
            offset_for_thumb_start(track, content, view, 75.0, 999.0),
            75.0
        );
        // A thumb as long as the bar cannot move
        assert_eq!(offset_for_thumb_start(track, 10.0, 20.0, 300.0, 50.0), 0.0);
    }

    #[test]
    fn the_rows_scroll_only_when_they_do_not_fit() {
        assert_eq!(max_scroll_y(3, 64.0, 300.0), 0.0);
        assert_eq!(max_scroll_y(6, 64.0, 300.0), 84.0);
        assert_eq!(max_scroll_y(0, 64.0, 300.0), 0.0);
    }

    #[test]
    fn zooming_keeps_the_anchor_where_it_is_on_the_screen() {
        // 100 px/s, the view starts at 10 s, the cursor is at 12 s: 200 px from the left edge
        let (zoom, scroll) = zoom_about(100.0, 10.0, 12.0, 2.0, 15.0, 480_000.0);
        assert_eq!(zoom, 200.0);
        assert!(
            (scroll - 11.0).abs() < 1e-9,
            "the cursor stays 200 px in: {scroll}"
        );
        // Zooming back out returns to where it was
        let (zoom, scroll) = zoom_about(zoom, scroll, 12.0, 0.5, 15.0, 480_000.0);
        assert_eq!((zoom, (scroll - 10.0).abs() < 1e-9), (100.0, true));
        // The limits hold, and the view never starts before zero
        assert_eq!(zoom_about(20.0, 0.0, 5.0, 0.1, 15.0, 480_000.0).0, 15.0);
        assert_eq!(
            zoom_about(400_000.0, 0.0, 5.0, 10.0, 15.0, 480_000.0).0,
            480_000.0
        );
        assert_eq!(zoom_about(100.0, 0.0, 0.1, 0.5, 15.0, 480_000.0).1, 0.0);
    }

    #[test]
    fn a_range_is_fitted_with_a_margin_and_one_frame_gets_a_thumbnail_wide() {
        let (zoom, scroll) = fit_range(10.0, 20.0, 1100.0, 15.0, 480_000.0).unwrap();
        assert!((zoom - 100.0).abs() < 1e-9, "{zoom}");
        assert!(
            (scroll - 9.5).abs() < 1e-9,
            "a twentieth of the span before it: {scroll}"
        );
        assert!(fit_range(5.0, 5.0, 800.0, 15.0, 480_000.0).is_none());
        assert!(fit_range(0.0, 1.0, 0.0, 15.0, 480_000.0).is_none());
        // A range at the very start does not scroll to a negative time
        assert_eq!(
            fit_range(0.0, 10.0, 1100.0, 15.0, 480_000.0).unwrap().1,
            0.0
        );
        assert_eq!(pixels_for_frame(25.0, 114.0, 15.0, 480_000.0), 2850.0);
        assert_eq!(pixels_for_frame(f64::NAN, 100.0, 15.0, 480_000.0), 3000.0);
        assert_eq!(
            pixels_for_frame(30.0, 100_000.0, 15.0, 480_000.0),
            480_000.0
        );
    }

    #[test]
    fn the_wheel_alone_zooms_shift_scrolls_in_time_and_ctrl_scrolls_the_tracks() {
        // The wheel forward zooms in, backward out, by the same factor
        let Wheel::Zoom(zin) = wheel((0.0, 50.0), 1.0, false, false) else {
            panic!("the wheel alone zooms")
        };
        let Wheel::Zoom(zout) = wheel((0.0, -50.0), 1.0, false, false) else {
            panic!("the wheel alone zooms")
        };
        assert!(
            zin > 1.2 && zin < 1.35 && (zin * zout - 1.0).abs() < 1e-9,
            "{zin} {zout}"
        );
        // Shift: the wheel forward goes back in time
        assert_eq!(
            wheel((0.0, 50.0), 1.0, false, true),
            Wheel::ScrollTime(-50.0)
        );
        assert_eq!(
            wheel((0.0, -50.0), 1.0, false, true),
            Wheel::ScrollTime(50.0)
        );
        // Ctrl arrives as a zoom delta and goes back as a scroll of the tracks, forward is up
        let Wheel::ScrollTracks(up) = wheel((0.0, 0.0), (50.0f64 / 200.0).exp(), true, false)
        else {
            panic!("ctrl scrolls the tracks")
        };
        assert!((up + 50.0).abs() < 1e-6, "{up}");
        // A pinch zooms by what it says; a sideways wheel scrolls in time; nothing does nothing
        assert_eq!(wheel((0.0, 0.0), 1.5, false, false), Wheel::Zoom(1.5));
        assert_eq!(
            wheel((30.0, 0.0), 1.0, false, false),
            Wheel::ScrollTime(-30.0)
        );
        assert_eq!(wheel((0.0, 0.0), 1.0, false, false), Wheel::Nothing);
        assert_eq!(wheel((0.0, 0.0), 1.0, true, true), Wheel::Nothing);
    }
}
