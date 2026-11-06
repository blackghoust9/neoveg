// Adapted from `craft-media` (`crates/media/src/still/psd.rs` and `psd/tests.rs`) of
// https://github.com/storytold/filmcraft (commit 57dc82e).
// Copyright (c) 2026 ArtCraft Team and the FilmCraft contributors. MIT OR Apache-2.0; Neoveg uses
// it under the MIT terms, see third_party/filmcraft/LICENSE-MIT.
// Changes: adapted to edition 2021, Portuguese diagnostic strings, header-only `probe_file` /
// `probe_bytes` entry points, and `PsdRaster` output wired into `neoveg_video::still`.

//! Pure-Rust reader for `.psd` (`8BPS`) composite images.
//!
//! Reads the 26-byte header, skips the colour-mode, image-resource and layer/mask sections by
//! their declared byte lengths (inspecting image resource `1057` to ensure a real merged composite
//! was stored and the layer count sign to detect merged transparency), and decodes the planar
//! uncompressed or PackBits RLE composite image at 8 or 16 bits per channel (grayscale or RGB)
//! into straight-alpha RGBA8 (unblending white-matted semi-transparent pixels).

use std::io::Read;
use std::path::Path;

pub mod layers;

/// Largest side allowed by the classic `8BPS` v1 specification.
const MAX_SIDE: u32 = 30_000;
/// Largest picture decoded (`1 << 28` pixels).
const MAX_PIXELS: usize = 1 << 28;
/// Maximum channel count allowed by the specification.
const MAX_CHANNELS: usize = 56;

/// Decoded RGBA8 composite image from a `.psd` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsdRaster {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub has_alpha: bool,
}

/// Returns `true` when `b` starts with the `8BPS` signature.
pub fn sniff(b: &[u8]) -> bool {
    b.starts_with(b"8BPS")
}

fn err(name: &str, what: &str) -> String {
    format!("{name}: PSD inválido ({what})")
}

fn unsupported(name: &str, what: &str) -> String {
    format!("{name}: PSD não suportado ({what})")
}

fn be16(b: &[u8], at: usize) -> Option<u16> {
    let bytes: [u8; 2] = b.get(at..at.checked_add(2)?)?.try_into().ok()?;
    Some(u16::from_be_bytes(bytes))
}

fn be32(b: &[u8], at: usize) -> Option<u32> {
    let bytes: [u8; 4] = b.get(at..at.checked_add(4)?)?.try_into().ok()?;
    Some(u32::from_be_bytes(bytes))
}

#[derive(Debug, Clone, Copy)]
struct Header {
    channels: usize,
    width: u32,
    height: u32,
    colours: usize,
    bytes_per_sample: usize,
    pixels: usize,
}

fn parse_header(name: &str, b: &[u8]) -> Result<Header, String> {
    if !sniff(b) {
        return Err(err(name, "assinatura 8BPS ausente"));
    }
    let h = b.get(..26).ok_or_else(|| err(name, "cabeçalho truncado"))?;
    let field16 = |at| be16(h, at).unwrap_or(0);
    let field32 = |at| be32(h, at).unwrap_or(0);
    match field16(4) {
        1 => {}
        2 => return Err(unsupported(name, "formato de documento grande PSB")),
        v => return Err(err(name, &format!("versão desconhecida {v}"))),
    }
    let channels = usize::from(field16(12));
    let height = field32(14);
    let width = field32(18);
    let depth = field16(22);
    let mode = field16(24);
    let colours = match mode {
        1 => 1,
        3 => 3,
        0 => return Err(unsupported(name, "modo bitmap")),
        2 => return Err(unsupported(name, "cor indexada")),
        4 => return Err(unsupported(name, "modo CMYK")),
        7 => return Err(unsupported(name, "multicanal")),
        8 => return Err(unsupported(name, "duotone")),
        9 => return Err(unsupported(name, "modo Lab")),
        m => return Err(err(name, &format!("modo de cor desconhecido {m}"))),
    };
    let bytes_per_sample = match depth {
        8 => 1,
        16 => 2,
        1 => return Err(unsupported(name, "profundidade de 1 bit")),
        32 => return Err(unsupported(name, "profundidade de 32 bits")),
        d => return Err(err(name, &format!("profundidade desconhecida {d}"))),
    };
    if channels < colours || channels > MAX_CHANNELS {
        return Err(err(
            name,
            &format!("número inválido de canais ({channels})"),
        ));
    }
    if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
        return Err(err(name, "dimensões fora do intervalo permitido"));
    }
    let pixels = (width as usize)
        .checked_mul(height as usize)
        .filter(|&n| n <= MAX_PIXELS)
        .ok_or_else(|| err(name, "imagem grande demais"))?;
    Ok(Header {
        channels,
        width,
        height,
        colours,
        bytes_per_sample,
        pixels,
    })
}

