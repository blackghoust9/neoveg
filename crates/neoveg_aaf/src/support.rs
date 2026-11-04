// Adapted from FilmCraft `crates/interchange` and `crates/time` (storytold/filmcraft, MIT OR Apache-2.0, ArtCraft Team and the FilmCraft contributors); see third_party/filmcraft/README.md.
//! Small helpers the AAF layers share: file URLs, file names, the WAV header of embedded sound,
//! and the error and report types.

use serde::{Deserialize, Serialize};

fn is_windows_abs(p: &str) -> bool {
    let b = p.as_bytes();
    b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'/' || b[2] == b'\\')
}

pub fn is_absolute(p: &str) -> bool {
    p.starts_with('/') || p.starts_with("\\\\") || is_windows_abs(p)
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let h = std::str::from_utf8(&b[i + 1..i + 3])
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok());
            if let Some(v) = h {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
}

fn percent_encode_path(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &c in s.as_bytes() {
        if c.is_ascii_alphanumeric() || b"/-._~!$&'()*+,;=:@".contains(&c) {
            out.push(c as char);
        } else {
            out.push_str(&format!("%{c:02X}"));
        }
    }
    out
}

/// Convert a `file://` URL (`file:///a/b`, `file://localhost/a/b`, `file:///C:/x`) to a path.
/// Strings that are not file URLs are returned unchanged.
pub fn file_url_to_path(url: &str) -> String {
    let lower = url.get(..7).map(|s| s.to_ascii_lowercase());
    if lower.as_deref() != Some("file://") {
        return url.to_string();
    }
    let rest = &url[7..];
    let is_localhost = rest
        .get(..9)
        .is_some_and(|s| s.eq_ignore_ascii_case("localhost"))
        && rest
            .get(9..)
            .is_some_and(|after| after.is_empty() || after.starts_with('/'));
    let rest = if is_localhost { &rest[9..] } else { rest };
    let decoded = percent_decode(rest);
    // file:///C:/x -> C:/x
    if decoded.len() >= 3 && decoded.starts_with('/') && is_windows_abs(&decoded[1..]) {
        return decoded[1..].to_string();
    }
    if !decoded.starts_with('/') {
        // file://server/share -> UNC
        return format!("//{decoded}");
    }
    decoded
}

/// Convert an absolute path to a `file://` URL (`localhost` form for FCP7 XML).
pub fn path_to_file_url(path: &str, localhost: bool) -> String {
    let p = path.replace('\\', "/");
    // Windows verbatim paths (`\\?\C:\x`, `\\?\UNC\server\share`, as `canonicalize` returns them)
    // are not network shares: drop the prefix so `?` doesn't become the URL's host.
    let p = if let Some(unc) = p.strip_prefix("//?/UNC/") {
        format!("//{unc}")
    } else if let Some(local) = p.strip_prefix("//?/") {
        local.to_string()
    } else {
        p
    };
    let host = if localhost { "localhost" } else { "" };
    if is_windows_abs(&p) {
        format!("file://{host}/{}", percent_encode_path(&p))
    } else if let Some(unc) = p.strip_prefix("//") {
        format!("file://{}", percent_encode_path(unc))
    } else {
        format!("file://{host}{}", percent_encode_path(&p))
    }
}

/// Final path component.
pub fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// File name without extension.
pub fn file_stem(path: &str) -> &str {
    let n = file_name(path);
    match n.rfind('.') {
        Some(i) if i > 0 => &n[..i],
        _ => n,
    }
}

/// PCM, channels, sample rate and bits of a WAVE file (integer PCM only).
pub fn parse_wav(b: &[u8]) -> Option<(Vec<u8>, u16, u32, u16)> {
    if b.len() < 12 || &b[..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return None;
    }
    let mut at = 12;
    let mut fmt = None;
    while at + 8 <= b.len() {
        let id = &b[at..at + 4];
        let len = u32::from_le_bytes(b[at + 4..at + 8].try_into().ok()?) as usize;
        let body = &b[at + 8..(at + 8).saturating_add(len).min(b.len())];
        if id == b"fmt " && body.len() >= 16 {
            let tag = u16::from_le_bytes([body[0], body[1]]);
            let ch = u16::from_le_bytes([body[2], body[3]]);
            let sr = u32::from_le_bytes(body[4..8].try_into().ok()?);
            let bits = u16::from_le_bytes([body[14], body[15]]);
            if tag != 1 && tag != 0xFFFE {
                return None;
            }
            fmt = Some((ch, sr, bits));
        } else if id == b"data" {
            let (ch, sr, bits) = fmt?;
            return Some((body.to_vec(), ch, sr, bits));
        }
        at = at.checked_add(8 + len + (len & 1))?;
    }
    None
}

/// Error from reading or writing an AAF file. Features that cannot be represented never error;
/// they go in the [`Report`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("{format} parse error: {message}")]
    Parse {
        format: &'static str,
        message: String,
    },
    #[error("the document contains no timeline")]
    Empty,
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Severity of a report entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Level {
    /// Informational (an assumption that was made).
    Info,
    /// Something was dropped or approximated.
    Warning,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportEntry {
    pub level: Level,
    pub message: String,
    /// How many times this message occurred.
    pub count: u32,
}

/// What an import or export could not represent exactly. Identical messages are merged with a count.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub entries: Vec<ReportEntry>,
}

impl Report {
    pub fn push(&mut self, level: Level, message: impl Into<String>) {
        let message = message.into();
        if let Some(e) = self
            .entries
            .iter_mut()
            .find(|e| e.level == level && e.message == message)
        {
            e.count += 1;
        } else {
            self.entries.push(ReportEntry {
                level,
                message,
                count: 1,
            });
        }
    }
    pub fn warn(&mut self, message: impl Into<String>) {
        self.push(Level::Warning, message);
    }
    pub fn info(&mut self, message: impl Into<String>) {
        self.push(Level::Info, message);
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn warnings(&self) -> impl Iterator<Item = &ReportEntry> {
        self.entries.iter().filter(|e| e.level == Level::Warning)
    }
    pub fn has_warnings(&self) -> bool {
        self.warnings().next().is_some()
    }
}
