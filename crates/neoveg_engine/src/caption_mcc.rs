// Adapted from `filmcraft-captions` (`crates/captions/src/mcc.rs`) of
// https://github.com/storytold/filmcraft (commit 57dc82e).
// Copyright (c) 2026 ArtCraft Team and the FilmCraft contributors. MIT OR Apache-2.0; Neoveg uses
// it under the MIT terms, see third_party/filmcraft/LICENSE-MIT.
// Changes: cue timestamps use `f64` seconds instead of integer ticks; `let` chains rewritten for
// Rust edition 2021; malformed data lines record Portuguese diagnostics in `Document::warnings`;
// header program identifier set to `Neoveg`.

//! MacCaption MCC (`.mcc`): closed-caption data as text, one line per frame carrying data, each
//! line a SMPTE ST 334-2 caption distribution packet (CDP) carrying CEA-608 byte pairs and
//! CEA-708 (DTVCC) service packets, encoded in hexadecimal with MacCaption's letter runs.

use std::collections::BTreeMap;

use crate::caption_scc::{self, with_parity};
use crate::captions::{Cue, Document, Error, Result};

/// Seconds per frame for 29.97 fps MCC output (`30DF` / `30`).
pub const FRAME_29_97: f64 = 1001.0 / 30_000.0;

/// Header line of every MCC file.
pub const HEADER: &str = "File Format=MacCaption_MCC V1.0";

const CC_COUNT: usize = 20;
/// `cc_data` triplets per frame left for CEA-708 after the two CEA-608 fields.
const DTVCC_PAIRS: usize = CC_COUNT - 2;

// ---- Hex with MacCaption abbreviations ------------------------------------------------------------

const ABBREV: &[(char, &[u8])] = &[
    ('P', &[0xfb, 0x80, 0x80]),
    ('Q', &[0xfc, 0x80, 0x80]),
    ('R', &[0xfd, 0x80, 0x80]),
    ('S', &[0x96, 0x69]),
    ('T', &[0x61, 0x01]),
    ('U', &[0xe1, 0x00, 0x00, 0x00]),
    ('Z', &[0x00]),
];

/// Encodes raw packet bytes into MCC uppercase hex with run abbreviations.
pub fn compress(b: &[u8]) -> String {
    let mut out = String::new();
    let mut i = 0;
    'outer: while i < b.len() {
        let mut n = 0;
        while n < 9 && b[i + 3 * n..].starts_with(&[0xfa, 0x00, 0x00]) {
            n += 1;
        }
        if n > 0 {
            out.push((b'G' + n as u8 - 1) as char);
            i += 3 * n;
            continue;
        }
        for (c, seq) in ABBREV {
            if b[i..].starts_with(seq) {
                out.push(*c);
                i += seq.len();
                continue 'outer;
            }
        }
        out.push_str(&format!("{:02X}", b[i]));
        i += 1;
    }
    out
}

/// Expands an MCC payload string back into raw bytes (`None` if malformed).
pub fn expand(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut hex: Option<u8> = None;
    for c in s.chars().filter(|c| !c.is_whitespace()) {
        if let Some(d) = c.to_digit(16) {
            match hex.take() {
                Some(h) => out.push((h << 4) | d as u8),
                None => hex = Some(d as u8),
            }
            continue;
        }
        if hex.is_some() {
            return None;
        }
        let c = c.to_ascii_uppercase();
        match c {
            'G'..='O' => {
                for _ in 0..(c as u8 - b'G' + 1) {
                    out.extend_from_slice(&[0xfa, 0x00, 0x00]);
                }
            }
            _ => out.extend_from_slice(ABBREV.iter().find(|a| a.0 == c)?.1),
        }
    }
    if hex.is_some() {
        None
    } else {
        Some(out)
    }
}

// ---- Timecode rates -------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
struct TcRate {
    frame_seconds: f64,
    nominal: u64,
    drop_frame: bool,
    cdp_code: u8,
}

impl TcRate {
    const NTSC_DF: Self = Self {
        frame_seconds: FRAME_29_97,
        nominal: 30,
        drop_frame: true,
        cdp_code: 4,
    };
}