/// Probes `(width, height)` from the first 26 bytes of a `.psd` header.
pub fn probe_bytes(name: &str, b: &[u8]) -> Result<(u32, u32), String> {
    let hdr = parse_header(name, b)?;
    Ok((hdr.width, hdr.height))
}

/// Probes `(width, height)` from a `.psd` file on disk by reading only its 26-byte header.
pub fn probe_file(path: &Path) -> Result<(u32, u32), String> {
    let label = path.display().to_string();
    let mut f = std::fs::File::open(path)
        .map_err(|e| format!("falha ao abrir PSD {}: {e}", path.display()))?;
    let mut hdr = [0u8; 26];
    let n = f
        .read(&mut hdr)
        .map_err(|e| format!("falha ao ler cabeçalho PSD {}: {e}", path.display()))?;
    probe_bytes(&label, &hdr[..n])
}

/// Returns the section starting at `pos` (4-byte big-endian length followed by payload) and the
/// byte offset immediately after it.
fn section<'a>(name: &str, b: &'a [u8], pos: usize) -> Result<(&'a [u8], usize), String> {
    let len = be32(b, pos).ok_or_else(|| err(name, "arquivo truncado"))? as usize;
    let start = pos + 4;
    let end = start
        .checked_add(len)
        .ok_or_else(|| err(name, "arquivo truncado"))?;
    let data = b
        .get(start..end)
        .ok_or_else(|| err(name, "arquivo truncado"))?;
    Ok((data, end))
}

/// Inspects version-info image resource `1057`: returns `Some(false)` when `hasRealMergedData == 0`.
fn real_merged_data(s: &[u8]) -> Option<bool> {
    let mut p = 0usize;
    while let Some(head) = p.checked_add(7).and_then(|end| s.get(p..end)) {
        if !head.starts_with(b"8BIM") {
            return None;
        }
        let id = be16(head, 4)?;
        let name_len = (usize::from(*head.get(6)?) + 2) & !1;
        let len_at = p.checked_add(6)?.checked_add(name_len)?;
        let len = be32(s, len_at)? as usize;
        let data = len_at + 4;
        if id == 1057 {
            return s.get(data + 4).map(|&v| v != 0);
        }
        p = data.checked_add(len)?.checked_add(len & 1)?;
    }
    None
}

/// Checks whether the layer/mask section has a negative layer count, indicating that the first
/// extra channel of the merged image holds document transparency rather than a saved selection.
fn merged_alpha(s: &[u8]) -> bool {
    let negative = |at: usize| be16(s, at).is_some_and(|n| (n as i16) < 0);
    let Some(info) = be32(s, 0) else {
        return false;
    };
    if info >= 2 {
        return negative(4);
    }
    let Some(mut p) = (info as usize).checked_add(4) else {
        return false;
    };
    let Some(mask) = be32(s, p) else {
        return false;
    };
    let Some(next) = p.checked_add(4).and_then(|q| q.checked_add(mask as usize)) else {
        return false;
    };
    p = next;
    while let Some(head) = p.checked_add(12).and_then(|end| s.get(p..end)) {
        let (sig, key) = head.split_at(4);
        if sig != b"8BIM" && sig != b"8B64" {
            return false;
        }
        if key.starts_with(b"Lr16") || key.starts_with(b"Lr32") {
            return negative(p + 12);
        }
        let Some(next) = be32(head, 8).and_then(|len| {
            let padded = (len as usize).checked_add(3)? & !3;
            (p + 12).checked_add(padded)
        }) else {
            return false;
        };
        p = next;
    }
    false
}

