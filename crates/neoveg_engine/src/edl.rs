//! CMX 3600 edit decision lists (`.edl`): writing the events of a track and reading a list back
//! into the timeline.
//!
//! The format is the old, plain-text one every editing system still reads: a `TITLE:` line, an
//! `FCM:` line (drop-frame or not) and one line per event with the reel, the track (`V`, `A`...),
//! the transition (`C` cut) and four timecodes: source in and out, record in and out. Comment lines
//! that start with `*` carry what the numbers cannot: `* FROM CLIP NAME:` and `* SOURCE FILE:`.
//!
//! Writing exports the cuts of one track. Reading accepts what other systems write: cuts, and
//! dissolves, wipes and keys taken as cuts (with a warning), speed-change lines ignored (warning),
//! audio channel variants of the track field, drop-frame and non-drop timecode, and the clip name
//! and source file comments. An EDL has no frame rate, so the project's is used.
//!
//! Which media an event refers to is decided by the caller (the engine knows reels and names, not
//! the media pool); see [`import_into_timeline`].

use neoveg_common::timecode::{frame_to_timecode, is_drop_frame_rate, parse_timecode};

use crate::command::EditCommand;
use crate::timeline::{Timeline, TrackEvent, TrackId, TrackKind};

/// What an event puts on the record side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdlTrack {
    Video,
    Audio,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transition {
    Cut,
    /// Dissolve, wipe or key: read as a cut.
    Other,
}

/// One event of a list. Times are seconds (timecode frames divided by the frame rate).
#[derive(Clone, Debug, PartialEq)]
pub struct EdlEvent {
    pub number: u32,
    pub reel: String,
    pub track: EdlTrack,
    pub transition: Transition,
    pub source_in: f64,
    pub source_out: f64,
    pub record_in: f64,
    pub record_out: f64,
    /// `* FROM CLIP NAME:`
    pub clip_name: Option<String>,
    /// `* SOURCE FILE:`
    pub source_file: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EdlDocument {
    pub title: String,
    pub drop_frame: bool,
    pub events: Vec<EdlEvent>,
    /// Problems that did not stop the reading (unsupported effects, bad lines...).
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EdlError {
    NotAnEdl,
}

impl std::fmt::Display for EdlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EdlError::NotAnEdl => write!(f, "o arquivo não é uma lista de edição CMX 3600"),
        }
    }
}

impl std::error::Error for EdlError {}

// ---- writing ----------------------------------------------------------------------------------------

/// What the writer needs to know about the media behind an event.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClipInfo {
    pub name: String,
    pub path: Option<String>,
}

/// Reel name: the first 8 letters and digits of the clip name, upper-case, or `AX` when it has none.
pub fn reel_name(name: &str) -> String {
    let reel: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(8)
        .collect::<String>()
        .to_ascii_uppercase();
    if reel.is_empty() {
        "AX".into()
    } else {
        reel
    }
}

/// Writes the cuts of `track_id` as a CMX 3600 list: events sorted by record start, record
/// timecode starting at `start_hour` hours (1 is the convention). `clip_info` names the media of an
/// event (by its media id). `None` when the track does not exist or has no event.
pub fn export_track(
    timeline: &Timeline,
    track_id: TrackId,
    title: &str,
    fps: f64,
    start_hour: u32,
    clip_info: impl Fn(Option<u64>) -> Option<ClipInfo>,
) -> Option<String> {
    let track = timeline.track(track_id)?;
    if track.events.is_empty() {
        return None;
    }
    let drop = is_drop_frame_rate(fps);
    let kind = match track.kind {
        TrackKind::Video => "V",
        TrackKind::Audio => "A",
    };
    let hour_frames =
        neoveg_common::timecode::timecode_to_frame(i64::from(start_hour), 0, 0, 0, fps, drop);
    let tc = |seconds: f64, record: bool| {
        let frame = (seconds.max(0.0) * fps).round() as i64 + if record { hour_frames } else { 0 };
        frame_to_timecode(frame, fps, drop)
    };
    let mut out = format!(
        "TITLE: {}\nFCM: {}\n\n",
        title.trim(),
        if drop { "DROP FRAME" } else { "NON-DROP FRAME" }
    );
    let mut events: Vec<&TrackEvent> = track.events.iter().collect();
    events.sort_by(|a, b| a.start.total_cmp(&b.start));
    for (n, ev) in events.iter().enumerate() {
        let info = clip_info(ev.media_id).unwrap_or_else(|| ClipInfo {
            name: ev.name.clone(),
            path: None,
        });
        out.push_str(&format!(
            "{:03}  {:<8} {:<5} {:<8} {} {} {} {}\n",
            n + 1,
            reel_name(&info.name),
            kind,
            "C",
            tc(ev.trim_in, false),
            tc(ev.trim_in + ev.duration, false),
            tc(ev.start, true),
            tc(ev.end(), true)
        ));
        out.push_str(&format!("* FROM CLIP NAME: {}\n", info.name));
        if let Some(path) = &info.path {
            out.push_str(&format!("* SOURCE FILE: {path}\n"));
        }
    }
    Some(out)
}

