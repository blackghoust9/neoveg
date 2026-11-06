//! Subtitles from a plain table: a `.csv` file (`start,end,text` or `start,text`, with a comma,
//! semicolon or tab between the columns and `"` around a text that has the separator or lines in it)
//! or a `.txt` file (one subtitle per line, each optionally starting with its time). Times are
//! seconds (`12.5`, `12,5`, `12.5s`) or clocks (`00:12.500`, `0:00:12,500`).
//!
//! Where a subtitle has no end it lasts until the next one starts, the last one [`DEFAULT_SECONDS`];
//! where it has no start it follows the one before. The result is a [`Document`] like the ones of
//! the subtitle formats, so it goes through `captions::import_document`.

use crate::captions::{decode_text, parse_clock, Cue, Document};

/// How long a subtitle lasts when nothing says (and it is the last one).
pub const DEFAULT_SECONDS: f64 = 3.0;

/// `true` for the extensions this reader takes.
pub fn is_table_extension(extension: Option<&str>) -> bool {
    matches!(
        extension.map(str::to_ascii_lowercase).as_deref(),
        Some("txt" | "csv")
    )
}

/// Reads a `.txt` or `.csv` file by its `extension`.
pub fn parse_bytes(bytes: &[u8], extension: Option<&str>) -> Document {
    let text = decode_text(bytes);
    let csv = extension.is_some_and(|e| e.eq_ignore_ascii_case("csv"));
    parse(&text, csv)
}

pub fn parse(text: &str, csv: bool) -> Document {
    let mut doc = Document::default();
    // (start, end, text) with the parts that may be missing
    let mut rows: Vec<(Option<f64>, Option<f64>, String)> = Vec::new();
    if csv {
        let delimiter = guess_delimiter(text);
        for (index, record) in records(text, delimiter).into_iter().enumerate() {
            match csv_row(&record) {
                Some(row) => rows.push(row),
                None if index == 0 => {} // a header row
                None => doc
                    .warnings
                    .push(format!("linha {} ignorada: sem tempo nem texto", index + 1)),
            }
        }
    } else {
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
            rows.push(txt_row(line));
        }
    }
    // Fill in what is missing: a start follows the one before, an end is the next start
    let mut cursor = 0.0;
    let mut starts = Vec::with_capacity(rows.len());
    for (start, end, _) in &rows {
        let s = start.unwrap_or(cursor);
        starts.push(s);
        cursor = end.unwrap_or(s + DEFAULT_SECONDS);
    }
    for (i, (_, end, text)) in rows.iter().enumerate() {
        let start = starts[i];
        let end = end.unwrap_or_else(|| {
            starts
                .get(i + 1)
                .copied()
                .filter(|next| *next > start)
                .unwrap_or(start + DEFAULT_SECONDS)
        });
        doc.cues.push(Cue {
            start,
            end,
            text: text.trim().to_string(),
            ..Cue::default()
        });
    }
    doc.cues.retain(|c| !c.text.is_empty());
    doc.normalize();
    doc
}

/// *Legendas de regiões*: one subtitle per named region of the timeline, with the region's name as
/// text and its range as time. Regions with no name or no length are left out.
pub fn from_regions(timeline: &crate::timeline::Timeline) -> Document {
    let mut doc = Document::default();
    for region in &timeline.regions {
        if region.name.trim().is_empty() || region.end <= region.start {
            continue;
        }
        doc.cues.push(Cue {
            start: region.start,
            end: region.end,
            text: region.name.trim().to_string(),
            ..Cue::default()
        });
    }
    doc.normalize();
    doc
}

/// A time as written: clock, or seconds with a point or a comma and an optional `s`.
pub fn parse_time(field: &str) -> Option<f64> {
    let f = field.trim().trim_end_matches(['s', 'S']).trim();
    if f.is_empty() {
        return None;
    }
    if f.contains(':') {
        return parse_clock(f);
    }
    let seconds: f64 = f.replace(',', ".").parse().ok()?;
    (seconds.is_finite() && seconds >= 0.0).then_some(seconds)
}

fn txt_row(line: &str) -> (Option<f64>, Option<f64>, String) {
    // "start text", "start end text" or only "text"
    let mut parts = line.splitn(3, char::is_whitespace);
    let first = parts.next().unwrap_or("");
    let Some(start) = parse_time(first) else {
        return (None, None, line.to_string());
    };
    let rest = line[first.len()..].trim_start();
    let mut tokens = rest.splitn(2, char::is_whitespace);
    let second = tokens.next().unwrap_or("");
    match (parse_time(second), tokens.next()) {
        (Some(end), Some(text)) if end > start => (Some(start), Some(end), text.trim().to_string()),
        _ => (Some(start), None, rest.to_string()),
    }
}

fn csv_row(record: &[String]) -> Option<(Option<f64>, Option<f64>, String)> {
    let start = parse_time(record.first()?)?;
    match record.len() {
        0 | 1 => None,
        2 => Some((Some(start), None, record[1].clone())),
        _ => match parse_time(&record[1]) {
            Some(end) if end > start => Some((Some(start), Some(end), record[2].clone())),
            Some(_) => Some((Some(start), None, record[2].clone())),
            // "start,text,more text": the second column is already the text
            None => Some((Some(start), None, record[1..].join(" "))),
        },
    }
}

