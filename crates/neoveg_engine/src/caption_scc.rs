//! SCC (Scenarist Closed Caption) files: CEA-608 captions as hexadecimal byte pairs with a
//! timecode in front of each line.
//!
//! Written from the public CEA-608 description of the control codes and the character sets. The
//! reader follows the three caption modes of the standard (pop-on, roll-up and paint-on) on a
//! 15 by 32 screen and turns what the screen shows into cues; it reads data channels 1 and 2 as one.
//! The writer produces pop-on captions on channel 1, bottom-aligned in at most four rows of 32
//! characters, at 29.97 frames per second with drop-frame timecode. Italics, colours and
//! underlines of the file are dropped on reading, and tags in the text are dropped on writing.

use crate::captions::{Cue, Document, Error, Result};

/// Seconds in one frame at 29.97 fps.
const FRAME: f64 = 1001.0 / 30000.0;
const COLS: usize = 32;
const ROWS: usize = 15;
/// Rows of text a caption may have.
const MAX_ROWS: usize = 4;

/// Header line of every SCC file.
pub const HEADER: &str = "Scenarist_SCC V1.0";

const SPECIAL: [char; 16] = [
    '®', '°', '½', '¿', '™', '¢', '£', '♪', 'à', ' ', 'è', 'â', 'ê', 'î', 'ô', 'û',
];
const EXTENDED_12: [char; 32] = [
    'Á', 'É', 'Ó', 'Ú', 'Ü', 'ü', '‘', '¡', '*', '\'', '—', '©', '℠', '•', '“', '”', 'À', 'Â', 'Ç',
    'È', 'Ê', 'Ë', 'ë', 'Î', 'Ï', 'ï', 'Ô', 'Ù', 'ù', 'Û', '«', '»',
];
const EXTENDED_13: [char; 32] = [
    'Ã', 'ã', 'Í', 'Ì', 'ì', 'Ò', 'ò', 'Õ', 'õ', '{', '}', '\\', '^', '_', '|', '~', 'Ä', 'ä', 'Ö',
    'ö', 'ß', '¥', '¤', '│', 'Å', 'å', 'Ø', 'ø', '┌', '┐', '└', '┘',
];

// ---- timecode -------------------------------------------------------------------------------------

/// Seconds of a timecode `HH:MM:SS:FF` (non-drop) or `HH:MM:SS;FF` (drop-frame), both counted at
/// 29.97 fps.
pub fn timecode_to_seconds(tc: &str) -> Option<f64> {
    frames_of(tc).map(|f| f as f64 * FRAME)
}

fn frames_of(tc: &str) -> Option<u64> {
    let drop = tc.contains(';');
    let parts: Vec<u64> = tc
        .trim()
        .split([':', ';'])
        .map(|p| p.trim().parse().ok())
        .collect::<Option<_>>()?;
    let [h, m, s, f] = parts[..] else {
        return None;
    };
    if m > 59 || s > 59 || f > 29 {
        return None;
    }
    let frames = 108_000 * h + 1_800 * m + 30 * s + f;
    if drop {
        let minutes = 60 * h + m;
        frames.checked_sub(2 * (minutes - minutes / 10))
    } else {
        Some(frames)
    }
}

/// The drop-frame timecode of frame number `frames`.
fn timecode_of(frames: u64) -> String {
    let tens = frames / 17_982;
    let rest = frames % 17_982;
    let skipped = if rest >= 2 {
        18 * tens + 2 * ((rest - 2) / 1_798)
    } else {
        18 * tens
    };
    let n = frames + skipped;
    format!(
        "{:02}:{:02}:{:02};{:02}",
        n / 108_000,
        (n / 1_800) % 60,
        (n / 30) % 60,
        n % 30
    )
}

fn frame_of(seconds: f64) -> u64 {
    (seconds.max(0.0) / FRAME).round() as u64
}

// ---- the characters -------------------------------------------------------------------------------

/// The character of a printable byte of the basic set.
fn basic_char(b: u8) -> char {
    match b {
        0x2a => 'á',
        0x5c => 'é',
        0x5e => 'í',
        0x5f => 'ó',
        0x60 => 'ú',
        0x7b => 'ç',
        0x7c => '÷',
        0x7d => 'Ñ',
        0x7e => 'ñ',
        0x7f => '■',
        b => b as char,
    }
}

