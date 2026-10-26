// The SubRip and WebVTT reading and writing below is adapted from `filmcraft-captions`
// (`src/srt.rs`, `src/vtt.rs` and the clock helpers of `src/lib.rs`) of
// https://github.com/storytold/filmcraft (commit 8fda2f2).
// Copyright (c) 2026 ArtCraft Team and the FilmCraft contributors. MIT OR Apache-2.0; Neoveg uses
// it under the MIT terms, see third_party/filmcraft/LICENSE-MIT.
// Changes: times are `f64` seconds instead of ticks; `let` chains rewritten for edition 2021;
// the project model (caption tracks, ticks) is replaced by Neoveg's `Caption` and the timeline
// commands below, which are Neoveg's own.

//! Subtitles: SubRip (`.srt`) and WebVTT (`.vtt`) files, and the captions of a project.
//!
//! The parsers are forgiving, as real files are: missing or wrong indexes, `.` instead of `,`,
//! 1 to 3 digit fractions, missing hours, trailing SSA coordinates, cues not separated by a blank
//! line, a byte order mark, UTF-16 and Windows-1252 text. The writers are strict. The functions on
//! the timeline are pure and return one [`EditCommand`] each, so an import or an edit is one undo
//! step.

use serde::{Deserialize, Serialize};

use crate::command::EditCommand;
use crate::selection::TimeRange;
use crate::timeline::{Timeline, TIME_EPSILON};

// ---- files --------------------------------------------------------------------------------------

/// A subtitle file format Neoveg reads and writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Srt,
    WebVtt,
}

impl Format {
    pub const ALL: [Format; 2] = [Format::Srt, Format::WebVtt];

    pub fn extension(self) -> &'static str {
        match self {
            Format::Srt => "srt",
            Format::WebVtt => "vtt",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Format::Srt => "SubRip (SRT)",
            Format::WebVtt => "WebVTT",
        }
    }

    pub fn from_extension(ext: &str) -> Option<Format> {
        match ext.trim_start_matches('.').to_ascii_lowercase().as_str() {
            "srt" | "subrip" => Some(Format::Srt),
            "vtt" | "webvtt" => Some(Format::WebVtt),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    NotFormat(&'static str),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotFormat(name) => write!(f, "o arquivo não é {name}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// One cue of a subtitle file. Times are seconds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cue {
    pub start: f64,
    pub end: f64,
    /// Text as written (lines separated by `\n`, inline tags kept).
    pub text: String,
    /// WebVTT voice (`<v Name>`).
    pub speaker: Option<String>,
    /// WebVTT cue identifier.
    pub id: Option<String>,
    /// WebVTT cue settings, verbatim.
    pub settings: String,
}

/// A parsed subtitle file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Document {
    pub cues: Vec<Cue>,
    /// WebVTT STYLE / REGION / NOTE blocks (verbatim, without trailing blank lines).
    pub blocks: Vec<String>,
    /// Non-fatal problems found while reading (skipped cues...).
    pub warnings: Vec<String>,
}

impl Document {
    /// Sorts by start, drops empty cues and cuts overlaps (a cue ends where the next begins).
    pub fn normalize(&mut self) {
        self.cues.retain(|c| c.end > c.start);
        self.cues
            .sort_by(|a, b| a.start.total_cmp(&b.start).then(a.end.total_cmp(&b.end)));
        let mut out: Vec<Cue> = Vec::with_capacity(self.cues.len());
        for mut c in std::mem::take(&mut self.cues) {
            if let Some(prev) = out.last_mut() {
                if c.start < prev.end {
                    if c.start > prev.start {
                        prev.end = c.start;
                    } else {
                        // Same start: keep the first, move this one after it
                        c.start = prev.end;
                        if c.end <= c.start {
                            self.warnings.push(format!(
                                "legenda sobreposta descartada: \"{}\"",
                                c.text.lines().next().unwrap_or("")
                            ));
                            continue;
                        }
                    }
                }
            }
            out.push(c);
        }
        self.cues = out;
    }
}

/// Text of a file as a string: UTF-8 (with or without BOM), UTF-16 with BOM, or Windows-1252;
/// line endings become `\n`.
pub fn decode_text(bytes: &[u8]) -> String {
    let s = if let Some(rest) = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]) {
        String::from_utf8_lossy(rest).into_owned()
    } else if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        let little = bytes[0] == 0xff;
        let units: Vec<u16> = bytes[2..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| {
                if little {
                    u16::from_le_bytes([c[0], c[1]])
                } else {
                    u16::from_be_bytes([c[0], c[1]])
                }
            })
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        match std::str::from_utf8(bytes) {
            Ok(s) => s.to_string(),
            Err(_) => bytes.iter().map(|&b| cp1252(b)).collect(),
        }
    };
    let s = s.strip_prefix('\u{feff}').map(str::to_string).unwrap_or(s);
    if s.contains('\r') {
        s.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        s
    }
}

