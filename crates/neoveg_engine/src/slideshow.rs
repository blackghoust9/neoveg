//! The slideshow creator's rules: the order of the pictures and where each one lands on the
//! timeline.
//!
//! The reference manuals describe a window where still pictures are chosen, arranged (dragged,
//! moved one place, sorted from A to Z or Z to A, by date from new to old or old to new, shuffled,
//! deleted) and given a length, either per picture or for the whole show, with the transition
//! between them. These are the pure parts of that: ordering and layout, tested without a window.

use std::cmp::Ordering;

/// One picture of the show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slide {
    pub path: String,
    /// File name, for sorting and for the event title.
    pub name: String,
    /// Last modification, in seconds since the Unix epoch (`0` when unknown).
    pub modified: u64,
}

impl Slide {
    pub fn new(path: &str, modified: u64) -> Self {
        let name = std::path::Path::new(path)
            .file_name()
            .map_or_else(|| path.to_owned(), |n| n.to_string_lossy().into_owned());
        Self {
            path: path.to_owned(),
            name,
            modified,
        }
    }
}

/// How the pictures are put in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlideOrder {
    NameAscending,
    NameDescending,
    NewestFirst,
    OldestFirst,
    /// Random order; the same seed gives the same order.
    Shuffle(u64),
}

/// Compares file names the way people expect: case does not matter and numbers count by value, so
/// `foto2` comes before `foto10`.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut ai, mut bi) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (ai.peek().copied(), bi.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let take = |it: &mut std::iter::Peekable<std::str::Chars<'_>>| {
                    let mut digits = String::new();
                    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
                        digits.push(c);
                        it.next();
                    }
                    digits
                };
                let (da, db) = (take(&mut ai), take(&mut bi));
                let (ta, tb) = (da.trim_start_matches('0'), db.trim_start_matches('0'));
                // More digits is a bigger number; the same length compares digit by digit
                let order = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb));
                if order != Ordering::Equal {
                    return order;
                }
            }
            (Some(x), Some(y)) => {
                let order = x.to_lowercase().cmp(y.to_lowercase());
                if order != Ordering::Equal {
                    return order;
                }
                ai.next();
                bi.next();
            }
        }
    }
}

/// Puts the pictures in `order`.
pub fn sort_slides(slides: &mut [Slide], order: SlideOrder) {
    match order {
        SlideOrder::NameAscending => slides.sort_by(|a, b| natural_cmp(&a.name, &b.name)),
        SlideOrder::NameDescending => slides.sort_by(|a, b| natural_cmp(&b.name, &a.name)),
        SlideOrder::NewestFirst => slides.sort_by(|a, b| {
            b.modified
                .cmp(&a.modified)
                .then_with(|| natural_cmp(&a.name, &b.name))
        }),
        SlideOrder::OldestFirst => slides.sort_by(|a, b| {
            a.modified
                .cmp(&b.modified)
                .then_with(|| natural_cmp(&a.name, &b.name))
        }),
        SlideOrder::Shuffle(seed) => {
            // Fisher-Yates with a small xorshift generator
            let mut state = seed | 1;
            let mut next = move || {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state
            };
            for i in (1..slides.len()).rev() {
                let j = (next() % (i as u64 + 1)) as usize;
                slides.swap(i, j);
            }
        }
    }
}

/// Moves the pictures at `selected` one place earlier (`delta` -1) or later (`delta` 1) and returns
/// where they ended up. Pictures at the end of the list stay there, and the others keep their order.
pub fn move_selected(slides: &mut [Slide], selected: &[usize], delta: i32) -> Vec<usize> {
    let mut chosen: Vec<usize> = selected
        .iter()
        .copied()
        .filter(|&i| i < slides.len())
        .collect();
    chosen.sort_unstable();
    chosen.dedup();
    if delta < 0 {
        for k in 0..chosen.len() {
            let i = chosen[k];
            // A picture that is already first (or right behind one that is) cannot go further
            if i > 0 && (k == 0 || chosen[k - 1] != i - 1) {
                slides.swap(i, i - 1);
                chosen[k] = i - 1;
            }
        }
    } else if delta > 0 {
        for k in (0..chosen.len()).rev() {
            let i = chosen[k];
            if i + 1 < slides.len() && (k + 1 == chosen.len() || chosen[k + 1] != i + 1) {
                slides.swap(i, i + 1);
                chosen[k] = i + 1;
            }
        }
    }
    chosen
}

/// Where the show starts and how long each picture stays.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlideshowSettings {
    /// Timeline position of the first picture, in seconds.
    pub start: f64,
    /// Length of each picture, in seconds.
    pub per_photo: f64,
    /// Overlap of two neighbouring pictures, in seconds (the length of the transition).
    pub overlap: f64,
}

/// Seconds each picture needs for the whole show to last `total` seconds with `count` pictures.
pub fn per_photo_for_total(count: usize, total: f64, overlap: f64) -> f64 {
    if count == 0 {
        return 0.0;
    }
    let n = count as f64;
    ((total + overlap * (n - 1.0)) / n).max(0.0)
}

/// Start and length of every picture. The overlap is limited to under half a picture so that no
/// picture is hidden entirely by its neighbours.
pub fn layout(count: usize, settings: &SlideshowSettings) -> Vec<(f64, f64)> {
    let per = settings.per_photo.max(0.0);
    let overlap = settings.overlap.clamp(0.0, per * 0.49);
    (0..count)
        .map(|i| (settings.start.max(0.0) + i as f64 * (per - overlap), per))
        .collect()
}