/// How a character is sent.
enum Sent {
    /// One byte of the basic set.
    Basic(u8),
    /// A special character: the pair `0x11, code`.
    Special(u8),
    /// An extended character: a stand-in character, then the pair `first, code`; the receiver backs
    /// up over the stand-in.
    Extended { first: u8, code: u8, stand_in: u8 },
}

fn stand_in(c: char) -> u8 {
    match c {
        'Á' | 'À' | 'Â' | 'Ã' | 'Ä' | 'Å' => b'A',
        'É' | 'È' | 'Ê' | 'Ë' => b'E',
        'Í' | 'Ì' | 'Î' | 'Ï' => b'I',
        'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' | 'Ø' => b'O',
        'Ú' | 'Ù' | 'Û' | 'Ü' => b'U',
        'Ç' => b'C',
        'ã' | 'ä' | 'å' => b'a',
        'ì' | 'ï' => b'i',
        'ò' | 'õ' | 'ö' | 'ø' => b'o',
        'ù' | 'ü' => b'u',
        'ë' => b'e',
        'ß' => b's',
        _ => b' ',
    }
}

fn send(c: char) -> Sent {
    match c {
        'á' => return Sent::Basic(0x2a),
        'é' => return Sent::Basic(0x5c),
        'í' => return Sent::Basic(0x5e),
        'ó' => return Sent::Basic(0x5f),
        'ú' => return Sent::Basic(0x60),
        'ç' => return Sent::Basic(0x7b),
        '÷' => return Sent::Basic(0x7c),
        'Ñ' => return Sent::Basic(0x7d),
        'ñ' => return Sent::Basic(0x7e),
        '■' => return Sent::Basic(0x7f),
        _ => {}
    }
    let plain = matches!(c, ' '..='~')
        && !matches!(c, '*' | '\\' | '^' | '_' | '`' | '{' | '|' | '}' | '~');
    if plain {
        return Sent::Basic(c as u8);
    }
    if let Some(i) = SPECIAL.iter().position(|&s| s == c).filter(|&i| i != 9) {
        return Sent::Special(0x30 + i as u8);
    }
    if let Some(i) = EXTENDED_12.iter().position(|&s| s == c) {
        return Sent::Extended {
            first: 0x12,
            code: 0x20 + i as u8,
            stand_in: stand_in(c),
        };
    }
    if let Some(i) = EXTENDED_13.iter().position(|&s| s == c) {
        return Sent::Extended {
            first: 0x13,
            code: 0x20 + i as u8,
            stand_in: stand_in(c),
        };
    }
    Sent::Basic(b'?')
}

/// The byte with the odd parity bit CEA-608 asks for.
fn with_parity(b: u8) -> u8 {
    let b = b & 0x7f;
    if b.count_ones().is_multiple_of(2) {
        b | 0x80
    } else {
        b
    }
}

// ---- reading --------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    PopOn,
    RollUp(usize),
    PaintOn,
}

#[derive(Clone)]
struct Screen {
    cells: Vec<[char; COLS]>,
}

impl Screen {
    fn new() -> Self {
        Self {
            cells: vec![[' '; COLS]; ROWS],
        }
    }

    fn clear(&mut self) {
        *self = Self::new();
    }

    fn is_empty(&self) -> bool {
        self.cells.iter().all(|r| r.iter().all(|&c| c == ' '))
    }

