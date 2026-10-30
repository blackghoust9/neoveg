//! Geometry of the timeline scrollbars: the thumb of a bar and the scroll offset a drag asks for.
//!
//! Pure functions, so the rules (what a bar spans, how long the thumb is, where a drag lands) are
//! tested without a window. The widget that draws and reads the bar lives in `app.rs`.

/// Thickness of a scrollbar, in points.
pub const BAR: f32 = 12.0;
/// The thumb never gets shorter than this, so it stays easy to grab on a long project.
const MIN_THUMB: f32 = 24.0;

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
}
