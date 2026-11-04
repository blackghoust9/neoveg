//! Build script of the application crate.
//!
//! * Writes the date of the build as `NEOVEG_BUILD_DATE` (`dd/mm/aaaa`) for the startup screen.
//!   `SOURCE_DATE_EPOCH` makes it reproducible.
//! * On Windows, generates the multi-resolution "Nv" icon (`$OUT_DIR/app_icon.ico`, 16 to 256 px)
//!   directly from its polygon geometry in pure `std` Rust and embeds it together with the version
//!   information via `winres`, so plain `cargo build` produces an executable with its icon in
//!   Explorer, the task bar and alt-tab without tracking `*.ico` in Git or requiring Pillow. A
//!   failure to compile the resource (no Windows SDK) is a warning, not an error.

use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(any(windows, test))]
const ICON_CANVAS: f64 = 1024.0;
#[cfg(any(windows, test))]
const ICON_SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];
#[cfg(any(windows, test))]
const BACKGROUND: [u8; 4] = [0x12, 0x1A, 0x24, 0xFF];
#[cfg(any(windows, test))]
const LETTER_N: [u8; 4] = [0xF2, 0xF5, 0xF8, 0xFF];
#[cfg(any(windows, test))]
const LETTER_V: [u8; 4] = [0x3E, 0xA6, 0xE0, 0xFF];
#[cfg(any(windows, test))]
const ACCENT_BAR: [u8; 4] = [0x29, 0x80, 0xB9, 0xFF];
#[cfg(any(windows, test))]
const N_DIAGONAL: [(f64, f64); 4] = [
    (170.0, 230.0),
    (270.0, 230.0),
    (570.0, 794.0),
    (470.0, 794.0),
];
#[cfg(any(windows, test))]
const V_SHAPE: [(f64, f64); 7] = [
    (610.0, 420.0),
    (706.0, 420.0),
    (755.0, 650.0),
    (804.0, 420.0),
    (900.0, 420.0),
    (803.0, 794.0),
    (707.0, 794.0),
];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
    println!("cargo:rustc-env=NEOVEG_BUILD_DATE={}", build_date());
    embed_windows_resources();
}

/// `dd/mm/aaaa` for the day of the build (UTC).
fn build_date() -> String {
    let seconds = std::env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .ok()
                .map(|d| d.as_secs())
        })
        .unwrap_or(0);
    format_epoch_date(seconds)
}

fn format_epoch_date(seconds: u64) -> String {
    let (year, month, day) = civil_from_days((seconds / 86_400) as i64);
    format!("{day:02}/{month:02}/{year:04}")
}

/// Calendar date of a day count since 1970-01-01 (Howard Hinnant's algorithm).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(any(windows, test))]
fn point_in_polygon(x: f64, y: f64, poly: &[(f64, f64)]) -> bool {
    let Some(&(mut xj, mut yj)) = poly.last() else {
        return false;
    };
    let mut inside = false;
    for &(xi, yi) in poly {
        if ((yi > y) != (yj > y)) && (x < (xj - xi) * (y - yi) / (yj - yi) + xi) {
            inside = !inside;
        }
        (xj, yj) = (xi, yi);
    }
    inside
}

/// Evaluates the "Nv" mark on the `1024 x 1024` reference canvas (matching `scripts/generate_app_icon.py`).
#[cfg(any(windows, test))]
fn sample_mark(x: f64, y: f64) -> [u8; 4] {
    if (170.0..=900.0).contains(&x) && (850.0..=884.0).contains(&y) {
        return ACCENT_BAR;
    }
    if (170.0..=570.0).contains(&x)
        && (230.0..=794.0).contains(&y)
        && (x <= 270.0 || x >= 470.0 || point_in_polygon(x, y, &N_DIAGONAL))
    {
        return LETTER_N;
    }
    if (610.0..=900.0).contains(&x)
        && (420.0..=794.0).contains(&y)
        && point_in_polygon(x, y, &V_SHAPE)
    {
        return LETTER_V;
    }
    BACKGROUND
}