fn parse_tc_rate(raw: &str) -> Option<TcRate> {
    Some(match raw.trim().to_ascii_uppercase().as_str() {
        "30DF" => TcRate::NTSC_DF,
        "30" => TcRate {
            frame_seconds: FRAME_29_97,
            nominal: 30,
            drop_frame: false,
            cdp_code: 4,
        },
        "60DF" => TcRate {
            frame_seconds: 1001.0 / 60_000.0,
            nominal: 60,
            drop_frame: true,
            cdp_code: 7,
        },
        "60" => TcRate {
            frame_seconds: 1001.0 / 60_000.0,
            nominal: 60,
            drop_frame: false,
            cdp_code: 7,
        },
        "23.98" | "23.976" => TcRate {
            frame_seconds: 1001.0 / 24_000.0,
            nominal: 24,
            drop_frame: false,
            cdp_code: 1,
        },
        "24" => TcRate {
            frame_seconds: 1.0 / 24.0,
            nominal: 24,
            drop_frame: false,
            cdp_code: 2,
        },
        "25" => TcRate {
            frame_seconds: 1.0 / 25.0,
            nominal: 25,
            drop_frame: false,
            cdp_code: 3,
        },
        "50" => TcRate {
            frame_seconds: 1.0 / 50.0,
            nominal: 50,
            drop_frame: false,
            cdp_code: 6,
        },
        _ => return None,
    })
}

fn frames_to_tc(frames: u64, nominal: u64, drop_frame: bool) -> (u64, u64, u64, u64) {
    let n = if drop_frame && (nominal == 30 || nominal == 60) {
        let drop = nominal / 15;
        let per_10m = nominal * 600 - drop * 9;
        let per_1m = nominal * 60 - drop;
        let tens = frames / per_10m;
        let rest = frames % per_10m;
        let skipped = if rest >= drop {
            9 * drop * tens + drop * ((rest - drop) / per_1m)
        } else {
            9 * drop * tens
        };
        frames + skipped
    } else {
        frames
    };
    (
        n / (nominal * 3600),
        (n / (nominal * 60)) % 60,
        (n / nominal) % 60,
        n % nominal,
    )
}

fn tc_to_frames(tc: &str, rate: TcRate) -> Option<u64> {
    let drop = rate.drop_frame || tc.contains(';');
    let base = tc.split('.').next().unwrap_or(tc);
    let mut parts = base.split([':', ';']);
    let h: u64 = parts.next()?.parse().ok()?;
    let m: u64 = parts.next()?.parse().ok()?;
    let s: u64 = parts.next()?.parse().ok()?;
    let f: u64 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || m >= 60 || s >= 60 || f >= rate.nominal {
        return None;
    }
    let total = h
        .checked_mul(3600)?
        .checked_add(m * 60 + s)?
        .checked_mul(rate.nominal)?
        .checked_add(f)?;
    if drop && (rate.nominal == 30 || rate.nominal == 60) {
        let drop_n = rate.nominal / 15;
        let minutes = h.checked_mul(60)?.checked_add(m)?;
        total.checked_sub(drop_n * (minutes - minutes / 10))
    } else {
        Some(total)
    }
}

// ---- SMPTE ST 334-2 CDP + ST 291 ANC --------------------------------------------------------------

/// One frame's caption data: an optional field-1 CEA-608 pair (without parity) and DTVCC pairs
/// `(start_of_packet, b1, b2)`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameData {
    pub f1: Option<(u8, u8)>,
    pub dtvcc: Vec<(bool, u8, u8)>,
}

/// Builds a 29.97 fps CDP prefixed with ancillary DID/SDID (`61 01`), data count, and ST 291 checksum.
pub fn build_cdp(seq: u16, data: &FrameData) -> Vec<u8> {
    let mut cc: Vec<u8> = Vec::with_capacity(CC_COUNT * 3);
    match data.f1 {
        Some((a, b)) => cc.extend_from_slice(&[0xfc, with_parity(a), with_parity(b)]),
        None => cc.extend_from_slice(&[0xfc, 0x80, 0x80]),
    }
    cc.extend_from_slice(&[0xfd, 0x80, 0x80]);
    for &(start, a, b) in data.dtvcc.iter().take(DTVCC_PAIRS) {
        cc.extend_from_slice(&[if start { 0xff } else { 0xfe }, a, b]);
    }
    while cc.len() < CC_COUNT * 3 {
        cc.extend_from_slice(&[0xfa, 0x00, 0x00]);
    }
    let len = 7 + 2 + cc.len() + 4;
    let [s1, s0] = seq.to_be_bytes();
    let mut cdp = vec![
        0x96,
        0x69,
        len as u8,
        (TcRate::NTSC_DF.cdp_code << 4) | 0x0f,
        0x43,
        s1,
        s0,
        0x72,
        0xe0 | CC_COUNT as u8,
    ];
    cdp.extend_from_slice(&cc);
    cdp.extend_from_slice(&[0x74, s1, s0]);
    let sum: u32 = cdp.iter().map(|&b| u32::from(b)).sum();
    cdp.push(((256 - sum % 256) % 256) as u8);
    let mut out = vec![0x61, 0x01, cdp.len() as u8];
    out.extend(cdp);
    // SMPTE ST 291 ancillary checksum: low 8 bits of DID + SDID + data count + CDP bytes.
    let anc_sum: u32 = out.iter().map(|&b| u32::from(b)).sum();
    out.push((anc_sum & 0xff) as u8);
    out
}

