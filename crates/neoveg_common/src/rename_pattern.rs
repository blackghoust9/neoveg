//! *Renomear por metadados*: names for events and media built from a template.
//!
//! A template holds plain text and fields in braces: `{data}` (`2026-11-06`), `{hora}` (`14-30-05`),
//! `{camera}`, `{nome}` (the current name without its extension), `{n}` (a running number) and
//! `{n:3}` (the same, padded to 3 digits). Example: `{data}_{camera}_{n:3}` gives
//! `2026-11-06_CamA_001`. The fields come from [`ItemMetadata`], read by the interface from the
//! file (creation time, camera model). A field the item has no value for is left out together with
//! the separator before it, so `{data}_{camera}_{n}` of a file without camera reads
//! `2026-11-06_1`.

/// What is known about one event or media file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ItemMetadata {
    /// The current name, with or without extension.
    pub name: String,
    /// Creation moment, `(year, month, day, hour, minute, second)`.
    pub created: Option<(i32, u32, u32, u32, u32, u32)>,
    pub camera: Option<String>,
}

impl ItemMetadata {
    /// The metadata of a file name and a Unix time in seconds (UTC), such as the modification time
    /// of the file when the container carries no creation date.
    pub fn from_unix(name: &str, unix_seconds: i64, camera: Option<String>) -> Self {
        Self {
            name: name.to_string(),
            created: Some(civil_from_unix(unix_seconds)),
            camera,
        }
    }
}

/// `(year, month, day, hour, minute, second)` of a Unix time in seconds, UTC (days-from-civil
/// algorithm of H. Hinnant).
pub fn civil_from_unix(secs: i64) -> (i32, u32, u32, u32, u32, u32) {
    let days = secs.div_euclid(86_400);
    let rest = secs.rem_euclid(86_400) as u32;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y } as i32;
    (y, m, d, rest / 3600, rest % 3600 / 60, rest % 60)
}

/// Characters that stand between fields and are dropped with an empty field.
const SEPARATORS: &[char] = &['_', '-', ' ', '.'];

#[derive(Debug, Clone, PartialEq)]
enum Piece {
    Text(String),
    Field(String, Option<usize>),
}

fn parse(template: &str) -> Result<Vec<Piece>, String> {
    let mut pieces = Vec::new();
    let mut text = String::new();
    let mut chars = template.chars();
    while let Some(c) = chars.next() {
        match c {
            '{' => {
                let mut body = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some(c) => body.push(c),
                        None => return Err("chave '{' sem fechamento".into()),
                    }
                }
                if !text.is_empty() {
                    pieces.push(Piece::Text(std::mem::take(&mut text)));
                }
                let (name, width) = match body.split_once(':') {
                    Some((n, w)) => (
                        n,
                        Some(
                            w.parse::<usize>()
                                .map_err(|_| format!("largura inválida em {{{body}}}"))?
                                .min(12),
                        ),
                    ),
                    None => (body.as_str(), None),
                };
                if !["data", "hora", "camera", "nome", "n"].contains(&name) {
                    return Err(format!("campo desconhecido: {{{name}}}"));
                }
                pieces.push(Piece::Field(name.to_string(), width));
            }
            '}' => return Err("chave '}' sem abertura".into()),
            c => text.push(c),
        }
    }
    if !text.is_empty() {
        pieces.push(Piece::Text(text));
    }
    Ok(pieces)
}

/// Checks the template: `Ok` when it parses and has at least one piece.
pub fn validate(template: &str) -> Result<(), String> {
    if parse(template)?.is_empty() {
        return Err("o modelo está vazio".into());
    }
    Ok(())
}

fn file_safe(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect()
}