/// Expands one PackBits RLE scanline into `out`.
fn unpack(name: &str, src: &[u8], out: &mut [u8]) -> Result<(), String> {
    let (mut i, mut o) = (0usize, 0usize);
    while o < out.len() {
        let n = *src.get(i).ok_or_else(|| err(name, "linha RLE truncada"))? as i8;
        i += 1;
        if n >= 0 {
            let k = n as usize + 1;
            let lit = src
                .get(i..i + k)
                .ok_or_else(|| err(name, "linha RLE truncada"))?;
            out.get_mut(o..o + k)
                .ok_or_else(|| err(name, "linha RLE excede largura da imagem"))?
                .copy_from_slice(lit);
            i += k;
            o += k;
        } else if n != -128 {
            let k = usize::from(n.unsigned_abs()) + 1;
            let v = *src.get(i).ok_or_else(|| err(name, "linha RLE truncada"))?;
            out.get_mut(o..o + k)
                .ok_or_else(|| err(name, "linha RLE excede largura da imagem"))?
                .fill(v);
            i += 1;
            o += k;
        }
    }
    Ok(())
}

/// Decodes the merged composite image of a `.psd` byte slice into straight-alpha RGBA8.
pub fn decode_bytes(name: &str, b: &[u8]) -> Result<PsdRaster, String> {
    let hdr = parse_header(name, b)?;
    let (w, ht) = (hdr.width as usize, hdr.height as usize);

    let (_, pos) = section(name, b, 26)?;
    let (resources, pos) = section(name, b, pos)?;
    if real_merged_data(resources) == Some(false) {
        return Err(unsupported(
            name,
            "salvo sem compatibilidade máxima; o arquivo não contém imagem composta",
        ));
    }
    let (layers, pos) = section(name, b, pos)?;
    let alpha = hdr.channels > hdr.colours && merged_alpha(layers);
    let used = hdr.colours + usize::from(alpha);
    let compression = be16(b, pos).ok_or_else(|| err(name, "arquivo truncado"))?;
    let data = b.get(pos + 2..).unwrap_or_default();
    let row_bytes = w * hdr.bytes_per_sample;
    let needed = (row_bytes as u64) * (ht as u64) * (used as u64);
    let ratio = match compression {
        0 => 1,
        1 => 64,
        2 | 3 => return Err(unsupported(name, "compressão ZIP")),
        c => return Err(err(name, &format!("compressão desconhecida {c}"))),
    };
    if (data.len() as u64).saturating_mul(ratio) < needed {
        return Err(err(name, "dados de imagem truncados"));
    }

    let mut rows = Vec::with_capacity(ht * used);
    if compression == 0 {
        rows.extend((0..ht * used).map(|i| (i * row_bytes, row_bytes)));
    } else {
        let mut at = ht * hdr.channels * 2;
        for i in 0..ht * used {
            let len =
                usize::from(be16(data, i * 2).ok_or_else(|| err(name, "tabela RLE truncada"))?);
            rows.push((at, len));
            at = at
                .checked_add(len)
                .ok_or_else(|| err(name, "tabela RLE inválida"))?;
        }
    }

    let max: u32 = if hdr.bytes_per_sample == 1 {
        255
    } else {
        65_535
    };
    let sample = |s: &[u8]| match *s {
        [v] => u16::from(v),
        [hi, lo] => u16::from_be_bytes([hi, lo]),
        _ => 0,
    };
    let to8 = |v: u32| ((u64::from(v) * 255 + u64::from(max / 2)) / u64::from(max)) as u8;
    let mut row = vec![0u8; row_bytes];
    let read_row = |i: usize, row: &mut [u8]| -> Result<(), String> {
        let &(at, len) = rows
            .get(i)
            .ok_or_else(|| err(name, "dados de imagem truncados"))?;
        let end = at
            .checked_add(len)
            .ok_or_else(|| err(name, "dados de imagem truncados"))?;
        let src = data
            .get(at..end)
            .ok_or_else(|| err(name, "dados de imagem truncados"))?;
        if compression == 0 {
            row.copy_from_slice(src);
            Ok(())
        } else {
            unpack(name, src, row)
        }
    };

    let mut opacity = Vec::new();
    if alpha {
        opacity.reserve(hdr.pixels);
        for y in 0..ht {
            read_row(hdr.colours * ht + y, &mut row)?;
            opacity.extend(row.chunks_exact(hdr.bytes_per_sample).map(sample));
        }
    }

    let mut rgba = vec![255u8; hdr.pixels * 4];
    for c in 0..hdr.colours {
        for y in 0..ht {
            read_row(c * ht + y, &mut row)?;
            let out = rgba
                .get_mut(y * w * 4..(y + 1) * w * 4)
                .ok_or_else(|| err(name, "imagem grande demais"))?;
            for (x, (px, s)) in out
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(row.chunks_exact(hdr.bytes_per_sample))
                .enumerate()
            {
                let mut v = u32::from(sample(s));
                if let Some(&a) = opacity.get(y * w + x) {
                    let a = u32::from(a);
                    if a > 0 && a < max {
                        let (v64, a64, m64) = (u64::from(v), u64::from(a), u64::from(max));
                        v = ((v64 + a64).saturating_sub(m64) * m64 + a64 / 2)
                            .checked_div(a64)
                            .unwrap_or(0)
                            .min(m64) as u32;
                    }
                    px[3] = to8(a);
                }
                if hdr.colours == 1 {
                    px[..3].fill(to8(v));
                } else if let Some(d) = px.get_mut(c) {
                    *d = to8(v);
                }
            }
        }
    }

    let has_alpha = alpha && rgba.as_chunks::<4>().0.iter().any(|p| p[3] < 255);
    Ok(PsdRaster {
        width: hdr.width,
        height: hdr.height,
        rgba,
        has_alpha,
    })
}