fn cp1252(b: u8) -> char {
    const HI: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž',
        '\u{8f}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}',
        'ž', 'Ÿ',
    ];
    if (0x80..0xa0).contains(&b) {
        HI[(b - 0x80) as usize]
    } else {
        b as char
    }
}

/// The format a file's text looks like, using the extension when it is known.
pub fn detect(text: &str, extension: Option<&str>) -> Option<Format> {
    let head = text.trim_start_matches('\u{feff}').trim_start();
    if head.starts_with("WEBVTT") {
        return Some(Format::WebVtt);
    }
    let timing = head
        .lines()
        .take(12)
        .any(|l| parse_timing_line(l).is_some());
    match (timing, extension.and_then(Format::from_extension)) {
        (true, Some(f)) => Some(f),
        (true, None) => Some(Format::Srt),
        _ => None,
    }
}

/// Parses `h:mm:ss,fff`, `hh:mm:ss.fff` or `mm:ss.fff` into seconds.
pub fn parse_clock(s: &str) -> Option<f64> {
    let s = s.trim();
    let (hms, frac) = match s.find([',', '.']) {
        Some(i) => (&s[..i], &s[i + 1..]),
        None => (s, ""),
    };
    let parts: Vec<&str> = hms.split(':').collect();
    let num = |p: &str| -> Option<u32> {
        (!p.is_empty() && p.len() <= 9 && p.bytes().all(|b| b.is_ascii_digit()))
            .then(|| p.parse().ok())?
    };
    let (h, m, sec) = match parts.as_slice() {
        [h, m, s] => (num(h)?, num(m)?, num(s)?),
        [m, s] => (0, num(m)?, num(s)?),
        _ => return None,
    };
    if m >= 60 || sec >= 60 || h > 9999 {
        return None;
    }
    let mut seconds = f64::from(h * 3600 + m * 60 + sec);
    if !frac.is_empty() {
        if frac.len() > 9 || !frac.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let digits: f64 = frac.parse().ok()?;
        seconds += digits / 10f64.powi(frac.len() as i32);
    }
    Some(seconds)
}

/// `hh:mm:ss<sep>fff`, rounded to milliseconds.
pub fn format_clock(seconds: f64, sep: char) -> String {
    let ms = (seconds.max(0.0) * 1000.0).round() as u64;
    let (h, m, s, f) = (
        ms / 3_600_000,
        (ms / 60_000) % 60,
        (ms / 1000) % 60,
        ms % 1000,
    );
    format!("{h:02}:{m:02}:{s:02}{sep}{f:03}")
}

/// Parses `start --> end[ settings]` (SRT or VTT timing line). Returns (start, end, settings).
pub fn parse_timing_line(line: &str) -> Option<(f64, f64, &str)> {
    let (a, rest) = line.split_once("-->")?;
    let start = parse_clock(a.trim())?;
    let rest = rest.trim_start();
    let end_len = rest.find(char::is_whitespace).unwrap_or(rest.len());
    let end = parse_clock(&rest[..end_len])?;
    Some((start, end, rest[end_len..].trim()))
}

fn is_index(line: &str) -> bool {
    let l = line.trim();
    !l.is_empty() && l.len() <= 10 && l.bytes().all(|b| b.is_ascii_digit())
}