/// Extracts `cc_data` triplets from one MCC line's bytes (ancillary packet + CDP, or bare CDP).
pub fn cc_triplets(b: &[u8]) -> Option<Vec<[u8; 3]>> {
    let cdp = if b.starts_with(&[0x61, 0x01]) {
        b.get(3..)?
    } else {
        b
    };
    if !cdp.starts_with(&[0x96, 0x69]) {
        return None;
    }
    let len = (*cdp.get(2)? as usize).min(cdp.len());
    let cdp = &cdp[..len];
    let mut i = 7;
    let mut out = Vec::new();
    while i < cdp.len() {
        match cdp[i] {
            0x71 => i += 5,
            0x72 => {
                let n = (*cdp.get(i + 1)? & 0x1f) as usize;
                for k in 0..n {
                    let at = i + 2 + 3 * k;
                    out.push([*cdp.get(at)?, *cdp.get(at + 1)?, *cdp.get(at + 2)?]);
                }
                i += 2 + 3 * n;
            }
            0x73 => i += 2 + 7 * (*cdp.get(i + 1)? & 0x0f) as usize,
            0x74 => break,
            0x75..=0xef => i += 2 + *cdp.get(i + 1)? as usize,
            _ => return None,
        }
    }
    Some(out)
}

// ---- CEA-708 service 1 ----------------------------------------------------------------------------

/// G2 characters supported after EXT1 (`0x10`).
const G2: &[(u8, char)] = &[
    (0x25, '…'),
    (0x2a, 'Š'),
    (0x2c, 'Œ'),
    (0x31, '‘'),
    (0x32, '’'),
    (0x33, '“'),
    (0x34, '”'),
    (0x35, '•'),
    (0x39, '™'),
    (0x3a, 'š'),
    (0x3c, 'œ'),
    (0x3d, '℠'),
    (0x3f, 'Ÿ'),
    (0x76, '⅛'),
    (0x77, '⅜'),
    (0x78, '⅝'),
    (0x79, '⅞'),
];

fn char_atom(c: char) -> Option<Vec<u8>> {
    Some(match c {
        '♪' => vec![0x7f],
        ' '..='~' => vec![c as u8],
        '\u{a0}'..='\u{ff}' => vec![c as u32 as u8],
        _ => vec![0x10, G2.iter().find(|g| g.1 == c)?.0],
    })
}

/// Returns the character CEA-708 displays for `c` (`?` if unrepresentable).
pub fn shown_as_708(c: char) -> char {
    if char_atom(c).is_some() {
        c
    } else {
        '?'
    }
}

fn load_atoms(win: u8, rows: &[String]) -> Vec<Vec<u8>> {
    let rc = rows.len().clamp(1, 12) as u8 - 1;
    let cols = rows
        .iter()
        .map(|r| r.chars().count())
        .max()
        .unwrap_or(1)
        .clamp(1, 42) as u8
        - 1;
    let mut v = vec![
        // DefineWindow: hidden, row/column lock, relative position 90% down / 50% across,
        // bottom-centre anchor, rows / columns, window style 1, pen style 1.
        vec![
            0x98 + win,
            0x18,
            0x80 | 90,
            50,
            (7 << 4) | rc,
            cols,
            (1 << 3) | 1,
        ],
        vec![0x88, 1 << win], // ClearWindows
    ];
    for (i, r) in rows.iter().enumerate() {
        if i > 0 {
            v.push(vec![0x0d]);
        }
        v.extend(
            r.chars()
                .map(|c| char_atom(c).unwrap_or_else(|| vec![b'?'])),
        );
    }
    v
}

fn packets(atoms: &[Vec<u8>], seq: &mut u8) -> Vec<Vec<u8>> {
    let mut blocks: Vec<Vec<u8>> = Vec::new();
    let mut cur: Vec<u8> = Vec::new();
    for a in atoms {
        if cur.len() + a.len() > 31 {
            blocks.push(std::mem::take(&mut cur));
        }
        cur.extend_from_slice(a);
    }
    if !cur.is_empty() {
        blocks.push(cur);
    }
    blocks
        .into_iter()
        .map(|b| {
            let mut p = vec![0, (1 << 5) | b.len() as u8];
            p.extend(b);
            if p.len() % 2 == 1 {
                p.push(0);
            }
            p[0] = ((*seq & 3) << 6) | (p.len() / 2) as u8;
            *seq = seq.wrapping_add(1);
            p
        })
        .collect()
}