/// The name for one item. `number` is the value of `{n}` (the interface counts from the start
/// number the user chose).
pub fn apply(template: &str, meta: &ItemMetadata, number: u32) -> Result<String, String> {
    let pieces = parse(template)?;
    let stem = std::path::Path::new(&meta.name)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| meta.name.clone());
    let mut out = String::new();
    // Separator text waiting for the next field that has a value.
    let mut pending_sep = String::new();
    let mut any_field = false;
    for piece in &pieces {
        match piece {
            Piece::Text(t) => {
                if t.chars().all(|c| SEPARATORS.contains(&c)) {
                    pending_sep.push_str(t);
                } else {
                    out.push_str(&pending_sep);
                    pending_sep.clear();
                    out.push_str(t);
                }
            }
            Piece::Field(name, width) => {
                let value = match name.as_str() {
                    "data" => meta
                        .created
                        .map(|(y, m, d, ..)| format!("{y:04}-{m:02}-{d:02}")),
                    "hora" => meta
                        .created
                        .map(|(_, _, _, h, mi, s)| format!("{h:02}-{mi:02}-{s:02}")),
                    "camera" => meta.camera.clone().filter(|c| !c.trim().is_empty()),
                    "nome" => Some(stem.clone()).filter(|s| !s.is_empty()),
                    _ => Some(format!("{:0w$}", number, w = width.unwrap_or(0))),
                };
                if let Some(v) = value {
                    if any_field {
                        out.push_str(&pending_sep);
                    }
                    pending_sep.clear();
                    out.push_str(&file_safe(&v));
                    any_field = true;
                } else {
                    // The separator before an empty field goes with it.
                    pending_sep.clear();
                }
            }
        }
    }
    let out = out.trim().to_string();
    if out.is_empty() {
        Err("o modelo não gerou nenhum nome".into())
    } else {
        Ok(out)
    }
}

/// Names for a list of items, numbered from `start`. Names that come out equal get `_2`, `_3`...
/// so no two items share one.
pub fn apply_all(template: &str, items: &[ItemMetadata], start: u32) -> Result<Vec<String>, String> {
    let mut names: Vec<String> = Vec::with_capacity(items.len());
    for (i, meta) in items.iter().enumerate() {
        let name = apply(template, meta, start.saturating_add(i as u32))?;
        let mut candidate = name.clone();
        let mut n = 2;
        while names.iter().any(|x| x.eq_ignore_ascii_case(&candidate)) {
            candidate = format!("{name}_{n}");
            n += 1;
        }
        names.push(candidate);
    }
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(camera: Option<&str>) -> ItemMetadata {
        ItemMetadata {
            name: "MVI_0042.MP4".into(),
            created: Some((2026, 11, 6, 14, 30, 5)),
            camera: camera.map(String::from),
        }
    }

    #[test]
    fn fields_fill_in_and_numbers_are_padded() {
        let n = apply("{data}_{camera}_{n:3}", &meta(Some("CamA")), 7).unwrap();
        assert_eq!(n, "2026-11-06_CamA_007");
        assert_eq!(
            apply("{nome} {hora}", &meta(None), 1).unwrap(),
            "MVI_0042 14-30-05"
        );
    }

    #[test]
    fn an_empty_field_takes_its_separator_with_it() {
        assert_eq!(apply("{data}_{camera}_{n}", &meta(None), 1).unwrap(), "2026-11-06_1");
        let no_date = ItemMetadata {
            name: "x.mp4".into(),
            ..ItemMetadata::default()
        };
        assert_eq!(apply("{data}_{n}", &no_date, 4).unwrap(), "4");
    }

    #[test]
    fn bad_templates_are_reported() {
        assert!(validate("{data").is_err());
        assert!(validate("{foo}").is_err());
        assert!(validate("a}").is_err());
        assert!(validate("{n:x}").is_err());
        assert!(validate("").is_err());
        assert!(validate("Cena_{n:2}").is_ok());
        assert!(apply("{camera}", &meta(None), 1).is_err(), "no name at all");
    }

    #[test]
    fn unix_time_becomes_a_calendar_date() {
        assert_eq!(civil_from_unix(0), (1970, 1, 1, 0, 0, 0));
        assert_eq!(civil_from_unix(1_000_000_000), (2001, 9, 9, 1, 46, 40));
        // 2024-02-29 12:00:00 (leap day)
        assert_eq!(civil_from_unix(1_709_208_000), (2024, 2, 29, 12, 0, 0));
        assert_eq!(civil_from_unix(-1), (1969, 12, 31, 23, 59, 59));
        let m = ItemMetadata::from_unix("a.mp4", 1_000_000_000, None);
        assert_eq!(apply("{data}_{n:2}", &m, 3).unwrap(), "2001-09-09_03");
    }

    #[test]
    fn equal_names_are_told_apart() {
        let items = vec![meta(Some("A")), meta(Some("A")), meta(Some("a"))];
        let names = apply_all("{camera}", &items, 1).unwrap();
        assert_eq!(names, vec!["A", "A_2", "a_3"]);
        let names = apply_all("{camera}_{n}", &items, 10).unwrap();
        assert_eq!(names, vec!["A_10", "A_11", "a_12"]);
    }

    #[test]
    fn file_unsafe_characters_in_a_value_are_replaced() {
        let m = meta(Some("Cam/A:1"));
        assert_eq!(apply("{camera}", &m, 1).unwrap(), "Cam_A_1");
    }
}