/// Reads SubRip text.
pub fn parse_srt(text: &str) -> Result<Document> {
    let lines: Vec<&str> = text.lines().collect();
    let mut doc = Document::default();
    let mut i = 0;
    let mut found_any = false;
    while i < lines.len() {
        let Some((start, end, _settings)) = parse_timing_line(lines[i]) else {
            let l = lines[i].trim();
            if !l.is_empty() && !is_index(l) && found_any {
                doc.warnings
                    .push(format!("linha {}: texto solto ignorado \"{l}\"", i + 1));
            }
            i += 1;
            continue;
        };
        found_any = true;
        i += 1;
        let mut text_lines: Vec<&str> = Vec::new();
        while i < lines.len() {
            let l = lines[i];
            if l.trim().is_empty() || parse_timing_line(l).is_some() {
                break;
            }
            if is_index(l)
                && lines
                    .get(i + 1)
                    .is_some_and(|n| parse_timing_line(n).is_some())
            {
                break;
            }
            text_lines.push(l.trim_end());
            i += 1;
        }
        if end <= start {
            doc.warnings.push(format!(
                "legenda em {} termina antes de começar; ignorada",
                format_clock(start, ',')
            ));
            continue;
        }
        doc.cues.push(Cue {
            start,
            end,
            text: text_lines.join("\n"),
            ..Default::default()
        });
    }
    if !found_any && !text.trim().is_empty() {
        return Err(Error::NotFormat("SubRip"));
    }
    Ok(doc)
}

/// Writes SubRip text, numbered from 1.
pub fn write_srt(doc: &Document) -> String {
    let mut out = String::new();
    for (n, c) in doc.cues.iter().enumerate() {
        out.push_str(&format!(
            "{}\n{} --> {}\n",
            n + 1,
            format_clock(c.start, ','),
            format_clock(c.end, ',')
        ));
        // A blank line inside the text would end the cue: drop them
        for l in c.text.lines().filter(|l| !l.trim().is_empty()) {
            out.push_str(l);
            out.push('\n');
        }
        out.push('\n');
    }
    out
}

/// Splits a leading `<v Name>` / `<v.class Name>` voice span off cue text.
pub fn split_voice(text: &str) -> (Option<String>, String) {
    let t = text.trim_start();
    if let Some(rest) = t.strip_prefix("<v") {
        if rest.starts_with(' ') || rest.starts_with('.') || rest.starts_with('\t') {
            if let Some(close) = rest.find('>') {
                let inner = &rest[..close];
                // Skip classes: `.loud.x Name`
                let name = match inner.find([' ', '\t']) {
                    Some(sp) => inner[sp..].trim(),
                    None => "",
                };
                let body = &rest[close + 1..];
                // A closing </v> at the very end goes too; one in the middle means several
                // voices: keep everything
                let body = match body.find("</v>") {
                    Some(p) if p + 4 == body.trim_end().len() => body[..p].to_string(),
                    Some(_) => return (None, text.to_string()),
                    None => body.to_string(),
                };
                if body.contains("<v ") || body.contains("<v.") || body.contains("<v\t") {
                    return (None, text.to_string());
                }
                return ((!name.is_empty()).then(|| name.to_string()), body);
            }
        }
    }
    (None, text.to_string())
}

