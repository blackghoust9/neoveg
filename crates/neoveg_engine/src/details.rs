//! The edit details list: every event of the project in one table, in time order.
//!
//! The reference manuals describe an Edit Details window that lists the events with their start,
//! length and end so that a project can be read like a cut list. This is the pure part: building
//! the rows (and the tab-separated text of the table, for the clipboard), tested without a window.

use neoveg_common::timecode::seconds_to_timecode;

use crate::selection::EventRef;
use crate::timeline::{Timeline, TrackKind};

/// One line of the table.
#[derive(Debug, Clone, PartialEq)]
pub struct DetailRow {
    pub event: EventRef,
    /// Position of the track in the project, from 1.
    pub track_number: usize,
    pub track_name: String,
    pub kind: TrackKind,
    pub name: String,
    pub start: f64,
    pub duration: f64,
    pub end: f64,
    /// Where in the source the event begins.
    pub trim_in: f64,
    pub playback_rate: f64,
    /// Short tags of the switches that are on: `M` mute, `B` lock, `L` loop, `N` normalize, `F` phase.
    pub flags: String,
}

fn flags_of(e: &crate::timeline::TrackEvent) -> String {
    let mut flags = String::new();
    for (on, tag) in [
        (e.muted, 'M'),
        (e.locked, 'B'),
        (e.looping, 'L'),
        (e.normalize, 'N'),
        (e.invert_phase, 'F'),
    ] {
        if on {
            flags.push(tag);
        }
    }
    flags
}

/// Every event, ordered by start time, then by track, then by name.
pub fn rows(timeline: &Timeline) -> Vec<DetailRow> {
    let mut rows: Vec<DetailRow> = timeline
        .tracks
        .iter()
        .enumerate()
        .flat_map(|(index, track)| {
            track.events.iter().map(move |e| DetailRow {
                event: (track.id, e.id),
                track_number: index + 1,
                track_name: track.name.clone(),
                kind: track.kind,
                name: e.name.clone(),
                start: e.start,
                duration: e.duration,
                end: e.end(),
                trim_in: e.trim_in,
                playback_rate: e.playback_rate,
                flags: flags_of(e),
            })
        })
        .collect();
    rows.sort_by(|a, b| {
        a.start
            .total_cmp(&b.start)
            .then(a.track_number.cmp(&b.track_number))
            .then_with(|| a.name.cmp(&b.name))
    });
    rows
}

/// The table as tab-separated text with a header line, for pasting into a spreadsheet.
pub fn to_tsv(rows: &[DetailRow], fps: f64, drop_frame: bool) -> String {
    let tc = |t: f64| seconds_to_timecode(t, fps, drop_frame);
    let mut out = String::from(
        "N\tFaixa\tTipo\tNome\tInicio\tDuracao\tFim\tEntrada na fonte\tVelocidade\tOpcoes\n",
    );
    for (i, r) in rows.iter().enumerate() {
        let kind = match r.kind {
            TrackKind::Video => "video",
            TrackKind::Audio => "audio",
        };
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.3}\t{}\n",
            i + 1,
            r.track_number,
            kind,
            r.name.replace(['\t', '\n'], " "),
            tc(r.start),
            tc(r.duration),
            tc(r.end),
            tc(r.trim_in),
            r.playback_rate,
            r.flags
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::TrackEvent;

    fn timeline() -> Timeline {
        let mut tl = Timeline::new();
        let mut video = tl.new_track(TrackKind::Video);
        let mut audio = tl.new_track(TrackKind::Audio);
        video.name = "Vídeo 1".into();
        for (on_audio, name, start, duration) in [
            (false, "cena", 5.0, 3.0),
            (true, "voz", 5.0, 4.0),
            (true, "musica", 0.0, 10.0),
        ] {
            let id = tl.alloc_id();
            let track = if on_audio { &mut audio } else { &mut video };
            track.events.push(TrackEvent {
                id,
                name: name.into(),
                start,
                duration,
                ..TrackEvent::default()
            });
        }
        tl.tracks.extend([video, audio]);
        tl
    }

    #[test]
    fn rows_come_in_time_order_then_track_order() {
        let list = rows(&timeline());
        let names: Vec<&str> = list.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["musica", "cena", "voz"],
            "same start: the first track first"
        );
        assert_eq!((list[1].track_number, list[1].kind), (1, TrackKind::Video));
        assert_eq!(list[0].end, 10.0);
        assert!(rows(&Timeline::new()).is_empty());
    }

    #[test]
    fn the_flags_name_the_switches_that_are_on() {
        let mut tl = timeline();
        let e = &mut tl.tracks[1].events[0];
        e.muted = true;
        e.looping = true;
        e.invert_phase = true;
        let flagged = rows(&tl).into_iter().find(|r| r.name == "voz").unwrap();
        assert_eq!(flagged.flags, "MLF");
        let plain = rows(&tl).into_iter().find(|r| r.name == "cena").unwrap();
        assert_eq!(plain.flags, "");
    }

    #[test]
    fn the_tsv_has_a_header_one_line_per_event_and_timecodes() {
        let text = to_tsv(&rows(&timeline()), 25.0, false);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 4);
        assert!(lines[0].starts_with("N\tFaixa\t"));
        assert!(lines[1].contains("musica") && lines[1].contains("00:00:10:00"));
        assert_eq!(lines[1].split('\t').count(), lines[0].split('\t').count());
    }

    #[test]
    fn a_tab_or_line_break_in_a_name_cannot_break_the_table() {
        let mut tl = timeline();
        tl.tracks[0].events[0].name = "a\tb\nc".into();
        let text = to_tsv(&rows(&tl), 25.0, false);
        assert_eq!(text.lines().count(), 4);
        assert!(text.contains("a b c"));
    }
}