fn pairs_of(packet: &[u8]) -> Vec<(bool, u8, u8)> {
    packet
        .chunks(2)
        .enumerate()
        .map(|(i, c)| (i == 0, c[0], *c.get(1).unwrap_or(&0)))
        .collect()
}

struct Dtv {
    frames: BTreeMap<u64, Vec<(bool, u8, u8)>>,
}

impl Dtv {
    fn room(&self, f: u64) -> usize {
        DTVCC_PAIRS - self.frames.get(&f).map_or(0, Vec::len)
    }

    fn place(&mut self, mut f: u64, packet: &[u8]) -> u64 {
        let pairs = pairs_of(packet);
        while self.room(f) < pairs.len() {
            f += 1;
        }
        self.frames.entry(f).or_default().extend(pairs);
        f
    }
}

fn schedule_708(doc: &Document) -> BTreeMap<u64, Vec<(bool, u8, u8)>> {
    let mut d = Dtv {
        frames: BTreeMap::new(),
    };
    let mut seq = 0u8;
    let mut cursor = 0u64;
    let mut cues: Vec<&Cue> = doc
        .cues
        .iter()
        .filter(|c| c.end > c.start && !caption_scc::rows_of(&c.text).is_empty())
        .collect();
    cues.sort_by(|a, b| a.start.total_cmp(&b.start));
    for (i, c) in cues.iter().enumerate() {
        let win = (i % 2) as u8;
        let s = caption_scc::frame_of(c.start).max(cursor);
        let end_sec = cues.get(i + 1).map_or(c.end, |n| c.end.min(n.start));
        let e = caption_scc::frame_of(end_sec);
        let rows = caption_scc::rows_of(&c.text);
        let load = packets(&load_atoms(win, &rows), &mut seq);
        let mut f = s.saturating_sub(load.len() as u64).max(cursor);
        let mut last = f;
        for p in &load {
            last = d.place(f, p);
            f = last;
        }
        let show = packets(
            &[vec![0x8a, 1 << (1 - win)], vec![0x89, 1 << win]],
            &mut seq,
        );
        let shown = d.place(s.max(last), &show[0]);
        cursor = shown + 1;
        let next_start = cues.get(i + 1).map(|n| caption_scc::frame_of(n.start));
        if next_start != Some(e) {
            let hide = packets(&[vec![0x8a, 1 << win]], &mut seq);
            d.place(e.max(cursor), &hide[0]);
        }
    }
    d.frames
}

#[derive(Clone, Default)]
struct Win {
    defined: bool,
    visible: bool,
    rows: Vec<String>,
    max_rows: usize,
    max_cols: usize,
}

struct Dec708 {
    win: [Win; 8],
    cur: usize,
    packet: Vec<u8>,
    want: usize,
    open: Option<(u64, String)>,
    cues: Vec<(u64, u64, String)>,
}

impl Dec708 {
    fn new() -> Self {
        Self {
            win: Default::default(),
            cur: 0,
            packet: Vec::new(),
            want: 0,
            open: None,
            cues: Vec::new(),
        }
    }

    fn visible_text(&self) -> String {
        let mut lines = Vec::new();
        for w in self.win.iter().filter(|w| w.defined && w.visible) {
            lines.extend(
                w.rows
                    .iter()
                    .map(|r| r.trim().to_string())
                    .filter(|r| !r.is_empty()),
            );
        }
        lines.join("\n")
    }

    fn sync(&mut self, f: u64) {
        let now = self.visible_text();
        if self.open.as_ref().map(|o| o.1.as_str()) == Some(now.as_str())
            || (self.open.is_none() && now.is_empty())
        {
            return;
        }
        if let Some((s, t)) = self.open.take() {
            if f > s {
                self.cues.push((s, f, t));
            }
        }
        if !now.is_empty() {
            self.open = Some((f, now));
        }
    }

    fn pair(&mut self, f: u64, start: bool, a: u8, b: u8) {
        if start {
            self.packet.clear();
            let size = (a & 0x3f) as usize;
            self.want = if size == 0 { 128 } else { size * 2 };
        } else if self.want == 0 {
            return;
        }
        self.packet.extend_from_slice(&[a, b]);
        if self.packet.len() >= self.want {
            let p = std::mem::take(&mut self.packet);
            self.want = 0;
            self.service_blocks(&p[1..]);
            self.sync(f);
        }
    }