    fn text(&self) -> String {
        self.cells
            .iter()
            .map(|r| r.iter().collect::<String>().trim().to_string())
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

struct Decoder {
    mode: Mode,
    shown: Screen,
    hidden: Screen,
    shown_since: Option<f64>,
    row: usize,
    col: usize,
    now: f64,
    last_control: Option<(u8, u8)>,
    cues: Vec<Cue>,
}

/// `true` for the codes a sender repeats on the next frame (and a receiver must act on once).
fn is_repeated_code(b1: u8, b2: u8) -> bool {
    let misc = matches!(b1, 0x14 | 0x15 | 0x17) && (0x20..=0x2f).contains(&b2);
    let mid_row = b1 == 0x11 && (0x20..=0x2f).contains(&b2);
    misc || mid_row || ((0x10..=0x17).contains(&b1) && b2 >= 0x40)
}

/// Row (1 to 15) and column of a preamble address code.
fn pac_position(b1: u8, b2: u8) -> Option<(usize, usize)> {
    let high = usize::from(b2 >= 0x60);
    let row = match b1 {
        0x11 => 1 + high,
        0x12 => 3 + high,
        0x15 => 5 + high,
        0x16 => 7 + high,
        0x17 => 9 + high,
        0x10 if high == 0 => 11,
        0x13 => 12 + high,
        0x14 => 14 + high,
        _ => return None,
    };
    let attribute = usize::from((b2 & 0x1e) >> 1);
    Some((
        row,
        if attribute >= 8 {
            4 * (attribute - 8)
        } else {
            0
        },
    ))
}

impl Decoder {
    fn new() -> Self {
        Self {
            mode: Mode::PopOn,
            shown: Screen::new(),
            hidden: Screen::new(),
            shown_since: None,
            row: ROWS - 1,
            col: 0,
            now: 0.0,
            last_control: None,
            cues: Vec::new(),
        }
    }

    /// Closes the caption on the screen at the current time.
    fn end_display(&mut self) {
        if let Some(start) = self.shown_since.take() {
            let text = self.shown.text();
            if !text.is_empty() && self.now > start {
                self.cues.push(Cue {
                    start,
                    end: self.now,
                    text,
                    ..Cue::default()
                });
            }
        }
    }

    fn begin_display(&mut self) {
        self.shown_since = (!self.shown.is_empty()).then_some(self.now);
    }

    fn put(&mut self, c: char) {
        let direct = self.mode != Mode::PopOn;
        let row = if matches!(self.mode, Mode::RollUp(_)) {
            ROWS - 1
        } else {
            self.row
        };
        let col = self.col.min(COLS - 1);
        if direct {
            if self.shown.is_empty() && self.shown_since.is_none() {
                self.shown_since = Some(self.now);
            }
            self.shown.cells[row][col] = c;
        } else {
            self.hidden.cells[row][col] = c;
        }
        self.col = col + 1;
    }

    fn back(&mut self) {
        self.col = self.col.saturating_sub(1).min(COLS - 1);
        let row = if matches!(self.mode, Mode::RollUp(_)) {
            ROWS - 1
        } else {
            self.row
        };
        if self.mode == Mode::PopOn {
            self.hidden.cells[row][self.col] = ' ';
        } else {
            self.shown.cells[row][self.col] = ' ';
        }
    }

    /// Roll-up carriage return: the rows move up one, and each state of the window is a cue.
    fn scroll(&mut self, rows: usize) {
        self.end_display();
        for r in 0..ROWS - 1 {
            self.shown.cells[r] = self.shown.cells[r + 1];
        }
        self.shown.cells[ROWS - 1] = [' '; COLS];
        for r in 0..ROWS.saturating_sub(rows) {
            self.shown.cells[r] = [' '; COLS];
        }
        self.col = 0;
        self.begin_display();
    }

    fn control(&mut self, b1: u8, b2: u8) {
        // Channel 2 acts like channel 1; the field 2 form of a miscellaneous code is the same code
        let b1 = if b1 >= 0x18 { b1 - 8 } else { b1 };
        let b1 = if b1 == 0x15 && (0x20..=0x2f).contains(&b2) {
            0x14
        } else {
            b1
        };
        match (b1, b2) {
            (0x14, 0x20) => self.mode = Mode::PopOn,
            (0x14, 0x21) => self.back(),
            (0x14, 0x25..=0x27) => {
                let rows = usize::from(b2 - 0x23);
                if !matches!(self.mode, Mode::RollUp(_)) {
                    self.end_display();
                    self.shown.clear();
                }
                self.mode = Mode::RollUp(rows);
                self.row = ROWS - 1;
            }
            (0x14, 0x29) => self.mode = Mode::PaintOn,
            (0x14, 0x2c) => {
                self.end_display();
                self.shown.clear();
            }
            (0x14, 0x2d) => {
                if let Mode::RollUp(rows) = self.mode {
                    self.scroll(rows);
                }
            }
            (0x14, 0x2e) => self.hidden.clear(),
            (0x14, 0x2f) => {
                self.end_display();
                std::mem::swap(&mut self.shown, &mut self.hidden);
                self.hidden.clear();
                self.begin_display();
            }
            (0x17, 0x21..=0x23) => self.col += usize::from(b2 - 0x20),
            (0x11, 0x20..=0x2f) => self.put(' '),
            (0x11, 0x30..=0x3f) => self.put(SPECIAL[usize::from(b2 - 0x30)]),
            (0x12, 0x20..=0x3f) => {
                self.back();
                self.put(EXTENDED_12[usize::from(b2 - 0x20)]);
            }
            (0x13, 0x20..=0x3f) => {
                self.back();
                self.put(EXTENDED_13[usize::from(b2 - 0x20)]);
            }
            _ if b2 >= 0x40 => {
                if let Some((row, col)) = pac_position(b1, b2) {
                    if !matches!(self.mode, Mode::RollUp(_)) {
                        self.row = row - 1;
                    }
                    self.col = col;
                }
            }
            _ => {}
        }
    }

    fn word(&mut self, b1: u8, b2: u8) {
        if b1 == 0 && b2 == 0 {
            return;
        }
        if (0x10..=0x1f).contains(&b1) {
            let pair = (b1, b2);
            if is_repeated_code(b1 & !0x08, b2) {
                if self.last_control == Some(pair) {
                    self.last_control = None;
                    return;
                }
                self.last_control = Some(pair);
            } else {
                self.last_control = None;
            }
            self.control(b1, b2);
        } else {
            self.last_control = None;
            for b in [b1, b2] {
                if b >= 0x20 {
                    self.put(basic_char(b));
                }
            }
        }
    }
}

/// Parses the text of an SCC file.
pub fn parse_scc(text: &str) -> Result<Document> {
    let mut doc = Document::default();
    let mut decoder = Decoder::new();
    let mut found = false;
    let mut last_time = 0.0;
    for line in text.trim_start_matches('\u{feff}').lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("Scenarist_SCC") {
            continue;
        }
        let Some((tc, data)) = line.split_once(char::is_whitespace) else {
            doc.warnings
                .push(format!("linha sem dados ignorada: {line}"));
            continue;
        };
        let Some(first) = frames_of(tc) else {
            doc.warnings
                .push(format!("timecode inválido ignorado: {tc}"));
            continue;
        };
        found = true;
        for (k, word) in data.split_whitespace().enumerate() {
            let bytes = (word.len() == 4)
                .then(|| u16::from_str_radix(word, 16).ok())
                .flatten();
            let Some(value) = bytes else {
                doc.warnings
                    .push(format!("palavra inválida ignorada: {word}"));
                continue;
            };
            decoder.now = (first + k as u64) as f64 * FRAME;
            last_time = decoder.now;
            decoder.word((value >> 8) as u8 & 0x7f, value as u8 & 0x7f);
        }
    }
    if !found {
        return Err(Error::NotFormat("SCC"));
    }
    if decoder.shown_since.is_some() {
        decoder.now = last_time + 2.0;
        decoder.end_display();
        doc.warnings
            .push("a última legenda não foi apagada: dura 2 s".into());
    }
    doc.cues = decoder.cues;
    doc.normalize();
    Ok(doc)
}

// ---- writing --------------------------------------------------------------------------------------

/// The rows of a cue: tags dropped, lines wrapped at 32 characters, at most four rows.
fn rows_of(text: &str) -> Vec<String> {
    let mut plain = String::new();
    let (mut in_tag, mut in_brace) = (false, false);
    for c in text.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            '{' => in_brace = true,
            '}' if in_brace => in_brace = false,
            _ if in_tag || in_brace => {}
            c => plain.push(c),
        }
    }
    let mut rows = Vec::new();
    for line in plain.lines() {
        let mut row = String::new();
        for word in line.split_whitespace() {
            let width = row.chars().count();
            if width > 0 && width + 1 + word.chars().count() > COLS {
                rows.push(std::mem::take(&mut row));
            }
            if !row.is_empty() {
                row.push(' ');
            }
            row.push_str(word);
        }
        if !row.is_empty() {
            rows.push(row);
        }
    }
    for row in &mut rows {
        if row.chars().count() > COLS {
            *row = row.chars().take(COLS).collect();
        }
    }
    rows.truncate(MAX_ROWS);
    rows
}