// ---- reading ------------------------------------------------------------------------------------------

/// Splits an event line into its fields; `None` when it is not one.
fn parse_event_line(line: &str, fps: f64) -> Option<Result<EdlEvent, String>> {
    let f: Vec<&str> = line.split_whitespace().collect();
    // number, reel, track, transition [, transition duration] then four timecodes
    if f.len() < 8 || !f[0].bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let tcs: Vec<(i64, bool)> = f[f.len() - 4..]
        .iter()
        .filter_map(|t| parse_timecode(t, fps))
        .collect();
    if tcs.len() != 4 {
        return None;
    }
    let number: u32 = f[0].parse().ok()?;
    let track_field = f[2].to_ascii_uppercase();
    let track = if track_field.starts_with('V') || track_field == "B" {
        EdlTrack::Video
    } else if track_field.starts_with('A') || track_field == "NONE" {
        EdlTrack::Audio
    } else {
        return Some(Err(format!(
            "evento {number}: trilha \"{}\" desconhecida",
            f[2]
        )));
    };
    let transition = if f[3].eq_ignore_ascii_case("C") {
        Transition::Cut
    } else {
        Transition::Other
    };
    let secs = |i: usize| tcs[i].0 as f64 / fps;
    Some(Ok(EdlEvent {
        number,
        reel: f[1].to_owned(),
        track,
        transition,
        source_in: secs(0),
        source_out: secs(1),
        record_in: secs(2),
        record_out: secs(3),
        clip_name: None,
        source_file: None,
    }))
}

/// Reads a CMX 3600 list at `fps`. Errors only when the text has no event at all.
pub fn parse(text: &str, fps: f64) -> Result<EdlDocument, EdlError> {
    let mut doc = EdlDocument {
        drop_frame: is_drop_frame_rate(fps),
        ..Default::default()
    };
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(title) = line.strip_prefix("TITLE:") {
            doc.title = title.trim().to_owned();
            continue;
        }
        if let Some(mode) = line.strip_prefix("FCM:") {
            doc.drop_frame = mode.trim().to_ascii_uppercase().starts_with("DROP");
            continue;
        }
        if let Some(comment) = line.strip_prefix('*') {
            let comment = comment.trim();
            let upper = comment.to_ascii_uppercase();
            if let Some(last) = doc.events.last_mut() {
                if upper.starts_with("FROM CLIP NAME:") {
                    last.clip_name = Some(comment["FROM CLIP NAME:".len()..].trim().to_owned());
                } else if upper.starts_with("SOURCE FILE:") {
                    last.source_file = Some(comment["SOURCE FILE:".len()..].trim().to_owned());
                }
            }
            continue;
        }
        if line.starts_with("M2") {
            doc.warnings
                .push(format!("linha {}: mudança de velocidade ignorada", i + 1));
            continue;
        }
        match parse_event_line(line, fps) {
            Some(Ok(event)) => {
                if event.transition == Transition::Other {
                    doc.warnings.push(format!(
                        "evento {}: transição lida como corte",
                        event.number
                    ));
                }
                if event.record_out <= event.record_in {
                    doc.warnings
                        .push(format!("evento {}: duração zero; ignorado", event.number));
                    continue;
                }
                doc.events.push(event);
            }
            Some(Err(warning)) => doc.warnings.push(warning),
            None => {}
        }
    }
    if doc.events.is_empty() {
        return Err(EdlError::NotAnEdl);
    }
    Ok(doc)
}