/// Reads WebVTT text. A missing `WEBVTT` signature is tolerated.
pub fn parse_vtt(text: &str) -> Result<Document> {
    let mut doc = Document::default();
    let mut lines = text.lines().peekable();
    let has_sig = lines.peek().is_some_and(|l| {
        let l = l.trim_start_matches('\u{feff}');
        l == "WEBVTT" || l.starts_with("WEBVTT ") || l.starts_with("WEBVTT\t")
    });
    if has_sig {
        for l in lines.by_ref() {
            if l.trim().is_empty() {
                break;
            }
        }
    }
    let mut block: Vec<&str> = Vec::new();
    let mut blocks: Vec<Vec<&str>> = Vec::new();
    for l in lines {
        if l.trim().is_empty() {
            if !block.is_empty() {
                blocks.push(std::mem::take(&mut block));
            }
        } else {
            block.push(l);
        }
    }
    if !block.is_empty() {
        blocks.push(block);
    }
    let mut seen_cue = false;
    for b in blocks {
        let first = b[0].trim();
        let has_timing = b.iter().any(|l| l.contains("-->"));
        if !has_timing
            && (first == "NOTE" || first.starts_with("NOTE ") || first.starts_with("NOTE\t"))
        {
            doc.blocks.push(b.join("\n"));
            continue;
        }
        if !seen_cue && !has_timing && (first == "STYLE" || first == "REGION") {
            doc.blocks.push(b.join("\n"));
            continue;
        }
        let (id, timing_idx) = if b[0].contains("-->") {
            (None, 0)
        } else {
            (Some(b[0].trim().to_string()), 1)
        };
        let Some(timing) = b.get(timing_idx) else {
            doc.warnings.push(format!("bloco ignorado \"{first}\""));
            continue;
        };
        let Some((start, end, settings)) = parse_timing_line(timing) else {
            doc.warnings
                .push(format!("linha de tempo inválida \"{}\"", timing.trim()));
            continue;
        };
        seen_cue = true;
        if end <= start {
            doc.warnings.push(format!(
                "legenda em {} termina antes de começar; ignorada",
                format_clock(start, '.')
            ));
            continue;
        }
        let raw = b[timing_idx + 1..]
            .iter()
            .map(|l| l.trim_end())
            .collect::<Vec<_>>()
            .join("\n");
        let (speaker, text) = split_voice(&raw);
        doc.cues.push(Cue {
            start,
            end,
            text,
            speaker,
            id,
            settings: settings.to_string(),
        });
    }
    if !has_sig && doc.cues.is_empty() && !text.trim().is_empty() {
        return Err(Error::NotFormat("WebVTT"));
    }
    Ok(doc)
}

/// Writes WebVTT text.
pub fn write_vtt(doc: &Document) -> String {
    let mut out = String::from("WEBVTT\n\n");
    for b in &doc.blocks {
        let b = b.trim_end();
        if !b.is_empty() {
            out.push_str(b);
            out.push_str("\n\n");
        }
    }
    for c in &doc.cues {
        if let Some(id) =
            c.id.as_deref()
                .map(str::trim)
                .filter(|i| !i.is_empty() && !i.contains("-->"))
        {
            out.push_str(id);
            out.push('\n');
        }
        out.push_str(&format!(
            "{} --> {}",
            format_clock(c.start, '.'),
            format_clock(c.end, '.')
        ));
        if !c.settings.trim().is_empty() {
            out.push(' ');
            out.push_str(c.settings.trim());
        }
        out.push('\n');
        let mut text = String::new();
        if let Some(s) = c.speaker.as_deref().filter(|s| !s.is_empty()) {
            text.push_str(&format!("<v {s}>"));
        }
        text.push_str(&c.text);
        for l in text.lines().filter(|l| !l.trim().is_empty()) {
            out.push_str(&l.replace("-->", "--&gt;"));
            out.push('\n');
        }
        out.push('\n');
    }
    out
}

/// Parses a file's bytes in `format` (the format is detected from the content when `None`).
pub fn parse_bytes(
    bytes: &[u8],
    format: Option<Format>,
    extension: Option<&str>,
) -> Result<(Format, Document)> {
    let text = decode_text(bytes);
    let format = format
        .or_else(|| detect(&text, extension))
        .ok_or(Error::NotFormat("SRT ou WebVTT"))?;
    let doc = match format {
        Format::Srt => parse_srt(&text)?,
        Format::WebVtt => parse_vtt(&text)?,
    };
    Ok((format, doc))
}

pub fn write(doc: &Document, format: Format) -> String {
    match format {
        Format::Srt => write_srt(doc),
        Format::WebVtt => write_vtt(doc),
    }
}

// ---- the captions of a project --------------------------------------------------------------------

pub type CaptionId = u64;

/// A subtitle on the timeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Caption {
    pub id: CaptionId,
    pub start: f64,
    pub end: f64,
    pub text: String,
    /// Who speaks (WebVTT voice), kept so a file round-trips.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker: Option<String>,
}