    fn service_blocks(&mut self, mut b: &[u8]) {
        while let Some(&h) = b.first() {
            if h == 0 {
                break;
            }
            let (svc, len) = (h >> 5, (h & 0x1f) as usize);
            let (svc, skip) = if svc == 7 {
                (b.get(1).map_or(0, |x| x & 0x3f), 2)
            } else {
                (svc, 1)
            };
            let data = b.get(skip..skip + len).unwrap_or(&[]);
            if svc == 1 {
                self.service_data(data);
            }
            b = b.get(skip + len..).unwrap_or(&[]);
        }
    }

    fn w(&mut self) -> &mut Win {
        &mut self.win[self.cur]
    }

    fn put(&mut self, c: char) {
        let w = self.w();
        match w.rows.last_mut() {
            Some(row) if row.chars().count() < w.max_cols => row.push(c),
            None if w.max_rows > 0 && w.max_cols > 0 => w.rows.push(c.to_string()),
            _ => {}
        }
    }

    fn bitmap(&mut self, m: u8, f: impl Fn(&mut Win)) {
        for (i, w) in self.win.iter_mut().enumerate() {
            if m & (1 << i) != 0 {
                f(w);
            }
        }
    }

    fn service_data(&mut self, d: &[u8]) {
        let mut i = 0;
        let arg = |k: usize| d.get(k).copied().unwrap_or(0);
        while i < d.len() {
            let c = d[i];
            i += 1;
            match c {
                0x00 | 0x03 => {}
                0x08 => {
                    if let Some(r) = self.w().rows.last_mut() {
                        r.pop();
                    }
                }
                0x0c => self.w().rows = vec![String::new()],
                0x0d if self.w().rows.len() < self.w().max_rows => {
                    self.w().rows.push(String::new());
                }
                0x0d => {}
                0x0e => {
                    if let Some(r) = self.w().rows.last_mut() {
                        r.clear();
                    }
                }
                0x10 => {
                    let x = arg(i);
                    i += 1;
                    match x {
                        0x00..=0x07 => {}
                        0x08..=0x0f => i += 1,
                        0x10..=0x17 => i += 2,
                        0x18..=0x1f => i += 3,
                        0x20 | 0x21 => self.put(' '),
                        0x22..=0x7f => {
                            if let Some((_, ch)) = G2.iter().find(|g| g.0 == x) {
                                self.put(*ch);
                            }
                        }
                        0x80..=0x87 => i += 4,
                        0x88..=0x8f => i += 5,
                        0x90..=0x9f => i += 1 + (arg(i) & 0x1f) as usize,
                        _ => {}
                    }
                }
                0x11..=0x17 => i += 1,
                0x18..=0x1f => i += 2,
                0x20..=0x7e => self.put(c as char),
                0x7f => self.put('♪'),
                0x80..=0x87 => self.cur = (c - 0x80) as usize,
                0x88 => {
                    self.bitmap(arg(i), |w| w.rows = vec![String::new()]);
                    i += 1;
                }
                0x89 => {
                    self.bitmap(arg(i), |w| w.visible = true);
                    i += 1;
                }
                0x8a => {
                    self.bitmap(arg(i), |w| w.visible = false);
                    i += 1;
                }
                0x8b => {
                    self.bitmap(arg(i), |w| w.visible = !w.visible);
                    i += 1;
                }
                0x8c => {
                    self.bitmap(arg(i), |w| *w = Win::default());
                    i += 1;
                }
                0x8d => i += 1,
                0x8e | 0x8f | 0x93..=0x96 => {}
                0x90 | 0x92 => i += 2,
                0x91 => i += 3,
                0x97 => i += 4,
                0x98..=0x9f => {
                    let n = (c - 0x98) as usize;
                    let vis = arg(i) & 0x20 != 0;
                    let max_rows = (arg(i + 3) & 0x0f) as usize + 1;
                    let max_cols = (arg(i + 4) & 0x3f) as usize + 1;
                    let w = &mut self.win[n];
                    if !w.defined {
                        *w = Win {
                            defined: true,
                            visible: vis,
                            rows: vec![String::new()],
                            max_rows,
                            max_cols,
                        };
                    } else {
                        w.visible = vis;
                        w.max_rows = max_rows;
                        w.max_cols = max_cols;
                        w.rows.truncate(max_rows);
                        for row in &mut w.rows {
                            if let Some((at, _)) = row.char_indices().nth(max_cols) {
                                row.truncate(at);
                            }
                        }
                        if w.rows.is_empty() {
                            w.rows.push(String::new());
                        }
                    }
                    self.cur = n;
                    i += 6;
                }
                0xa0..=0xff => self.put(c as char),
                _ => {}
            }
        }
    }
}