// ---- into the timeline -----------------------------------------------------------------------------------

/// The media an event was matched with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MediaMatch {
    pub media_id: u64,
    /// Length of the media, seconds (the event is cut to it).
    pub duration: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImportReport {
    pub added: usize,
    /// Events that could not be placed, with the reason.
    pub skipped: Vec<String>,
}

/// Puts the events of `doc` on the timeline: video events on the first video track and audio events
/// on the first audio track (created when missing), at their record time minus the whole hours the
/// list starts at. `resolve` finds the media of an event; events without media are skipped and
/// reported. One undo step. `None` when nothing could be placed.
pub fn import_into_timeline(
    timeline: &mut Timeline,
    doc: &EdlDocument,
    resolve: impl Fn(&EdlEvent) -> Option<MediaMatch>,
) -> Option<(EditCommand, ImportReport)> {
    let mut report = ImportReport::default();
    let first_in = doc
        .events
        .iter()
        .map(|e| e.record_in)
        .fold(f64::INFINITY, f64::min);
    let offset = if first_in.is_finite() {
        (first_in / 3600.0).floor().max(0.0) * 3600.0
    } else {
        0.0
    };

    let mut commands = Vec::new();
    let mut track_for = |timeline: &mut Timeline, kind: TrackKind| -> TrackId {
        let existing = timeline
            .tracks
            .iter()
            .chain(commands.iter().filter_map(|c| match c {
                EditCommand::AddTrack { track } => Some(track),
                _ => None,
            }))
            .find(|t| t.kind == kind)
            .map(|t| t.id);
        existing.unwrap_or_else(|| {
            let track = timeline.new_track(kind);
            let id = track.id;
            commands.push(EditCommand::AddTrack { track });
            id
        })
    };

    let mut placed: Vec<(TrackId, TrackEvent)> = Vec::new();
    for event in &doc.events {
        let Some(media) = resolve(event) else {
            report.skipped.push(format!(
                "evento {}: mídia \"{}\" não encontrada no projeto",
                event.number,
                event
                    .clip_name
                    .as_deref()
                    .or(event.source_file.as_deref())
                    .unwrap_or(&event.reel)
            ));
            continue;
        };
        let kind = match event.track {
            EdlTrack::Video => TrackKind::Video,
            EdlTrack::Audio => TrackKind::Audio,
        };
        let start = (event.record_in - offset).max(0.0);
        let mut duration = event.record_out - event.record_in;
        let trim_in = event.source_in.max(0.0);
        // Never past the end of the media
        duration = duration.min((media.duration - trim_in).max(0.0));
        if duration <= 1e-6 {
            report.skipped.push(format!(
                "evento {}: o trecho da mídia está fora da duração dela",
                event.number
            ));
            continue;
        }
        let track = track_for(timeline, kind);
        placed.push((
            track,
            TrackEvent {
                id: timeline.alloc_id(),
                name: event
                    .clip_name
                    .clone()
                    .unwrap_or_else(|| event.reel.clone()),
                media_id: Some(media.media_id),
                start,
                duration,
                trim_in,
                ..Default::default()
            },
        ));
        report.added += 1;
    }
    if placed.is_empty() {
        return None;
    }
    for (track_id, event) in placed {
        commands.push(EditCommand::AddEvent { track_id, event });
    }
    Some((EditCommand::Batch(commands), report))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;

    fn event(start: f64, duration: f64, trim_in: f64, media: u64, name: &str) -> TrackEvent {
        TrackEvent {
            id: 0,
            name: name.into(),
            media_id: Some(media),
            start,
            duration,
            trim_in,
            ..Default::default()
        }
    }

    /// A video track with two clips, 25 fps.
    fn timeline() -> (Timeline, TrackId) {
        let mut tl = Timeline::new();
        let mut track = tl.new_track(TrackKind::Video);
        let a = TrackEvent {
            id: tl.alloc_id(),
            ..event(0.0, 4.0, 2.0, 1, "Entrevista")
        };
        let b = TrackEvent {
            id: tl.alloc_id(),
            ..event(6.0, 2.4, 0.0, 2, "Cobertura")
        };
        track.events = vec![b, a];
        let id = track.id;
        tl.tracks.push(track);
        (tl, id)
    }

    fn info(media: Option<u64>) -> Option<ClipInfo> {
        match media? {
            1 => Some(ClipInfo {
                name: "Entrevista".into(),
                path: Some("C:\\media\\entrevista 01.mp4".into()),
            }),
            2 => Some(ClipInfo {
                name: "Cobertura.mov".into(),
                path: None,
            }),
            _ => None,
        }
    }

    #[test]
    fn the_list_has_a_header_one_event_per_clip_and_the_comments() {
        let (tl, id) = timeline();
        let text = export_track(&tl, id, "Meu filme", 25.0, 1, info).unwrap();
        assert_eq!(
            text,
            "TITLE: Meu filme\nFCM: NON-DROP FRAME\n\n\
             001  ENTREVIS V     C        00:00:02:00 00:00:06:00 01:00:00:00 01:00:04:00\n\
             * FROM CLIP NAME: Entrevista\n\
             * SOURCE FILE: C:\\media\\entrevista 01.mp4\n\
             002  COBERTUR V     C        00:00:00:00 00:00:02:10 01:00:06:00 01:00:08:10\n\
             * FROM CLIP NAME: Cobertura.mov\n"
        );
    }

    #[test]
    fn drop_frame_rates_get_drop_frame_timecode() {
        let (tl, id) = timeline();
        let text = export_track(&tl, id, "x", 29.97, 1, info).unwrap();
        assert!(text.contains("FCM: DROP FRAME"));
        assert!(text.contains("01:00:00;00"));
    }

    #[test]
    fn a_missing_or_empty_track_exports_nothing() {
        let (mut tl, id) = timeline();
        assert!(export_track(&tl, 9999, "x", 25.0, 1, info).is_none());
        tl.track_mut(id).unwrap().events.clear();
        assert!(export_track(&tl, id, "x", 25.0, 1, info).is_none());
    }

    #[test]
    fn what_is_written_reads_back_to_the_same_events() {
        let (tl, id) = timeline();
        let text = export_track(&tl, id, "Meu filme", 25.0, 1, info).unwrap();
        let doc = parse(&text, 25.0).unwrap();
        assert_eq!(doc.title, "Meu filme");
        assert!(!doc.drop_frame);
        assert_eq!(doc.events.len(), 2);
        let first = &doc.events[0];
        assert_eq!((first.source_in, first.source_out), (2.0, 6.0));
        assert_eq!((first.record_in, first.record_out), (3600.0, 3604.0));
        assert_eq!(first.clip_name.as_deref(), Some("Entrevista"));
        assert_eq!(
            first.source_file.as_deref(),
            Some("C:\\media\\entrevista 01.mp4")
        );
        assert_eq!(doc.events[1].clip_name.as_deref(), Some("Cobertura.mov"));
        assert!(doc.warnings.is_empty());
    }

    #[test]
    fn foreign_lists_are_read_leniently_with_warnings() {
        let text = "TITLE: SEM TITULO\nFCM: DROP FRAME\n\n\
            001  A001     V     C        01:00:00;00 01:00:05;00 01:00:00;00 01:00:05;00\n\
            * FROM CLIP NAME: plano 1\n\
            002  A002     V     D    030 01:00:10;00 01:00:12;00 01:00:05;00 01:00:07;00\n\
            M2   A002     48.0                01:00:10;00\n\
            003  A003     AA    C        00:00:00:00 00:00:03:00 01:00:07:00 01:00:10:00\n\
            004  A004     X     C        00:00:00:00 00:00:03:00 01:00:10:00 01:00:13:00\n\
            005  A005     V     C        00:00:00:00 00:00:00:00 01:00:13:00 01:00:13:00\n";
        let doc = parse(text, 29.97).unwrap();
        assert!(doc.drop_frame);
        let kinds: Vec<(u32, EdlTrack)> = doc.events.iter().map(|e| (e.number, e.track)).collect();
        assert_eq!(
            kinds,
            [
                (1, EdlTrack::Video),
                (2, EdlTrack::Video),
                (3, EdlTrack::Audio)
            ]
        );
        assert_eq!(doc.events[1].transition, Transition::Other);
        let warnings = doc.warnings.join(" | ");
        assert!(warnings.contains("transição lida como corte"), "{warnings}");
        assert!(warnings.contains("velocidade"), "{warnings}");
        assert!(warnings.contains("trilha"), "{warnings}");
        assert!(warnings.contains("duração zero"), "{warnings}");
    }

    #[test]
    fn text_that_is_not_an_edl_is_an_error() {
        assert_eq!(parse("hello\nworld", 25.0), Err(EdlError::NotAnEdl));
        assert_eq!(parse("", 25.0), Err(EdlError::NotAnEdl));
        assert_eq!(
            parse("TITLE: x\nFCM: NON-DROP FRAME\n", 25.0),
            Err(EdlError::NotAnEdl)
        );
    }

    fn doc() -> EdlDocument {
        let (tl, id) = timeline();
        let text = export_track(&tl, id, "x", 25.0, 1, info).unwrap();
        parse(&text, 25.0).unwrap()
    }

    #[test]
    fn importing_places_events_at_their_time_without_the_start_hour_and_is_one_undo_step() {
        let mut target = Timeline::new();
        let mut history = History::new();
        let resolve = |e: &EdlEvent| match e.clip_name.as_deref() {
            Some("Entrevista") => Some(MediaMatch {
                media_id: 7,
                duration: 60.0,
            }),
            Some("Cobertura.mov") => Some(MediaMatch {
                media_id: 8,
                duration: 2.0,
            }),
            _ => None,
        };
        let (command, report) = import_into_timeline(&mut target, &doc(), resolve).unwrap();
        assert_eq!(report.added, 2);
        assert!(history.execute(&mut target, command));

        assert_eq!(target.tracks.len(), 1, "a video track was created");
        let events = &target.tracks[0].events;
        assert_eq!(events.len(), 2);
        assert_eq!(
            (events[0].start, events[0].duration, events[0].trim_in),
            (0.0, 4.0, 2.0)
        );
        assert_eq!(events[0].media_id, Some(7));
        // The second clip is cut to the 2 s of its media
        assert_eq!((events[1].start, events[1].duration), (6.0, 2.0));
        assert_eq!(events[0].name, "Entrevista");

        assert!(history.undo(&mut target));
        assert!(
            target.tracks.is_empty(),
            "the track and the events go together"
        );
        assert!(history.redo(&mut target));
        assert_eq!(target.tracks[0].events.len(), 2);
    }

    #[test]
    fn events_without_media_are_skipped_and_reported_and_audio_gets_an_audio_track() {
        let mut target = Timeline::new();
        let only_first = |e: &EdlEvent| {
            (e.number == 1).then_some(MediaMatch {
                media_id: 1,
                duration: 100.0,
            })
        };
        let (_, report) = import_into_timeline(&mut target, &doc(), only_first).unwrap();
        assert_eq!(report.added, 1);
        assert_eq!(report.skipped.len(), 1);
        assert!(
            report.skipped[0].contains("Cobertura.mov"),
            "{:?}",
            report.skipped
        );
        assert!(import_into_timeline(&mut target, &doc(), |_| None).is_none());

        // An audio event goes to an audio track, reusing the one that is there
        let mut with_audio = Timeline::new();
        let track = with_audio.new_track(TrackKind::Audio);
        let audio_id = track.id;
        with_audio.tracks.push(track);
        let audio_doc = parse(
            "TITLE: a\nFCM: NON-DROP FRAME\n001  X  A  C  00:00:00:00 00:00:02:00 01:00:00:00 01:00:02:00\n",
            25.0,
        )
        .unwrap();
        let (command, _) = import_into_timeline(&mut with_audio, &audio_doc, |_| {
            Some(MediaMatch {
                media_id: 3,
                duration: 10.0,
            })
        })
        .unwrap();
        let mut history = History::new();
        assert!(history.execute(&mut with_audio, command));
        assert_eq!(with_audio.tracks.len(), 1);
        assert_eq!(with_audio.track(audio_id).unwrap().events.len(), 1);
    }

    #[test]
    fn reels_are_short_upper_case_names() {
        assert_eq!(reel_name("Entrevista final.mp4"), "ENTREVIS");
        assert_eq!(reel_name("a b"), "AB");
        assert_eq!(reel_name("???"), "AX");
    }
}