/// The bytes of one row of text; an extended character is kept on a word boundary.
fn encode_row(text: &str) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    for c in text.chars() {
        match send(c) {
            Sent::Basic(b) => out.push(b),
            Sent::Special(code) => {
                if out.len() % 2 == 1 {
                    out.push(0);
                }
                out.extend([0x11, code]);
            }
            Sent::Extended {
                first,
                code,
                stand_in,
            } => {
                let at = out.len();
                out.push(stand_in);
                if at.is_multiple_of(2) {
                    out.push(0);
                }
                out.extend([first, code]);
            }
        }
    }
    if out.len() % 2 == 1 {
        out.push(0);
    }
    out
}

/// The preamble address code of `row` (1 to 15) with `indent` steps of four columns.
fn pac(row: usize, indent: usize) -> (u8, u8) {
    const ROWS_TABLE: [(u8, u8); 15] = [
        (0x11, 0),
        (0x11, 1),
        (0x12, 0),
        (0x12, 1),
        (0x15, 0),
        (0x15, 1),
        (0x16, 0),
        (0x16, 1),
        (0x17, 0),
        (0x17, 1),
        (0x10, 0),
        (0x13, 0),
        (0x13, 1),
        (0x14, 0),
        (0x14, 1),
    ];
    let (b1, high) = ROWS_TABLE[row.clamp(1, ROWS) - 1];
    (b1, 0x40 | (high << 5) | (((8 + indent.min(7)) as u8) << 1))
}