/// Reads and decodes a `.psd` file from disk into straight-alpha RGBA8.
pub fn decode_file(path: &Path) -> Result<PsdRaster, String> {
    let label = path.display().to_string();
    let bytes =
        std::fs::read(path).map_err(|e| format!("falha ao ler PSD {}: {e}", path.display()))?;
    decode_bytes(&label, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GRAY: u16 = 1;
    const RGB: u16 = 3;

    fn packbits(row: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < row.len() {
            let run = row[i..]
                .iter()
                .take(128)
                .take_while(|&&b| b == row[i])
                .count();
            if run >= 3 {
                out.extend([(1 - run as i16) as u8, row[i]]);
                i += run;
            } else {
                let mut j = i + 1;
                while j < row.len()
                    && j - i < 128
                    && !(j + 2 < row.len() && row[j] == row[j + 1] && row[j] == row[j + 2])
                {
                    j += 1;
                }
                out.push((j - i - 1) as u8);
                out.extend_from_slice(&row[i..j]);
                i = j;
            }
        }
        out
    }

    pub(crate) fn build_psd(
        mode: u16,
        depth: u16,
        w: u32,
        h: u32,
        planes: &[Vec<u16>],
        rle: bool,
        layers: &[u8],
    ) -> Vec<u8> {
        let mut v = b"8BPS".to_vec();
        v.extend(1u16.to_be_bytes());
        v.extend([0; 6]);
        v.extend((planes.len() as u16).to_be_bytes());
        v.extend(h.to_be_bytes());
        v.extend(w.to_be_bytes());
        v.extend(depth.to_be_bytes());
        v.extend(mode.to_be_bytes());
        v.extend(0u32.to_be_bytes());
        let res = b"8BIM\x04\x0c\0\0\0\0\0\x02ab";
        v.extend((res.len() as u32).to_be_bytes());
        v.extend(res);
        v.extend((layers.len() as u32).to_be_bytes());
        v.extend(layers);
        v.extend(u16::from(rle).to_be_bytes());
        let rows: Vec<Vec<u8>> = planes
            .iter()
            .flat_map(|p| {
                p.chunks(w.max(1) as usize).map(|r| {
                    r.iter()
                        .flat_map(|&s| {
                            if depth == 8 {
                                vec![s as u8]
                            } else {
                                s.to_be_bytes().to_vec()
                            }
                        })
                        .collect::<Vec<u8>>()
                })
            })
            .collect();
        if rle {
            let packed: Vec<Vec<u8>> = rows.iter().map(|r| packbits(r)).collect();
            for p in &packed {
                v.extend((p.len() as u16).to_be_bytes());
            }
            packed.iter().for_each(|p| v.extend(p));
        } else {
            rows.iter().for_each(|r| v.extend(r));
        }
        v
    }

    fn layer_count(n: i16) -> Vec<u8> {
        let mut v = 2u32.to_be_bytes().to_vec();
        v.extend(n.to_be_bytes());
        v.extend(0u32.to_be_bytes());
        v
    }

    fn lr16_count(n: i16) -> Vec<u8> {
        let mut v = 0u32.to_be_bytes().to_vec();
        v.extend(0u32.to_be_bytes());
        v.extend(b"8BIMLr16");
        v.extend(2u32.to_be_bytes());
        v.extend(n.to_be_bytes());
        v
    }

    fn rgba(r: &PsdRaster) -> Vec<[u8; 4]> {
        r.rgba.as_chunks::<4>().0.to_vec()
    }

    fn rgb_planes() -> Vec<Vec<u16>> {
        vec![
            vec![10, 10, 10, 10, 1, 2, 3, 4],
            vec![20, 21, 22, 23, 5, 5, 5, 6],
            vec![30, 31, 32, 33, 7, 8, 9, 9],
        ]
    }

    #[test]
    fn rgb_8bit_raw_and_rle() {
        for rle in [false, true] {
            let bytes = build_psd(RGB, 8, 4, 2, &rgb_planes(), rle, &[]);
            assert_eq!(probe_bytes("a.psd", &bytes[..26]).unwrap(), (4, 2));
            let r = decode_bytes("a.psd", &bytes).unwrap();
            assert_eq!((r.width, r.height, r.has_alpha), (4, 2, false), "rle {rle}");
            let p = rgba(&r);
            assert_eq!(p[0], [10, 20, 30, 255], "rle {rle}");
            assert_eq!(p[3], [10, 23, 33, 255], "rle {rle}");
            assert_eq!(p[4], [1, 5, 7, 255], "rle {rle}");
            assert_eq!(p[7], [4, 6, 9, 255], "rle {rle}");
        }
    }

    #[test]
    fn gray_8bit_raw_and_rle() {
        for rle in [false, true] {
            let r = decode_bytes(
                "a.psd",
                &build_psd(GRAY, 8, 3, 1, &[vec![0, 128, 255]], rle, &[]),
            )
            .unwrap();
            assert_eq!(
                rgba(&r),
                [[0, 0, 0, 255], [128, 128, 128, 255], [255, 255, 255, 255]],
                "rle {rle}"
            );
        }
    }

    #[test]
    fn sixteen_bit_is_rounded_to_eight() {
        let gray = vec![0, 0xffff, 257 * 100, 0x7fff, 0x8000];
        for rle in [false, true] {
            let r = decode_bytes(
                "a.psd",
                &build_psd(GRAY, 16, 5, 1, std::slice::from_ref(&gray), rle, &[]),
            )
            .unwrap();
            let y: Vec<u8> = rgba(&r).iter().map(|p| p[0]).collect();
            assert_eq!(y, [0, 255, 100, 127, 128], "rle {rle}");
            let planes = [
                gray.clone(),
                gray.iter().map(|&s| 0xffff - s).collect(),
                vec![257 * 7; 5],
            ];
            let r = decode_bytes("a.psd", &build_psd(RGB, 16, 5, 1, &planes, rle, &[])).unwrap();
            assert_eq!(rgba(&r)[2], [100, 155, 7, 255], "rle {rle}");
        }
    }

    #[test]
    fn merged_alpha_only_when_layer_count_is_negative() {
        let mut planes = rgb_planes();
        planes.push(vec![255, 0, 128, 255, 255, 255, 255, 255]);
        let r = decode_bytes(
            "a.psd",
            &build_psd(RGB, 8, 4, 2, &planes, true, &layer_count(-2)),
        )
        .unwrap();
        assert!(r.has_alpha);
        let a: Vec<u8> = rgba(&r).iter().map(|p| p[3]).collect();
        assert_eq!(a, [255, 0, 128, 255, 255, 255, 255, 255]);

        for layers in [vec![], layer_count(2)] {
            let r =
                decode_bytes("a.psd", &build_psd(RGB, 8, 4, 2, &planes, false, &layers)).unwrap();
            assert!(!r.has_alpha);
            assert!(rgba(&r).iter().all(|p| p[3] == 255));
            assert_eq!(rgba(&r)[1], [10, 21, 31, 255]);
        }

        let gray = [vec![257 * 9, 257 * 239], vec![0xffff, 257 * 64]];
        let r = decode_bytes(
            "a.psd",
            &build_psd(GRAY, 16, 2, 1, &gray, false, &lr16_count(-1)),
        )
        .unwrap();
        assert_eq!(rgba(&r), [[9, 9, 9, 255], [191, 191, 191, 64]]);
        let r = decode_bytes(
            "a.psd",
            &build_psd(GRAY, 16, 2, 1, &gray, false, &lr16_count(1)),
        )
        .unwrap();
        assert_eq!(rgba(&r), [[9, 9, 9, 255], [239, 239, 239, 255]]);
    }

    #[test]
    fn transparent_composite_is_unmatted_from_white() {
        for depth in [8u16, 16] {
            let k = if depth == 8 { 1 } else { 257 };
            let planes: Vec<Vec<u16>> = [
                [204u16, 227, 255],
                [115, 178, 255],
                [225, 239, 255],
                [255, 140, 0],
            ]
            .iter()
            .map(|c| c.iter().map(|&v| v * k).collect())
            .collect();
            for rle in [false, true] {
                let r = decode_bytes(
                    "a.psd",
                    &build_psd(RGB, depth, 3, 1, &planes, rle, &layer_count(-1)),
                )
                .unwrap();
                let p = rgba(&r);
                assert_eq!(p[0], [204, 115, 225, 255], "{depth} bit");
                assert_eq!(p[1], [204, 115, 226, 140], "{depth} bit");
                assert_eq!(p[2], [255, 255, 255, 0], "{depth} bit");
            }
        }
    }

    fn with_version_info(file: &[u8], real_merged_data: u8) -> Vec<u8> {
        let mut res = b"8BIM\x04\x21\0\0".to_vec();
        res.extend(5u32.to_be_bytes());
        res.extend([0, 0, 0, 1, real_merged_data, 0]);
        let old = u32::from_be_bytes(file[30..34].try_into().unwrap()) as usize;
        let mut v = file[..30].to_vec();
        v.extend((res.len() as u32).to_be_bytes());
        v.extend(res);
        v.extend(&file[34 + old..]);
        v
    }

    #[test]
    fn documents_without_real_composite_and_unsupported_modes_are_refused() {
        let file = build_psd(RGB, 8, 4, 2, &rgb_planes(), true, &[]);
        assert_eq!(
            rgba(&decode_bytes("a.psd", &with_version_info(&file, 1)).unwrap())[0],
            [10, 20, 30, 255]
        );
        let msg = decode_bytes("a.psd", &with_version_info(&file, 0)).unwrap_err();
        assert!(msg.contains("compatibilidade máxima"), "{msg}");

        let ok = build_psd(RGB, 8, 4, 2, &rgb_planes(), false, &[]);
        let with = |at: usize, bytes: &[u8]| {
            let mut b = ok.clone();
            b[at..at + bytes.len()].copy_from_slice(bytes);
            b
        };
        assert!(decode_bytes("a.psd", &with(4, &[0, 2]))
            .unwrap_err()
            .contains("PSB"));
        assert!(decode_bytes("a.psd", &with(24, &[0, 4]))
            .unwrap_err()
            .contains("CMYK"));
        assert!(decode_bytes("a.psd", &with(24, &[0, 9]))
            .unwrap_err()
            .contains("Lab"));
        assert!(decode_bytes("a.psd", &with(22, &[0, 32]))
            .unwrap_err()
            .contains("32 bits"));
        let zip = ok.len() - 3 * 8 - 2;
        assert!(decode_bytes("a.psd", &with(zip, &[0, 2]))
            .unwrap_err()
            .contains("ZIP"));
    }

    #[test]
    fn hostile_truncations_return_errors_never_panic() {
        let mut planes = rgb_planes();
        planes.push(vec![255, 0, 128, 255, 255, 255, 255, 255]);
        let file = build_psd(RGB, 8, 4, 2, &planes, true, &layer_count(-2));
        for cut in 0..file.len() {
            assert!(decode_bytes("a.psd", &file[..cut]).is_err(), "cut at {cut}");
        }
    }
}