#[cfg(any(windows, test))]
fn render_icon_rgba(size: u32) -> Vec<[u8; 4]> {
    let sub: u32 = if size <= 48 { 8 } else { 4 };
    let scale = ICON_CANVAS / f64::from(size);
    let sub_step = scale / f64::from(sub);
    let samples = sub * sub;
    let mut pixels = Vec::with_capacity((size * size) as usize);
    for py in 0..size {
        let y0 = f64::from(py) * scale;
        for px in 0..size {
            let x0 = f64::from(px) * scale;
            let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
            for sy in 0..sub {
                let y = y0 + (f64::from(sy) + 0.5) * sub_step;
                for sx in 0..sub {
                    let x = x0 + (f64::from(sx) + 0.5) * sub_step;
                    let [sr, sg, sb, _] = sample_mark(x, y);
                    r += u32::from(sr);
                    g += u32::from(sg);
                    b += u32::from(sb);
                }
            }
            pixels.push([
                ((r + samples / 2) / samples) as u8,
                ((g + samples / 2) / samples) as u8,
                ((b + samples / 2) / samples) as u8,
                0xFF,
            ]);
        }
    }
    pixels
}

#[cfg(any(windows, test))]
fn encode_ico_dib(size: u32, rgba: &[[u8; 4]]) -> Vec<u8> {
    let xor_bytes = size * size * 4;
    let and_row_bytes = size.div_ceil(32) * 4;
    let and_bytes = and_row_bytes * size;
    let mut dib = Vec::with_capacity((40 + xor_bytes + and_bytes) as usize);

    // BITMAPINFOHEADER (40 bytes): height is doubled in ICO DIBs (XOR colour + AND mask).
    dib.extend_from_slice(&40u32.to_le_bytes());
    dib.extend_from_slice(&(size as i32).to_le_bytes());
    dib.extend_from_slice(&((size * 2) as i32).to_le_bytes());
    dib.extend_from_slice(&1u16.to_le_bytes());
    dib.extend_from_slice(&32u16.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&(xor_bytes + and_bytes).to_le_bytes());
    dib.extend_from_slice(&0i32.to_le_bytes());
    dib.extend_from_slice(&0i32.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());

    // Bottom-up 32-bit BGRA XOR mask.
    for row in (0..size as usize).rev() {
        let start = row * size as usize;
        for &[r, g, b, a] in &rgba[start..start + size as usize] {
            dib.extend_from_slice(&[b, g, r, a]);
        }
    }

    // 1-bpp AND mask: 0 means opaque for every pixel.
    dib.resize(dib.len() + and_bytes as usize, 0);
    dib
}

#[cfg(any(windows, test))]
fn build_ico_bytes() -> Vec<u8> {
    let entries: Vec<(u32, Vec<u8>)> = ICON_SIZES
        .iter()
        .map(|&size| (size, encode_ico_dib(size, &render_icon_rgba(size))))
        .collect();

    let header_len = 6 + 16 * entries.len();
    let payload_len: usize = entries.iter().map(|(_, dib)| dib.len()).sum();
    let mut ico = Vec::with_capacity(header_len + payload_len);

    // ICONDIR
    ico.extend_from_slice(&0u16.to_le_bytes());
    ico.extend_from_slice(&1u16.to_le_bytes());
    ico.extend_from_slice(&(entries.len() as u16).to_le_bytes());

    let mut offset = header_len as u32;
    for (size, dib) in &entries {
        let dim = if *size >= 256 { 0u8 } else { *size as u8 };
        let len = dib.len() as u32;
        ico.push(dim);
        ico.push(dim);
        ico.push(0);
        ico.push(0);
        ico.extend_from_slice(&1u16.to_le_bytes());
        ico.extend_from_slice(&32u16.to_le_bytes());
        ico.extend_from_slice(&len.to_le_bytes());
        ico.extend_from_slice(&offset.to_le_bytes());
        offset += len;
    }

    for (_, dib) in entries {
        ico.extend_from_slice(&dib);
    }
    ico
}

#[cfg(any(windows, test))]
fn write_app_icon_ico(path: &std::path::Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, build_ico_bytes())
}

#[cfg(all(windows, not(test)))]
fn embed_windows_resources() {
    let Some(out_dir) = std::env::var_os("OUT_DIR") else {
        println!("cargo:warning=OUT_DIR ausente; nao foi possivel gerar app_icon.ico");
        return;
    };
    let icon_path = std::path::PathBuf::from(out_dir).join("app_icon.ico");
    if let Err(error) = write_app_icon_ico(&icon_path) {
        println!(
            "cargo:warning=nao foi possivel gravar {}: {error}",
            icon_path.display()
        );
        return;
    }
    let Some(icon_str) = icon_path.to_str() else {
        println!(
            "cargo:warning=caminho OUT_DIR nao e UTF-8 valido para o winres: {}",
            icon_path.display()
        );
        return;
    };
    let mut resource = winres::WindowsResource::new();
    resource.set_icon(icon_str);
    resource.set("ProductName", "Neoveg");
    resource.set(
        "FileDescription",
        "Neoveg - editor nao linear de video e audio",
    );
    resource.set("LegalCopyright", "Neoveg. MIT OR Apache-2.0");
    if let Err(error) = resource.compile() {
        println!("cargo:warning=nao foi possivel embutir o icone no executavel: {error}");
    }
}