impl Caption {
    pub fn range(&self) -> TimeRange {
        TimeRange::new(self.start, self.end)
    }
}

fn sorted(mut list: Vec<Caption>) -> Vec<Caption> {
    list.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.id.cmp(&b.id)));
    list
}

fn replace(timeline: &Timeline, after: Vec<Caption>) -> Option<EditCommand> {
    let before = timeline.captions.clone();
    let after = sorted(after);
    (before != after).then_some(EditCommand::ReplaceCaptions { before, after })
}

/// The captions showing at `time` (a caption shows from its start up to, not including, its end).
pub fn active_at(timeline: &Timeline, time: f64) -> Vec<&Caption> {
    timeline
        .captions
        .iter()
        .filter(|c| time >= c.start - TIME_EPSILON && time < c.end - TIME_EPSILON)
        .collect()
}

/// Adds the cues of `doc` to the timeline (replacing the existing captions when `replace_all`),
/// shifted by `offset` seconds. `None` when the file has no usable cue.
pub fn import_document(
    timeline: &mut Timeline,
    doc: &Document,
    offset: f64,
    replace_all: bool,
) -> Option<EditCommand> {
    let mut after = if replace_all {
        Vec::new()
    } else {
        timeline.captions.clone()
    };
    let mut added = 0;
    for cue in &doc.cues {
        let (start, end) = (cue.start + offset, cue.end + offset);
        if end <= start || end <= 0.0 {
            continue;
        }
        after.push(Caption {
            id: timeline.alloc_id(),
            start: start.max(0.0),
            end,
            text: cue.text.clone(),
            speaker: cue.speaker.clone(),
        });
        added += 1;
    }
    if added == 0 && !replace_all {
        return None;
    }
    replace(timeline, after)
}

