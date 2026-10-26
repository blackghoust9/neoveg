//! Color lookup tables in the Adobe/Resolve `.cube` text format (1D and 3D) applied to frames.
//!
//! The parser never panics on a bad file: it returns a message. Lookups interpolate linearly
//! (1D) or trilinearly (3D) and clamp the input to the table's domain.

use crate::frame::FrameBuffer;

/// Largest 3D size accepted (a 65³ table is 275 k entries); protects against absurd files.
const MAX_3D_SIZE: usize = 65;
const MAX_1D_SIZE: usize = 65_536;

#[derive(Debug, Clone, PartialEq)]
enum Table {
    /// `size` entries per channel shared by R, G and B (each entry is an RGB triple).
    One(Vec<[f32; 3]>),
    /// `size³` entries, red varying fastest.
    Three { size: usize, data: Vec<[f32; 3]> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Lut {
    pub title: String,
    domain_min: [f32; 3],
    domain_max: [f32; 3],
    table: Table,
}

fn parse_floats<const N: usize>(rest: &str) -> Option<[f32; N]> {
    let mut out = [0.0f32; N];
    let mut parts = rest.split_whitespace();
    for slot in out.iter_mut() {
        *slot = parts.next()?.parse().ok()?;
    }
    parts.next().is_none().then_some(out)
}

impl Lut {
    /// Parses the text of a `.cube` file.
    pub fn parse_cube(text: &str) -> Result<Self, String> {
        let mut title = String::new();
        let (mut size1, mut size3) = (None, None);
        let mut domain_min = [0.0f32; 3];
        let mut domain_max = [1.0f32; 3];
        let mut rows: Vec<[f32; 3]> = Vec::new();
        for (n, raw) in text.lines().enumerate() {
            let line = raw.trim().trim_start_matches('\u{feff}');
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (key, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
            let rest = rest.trim();
            let bad = |what: &str| format!("linha {}: {what}", n + 1);
            match key {
                "TITLE" => title = rest.trim_matches('"').to_owned(),
                "LUT_1D_SIZE" => {
                    size1 = Some(rest.parse::<usize>().map_err(|_| bad("tamanho inválido"))?)
                }
                "LUT_3D_SIZE" => {
                    size3 = Some(rest.parse::<usize>().map_err(|_| bad("tamanho inválido"))?)
                }
                "DOMAIN_MIN" => domain_min = parse_floats(rest).ok_or_else(|| bad("DOMAIN_MIN"))?,
                "DOMAIN_MAX" => domain_max = parse_floats(rest).ok_or_else(|| bad("DOMAIN_MAX"))?,
                k if k.starts_with("LUT_") || k.starts_with("DOMAIN_") => {
                    return Err(bad("palavra-chave não suportada"))
                }
                _ => {
                    let row: [f32; 3] =
                        parse_floats(line).ok_or_else(|| bad("esperava três números"))?;
                    if row.iter().any(|v| !v.is_finite()) {
                        return Err(bad("valor inválido"));
                    }
                    rows.push(row);
                }
            }
        }
        if domain_min.iter().zip(&domain_max).any(|(lo, hi)| {
            hi.partial_cmp(lo) != Some(std::cmp::Ordering::Greater)
                || !lo.is_finite()
                || !hi.is_finite()
        }) {
            return Err("domínio inválido (DOMAIN_MAX deve ser maior que DOMAIN_MIN)".into());
        }
        let table = match (size1, size3) {
            (Some(_), Some(_)) => return Err("o arquivo define LUT 1D e 3D ao mesmo tempo".into()),
            (Some(n), None) => {
                if !(2..=MAX_1D_SIZE).contains(&n) || rows.len() != n {
                    return Err(format!(
                        "LUT 1D de {n} entradas, mas há {} linhas",
                        rows.len()
                    ));
                }
                Table::One(rows)
            }
            (None, Some(n)) => {
                if !(2..=MAX_3D_SIZE).contains(&n) || rows.len() != n * n * n {
                    return Err(format!(
                        "LUT 3D de tamanho {n} pede {} linhas, mas há {}",
                        n * n * n,
                        rows.len()
                    ));
                }
                Table::Three {
                    size: n,
                    data: rows,
                }
            }
            (None, None) => return Err("faltou LUT_1D_SIZE ou LUT_3D_SIZE".into()),
        };
        Ok(Self {
            title,
            domain_min,
            domain_max,
            table,
        })
    }

    /// The identity 3D table of `size` entries per axis (useful for tests and as a starting point).
    pub fn identity_3d(size: usize) -> Self {
        let size = size.clamp(2, MAX_3D_SIZE);
        let step = 1.0 / (size - 1) as f32;
        let mut data = Vec::with_capacity(size * size * size);
        for b in 0..size {
            for g in 0..size {
                for r in 0..size {
                    data.push([r as f32 * step, g as f32 * step, b as f32 * step]);
                }
            }
        }
        Self {
            title: "Identidade".into(),
            domain_min: [0.0; 3],
            domain_max: [1.0; 3],
            table: Table::Three { size, data },
        }
    }

    fn normalize(&self, rgb: [f32; 3]) -> [f32; 3] {
        let mut out = [0.0; 3];
        for i in 0..3 {
            let span = self.domain_max[i] - self.domain_min[i];
            out[i] = ((rgb[i] - self.domain_min[i]) / span).clamp(0.0, 1.0);
        }
        out
    }

    /// Maps one color (components nominally in `0..=1`) through the table.
    pub fn apply(&self, rgb: [f32; 3]) -> [f32; 3] {
        let n = self.normalize(rgb);
        match &self.table {
            Table::One(entries) => {
                let last = entries.len() - 1;
                let mut out = [0.0; 3];
                for c in 0..3 {
                    let pos = n[c] * last as f32;
                    let i0 = (pos.floor() as usize).min(last);
                    let i1 = (i0 + 1).min(last);
                    let f = pos - i0 as f32;
                    out[c] = entries[i0][c] * (1.0 - f) + entries[i1][c] * f;
                }
                out
            }
            Table::Three { size, data } => {
                let last = size - 1;
                let at = |r: usize, g: usize, b: usize| data[(b * size + g) * size + r];
                let coord = |v: f32| {
                    let pos = v * last as f32;
                    let i0 = (pos.floor() as usize).min(last);
                    (i0, (i0 + 1).min(last), pos - i0 as f32)
                };
                let (r0, r1, fr) = coord(n[0]);
                let (g0, g1, fg) = coord(n[1]);
                let (b0, b1, fb) = coord(n[2]);
                let mut out = [0.0; 3];
                for (c, slot) in out.iter_mut().enumerate() {
                    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
                    let c00 = lerp(at(r0, g0, b0)[c], at(r1, g0, b0)[c], fr);
                    let c10 = lerp(at(r0, g1, b0)[c], at(r1, g1, b0)[c], fr);
                    let c01 = lerp(at(r0, g0, b1)[c], at(r1, g0, b1)[c], fr);
                    let c11 = lerp(at(r0, g1, b1)[c], at(r1, g1, b1)[c], fr);
                    *slot = lerp(lerp(c00, c10, fg), lerp(c01, c11, fg), fb);
                }
                out
            }
        }
    }

    /// Applies the table to every pixel of `frame` (alpha is left alone). `strength` blends from the
    /// original (`0`) to the fully graded picture (`1`).
    pub fn apply_to_frame(&self, frame: &mut FrameBuffer, strength: f32) {
        let k = if strength.is_finite() {
            strength.clamp(0.0, 1.0)
        } else {
            1.0
        };
        if k == 0.0 {
            return;
        }
        for px in frame.rgba.as_chunks_mut::<4>().0 {
            let src = [
                f32::from(px[0]) / 255.0,
                f32::from(px[1]) / 255.0,
                f32::from(px[2]) / 255.0,
            ];
            let graded = self.apply(src);
            for c in 0..3 {
                let v = src[c] + (graded[c] - src[c]) * k;
                px[c] = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f32; 3], b: [f32; 3]) -> bool {
        a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 1e-4)
    }

    #[test]
    fn the_identity_table_changes_nothing() {
        let lut = Lut::identity_3d(9);
        for rgb in [
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 1.0],
            [0.3, 0.6, 0.9],
            [0.123, 0.5, 0.77],
        ] {
            assert!(close(lut.apply(rgb), rgb), "{rgb:?}");
        }
    }

    #[test]
    fn a_three_d_cube_file_parses_and_inverts() {
        // 2x2x2 table that inverts every channel (red varies fastest)
        let mut text = String::from("TITLE \"Inverter\"\n# comentario\nLUT_3D_SIZE 2\n");
        for b in 0..2 {
            for g in 0..2 {
                for r in 0..2 {
                    text.push_str(&format!("{} {} {}\n", 1 - r, 1 - g, 1 - b));
                }
            }
        }
        let lut = Lut::parse_cube(&text).unwrap();
        assert_eq!(lut.title, "Inverter");
        assert!(close(lut.apply([0.0, 0.0, 0.0]), [1.0, 1.0, 1.0]));
        assert!(close(lut.apply([1.0, 0.25, 0.0]), [0.0, 0.75, 1.0]));
        let mut frame = FrameBuffer::solid(2, 2, [255, 0, 0, 200]);
        lut.apply_to_frame(&mut frame, 1.0);
        assert_eq!(frame.pixel(1, 1), [0, 255, 255, 200]);
        let mut half = FrameBuffer::solid(1, 1, [0, 0, 0, 255]);
        lut.apply_to_frame(&mut half, 0.5);
        assert_eq!(half.pixel(0, 0), [128, 128, 128, 255]);
    }

    #[test]
    fn a_one_d_table_with_a_domain_interpolates() {
        let text = "LUT_1D_SIZE 3\nDOMAIN_MIN 0 0 0\nDOMAIN_MAX 2 2 2\n0 0 0\n0.5 0.5 0.5\n1 1 1\n";
        let lut = Lut::parse_cube(text).unwrap();
        // Input 1.0 is the middle of the 0..2 domain
        assert!(close(lut.apply([1.0, 1.0, 1.0]), [0.5, 0.5, 0.5]));
        assert!(close(lut.apply([5.0, -3.0, 0.5]), [1.0, 0.0, 0.25]));
    }

    #[test]
    fn the_shipped_luts_load() {
        for name in ["BLUE_TINT", "CINEMATIC", "GREEN_TINT", "TEAL_ORANGE"] {
            let path = format!(
                "{}/../../assets/luts/{name}.cube",
                env!("CARGO_MANIFEST_DIR")
            );
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let lut = Lut::parse_cube(&text).unwrap_or_else(|e| panic!("{name}: {e}"));
            let out = lut.apply([0.5, 0.5, 0.5]);
            assert!(
                out.iter().all(|v| (0.0..=1.0).contains(v)),
                "{name}: {out:?}"
            );
        }
    }

    #[test]
    fn bad_files_are_errors_not_panics() {
        assert!(Lut::parse_cube("").is_err());
        assert!(Lut::parse_cube("LUT_3D_SIZE 2\n0 0 0\n")
            .unwrap_err()
            .contains("linhas"));
        assert!(Lut::parse_cube("LUT_3D_SIZE 2\nabc\n").is_err());
        assert!(Lut::parse_cube("LUT_1D_SIZE 2\nLUT_3D_SIZE 2\n").is_err());
        assert!(Lut::parse_cube("LUT_3D_SIZE 9999\n").is_err());
        assert!(Lut::parse_cube(
            "LUT_1D_SIZE 2\nDOMAIN_MIN 1 1 1\nDOMAIN_MAX 0 0 0\n0 0 0\n1 1 1\n"
        )
        .is_err());
        assert!(Lut::parse_cube("LUT_1D_SIZE 2\n0 0 0\n1 1 NaN\n").is_err());
    }
}