/// Length of the whole show for the given pictures and settings.
pub fn total_length(count: usize, settings: &SlideshowSettings) -> f64 {
    layout(count, settings)
        .last()
        .map_or(0.0, |(start, len)| start + len - settings.start.max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slides(names: &[&str]) -> Vec<Slide> {
        names
            .iter()
            .enumerate()
            .map(|(i, n)| Slide::new(&format!("/fotos/{n}"), 100 + i as u64))
            .collect()
    }

    fn names(slides: &[Slide]) -> Vec<&str> {
        slides.iter().map(|s| s.name.as_str()).collect()
    }

    #[test]
    fn natural_order_counts_numbers_by_value_and_ignores_case() {
        assert_eq!(natural_cmp("foto2.jpg", "foto10.jpg"), Ordering::Less);
        assert_eq!(natural_cmp("Foto2.jpg", "foto2.JPG"), Ordering::Equal);
        assert_eq!(natural_cmp("a", "ab"), Ordering::Less);
        assert_eq!(
            natural_cmp("img007", "img7"),
            Ordering::Equal,
            "leading zeros do not count"
        );
        assert_eq!(natural_cmp("img08", "img9"), Ordering::Less);
        assert_eq!(natural_cmp("b", "a1"), Ordering::Greater);
    }

    #[test]
    fn sorting_by_name_and_by_date_goes_both_ways() {
        let mut s = slides(&["foto10.jpg", "foto2.jpg", "Foto1.jpg"]);
        sort_slides(&mut s, SlideOrder::NameAscending);
        assert_eq!(names(&s), vec!["Foto1.jpg", "foto2.jpg", "foto10.jpg"]);
        sort_slides(&mut s, SlideOrder::NameDescending);
        assert_eq!(names(&s), vec!["foto10.jpg", "foto2.jpg", "Foto1.jpg"]);
        // The dates were assigned in the original order: foto10 100, foto2 101, Foto1 102
        let mut s = slides(&["foto10.jpg", "foto2.jpg", "Foto1.jpg"]);
        sort_slides(&mut s, SlideOrder::NewestFirst);
        assert_eq!(names(&s), vec!["Foto1.jpg", "foto2.jpg", "foto10.jpg"]);
        sort_slides(&mut s, SlideOrder::OldestFirst);
        assert_eq!(names(&s), vec!["foto10.jpg", "foto2.jpg", "Foto1.jpg"]);
    }

    #[test]
    fn shuffle_keeps_every_picture_and_the_same_seed_gives_the_same_order() {
        let original = slides(&["a", "b", "c", "d", "e", "f", "g", "h"]);
        let (mut one, mut two, mut other) = (original.clone(), original.clone(), original.clone());
        sort_slides(&mut one, SlideOrder::Shuffle(42));
        sort_slides(&mut two, SlideOrder::Shuffle(42));
        sort_slides(&mut other, SlideOrder::Shuffle(7));
        assert_eq!(one, two);
        assert_ne!(one, original, "the order changed");
        assert_ne!(one, other, "another seed, another order");
        let mut sorted = names(&one).to_vec();
        sorted.sort_unstable();
        assert_eq!(sorted, vec!["a", "b", "c", "d", "e", "f", "g", "h"]);
    }

    #[test]
    fn moving_pictures_one_place_keeps_the_others_in_order_and_stops_at_the_ends() {
        let mut s = slides(&["a", "b", "c", "d"]);
        assert_eq!(move_selected(&mut s, &[2], -1), vec![1]);
        assert_eq!(names(&s), vec!["a", "c", "b", "d"]);
        assert_eq!(move_selected(&mut s, &[0], -1), vec![0], "already first");
        assert_eq!(names(&s), vec!["a", "c", "b", "d"]);
        // Two neighbours move together, and the one at the end stays
        let mut s = slides(&["a", "b", "c", "d"]);
        assert_eq!(move_selected(&mut s, &[2, 3], 1), vec![2, 3]);
        assert_eq!(names(&s), vec!["a", "b", "c", "d"]);
        assert_eq!(move_selected(&mut s, &[0, 1], 1), vec![1, 2]);
        assert_eq!(names(&s), vec!["c", "a", "b", "d"]);
        assert_eq!(move_selected(&mut s, &[99], 1), Vec::<usize>::new());
    }

    #[test]
    fn the_layout_places_each_picture_after_the_last_less_the_overlap() {
        let settings = SlideshowSettings {
            start: 10.0,
            per_photo: 5.0,
            overlap: 1.0,
        };
        let laid = layout(3, &settings);
        assert_eq!(laid, vec![(10.0, 5.0), (14.0, 5.0), (18.0, 5.0)]);
        assert_eq!(total_length(3, &settings), 13.0);
        // No overlap: end to end
        let flat = SlideshowSettings {
            overlap: 0.0,
            ..settings
        };
        assert_eq!(layout(2, &flat), vec![(10.0, 5.0), (15.0, 5.0)]);
        assert!(layout(0, &flat).is_empty());
    }

    #[test]
    fn the_overlap_never_swallows_a_picture() {
        let settings = SlideshowSettings {
            start: 0.0,
            per_photo: 2.0,
            overlap: 5.0,
        };
        let laid = layout(3, &settings);
        // Limited to just under half a picture: each picture still starts well after the last
        assert!(laid[1].0 - laid[0].0 > 1.0);
        assert!(total_length(3, &settings) > 2.0);
    }

    #[test]
    fn a_total_length_gives_the_picture_length_that_fills_it() {
        let per = per_photo_for_total(4, 30.0, 1.0);
        let settings = SlideshowSettings {
            start: 0.0,
            per_photo: per,
            overlap: 1.0,
        };
        assert!((total_length(4, &settings) - 30.0).abs() < 1e-9);
        assert_eq!(per_photo_for_total(0, 30.0, 1.0), 0.0);
    }
}