/// The captions as a file document, cut so none overlaps the next.
pub fn export_document(timeline: &Timeline) -> Document {
    let mut doc = Document {
        cues: timeline
            .captions
            .iter()
            .map(|c| Cue {
                start: c.start,
                end: c.end,
                text: c.text.clone(),
                speaker: c.speaker.clone(),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };
    doc.normalize();
    doc
}

/// A new caption from `start` for `length` seconds. `None` for an empty span.
pub fn add_caption(
    timeline: &mut Timeline,
    start: f64,
    length: f64,
    text: &str,
) -> Option<EditCommand> {
    if length <= TIME_EPSILON {
        return None;
    }
    let mut after = timeline.captions.clone();
    after.push(Caption {
        id: timeline.alloc_id(),
        start: start.max(0.0),
        end: start.max(0.0) + length,
        text: text.to_owned(),
        speaker: None,
    });
    replace(timeline, after)
}

fn edit_caption(
    timeline: &Timeline,
    id: CaptionId,
    change: impl FnOnce(&mut Caption) -> bool,
) -> Option<EditCommand> {
    let mut after = timeline.captions.clone();
    let caption = after.iter_mut().find(|c| c.id == id)?;
    change(caption).then(|| replace(timeline, after))?
}

pub fn set_caption_text(timeline: &Timeline, id: CaptionId, text: &str) -> Option<EditCommand> {
    edit_caption(timeline, id, |c| {
        c.text = text.to_owned();
        true
    })
}

/// Moves a caption's start and end. `None` for an unknown id or a span shorter than a millisecond.
pub fn set_caption_times(
    timeline: &Timeline,
    id: CaptionId,
    start: f64,
    end: f64,
) -> Option<EditCommand> {
    if end - start < 0.001 {
        return None;
    }
    edit_caption(timeline, id, |c| {
        c.start = start.max(0.0);
        c.end = end;
        true
    })
}

pub fn remove_caption(timeline: &Timeline, id: CaptionId) -> Option<EditCommand> {
    let mut after = timeline.captions.clone();
    let before_len = after.len();
    after.retain(|c| c.id != id);
    (after.len() != before_len)
        .then(|| replace(timeline, after))
        .flatten()
}

/// Removes every caption.
pub fn clear_captions(timeline: &Timeline) -> Option<EditCommand> {
    replace(timeline, Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;

    fn ms(v: f64) -> f64 {
        v / 1000.0
    }

    fn millis(seconds: f64) -> i64 {
        (seconds * 1000.0).round() as i64
    }

    #[test]
    fn srt_basic_round_trip() {
        let src = "1\n00:00:01,000 --> 00:00:02,500\nHello\n<i>World</i>\n\n2\n00:00:03,000 --> 00:00:04,000\nBye\n";
        let d = parse_srt(src).unwrap();
        assert_eq!(d.cues.len(), 2);
        assert_eq!(d.cues[0].start, 1.0);
        assert_eq!(d.cues[0].end, 2.5);
        assert_eq!(d.cues[0].text, "Hello\n<i>World</i>");
        assert_eq!(
            write_srt(&d),
            "1\n00:00:01,000 --> 00:00:02,500\nHello\n<i>World</i>\n\n2\n00:00:03,000 --> 00:00:04,000\nBye\n\n"
        );
    }

    #[test]
    fn srt_sloppy_files_are_accepted() {
        // No indexes, period fractions, short fractions, no hours, SSA coordinates, no blank line
        let src = "00:00:01.5 --> 00:00:02.25 X1:10 X2:20 Y1:1 Y2:2\nA\n7\n00:03,000 --> 00:04,000\nB\n00:00:05,000-->00:00:06,000\n42\n\n\n";
        let d = parse_srt(src).unwrap();
        let got: Vec<(i64, i64, &str)> = d
            .cues
            .iter()
            .map(|c| (millis(c.start), millis(c.end), c.text.as_str()))
            .collect();
        assert_eq!(
            got,
            vec![(1500, 2250, "A"), (3000, 4000, "B"), (5000, 6000, "42")]
        );
    }

    #[test]
    fn srt_rejects_garbage_and_skips_inverted_cues() {
        assert!(parse_srt("hello world\nthis is not srt").is_err());
        assert!(parse_srt("").unwrap().cues.is_empty());
        let d = parse_srt(
            "1\n00:00:05,000 --> 00:00:04,000\nX\n\n2\n00:00:06,000 --> 00:00:07,000\nY\n",
        )
        .unwrap();
        assert_eq!(d.cues.len(), 1);
        assert_eq!(d.warnings.len(), 1);
    }

    const VTT: &str = "WEBVTT - with header\nKind: captions\n\nSTYLE\n::cue { color: yellow }\n\nNOTE a comment\nspanning lines\n\nintro\n00:01.000 --> 00:00:03.500 line:90% align:start\n<v.loud Ann Smith>Hello there\nsecond line\n\n00:00:04.000 --> 00:00:05.000\n<i>plain</i> &amp; tags\n";

    #[test]
    fn vtt_keeps_blocks_ids_settings_and_voices() {
        let d = parse_vtt(VTT).unwrap();
        assert_eq!(
            d.blocks,
            vec![
                "STYLE\n::cue { color: yellow }".to_string(),
                "NOTE a comment\nspanning lines".to_string()
            ]
        );
        assert_eq!(d.cues.len(), 2);
        let c = &d.cues[0];
        assert_eq!(c.id.as_deref(), Some("intro"));
        assert_eq!((c.start, c.end), (1.0, 3.5));
        assert_eq!(c.settings, "line:90% align:start");
        assert_eq!(c.speaker.as_deref(), Some("Ann Smith"));
        assert_eq!(c.text, "Hello there\nsecond line");
        let w = write_vtt(&d);
        assert!(w.starts_with("WEBVTT\n\nSTYLE\n::cue { color: yellow }\n\nNOTE a comment\nspanning lines\n\nintro\n00:00:01.000 --> 00:00:03.500 line:90% align:start\n<v Ann Smith>Hello there\n"));
        assert_eq!(parse_vtt(&w).unwrap().cues, d.cues);
    }

    #[test]
    fn vtt_voices_and_edge_cases() {
        assert_eq!(
            split_voice("<v Bob>hi</v>"),
            (Some("Bob".into()), "hi".into())
        );
        assert_eq!(
            split_voice("<v Bob>hi</v> <v Ann>yo</v>"),
            (None, "<v Bob>hi</v> <v Ann>yo</v>".into())
        );
        assert_eq!(split_voice("<vx>hi"), (None, "<vx>hi".into()));
        // Without the signature, and a NOTE-prefixed id is a cue
        assert_eq!(
            parse_vtt("00:00:01.000 --> 00:00:02.000\nx\n")
                .unwrap()
                .cues
                .len(),
            1
        );
        assert!(parse_vtt("just text").is_err());
        let d = parse_vtt("WEBVTT\n\nNOTE 1\n00:00:01.000 --> 00:00:02.000\nhello\n").unwrap();
        assert_eq!(d.cues[0].id.as_deref(), Some("NOTE 1"));
        assert!(d.blocks.is_empty());
    }

    #[test]
    fn clocks_parse_and_format() {
        assert_eq!(parse_clock("01:02:03,456"), Some(3723.456));
        assert_eq!(parse_clock("02:03.5"), Some(123.5));
        assert_eq!(parse_clock("00:60:00,000"), None);
        assert_eq!(parse_clock("1:2"), Some(62.0));
        assert_eq!(parse_clock("x"), None);
        assert_eq!(format_clock(3723.456, ','), "01:02:03,456");
        assert_eq!(format_clock(-1.0, '.'), "00:00:00.000");
        assert_eq!(format_clock(ms(1999.6), '.'), "00:00:02.000");
    }

    #[test]
    fn text_is_decoded_from_utf8_utf16_and_windows_1252() {
        assert_eq!(decode_text(b"\xef\xbb\xbfa\r\nb\rc"), "a\nb\nc");
        assert_eq!(decode_text(&[0xff, 0xfe, b'h', 0, b'i', 0]), "hi");
        assert_eq!(decode_text(b"caf\xe9 \x93x\x94"), "café “x”");
    }

    #[test]
    fn the_format_is_detected_from_the_content_and_the_extension() {
        assert_eq!(
            detect("WEBVTT\n\n00:01.000 --> 00:02.000\nhi\n", Some("vtt")),
            Some(Format::WebVtt)
        );
        assert_eq!(
            detect("1\n00:00:01,000 --> 00:00:02,000\nhi\n", Some("srt")),
            Some(Format::Srt)
        );
        assert_eq!(detect("hello", Some("srt")), None);
        assert_eq!(Format::from_extension(".VTT"), Some(Format::WebVtt));
        let (format, doc) = parse_bytes(
            b"\xef\xbb\xbf1\r\n00:00:01,000 --> 00:00:02,000\r\nhi\r\n",
            None,
            None,
        )
        .unwrap();
        assert_eq!((format, doc.cues.len()), (Format::Srt, 1));
        assert!(parse_bytes(b"nothing here", None, None).is_err());
    }

    #[test]
    fn normalize_cuts_overlaps_and_drops_empty_cues() {
        let c = |s: f64, e: f64| Cue {
            start: s,
            end: e,
            text: "x".into(),
            ..Default::default()
        };
        let mut d = Document {
            cues: vec![c(10.0, 20.0), c(0.0, 15.0), c(10.0, 12.0), c(30.0, 30.0)],
            ..Default::default()
        };
        d.normalize();
        let spans: Vec<(f64, f64)> = d.cues.iter().map(|c| (c.start, c.end)).collect();
        assert_eq!(spans, vec![(0.0, 10.0), (10.0, 12.0), (12.0, 20.0)]);
    }

    fn run(tl: &mut Timeline, h: &mut History, cmd: Option<EditCommand>) {
        assert!(h.execute(tl, cmd.expect("command")));
    }

    #[test]
    fn importing_a_file_is_one_undo_step_and_can_replace_or_add() {
        let mut tl = Timeline::new();
        let mut h = History::new();
        let doc = parse_srt(
            "1\n00:00:01,000 --> 00:00:02,000\nA\n\n2\n00:00:03,000 --> 00:00:04,000\nB\n",
        )
        .unwrap();
        let cmd = import_document(&mut tl, &doc, 0.0, false);
        run(&mut tl, &mut h, cmd);
        assert_eq!(tl.captions.len(), 2);

        // Adding again, shifted by 10 s, keeps the first two
        let cmd = import_document(&mut tl, &doc, 10.0, false);
        run(&mut tl, &mut h, cmd);
        assert_eq!(tl.captions.len(), 4);
        assert_eq!(tl.captions[2].start, 11.0);
        assert!(h.undo(&mut tl));
        assert_eq!(tl.captions.len(), 2);

        // Replacing swaps them
        let cmd = import_document(&mut tl, &doc, 5.0, true);
        run(&mut tl, &mut h, cmd);
        assert_eq!(
            tl.captions.iter().map(|c| c.start).collect::<Vec<_>>(),
            [6.0, 8.0]
        );
        assert!(import_document(&mut tl, &Document::default(), 0.0, false).is_none());
    }

    #[test]
    fn captions_are_added_edited_and_removed_with_undo() {
        let mut tl = Timeline::new();
        let mut h = History::new();
        let cmd = add_caption(&mut tl, 2.0, 1.5, "Olá");
        run(&mut tl, &mut h, cmd);
        let cmd = add_caption(&mut tl, 0.5, 1.0, "Antes");
        run(&mut tl, &mut h, cmd);
        assert_eq!(tl.captions[0].text, "Antes", "kept in time order");
        let id = tl.captions[1].id;

        let cmd = set_caption_text(&tl, id, "Olá, mundo");
        run(&mut tl, &mut h, cmd);
        let cmd = set_caption_times(&tl, id, 4.0, 6.0);
        run(&mut tl, &mut h, cmd);
        assert_eq!((tl.captions[1].start, tl.captions[1].end), (4.0, 6.0));
        assert!(set_caption_times(&tl, id, 5.0, 5.0).is_none());
        assert!(set_caption_text(&tl, 9999, "x").is_none());
        assert!(
            set_caption_text(&tl, id, "Olá, mundo").is_none(),
            "no change"
        );

        let cmd = remove_caption(&tl, id);
        run(&mut tl, &mut h, cmd);
        assert_eq!(tl.captions.len(), 1);
        assert!(h.undo(&mut tl));
        assert_eq!(tl.captions.len(), 2);
        assert!(add_caption(&mut tl, 1.0, 0.0, "x").is_none());
        let cmd = clear_captions(&tl);
        run(&mut tl, &mut h, cmd);
        assert!(tl.captions.is_empty());
        assert!(clear_captions(&tl).is_none());
    }

    #[test]
    fn captions_showing_at_a_time_and_the_exported_file() {
        let mut tl = Timeline::new();
        let mut h = History::new();
        for (start, length, text) in [(1.0, 2.0, "A"), (2.0, 2.0, "B")] {
            let cmd = add_caption(&mut tl, start, length, text);
            run(&mut tl, &mut h, cmd);
        }
        let texts = |t: f64| {
            active_at(&tl, t)
                .iter()
                .map(|c| c.text.clone())
                .collect::<Vec<_>>()
        };
        assert!(texts(0.5).is_empty());
        assert_eq!(texts(1.5), ["A"]);
        assert_eq!(texts(2.5), ["A", "B"], "overlapping captions both show");
        assert!(texts(4.0).is_empty(), "the end is exclusive");

        // The file cuts the overlap: A ends where B starts
        let srt = write(&export_document(&tl), Format::Srt);
        assert_eq!(
            srt,
            "1\n00:00:01,000 --> 00:00:02,000\nA\n\n2\n00:00:02,000 --> 00:00:04,000\nB\n\n"
        );
        let back = parse_srt(&srt).unwrap();
        assert_eq!(back.cues.len(), 2);
    }

    #[test]
    fn projects_saved_before_captions_still_load() {
        let tl: Timeline = serde_json::from_str(r#"{"tracks":[],"next_id":3}"#).unwrap();
        assert!(tl.captions.is_empty());
        let caption = Caption {
            id: 1,
            start: 0.0,
            end: 1.0,
            text: "x".into(),
            speaker: None,
        };
        let text = serde_json::to_string(&caption).unwrap();
        assert!(!text.contains("speaker"));
        assert_eq!(serde_json::from_str::<Caption>(&text).unwrap(), caption);
    }
}