fn words_of(bytes: &[(u8, u8)]) -> Vec<String> {
    bytes
        .iter()
        .map(|&(a, b)| format!("{:02x}{:02x}", with_parity(a), with_parity(b)))
        .collect()
}

/// A pair that is sent twice, as the standard asks of control codes.
fn doubled(pair: (u8, u8)) -> [(u8, u8); 2] {
    [pair, pair]
}

/// The words that load and display a caption: reset, the rows with their positions, flip.
fn load_words(rows: &[String]) -> Vec<String> {
    let mut pairs: Vec<(u8, u8)> = Vec::new();
    pairs.extend(doubled((0x14, 0x20)));
    pairs.extend(doubled((0x14, 0x2e)));
    let first_row = ROWS + 1 - rows.len();
    for (i, text) in rows.iter().enumerate() {
        let len = text.chars().count();
        let left = (COLS - len.min(COLS)) / 2;
        let indent = (left / 4).min(7);
        pairs.extend(doubled(pac(first_row + i, indent)));
        let padded = format!("{}{}", " ".repeat(left - 4 * indent), text);
        let bytes = encode_row(&padded);
        pairs.extend(bytes.as_chunks::<2>().0.iter().map(|p| (p[0], p[1])));
    }
    pairs.extend(doubled((0x14, 0x2f)));
    words_of(&pairs)
}

/// Puts `words` in the first free frames from `from` on; returns the frames used.
fn place_forward(
    slots: &mut std::collections::BTreeMap<u64, String>,
    from: u64,
    words: &[String],
) -> Vec<u64> {
    let mut at = from;
    let mut used = Vec::with_capacity(words.len());
    for word in words {
        while slots.contains_key(&at) {
            at += 1;
        }
        slots.insert(at, word.clone());
        used.push(at);
        at += 1;
    }
    used
}