// ---- File reading & writing -----------------------------------------------------------------------

/// Writes `doc` as a 29.97 fps drop-frame MCC file carrying both CEA-608 field 1 and CEA-708 service 1.
pub fn write_mcc(doc: &Document) -> String {
    write_mcc_with(doc, true, true, true)
}

/// Writes `doc` as MCC with selectable drop-frame labels, CEA-608 field 1, and CEA-708 service 1.
pub fn write_mcc_with(doc: &Document, drop_frame: bool, cea608: bool, cea708: bool) -> String {
    let f1 = if cea608 {
        caption_scc::schedule_pairs(doc)
    } else {
        BTreeMap::new()
    };
    let dtv = if cea708 {
        schedule_708(doc)
    } else {
        BTreeMap::new()
    };

    let mut out = format!("{HEADER}\n\n");
    out.push_str(
        "///////////////////////////////////////////////////////////////////////////////////\n",
    );
    out.push_str(
        "// Closed captions written by Neoveg: SMPTE ST 334-2 caption distribution packets\n",
    );
    out.push_str("// carrying CEA-608 field 1 and CEA-708 service 1 data; empty frames omitted.\n");
    out.push_str(
        "///////////////////////////////////////////////////////////////////////////////////\n\n",
    );

    let n = doc.cues.len() as u64;
    let h = doc.cues.iter().fold(0xcbf2_9ce4_8422_2325u64 ^ n, |h, c| {
        (h ^ c.start.to_bits() ^ c.end.to_bits().rotate_left(17)).wrapping_mul(0x100_0000_01b3)
    });
    out.push_str(&format!(
        "UUID={:08X}-{:04X}-4{:03X}-8{:03X}-{:012X}\n",
        h >> 32,
        (h >> 16) & 0xffff,
        h & 0xfff,
        (h >> 20) & 0xfff,
        h.rotate_left(29) & 0xffff_ffff_ffff
    ));
    out.push_str("Creation Program=Neoveg\n");
    out.push_str(&format!(
        "Time Code Rate={}\n\n",
        if drop_frame { "30DF" } else { "30" }
    ));

    let mut f1 = f1.into_iter().peekable();
    let mut dtv = dtv.into_iter().peekable();
    let mut seq = 0u16;
    while let Some(f) = f1
        .peek()
        .map(|(f, _)| *f)
        .into_iter()
        .chain(dtv.peek().map(|(f, _)| *f))
        .min()
    {
        let data = FrameData {
            f1: f1.next_if(|(frame, _)| *frame == f).map(|(_, pair)| pair),
            dtvcc: dtv
                .next_if(|(frame, _)| *frame == f)
                .map(|(_, pairs)| pairs)
                .unwrap_or_default(),
        };
        let (hh, mm, ss, ff) = frames_to_tc(f, 30, drop_frame);
        let sep = if drop_frame { ';' } else { ':' };
        out.push_str(&format!(
            "{hh:02}:{mm:02}:{ss:02}{sep}{ff:02}\t{}\n",
            compress(&build_cdp(seq, &data))
        ));
        seq = seq.wrapping_add(1);
    }
    out
}