#[cfg(any(not(windows), test))]
fn embed_windows_resources() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_dates_format_as_dd_mm_yyyy_utc() {
        assert_eq!(format_epoch_date(0), "01/01/1970");
        assert_eq!(format_epoch_date(1_730_592_000), "03/11/2024");
        let _ = build_date();
    }

    #[test]
    fn nv_mark_geometry_matches_reference_polygons() {
        assert_eq!(sample_mark(50.0, 50.0), BACKGROUND);
        assert_eq!(sample_mark(220.0, 500.0), LETTER_N);
        assert_eq!(sample_mark(520.0, 500.0), LETTER_N);
        assert_eq!(sample_mark(370.0, 512.0), LETTER_N);
        assert_eq!(sample_mark(370.0, 300.0), BACKGROUND);
        assert_eq!(sample_mark(755.0, 700.0), LETTER_V);
        assert_eq!(sample_mark(755.0, 500.0), BACKGROUND);
        assert_eq!(sample_mark(500.0, 867.0), ACCENT_BAR);
    }

    #[test]
    fn ico_contains_all_seven_sizes_with_valid_dib_headers() {
        let bytes = build_ico_bytes();
        assert_eq!(u16::from_le_bytes([bytes[0], bytes[1]]), 0);
        assert_eq!(u16::from_le_bytes([bytes[2], bytes[3]]), 1);
        assert_eq!(
            u16::from_le_bytes([bytes[4], bytes[5]]) as usize,
            ICON_SIZES.len()
        );

        let mut expected_offset = (6 + 16 * ICON_SIZES.len()) as u32;
        for (index, &size) in ICON_SIZES.iter().enumerate() {
            let entry = &bytes[6 + 16 * index..6 + 16 * (index + 1)];
            let expected_dim = if size >= 256 { 0 } else { size as u8 };
            assert_eq!(entry[0], expected_dim);
            assert_eq!(entry[1], expected_dim);
            assert_eq!(u16::from_le_bytes([entry[4], entry[5]]), 1);
            assert_eq!(u16::from_le_bytes([entry[6], entry[7]]), 32);

            let res_len = u32::from_le_bytes([entry[8], entry[9], entry[10], entry[11]]);
            let res_off = u32::from_le_bytes([entry[12], entry[13], entry[14], entry[15]]);
            assert_eq!(res_off, expected_offset);

            let expected_res_len = 40 + size * size * 4 + size.div_ceil(32) * 4 * size;
            assert_eq!(res_len, expected_res_len);

            let dib = &bytes[res_off as usize..(res_off + res_len) as usize];
            assert_eq!(u32::from_le_bytes([dib[0], dib[1], dib[2], dib[3]]), 40);
            assert_eq!(
                i32::from_le_bytes([dib[4], dib[5], dib[6], dib[7]]),
                size as i32
            );
            assert_eq!(
                i32::from_le_bytes([dib[8], dib[9], dib[10], dib[11]]),
                (size * 2) as i32
            );
            assert_eq!(u16::from_le_bytes([dib[12], dib[13]]), 1);
            assert_eq!(u16::from_le_bytes([dib[14], dib[15]]), 32);

            // Top-left pixel (stored in the last XOR row because DIBs are bottom-up) is BACKGROUND
            // in BGRA order.
            let last_row = 40 + ((size - 1) * size * 4) as usize;
            assert_eq!(
                &dib[last_row..last_row + 4],
                &[BACKGROUND[2], BACKGROUND[1], BACKGROUND[0], 0xFF]
            );

            expected_offset += res_len;
        }
        assert_eq!(bytes.len(), expected_offset as usize);
    }

    #[test]
    fn write_app_icon_ico_persists_expected_payload() {
        let out = std::env::temp_dir()
            .join(format!("neoveg_build_rs_icon_{}", std::process::id()))
            .join("app_icon.ico");
        write_app_icon_ico(&out).expect("write ico");
        let on_disk = std::fs::read(&out).expect("read ico");
        assert_eq!(on_disk, build_ico_bytes());
        if let Some(dir) = out.parent() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}