/// Writes the cues as an SCC file (pop-on captions on channel 1).
///
/// The stream carries one word a frame. The erases are placed first, on the frame a caption ends;
/// each caption is then loaded in the free frames before the frame it must appear on, never mixed
/// with the load of the one before it (they share the memory that is not on the screen). When there
/// is no room before the frame, the caption comes as soon as its load ends.
pub fn write_scc(doc: &Document) -> String {
    use std::collections::BTreeMap;
    let mut cues: Vec<&Cue> = doc
        .cues
        .iter()
        .filter(|c| c.end > c.start && !rows_of(&c.text).is_empty())
        .collect();
    cues.sort_by(|a, b| a.start.total_cmp(&b.start));

    // A caption ends where the next one begins, when they overlap
    let ends: Vec<u64> = cues
        .iter()
        .enumerate()
        .map(|(i, cue)| {
            let end = cues.get(i + 1).map_or(cue.end, |n| cue.end.min(n.start));
            frame_of(end)
        })
        .collect();

    let mut slots: BTreeMap<u64, String> = BTreeMap::new();
    for (i, &end) in ends.iter().enumerate() {
        // The next caption replaces this one when it comes right after it
        let replaced = cues
            .get(i + 1)
            .is_some_and(|next| frame_of(next.start) <= end + 2);
        if !replaced {
            place_forward(&mut slots, end, &words_of(&doubled((0x14, 0x2c))));
        }
    }

    let mut cursor = 0u64;
    for cue in &cues {
        let words = load_words(&rows_of(&cue.text));
        let before = words.len() - 2;
        let mut flip = frame_of(cue.start);
        while slots.contains_key(&flip) {
            flip += 1;
        }
        // The free frames just before the flip, nearest first
        let early: Vec<u64> = (0..flip)
            .rev()
            .filter(|f| !slots.contains_key(f))
            .take(before)
            .collect();
        let used = if early.len() == before && early.last().is_none_or(|&first| first >= cursor) {
            let mut frames: Vec<u64> = early.into_iter().rev().collect();
            frames.push(flip);
            let mut next = flip + 1;
            while slots.contains_key(&next) {
                next += 1;
            }
            frames.push(next);
            for (frame, word) in frames.iter().zip(&words) {
                slots.insert(*frame, word.clone());
            }
            frames
        } else {
            place_forward(&mut slots, cursor, &words)
        };
        cursor = used.last().map_or(cursor, |&last| last + 1);
    }

    // Runs of consecutive frames are the lines of the file
    let mut out = format!(
        "{HEADER}

"
    );
    let mut run: Option<(u64, u64, Vec<&str>)> = None;
    for (&frame, word) in &slots {
        match run.as_mut() {
            Some((_, last, words)) if frame == *last + 1 => {
                *last = frame;
                words.push(word);
            }
            _ => {
                if let Some((first, _, words)) = run.take() {
                    out.push_str(&format!(
                        "{}	{}

",
                        timecode_of(first),
                        words.join(" ")
                    ));
                }
                run = Some((frame, frame, vec![word]));
            }
        }
    }
    if let Some((first, _, words)) = run {
        out.push_str(&format!(
            "{}	{}

",
            timecode_of(first),
            words.join(" ")
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cue(start: f64, end: f64, text: &str) -> Cue {
        Cue {
            start,
            end,
            text: text.into(),
            ..Cue::default()
        }
    }

    fn doc(cues: Vec<Cue>) -> Document {
        Document {
            cues,
            ..Document::default()
        }
    }

    #[test]
    fn the_parity_bit_makes_every_byte_odd() {
        assert_eq!(with_parity(0x14), 0x94);
        assert_eq!(with_parity(0x20), 0x20);
        assert_eq!(with_parity(0x2e), 0xae);
        assert_eq!(with_parity(0x2f), 0x2f);
        assert_eq!(with_parity(0x00), 0x80);
        for b in 0..=0x7fu8 {
            assert_eq!(with_parity(b).count_ones() % 2, 1, "{b:#x}");
        }
    }

    #[test]
    fn timecodes_count_at_29_97_with_and_without_drop_frames() {
        assert_eq!(timecode_to_seconds("00:00:00:00"), Some(0.0));
        let one = timecode_to_seconds("00:00:01;00").unwrap();
        assert!((one - 30.0 * 1001.0 / 30000.0).abs() < 1e-9);
        // The first drop-frame minute starts at frame 2, and ten minutes are 17982 frames
        assert_eq!(frames_of("00:01:00;02"), Some(1800));
        assert_eq!(frames_of("00:10:00;00"), Some(17_982));
        assert_eq!(frames_of("00:00:00:30"), None, "frame 30 does not exist");
        assert_eq!(frames_of("not a time"), None);
        for frames in [
            0, 29, 30, 1799, 1800, 1801, 17_981, 17_982, 107_892, 400_000,
        ] {
            assert_eq!(frames_of(&timecode_of(frames)), Some(frames), "{frames}");
        }
        assert_eq!(timecode_of(1800), "00:01:00;02");
    }

    #[test]
    fn the_well_known_codes_have_their_well_known_words() {
        // Resume caption loading, erase non-displayed memory, row 15 at column 0, flip, erase
        let words = words_of(&[
            (0x14, 0x20),
            (0x14, 0x2e),
            pac(15, 0),
            (0x14, 0x2f),
            (0x14, 0x2c),
        ]);
        assert_eq!(words, ["9420", "94ae", "9470", "942f", "942c"]);
    }

    #[test]
    fn a_pop_on_caption_from_a_file_appears_at_the_flip_and_goes_at_the_erase() {
        let text = "Scenarist_SCC V1.0\n\n\
            00:00:01;00\t9420 9420 94ae 94ae 9470 9470 c8e5 ecec ef80 942f 942f\n\n\
            00:00:03;00\t942c 942c\n";
        let parsed = parse_scc(text).unwrap();
        assert_eq!(parsed.cues.len(), 1);
        assert_eq!(parsed.cues[0].text, "Hello");
        // The flip is the ninth word of its line
        let start = (frames_of("00:00:01;00").unwrap() + 9) as f64 * FRAME;
        assert!((parsed.cues[0].start - start).abs() < 1e-9);
        let end = timecode_to_seconds("00:00:03;00").unwrap();
        assert!((parsed.cues[0].end - end).abs() < 1e-9);
    }

    #[test]
    fn what_is_written_reads_back_with_its_text_and_times() {
        let original = doc(vec![
            cue(1.0, 3.0, "Olá, mundo!"),
            cue(
                4.0,
                7.5,
                "Não é à toa que ação, coração e pingüim\nficam bem: ÀÉÕ ñ ¿qué? ♪",
            ),
            cue(7.5, 9.0, "<i>Sem tags</i> {\\an8}e com `crases`"),
        ]);
        let scc = write_scc(&original);
        assert!(scc.starts_with("Scenarist_SCC V1.0\n\n"));
        let back = parse_scc(&scc).unwrap();
        assert_eq!(back.cues.len(), 3, "{scc}\n{:?}", back.warnings);
        assert_eq!(back.cues[0].text, "Olá, mundo!");
        assert!(
            back.cues[1].text.contains("Não é à toa"),
            "{}",
            back.cues[1].text
        );
        assert!(
            back.cues[1].text.contains("ÀÉÕ ñ ¿qué? ♪"),
            "{}",
            back.cues[1].text
        );
        assert!(
            back.cues[2].text.starts_with("Sem tags"),
            "{}",
            back.cues[2].text
        );
        for (a, b) in original.cues.iter().zip(&back.cues) {
            assert!(
                (a.start - b.start).abs() < 0.04,
                "{} vs {}",
                a.start,
                b.start
            );
            assert!((a.end - b.end).abs() < 0.04, "{} vs {}", a.end, b.end);
        }
    }

    #[test]
    fn long_lines_wrap_at_32_columns_and_only_four_rows_are_kept() {
        let rows = rows_of("uma legenda bem comprida que precisa quebrar em duas ou mais linhas");
        assert!(rows.len() >= 2 && rows.iter().all(|r| r.chars().count() <= COLS));
        let many = rows_of("a\nb\nc\nd\ne\nf");
        assert_eq!(many.len(), MAX_ROWS);
        assert!(rows_of("  \n ").is_empty());
        assert_eq!(rows_of("<b>x</b>"), ["x"]);
    }

    #[test]
    fn a_roll_up_caption_becomes_one_cue_per_state_of_the_window() {
        // RU2, a line, carriage return, another line, erase
        let text = "Scenarist_SCC V1.0\n\n\
            00:00:01;00\t9425 9425 9470 9470 c1c2 942d 942d c3c4\n\n\
            00:00:05;00\t942c 942c\n";
        let parsed = parse_scc(text).unwrap();
        let texts: Vec<&str> = parsed.cues.iter().map(|c| c.text.as_str()).collect();
        assert_eq!(texts, ["AB", "AB\nCD"], "{:?}", parsed.warnings);
        assert!(parsed.cues[0].end <= parsed.cues[1].start + 1e-9);
    }

    #[test]
    fn a_file_without_captions_is_not_an_scc_file_and_bad_words_are_skipped_with_a_warning() {
        assert!(parse_scc("1\n00:00:01,000 --> 00:00:02,000\nOi\n").is_err());
        assert!(parse_scc("").is_err());
        let parsed = parse_scc("Scenarist_SCC V1.0\n\n00:00:01;00\t9420 zzzz 942f 942f\n").unwrap();
        assert!(parsed.warnings.iter().any(|w| w.contains("zzzz")));
    }

    #[test]
    fn captions_that_follow_each_other_share_the_screen_without_an_erase_between_them() {
        let scc = write_scc(&doc(vec![cue(1.0, 2.0, "um"), cue(2.0, 3.0, "dois")]));
        let erases = scc.matches("942c 942c").count();
        assert_eq!(erases, 1, "only the last caption is erased:\n{scc}");
        let back = parse_scc(&scc).unwrap();
        assert_eq!(
            back.cues
                .iter()
                .map(|c| c.text.as_str())
                .collect::<Vec<_>>(),
            ["um", "dois"]
        );
    }
}
