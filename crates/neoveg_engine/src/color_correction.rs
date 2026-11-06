//! Primary color correction of an event as a file (export / import between events and projects),
//! and the review helper that finds the video events nobody has corrected yet.

use neoveg_common::grade::EventColorGrade;
use serde::{Deserialize, Serialize};

use crate::timeline::{EventId, Timeline, TrackEvent, TrackId, TrackKind};

/// Value of the `format` field of a correction file.
pub const FORMAT_TAG: &str = "neoveg-color-correction";
/// Version written by this build; newer files are refused.
pub const FORMAT_VERSION: u32 = 1;

/// Why a correction file could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorrectionFileError {
    /// The text is not valid JSON of the expected shape.
    Malformed(String),
    /// The JSON is valid but is not a correction file.
    WrongFormat,
    /// The file was written by a newer version.
    TooNew(u32),
}

impl std::fmt::Display for CorrectionFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed(e) => write!(f, "Arquivo de correção ilegível: {e}"),
            Self::WrongFormat => write!(f, "O arquivo não é uma correção de cor do Neoveg"),
            Self::TooNew(v) => write!(f, "Correção gravada por uma versão mais nova ({v})"),
        }
    }
}

impl std::error::Error for CorrectionFileError {}

/// The primary correction of one event, as written to disk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColorCorrectionFile {
    pub format: String,
    pub version: u32,
    /// Name of the event it was taken from (information only).
    #[serde(default)]
    pub source_name: String,
    pub grade: EventColorGrade,
}

impl ColorCorrectionFile {
    /// Takes the correction of `event`; `None` when the event has none (or only a neutral one).
    pub fn from_event(event: &TrackEvent) -> Option<Self> {
        let grade = event.color_grade.as_ref().filter(|g| !g.is_neutral())?;
        Some(Self {
            format: FORMAT_TAG.into(),
            version: FORMAT_VERSION,
            source_name: event.name.clone(),
            grade: sanitize(grade.clone()),
        })
    }

    /// Pretty JSON of the file.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Reads a file, bringing every value into range.
    pub fn from_json(text: &str) -> Result<Self, CorrectionFileError> {
        let mut file: Self = serde_json::from_str(text)
            .map_err(|e| CorrectionFileError::Malformed(e.to_string()))?;
        if file.format != FORMAT_TAG {
            return Err(CorrectionFileError::WrongFormat);
        }
        if file.version > FORMAT_VERSION {
            return Err(CorrectionFileError::TooNew(file.version));
        }
        file.grade = sanitize(file.grade);
        Ok(file)
    }

    /// Applies the correction to `event`, replacing the one it had.
    pub fn apply_to(&self, event: &mut TrackEvent) {
        event.color_grade = Some(sanitize(self.grade.clone()));
    }
}

fn sanitize(mut grade: EventColorGrade) -> EventColorGrade {
    grade.wheels = grade.wheels.sanitized();
    grade.white_balance = grade.white_balance.sanitized();
    grade.utilities = grade.utilities.sanitized();
    if !grade.lut_strength.is_finite() {
        grade.lut_strength = 1.0;
    }
    grade
}

/// True when the event is one of a video track that carries a correction worth the name.
pub fn has_color_correction(event: &TrackEvent) -> bool {
    event.color_grade.as_ref().is_some_and(|g| !g.is_neutral())
}

/// Events of the video tracks that have no color correction yet (adjustment events and empty ones
/// included only when they hold media or are adjustments), in track and time order, for a
/// review of the whole film.
pub fn events_without_color_correction(timeline: &Timeline) -> Vec<(TrackId, EventId)> {
    let mut out = Vec::new();
    for track in timeline
        .tracks
        .iter()
        .filter(|t| t.kind == TrackKind::Video)
    {
        let mut events: Vec<&TrackEvent> = track
            .events
            .iter()
            .filter(|e| (e.media_id.is_some() || e.adjustment) && !has_color_correction(e))
            .collect();
        events.sort_by(|a, b| a.start.total_cmp(&b.start));
        out.extend(events.into_iter().map(|e| (track.id, e.id)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::Track;
    use neoveg_common::grade::WhiteBalanceGrade;

    fn event(id: u64, start: f64, media: Option<u64>, graded: bool) -> TrackEvent {
        TrackEvent {
            id,
            name: format!("ev{id}"),
            media_id: media,
            start,
            duration: 1.0,
            color_grade: graded.then(|| EventColorGrade {
                white_balance: WhiteBalanceGrade {
                    temperature: 0.3,
                    tint: -0.1,
                },
                ..EventColorGrade::default()
            }),
            ..TrackEvent::default()
        }
    }

    fn track(tl: &mut Timeline, kind: TrackKind, events: Vec<TrackEvent>) -> Track {
        let mut t = tl.new_track(kind);
        t.events = events;
        t
    }

    #[test]
    fn a_correction_survives_a_trip_through_the_file_and_lands_on_another_event() {
        let src = event(1, 0.0, Some(9), true);
        let file = ColorCorrectionFile::from_event(&src).expect("graded event");
        let back = ColorCorrectionFile::from_json(&file.to_json()).unwrap();
        assert_eq!(back, file);
        let mut dst = event(2, 5.0, Some(9), false);
        assert!(dst.color_grade.is_none());
        back.apply_to(&mut dst);
        assert_eq!(dst.color_grade, src.color_grade);
        // An event without correction has nothing to export
        assert!(ColorCorrectionFile::from_event(&event(3, 0.0, Some(9), false)).is_none());
    }

    #[test]
    fn hostile_files_are_refused_or_brought_into_range() {
        assert!(matches!(
            ColorCorrectionFile::from_json("{\"a\":1}"),
            Err(CorrectionFileError::Malformed(_)) | Err(CorrectionFileError::WrongFormat)
        ));
        assert!(ColorCorrectionFile::from_json("nope").is_err());
        let mut f = ColorCorrectionFile::from_event(&event(1, 0.0, Some(1), true)).unwrap();
        f.format = "outro".into();
        assert_eq!(
            ColorCorrectionFile::from_json(&f.to_json()),
            Err(CorrectionFileError::WrongFormat)
        );
        f.format = FORMAT_TAG.into();
        f.version = 99;
        assert_eq!(
            ColorCorrectionFile::from_json(&f.to_json()),
            Err(CorrectionFileError::TooNew(99))
        );
        f.version = 1;
        f.grade.white_balance.temperature = 7.0;
        let ok = ColorCorrectionFile::from_json(&f.to_json()).unwrap();
        assert_eq!(ok.grade.white_balance.temperature, 1.0);
    }

    #[test]
    fn events_without_correction_are_found_on_video_tracks_only_in_time_order() {
        let mut tl = Timeline::default();
        let v = track(
            &mut tl,
            TrackKind::Video,
            vec![
                event(10, 4.0, Some(1), false),
                event(11, 2.0, Some(1), true),
                event(12, 0.0, Some(1), false),
                event(13, 6.0, None, false),
            ],
        );
        let vid = v.id;
        tl.tracks.push(v);
        let a = track(
            &mut tl,
            TrackKind::Audio,
            vec![event(20, 0.0, Some(2), false)],
        );
        tl.tracks.push(a);
        assert_eq!(
            events_without_color_correction(&tl),
            vec![(vid, 12), (vid, 10)]
        );
        // A neutral grade counts as no correction
        let mut tl2 = tl.clone();
        tl2.tracks[0].events[0].color_grade = Some(EventColorGrade::default());
        assert!(events_without_color_correction(&tl2).contains(&(vid, 10)));
    }
}