/// The separator that shows up most in the first line, outside quotes.
fn guess_delimiter(text: &str) -> char {
    let first = text.lines().next().unwrap_or("");
    let mut best = (',', 0usize);
    for d in [',', ';', '\t'] {
        let mut quoted = false;
        let mut n = 0;
        for c in first.chars() {
            if c == '"' {
                quoted = !quoted;
            } else if c == d && !quoted {
                n += 1;
            }
        }
        if n > best.1 {
            best = (d, n);
        }
    }
    best.0
}

/// Splits the text into records of fields; `"` quotes a field (`""` is a quote inside it).
fn records(text: &str, delimiter: char) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            c if c == delimiter && !quoted => row.push(std::mem::take(&mut field)),
            '\n' if !quoted => {
                row.push(std::mem::take(&mut field));
                if row.iter().any(|f| !f.trim().is_empty()) {
                    out.push(std::mem::take(&mut row));
                } else {
                    row.clear();
                }
            }
            '\r' if !quoted => {}
            c => field.push(c),
        }
    }
    row.push(field);
    if row.iter().any(|f| !f.trim().is_empty()) {
        out.push(row);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_csv_with_a_header_clock_times_and_quoted_text() {
        let csv = "inicio,fim,texto\n0:00:01.5,0:00:03,\"Olá, mundo\"\n";
        // The first line decides the separator: a comma
        let doc = parse(csv, true);
        assert_eq!(doc.cues.len(), 1, "{:?}", doc);
        assert_eq!(doc.cues[0].text, "Olá, mundo");
        assert_eq!((doc.cues[0].start, doc.cues[0].end), (1.5, 3.0));
        let csv = "start;end;text\n1,5;3;Primeira\n3;5.5;\"Segunda\nlinha\"\n";
        let doc = parse(csv, true);
        assert_eq!(doc.cues.len(), 2);
        assert_eq!((doc.cues[0].start, doc.cues[0].end), (1.5, 3.0));
        assert_eq!(doc.cues[1].text, "Segunda\nlinha");
        assert_eq!(doc.cues[1].end, 5.5);
    }

    #[test]
    fn two_columns_last_cue_and_overlap() {
        let doc = parse("0,um\n2.5,dois\n10,tres\n", true);
        let times: Vec<(f64, f64)> = doc.cues.iter().map(|c| (c.start, c.end)).collect();
        assert_eq!(times, vec![(0.0, 2.5), (2.5, 10.0), (10.0, 13.0)]);
    }

    #[test]
    fn a_txt_takes_a_line_per_subtitle_with_or_without_time() {
        let doc = parse("Bom dia\n\nBoa tarde\n", false);
        let times: Vec<(f64, f64)> = doc.cues.iter().map(|c| (c.start, c.end)).collect();
        assert_eq!(times, vec![(0.0, 3.0), (3.0, 6.0)]);
        let doc = parse("1.0 um\n4.0 10.0 dois e meio\n00:12.000 tres\n", false);
        assert_eq!(doc.cues.len(), 3);
        assert_eq!((doc.cues[0].start, doc.cues[0].end), (1.0, 4.0));
        assert_eq!(doc.cues[1].text, "dois e meio");
        assert_eq!(doc.cues[1].end, 10.0);
        assert_eq!(doc.cues[2].start, 12.0);
    }

    #[test]
    fn the_table_goes_onto_the_timeline_as_captions_in_one_undo_step() {
        use crate::command::History;
        let mut tl = crate::timeline::Timeline::new();
        let doc = parse_bytes(b"0 ola\n2 mundo\n", Some("TXT"));
        let command = crate::captions::import_document(&mut tl, &doc, 0.0, false).unwrap();
        let mut history = History::new();
        assert!(history.execute(&mut tl, command));
        assert_eq!(tl.captions.len(), 2);
        assert!(history.undo(&mut tl));
        assert!(tl.captions.is_empty());
    }

    #[test]
    fn named_regions_become_subtitles() {
        let tl2 = {
            let mut t = crate::timeline::Timeline::new();
            t.regions.push(crate::markers::Region {
                id: 1,
                start: 1.0,
                end: 2.0,
                name: "Cena 1".into(),
            });
            t.regions.push(crate::markers::Region {
                id: 2,
                start: 3.0,
                end: 4.0,
                name: "  ".into(),
            });
            t
        };
        let doc = from_regions(&tl2);
        assert_eq!(doc.cues.len(), 1);
        assert_eq!((doc.cues[0].text.as_str(), doc.cues[0].start, doc.cues[0].end), ("Cena 1", 1.0, 2.0));
    }

    #[test]
    fn extensions_and_times() {
        assert!(is_table_extension(Some("CSV")) && is_table_extension(Some("txt")));
        assert!(!is_table_extension(Some("srt")) && !is_table_extension(None));
        assert_eq!(parse_time("12,5s"), Some(12.5));
        assert_eq!(parse_time("01:02.5"), Some(62.5));
        assert_eq!(parse_time("-3"), None);
        assert_eq!(parse_time("abc"), None);
    }
}