/// Parses the text of a MacCaption `.mcc` file.
pub fn parse_mcc(text: &str) -> Result<Document> {
    let text = text.trim_start_matches('\u{feff}');
    let mut lines = text.lines().enumerate();
    match lines.by_ref().find(|(_, l)| !l.trim().is_empty()) {
        Some((_, l)) if l.trim().starts_with("File Format=MacCaption_MCC") => {}
        _ => return Err(Error::NotFormat("MacCaption (MCC)")),
    }

    let mut rate = TcRate::NTSC_DF;
    let mut doc = Document::default();
    let mut f1: Vec<(u64, u8, u8)> = Vec::new();
    let mut dtv: Vec<(u64, bool, u8, u8)> = Vec::new();
    let mut any608 = false;

    for (n, l) in lines {
        let l = l.trim_end();
        if l.trim().is_empty() || l.trim_start().starts_with("//") {
            continue;
        }
        if let Some(v) = l.trim_start().strip_prefix("Time Code Rate=") {
            match parse_tc_rate(v) {
                Some(r) => rate = r,
                None => doc.warnings.push(format!(
                    "linha {}: taxa de timecode não suportada ignorada: {}",
                    n + 1,
                    v.trim()
                )),
            }
            continue;
        }
        if l.contains('=') {
            continue;
        }
        let Some((tc, data)) = l.trim_start().split_once(['\t', ' ']) else {
            doc.warnings
                .push(format!("linha {}: linha sem dados ignorada", n + 1));
            continue;
        };
        let Some(frame) = tc_to_frames(tc, rate) else {
            doc.warnings
                .push(format!("linha {}: timecode inválido ignorado: {tc}", n + 1));
            continue;
        };
        let Some(bytes) = expand(data) else {
            doc.warnings
                .push(format!("linha {}: dados MCC inválidos ignorados", n + 1));
            continue;
        };
        let Some(trips) = cc_triplets(&bytes) else {
            doc.warnings
                .push(format!("linha {}: pacote CDP inválido ignorado", n + 1));
            continue;
        };
        for [m0, a, b] in trips {
            if m0 & 0x04 == 0 {
                continue;
            }
            match m0 & 0x03 {
                0 => {
                    any608 |= (a & 0x7f) != 0 || (b & 0x7f) != 0;
                    f1.push((frame, a, b));
                }
                1 => {}
                t => dtv.push((frame, t == 3, a, b)),
            }
        }
    }

    if any608 {
        doc.cues = caption_scc::decode_pairs(&f1, rate.frame_seconds, &mut doc.warnings);
    } else {
        let mut d = Dec708::new();
        let mut last = 0u64;
        for (f, start, a, b) in dtv {
            d.pair(f, start, a, b);
            last = f.saturating_add(1);
        }
        if let Some((s, t)) = d.open.take() {
            d.cues.push((s, last.max(s.saturating_add(1)), t));
            doc.warnings
                .push("a última legenda não foi apagada: encerrada no último quadro".into());
        }
        doc.cues = d
            .cues
            .into_iter()
            .map(|(s, e, text)| Cue {
                start: s as f64 * rate.frame_seconds,
                end: e as f64 * rate.frame_seconds,
                text,
                ..Cue::default()
            })
            .collect();
    }
    doc.normalize();
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cue_frames(s: u64, e: u64, text: &str) -> Cue {
        Cue {
            start: s as f64 * FRAME_29_97,
            end: e as f64 * FRAME_29_97,
            text: text.into(),
            ..Cue::default()
        }
    }

    fn frame_index(sec: f64) -> u64 {
        (sec / FRAME_29_97).round() as u64
    }

    #[test]
    fn abbreviations_round_trip() {
        let b = [
            0x61, 0x01, 0x49, 0x96, 0x69, 0xfa, 0, 0, 0xfa, 0, 0, 0xfc, 0x80, 0x80, 0x00, 0x12,
            0xe1, 0, 0, 0, 0xfb, 0x80, 0x80, 0xfd, 0x80, 0x80,
        ];
        let s = compress(&b);
        assert_eq!(s, "T49SHQZ12UPR");
        assert_eq!(expand(&s).unwrap(), b);
        assert_eq!(expand("T52S5"), None);
    }

    #[test]
    fn cdp_structure_and_checksum() {
        let d = FrameData {
            f1: Some((0x14, 0x20)),
            dtvcc: vec![(true, 0x02, 0x21), (false, 0x89, 0x01)],
        };
        let b = build_cdp(7, &d);
        assert_eq!(&b[..3], &[0x61, 0x01, 73]);
        assert_eq!(b.len(), 77);
        let sum: u32 = b[..76].iter().map(|&x| u32::from(x)).sum();
        assert_eq!(b[76], (sum & 0xff) as u8, "ST 291 ancillary checksum");
        let cdp = &b[3..76];
        assert_eq!(cdp.len(), 73);
        assert_eq!(&cdp[..9], &[0x96, 0x69, 73, 0x4f, 0x43, 0, 7, 0x72, 0xf4]);
        assert_eq!(
            cdp.iter().map(|&x| u32::from(x)).sum::<u32>() % 256,
            0,
            "ST 334-2 CDP checksum"
        );
        let t = cc_triplets(&b).unwrap();
        assert_eq!(t.len(), 20);
        assert_eq!(t[0], [0xfc, 0x94, 0x20]);
        assert_eq!(t[2], [0xff, 0x02, 0x21]);
        assert_eq!(t[19], [0xfa, 0, 0]);
    }

    #[test]
    fn writes_and_reads_608_and_708() {
        let doc = Document {
            cues: vec![
                cue_frames(60, 120, "Hello, world!"),
                cue_frames(120, 200, "Second caption\nwith two lines"),
                cue_frames(260, 300, "Café “q” ♪"),
            ],
            ..Document::default()
        };
        let s = write_mcc(&doc);
        assert!(s.starts_with("File Format=MacCaption_MCC V1.0\n"));
        assert!(s.contains("Creation Program=Neoveg\n"));
        assert!(s.contains("Time Code Rate=30DF"));
        let back = parse_mcc(&s).unwrap();
        let got: Vec<(u64, u64, &str)> = back
            .cues
            .iter()
            .map(|c| (frame_index(c.start), frame_index(c.end), c.text.as_str()))
            .collect();
        assert_eq!(
            got,
            vec![
                (60, 120, "Hello, world!"),
                (120, 200, "Second caption\nwith two lines"),
                (260, 300, "Café “q” ♪"),
            ]
        );

        // CEA-708-only stream round-trip
        let s708 = write_mcc_with(&doc, false, false, true);
        let back708 = parse_mcc(&s708).unwrap();
        let got708: Vec<(u64, u64, &str)> = back708
            .cues
            .iter()
            .map(|c| (frame_index(c.start), frame_index(c.end), c.text.as_str()))
            .collect();
        assert_eq!(
            got708,
            vec![
                (60, 120, "Hello, world!"),
                (120, 200, "Second caption\nwith two lines"),
                (260, 300, "Café “q” ♪"),
            ]
        );
        assert!(parse_mcc("garbage").is_err());
    }

    #[test]
    fn skips_malformed_timecodes_with_warnings() {
        let doc = Document {
            cues: vec![cue_frames(60, 120, "Caption")],
            ..Document::default()
        };
        let text = write_mcc_with(&doc, false, false, true);
        let line = text.lines().find(|line| line.contains('\t')).unwrap();
        let (tc, data) = line.split_once('\t').unwrap();
        for prefix in ["bad:", ":", "99999999999999999999:", "00:"] {
            let malformed = format!("{text}{prefix}{tc}\t{data}\n");
            let parsed = parse_mcc(&malformed).unwrap();
            assert_eq!(parsed.cues.len(), 1);
            assert!(
                !parsed.warnings.is_empty(),
                "missing warning for {prefix}{tc}"
            );
        }
    }

    #[test]
    fn export_size_depends_on_caption_data_not_empty_frames() {
        let doc = Document {
            cues: vec![
                cue_frames(60, 120, "First"),
                cue_frames(3_600, 3_660, "Last"),
            ],
            ..Document::default()
        };
        for drop_frame in [false, true] {
            for (cea608, cea708) in [(true, true), (true, false), (false, true)] {
                let text = write_mcc_with(&doc, drop_frame, cea608, cea708);
                let lines: Vec<_> = text
                    .lines()
                    .filter_map(|line| line.split_once('\t'))
                    .collect();
                assert!(
                    lines.len() < 100,
                    "{} lines for two short captions",
                    lines.len()
                );
                assert!(text.len() < 16_384);
                let parsed = parse_mcc(&text).unwrap();
                assert_eq!(parsed.cues.len(), 2);
                assert_eq!(frame_index(parsed.cues[0].start), 60);
                assert_eq!(frame_index(parsed.cues[1].start), 3_600);
                for (seq, (_, data)) in lines.iter().enumerate() {
                    let bytes = expand(data).unwrap();
                    assert_eq!(u16::from_be_bytes([bytes[8], bytes[9]]), seq as u16);
                    assert_eq!(
                        bytes[3..76].iter().map(|&b| u32::from(b)).sum::<u32>() % 256,
                        0
                    );
                }
            }
        }
    }

    #[test]
    fn empty_schedules_have_no_data_lines() {
        let doc = Document {
            cues: vec![cue_frames(60, 120, "Caption")],
            ..Document::default()
        };
        assert!(!write_mcc_with(&doc, true, false, false).contains('\t'));
        assert!(!write_mcc(&Document::default()).contains('\t'));
        assert!(parse_mcc(&write_mcc(&Document::default()))
            .unwrap()
            .cues
            .is_empty());
    }

    #[test]
    fn decoder_keeps_708_text_within_declared_window_dimensions() {
        let mut decoder = Dec708::new();
        decoder.service_data(&[0x98, 0, 0, 0, 0, 0, 0, b'A', b'B', 0x0d, b'C']);
        assert_eq!(decoder.win[0].rows, ["A"]);
    }
}
