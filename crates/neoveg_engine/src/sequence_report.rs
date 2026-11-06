//! The sequence report (CSV): the media the project uses, with its origin, first in point and last
//! out point, the time used, how many events use it and whether the file is missing. The edit
//! details window already shows the events; this one lists the media.

use neoveg_common::project::MediaRef;

use crate::edit::clamp_playback_rate;
use crate::timeline::Timeline;

/// One media file of the report.
#[derive(Debug, Clone, PartialEq)]
pub struct ReportRow {
    pub media_id: u64,
    pub name: String,
    /// Where the file is (its path).
    pub origin: String,
    /// The earliest point of the source that an event plays.
    pub first_in: f64,
    /// The latest point of the source that an event plays.
    pub last_out: f64,
    /// Sum of the source time that the events play.
    pub used: f64,
    pub events: usize,
    pub missing: bool,
}

/// One row per media of the pool, in the order of the pool. Media no event uses has zero events;
/// `exists` says whether a path is a file that can be read.
pub fn rows(
    timeline: &Timeline,
    pool: &[MediaRef],
    exists: &dyn Fn(&str) -> bool,
) -> Vec<ReportRow> {
    pool.iter()
        .map(|m| {
            let mut row = ReportRow {
                media_id: m.id,
                name: std::path::Path::new(&m.path)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| m.path.clone()),
                origin: m.path.clone(),
                first_in: 0.0,
                last_out: 0.0,
                used: 0.0,
                events: 0,
                missing: !exists(&m.path),
            };
            let mut first = f64::INFINITY;
            for e in timeline
                .tracks
                .iter()
                .flat_map(|t| &t.events)
                .filter(|e| e.media_id == Some(m.id))
            {
                let len = e.duration * clamp_playback_rate(e.playback_rate);
                first = first.min(e.trim_in);
                row.last_out = row.last_out.max(e.trim_in + len);
                row.used += len;
                row.events += 1;
            }
            if row.events > 0 {
                row.first_in = first;
            }
            row
        })
        .collect()
}

fn field(text: &str) -> String {
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_owned()
    }
}

/// The report as CSV text with a header line. Times are timecodes at `fps`.
pub fn to_csv(rows: &[ReportRow], fps: f64, drop_frame: bool) -> String {
    let tc = |t: f64| neoveg_common::timecode::seconds_to_timecode(t, fps, drop_frame);
    let mut out = String::from("Midia,Origem,Entrada,Saida,Duracao usada,Eventos,Situacao\n");
    for r in rows {
        out.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            field(&r.name),
            field(&r.origin),
            tc(r.first_in),
            tc(r.last_out),
            tc(r.used),
            r.events,
            if r.missing { "ausente" } else { "ok" }
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::{TrackEvent, TrackKind};

    fn media(id: u64, path: &str) -> MediaRef {
        MediaRef {
            id,
            path: path.into(),
            duration_seconds: 60.0,
            subproject: false,
            format: None,
        }
    }

    fn timeline() -> Timeline {
        let mut tl = Timeline::new();
        let mut v = tl.new_track(TrackKind::Video);
        for (start, trim, dur, id) in [
            (0.0, 5.0, 2.0, 1u64),
            (3.0, 1.0, 4.0, 1),
            (9.0, 0.0, 1.0, 2),
        ] {
            let eid = tl.alloc_id();
            v.events.push(TrackEvent {
                id: eid,
                media_id: Some(id),
                start,
                duration: dur,
                trim_in: trim,
                ..TrackEvent::default()
            });
        }
        tl.tracks.push(v);
        tl
    }

    #[test]
    fn each_media_has_its_in_out_use_and_event_count() {
        let pool = [
            media(1, "C:/m/a.mp4"),
            media(2, "C:/m/b.mp4"),
            media(3, "C:/m/c.mp4"),
        ];
        let rows = rows(&timeline(), &pool, &|p| !p.ends_with("b.mp4"));
        assert_eq!(rows[0].name, "a.mp4");
        assert_eq!((rows[0].first_in, rows[0].last_out), (1.0, 7.0));
        assert_eq!(
            (rows[0].used, rows[0].events, rows[0].missing),
            (6.0, 2, false)
        );
        assert!(rows[1].missing);
        assert_eq!(rows[2].events, 0);
    }

    #[test]
    fn the_csv_has_a_header_one_line_per_media_and_quotes_what_needs_it() {
        let pool = [media(1, "C:/m/a,x.mp4"), media(2, "C:/m/b.mp4")];
        let r = rows(&timeline(), &pool, &|p| !p.ends_with("b.mp4"));
        let csv = to_csv(&r, 25.0, false);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].starts_with("Midia,Origem"));
        assert!(lines[1].starts_with("\"a,x.mp4\",\"C:/m/a,x.mp4\",00:00:01:00"));
        assert!(lines[1].ends_with(",2,ok"));
        assert!(lines[2].ends_with(",1,ausente"));
    }
}
