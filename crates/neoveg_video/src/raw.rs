//! Pure-Rust camera RAW reader: uncompressed DNG / CinemaDNG (Bayer CFA & LinearRaw) plus
//! embedded-JPEG extraction for camera photo RAWs (`.cr2`, `.nef`, `.arw`, `.raf`, `.orf`, `.rw2`,
//! `.pef`) and explicit diagnostics for closed vendor-SDK cinema RAW formats (`.r3d`, `.braw`,
//! `.crm`, `.ari`).

use std::collections::HashSet;
use std::path::Path;

use image::ImageFormat;

/// Open / camera still RAW extensions handled natively (and as numbered sequences for CinemaDNG).
pub const RAW_IMAGE_EXTENSIONS: [&str; 8] = [
    "dng", "cr2", "nef", "arw", "raf", "orf", "rw2", "pef",
];

/// Closed cinema RAW extensions whose decoders require proprietary vendor SDKs incompatible with
/// Neoveg's `MIT OR Apache-2.0` licensing policy.
pub const PROPRIETARY_RAW_EXTENSIONS: [&str; 4] = ["r3d", "braw", "crm", "ari"];

const MAX_DIMENSION: u32 = 32_768;
const MAX_PIXELS: usize = 1 << 27; // 134 MP safety ceiling

/// Decoded RGBA8 image produced from a DNG or camera RAW file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawRaster {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Returns `true` when `path` has a supported camera RAW / DNG extension.
pub fn is_raw_image_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| {
            RAW_IMAGE_EXTENSIONS
                .iter()
                .any(|cand| ext.eq_ignore_ascii_case(cand))
        })
}

/// Returns `true` when `path` is a closed vendor-SDK cinema RAW file (`.r3d`, `.braw`, `.crm`, `.ari`).
pub fn is_proprietary_raw_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| {
            PROPRIETARY_RAW_EXTENSIONS
                .iter()
                .any(|cand| ext.eq_ignore_ascii_case(cand))
        })
}

/// Human-readable Portuguese diagnostic when the user attempts to import a closed cinema RAW format.
pub fn proprietary_raw_error(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    let label = match ext.as_str() {
        "r3d" => "RED R3D (.r3d)",
        "braw" => "Blackmagic RAW (.braw)",
        "crm" => "Canon Cinema RAW Light (.crm)",
        "ari" => "ARRI RAW (.ari)",
        _ => return None,
    };
    Some(format!(
        "{label} exige SDK proprietário do fabricante (incompatível com a licença aberta do Neoveg); transcodifique para CinemaDNG (.dng), ProRes, DNxHR ou MP4/MOV"
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Endian {
    Little,
    Big,
}

impl Endian {
    fn u16(self, b: &[u8], at: usize) -> Option<u16> {
        let s: [u8; 2] = b.get(at..at.checked_add(2)?)?.try_into().ok()?;
        Some(match self {
            Endian::Little => u16::from_le_bytes(s),
            Endian::Big => u16::from_be_bytes(s),
        })
    }

    fn u32(self, b: &[u8], at: usize) -> Option<u32> {
        let s: [u8; 4] = b.get(at..at.checked_add(4)?)?.try_into().ok()?;
        Some(match self {
            Endian::Little => u32::from_le_bytes(s),
            Endian::Big => u32::from_be_bytes(s),
        })
    }

    fn i32(self, b: &[u8], at: usize) -> Option<i32> {
        Some(self.u32(b, at)? as i32)
    }
}

#[derive(Debug, Clone, Default)]
struct IfdEntry {
    subfile_type: u32,
    width: u32,
    height: u32,
    bits_per_sample: Vec<u16>,
    compression: u16,
    photometric: u16,
    orientation: Option<u16>,
    samples_per_pixel: u16,
    rows_per_strip: u32,
    strip_offsets: Vec<u32>,
    strip_byte_counts: Vec<u32>,
    planar_config: u16,
    tile_width: u32,
    tile_height: u32,
    tile_offsets: Vec<u32>,
    tile_byte_counts: Vec<u32>,
    jpeg_offset: Option<u32>,
    jpeg_length: Option<u32>,
    rw2_jpg_offset: Option<(usize, usize)>,
    cfa_dim: [u16; 2],
    cfa_pattern: Vec<u8>,
    black_level: Vec<f64>,
    white_level: Option<f64>,
    crop_origin: Option<[u32; 2]>,
    crop_size: Option<[u32; 2]>,
    as_shot_neutral: Vec<f64>,
    sub_ifds: Vec<u32>,
}

impl IfdEntry {
    fn is_uncompressed_raw(&self) -> bool {
        self.compression == 1
            && self.width > 0
            && self.height > 0
            && matches!(self.photometric, 32803 | 34892 | 2 | 1)
            && (!self.strip_offsets.is_empty() || !self.tile_offsets.is_empty())
    }

    fn effective_dimensions(&self) -> (u32, u32) {
        if let Some([cw, ch]) = self.crop_size {
            let [ox, oy] = self.crop_origin.unwrap_or([0, 0]);
            if cw > 0 && ch > 0 && ox.saturating_add(cw) <= self.width && oy.saturating_add(ch) <= self.height {
                return (cw, ch);
            }
        }
        (self.width, self.height)
    }
}

struct ParsedRaw {
    endian: Endian,
    ifds: Vec<IfdEntry>,
    orientation: u16,
    raf_jpeg: Option<(usize, usize)>,
}

fn tiff_type_size(ty: u16) -> usize {
    match ty {
        1 | 2 | 6 | 7 => 1,
        3 | 8 => 2,
        4 | 9 | 11 | 13 => 4,
        5 | 10 | 12 => 8,
        _ => 0,
    }
}

fn read_u32_values(b: &[u8], endian: Endian, ty: u16, count: usize, val_off: usize) -> Vec<u32> {
    let elem = tiff_type_size(ty);
    if elem == 0 || count == 0 || count > 65_536 {
        return Vec::new();
    }
    let total = elem.saturating_mul(count);
    let data_pos = if total <= 4 {
        val_off
    } else {
        let Some(off) = endian.u32(b, val_off) else {
            return Vec::new();
        };
        off as usize
    };
    let mut out = Vec::with_capacity(count.min(256));
    for i in 0..count {
        let at = data_pos.saturating_add(i.saturating_mul(elem));
        let v = match ty {
            1 | 7 => b.get(at).copied().map(u32::from),
            3 => endian.u16(b, at).map(u32::from),
            4 | 13 => endian.u32(b, at),
            _ => None,
        };
        let Some(v) = v else { break };
        out.push(v);
    }
    out
}

fn read_f64_values(b: &[u8], endian: Endian, ty: u16, count: usize, val_off: usize) -> Vec<f64> {
    let elem = tiff_type_size(ty);
    if elem == 0 || count == 0 || count > 256 {
        return Vec::new();
    }
    let total = elem.saturating_mul(count);
    let data_pos = if total <= 4 {
        val_off
    } else {
        let Some(off) = endian.u32(b, val_off) else {
            return Vec::new();
        };
        off as usize
    };
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let at = data_pos.saturating_mul(1).saturating_add(i.saturating_mul(elem));
        let v = match ty {
            1 => b.get(at).copied().map(f64::from),
            3 => endian.u16(b, at).map(f64::from),
            4 => endian.u32(b, at).map(f64::from),
            5 => {
                let num = endian.u32(b, at);
                let den = endian.u32(b, at + 4);
                match (num, den) {
                    (Some(n), Some(d)) if d != 0 => Some(f64::from(n) / f64::from(d)),
                    _ => None,
                }
            }
            10 => {
                let num = endian.i32(b, at);
                let den = endian.i32(b, at + 4);
                match (num, den) {
                    (Some(n), Some(d)) if d != 0 => Some(f64::from(n) / f64::from(d)),
                    _ => None,
                }
            }
            _ => None,
        };
        let Some(v) = v else { break };
        out.push(v);
    }
    out
}

fn read_raw_bytes(b: &[u8], endian: Endian, ty: u16, count: usize, val_off: usize) -> Option<&[u8]> {
    let elem = tiff_type_size(ty).max(1);
    let total = elem.checked_mul(count)?;
    let pos = if total <= 4 {
        val_off
    } else {
        endian.u32(b, val_off)? as usize
    };
    b.get(pos..pos.checked_add(total)?)
}

fn parse_ifd(b: &[u8], endian: Endian, offset: usize) -> Option<(IfdEntry, u32)> {
    let entry_count = usize::from(endian.u16(b, offset)?);
    if entry_count > 4096 {
        return None;
    }
    let mut entry = IfdEntry {
        samples_per_pixel: 1,
        compression: 1,
        planar_config: 1,
        cfa_dim: [2, 2],
        ..Default::default()
    };
    let mut cursor = offset.checked_add(2)?;
    for _ in 0..entry_count {
        if cursor.checked_add(12)? > b.len() {
            break;
        }
        let tag = endian.u16(b, cursor)?;
        let ty = endian.u16(b, cursor + 2)?;
        let count = endian.u32(b, cursor + 4)? as usize;
        let val_off = cursor + 8;
        match tag {
            0x002e => {
                // Panasonic RW2 embedded JpgFromRaw bytes
                if let Some(slice) = read_raw_bytes(b, endian, ty, count, val_off) {
                    let start = slice.as_ptr() as usize - b.as_ptr() as usize;
                    entry.rw2_jpg_offset = Some((start, slice.len()));
                }
            }
            0x00fe => {
                if let Some(&v) = read_u32_values(b, endian, ty, count, val_off).first() {
                    entry.subfile_type = v;
                }
            }
            0x0100 => {
                if let Some(&v) = read_u32_values(b, endian, ty, count, val_off).first() {
                    entry.width = v;
                }
            }
            0x0101 => {
                if let Some(&v) = read_u32_values(b, endian, ty, count, val_off).first() {
                    entry.height = v;
                }
            }
            0x0102 => {
                entry.bits_per_sample = read_u32_values(b, endian, ty, count, val_off)
                    .into_iter()
                    .map(|v| v as u16)
                    .collect();
            }
            0x0103 => {
                if let Some(&v) = read_u32_values(b, endian, ty, count, val_off).first() {
                    entry.compression = v as u16;
                }
            }
            0x0106 => {
                if let Some(&v) = read_u32_values(b, endian, ty, count, val_off).first() {
                    entry.photometric = v as u16;
                }
            }
            0x0111 => {
                entry.strip_offsets = read_u32_values(b, endian, ty, count, val_off);
            }
            0x0112 => {
                if let Some(&v) = read_u32_values(b, endian, ty, count, val_off).first() {
                    entry.orientation = Some(v as u16);
                }
            }
            0x0115 => {
                if let Some(&v) = read_u32_values(b, endian, ty, count, val_off).first() {
                    entry.samples_per_pixel = (v as u16).max(1);
                }
            }
            0x0116 => {
                if let Some(&v) = read_u32_values(b, endian, ty, count, val_off).first() {
                    entry.rows_per_strip = v;
                }
            }
            0x0117 => {
                entry.strip_byte_counts = read_u32_values(b, endian, ty, count, val_off);
            }
            0x011c => {
                if let Some(&v) = read_u32_values(b, endian, ty, count, val_off).first() {
                    entry.planar_config = v as u16;
                }
            }
            0x0142 => {
                if let Some(&v) = read_u32_values(b, endian, ty, count, val_off).first() {
                    entry.tile_width = v;
                }
            }
            0x0143 => {
                if let Some(&v) = read_u32_values(b, endian, ty, count, val_off).first() {
                    entry.tile_height = v;
                }
            }
            0x0144 => {
                entry.tile_offsets = read_u32_values(b, endian, ty, count, val_off);
            }
            0x0145 => {
                entry.tile_byte_counts = read_u32_values(b, endian, ty, count, val_off);
            }
            0x014a => {
                entry.sub_ifds = read_u32_values(b, endian, ty, count, val_off);
            }
            0x0201 => {
                entry.jpeg_offset = read_u32_values(b, endian, ty, count, val_off).first().copied();
            }
            0x0202 => {
                entry.jpeg_length = read_u32_values(b, endian, ty, count, val_off).first().copied();
            }
            0x828d => {
                let vals = read_u32_values(b, endian, ty, count, val_off);
                if vals.len() >= 2 {
                    entry.cfa_dim = [vals[0] as u16, vals[1] as u16];
                }
            }
            0x828e => {
                if let Some(raw) = read_raw_bytes(b, endian, ty, count, val_off) {
                    entry.cfa_pattern = raw.to_vec();
                }
            }
            0xa302 => {
                // EXIF CFAPattern: 2-byte cols, 2-byte rows, then pattern bytes
                if let Some(raw) = read_raw_bytes(b, endian, ty, count, val_off) {
                    if raw.len() >= 8 {
                        let cols = endian.u16(raw, 0).unwrap_or(2);
                        let rows = endian.u16(raw, 2).unwrap_or(2);
                        entry.cfa_dim = [rows, cols];
                        entry.cfa_pattern = raw[4..].to_vec();
                    }
                }
            }
            0xc61a => {
                entry.black_level = read_f64_values(b, endian, ty, count, val_off);
            }
            0xc61d => {
                entry.white_level = read_f64_values(b, endian, ty, count, val_off).first().copied();
            }
            0xc61f => {
                let vals = read_f64_values(b, endian, ty, count, val_off);
                if vals.len() >= 2 {
                    entry.crop_origin = Some([vals[0].round() as u32, vals[1].round() as u32]);
                }
            }
            0xc620 => {
                let vals = read_f64_values(b, endian, ty, count, val_off);
                if vals.len() >= 2 {
                    entry.crop_size = Some([vals[0].round() as u32, vals[1].round() as u32]);
                }
            }
            0xc628 => {
                entry.as_shot_neutral = read_f64_values(b, endian, ty, count, val_off);
            }
            _ => {}
        }
        cursor += 12;
    }
    let next = endian.u32(b, cursor).unwrap_or(0);
    Some((entry, next))
}

fn parse_container(b: &[u8]) -> Result<ParsedRaw, String> {
    if b.len() >= 92 && b.starts_with(b"FUJIFILMCCD-RAW ") {
        let off = u32::from_be_bytes([b[84], b[85], b[86], b[87]]) as usize;
        let len = u32::from_be_bytes([b[88], b[89], b[90], b[91]]) as usize;
        if off < b.len() && len > 0 && off.saturating_add(len) <= b.len() {
            return Ok(ParsedRaw {
                endian: Endian::Big,
                ifds: Vec::new(),
                orientation: 1,
                raf_jpeg: Some((off, len)),
            });
        }
        return Err("cabeçalho Fujifilm RAF inválido (JPEG embutido truncado)".into());
    }

    if b.len() < 8 {
        return Err("cabeçalho RAW/DNG truncado".into());
    }
    let endian = match &b[0..4] {
        [0x49, 0x49, 0x2a, 0x00]
        | [0x49, 0x49, 0x55, 0x00]
        | [0x49, 0x49, 0x52, 0x4f]
        | [0x49, 0x49, 0x52, 0x53] => Endian::Little,
        [0x4d, 0x4d, 0x00, 0x2a] | [0x4d, 0x4d, 0x4f, 0x52] => Endian::Big,
        _ => return Err("assinatura TIFF/DNG/RAW inválida".into()),
    };

    let first_ifd = endian
        .u32(b, 4)
        .ok_or_else(|| "offset IFD0 ausente no cabeçalho RAW/DNG".to_string())?
        as usize;

    let mut queue = vec![first_ifd];
    let mut visited = HashSet::new();
    let mut ifds = Vec::new();
    let mut orientation = 1u16;

    while let Some(off) = queue.pop() {
        if off == 0 || off >= b.len() || !visited.insert(off) || ifds.len() >= 32 {
            continue;
        }
        if let Some((ifd, next)) = parse_ifd(b, endian, off) {
            if let Some(o) = ifd.orientation {
                if (1..=8).contains(&o) && orientation == 1 {
                    orientation = o;
                }
            }
            if next != 0 {
                queue.push(next as usize);
            }
            for &sub in ifd.sub_ifds.iter().rev() {
                if sub != 0 {
                    queue.push(sub as usize);
                }
            }
            ifds.push(ifd);
        }
    }

    if ifds.is_empty() {
        return Err("nenhum diretório IFD válido encontrado no arquivo RAW/DNG".into());
    }

    Ok(ParsedRaw {
        endian,
        ifds,
        orientation,
        raf_jpeg: None,
    })
}

/// Returns `true` when `jpeg` starts with `FF D8` and uses a standard 8-bit baseline/progressive
/// DCT frame (`SOF0` `FF C0` or `SOF2` `FF C2`), excluding 12/16-bit lossless JPEG (`SOF3` `FF C3`)
/// raw sensor tiles.
fn is_baseline_or_progressive_jpeg(jpeg: &[u8]) -> bool {
    if jpeg.len() < 4 || jpeg[0] != 0xff || jpeg[1] != 0xd8 {
        return false;
    }
    let mut i = 2usize;
    while i + 4 <= jpeg.len() {
        if jpeg[i] != 0xff {
            i += 1;
            continue;
        }
        while i < jpeg.len() && jpeg[i] == 0xff {
            i += 1;
        }
        if i >= jpeg.len() {
            break;
        }
        let marker = jpeg[i];
        i += 1;
        if marker == 0xd9 || marker == 0xda {
            break;
        }
        if matches!(marker, 0xd0..=0xd8 | 0x01) {
            continue;
        }
        if matches!(marker, 0xc0..=0xc2) {
            // Precision byte right after 2-byte length
            return jpeg.get(i + 2).copied() == Some(8);
        }
        if marker == 0xc3 {
            // Lossless JPEG raw sensor tile — not a displayable preview
            return false;
        }
        let Some(seg_len) = jpeg
            .get(i..i + 2)
            .map(|s| usize::from(u16::from_be_bytes([s[0], s[1]])))
        else {
            break;
        };
        if seg_len < 2 {
            break;
        }
        i = i.saturating_add(seg_len);
    }
    false
}

fn jpeg_dimensions(jpeg: &[u8]) -> Option<(u32, u32)> {
    if !is_baseline_or_progressive_jpeg(jpeg) {
        return None;
    }
    let reader = image::ImageReader::with_format(std::io::Cursor::new(jpeg), ImageFormat::Jpeg);
    reader.into_dimensions().ok().filter(|&(w, h)| w > 0 && h > 0)
}

fn collect_embedded_jpegs<'a>(b: &'a [u8], parsed: &ParsedRaw) -> Vec<&'a [u8]> {
    let mut candidates: Vec<&'a [u8]> = Vec::new();
    if let Some((off, len)) = parsed.raf_jpeg {
        if let Some(s) = b.get(off..off.saturating_add(len)) {
            if is_baseline_or_progressive_jpeg(s) {
                candidates.push(s);
            }
        }
    }
    for ifd in &parsed.ifds {
        if let Some((off, len)) = ifd.rw2_jpg_offset {
            if let Some(s) = b.get(off..off.saturating_add(len)) {
                if is_baseline_or_progressive_jpeg(s) {
                    candidates.push(s);
                }
            }
        }
        if let (Some(off), Some(len)) = (ifd.jpeg_offset, ifd.jpeg_length) {
            let (off, len) = (off as usize, len as usize);
            if let Some(s) = b.get(off..off.saturating_add(len)) {
                if is_baseline_or_progressive_jpeg(s) {
                    candidates.push(s);
                }
            }
        }
        if matches!(ifd.compression, 6 | 7)
            && ifd.strip_offsets.len() == 1
            && ifd.strip_byte_counts.len() == 1
        {
            let off = ifd.strip_offsets[0] as usize;
            let len = ifd.strip_byte_counts[0] as usize;
            if let Some(s) = b.get(off..off.saturating_add(len)) {
                if is_baseline_or_progressive_jpeg(s) {
                    candidates.push(s);
                }
            }
        }
    }
    candidates.sort_by_key(|s| std::cmp::Reverse(s.len()));
    candidates
}

fn orient_dimensions(w: u32, h: u32, orientation: u16) -> (u32, u32) {
    if matches!(orientation, 5..=8) {
        (h, w)
    } else {
        (w, h)
    }
}

fn apply_orientation(img: image::RgbaImage, orientation: u16) -> image::RgbaImage {
    match orientation {
        2 => image::imageops::flip_horizontal(&img),
        3 => image::imageops::rotate180(&img),
        4 => image::imageops::flip_vertical(&img),
        5 => image::imageops::flip_horizontal(&image::imageops::rotate90(&img)),
        6 => image::imageops::rotate90(&img),
        7 => image::imageops::flip_horizontal(&image::imageops::rotate270(&img)),
        8 => image::imageops::rotate270(&img),
        _ => img,
    }
}

fn select_raw_ifd(parsed: &ParsedRaw) -> Option<&IfdEntry> {
    parsed
        .ifds
        .iter()
        .filter(|i| i.is_uncompressed_raw())
        .max_by_key(|i| {
            let full_bonus = u64::from(i.subfile_type == 0) << 56;
            full_bonus | (u64::from(i.width) * u64::from(i.height))
        })
}

/// Probes pixel dimensions of a DNG or camera RAW file from its header / IFD directory.
pub fn probe_file(path: &Path) -> Result<(u32, u32), String> {
    let bytes = std::fs::read(path)
        .map_err(|e| format!("falha ao abrir arquivo RAW {}: {e}", path.display()))?;
    probe_bytes(&bytes)
}

/// Probes pixel dimensions of a DNG or camera RAW image in memory.
pub fn probe_bytes(bytes: &[u8]) -> Result<(u32, u32), String> {
    let parsed = parse_container(bytes)?;
    // Merge top-level sensor metadata (such as AsShotNeutral) and pick uncompressed RAW IFD first
    if let Some(raw_ifd) = select_raw_ifd(&parsed) {
        let (w, h) = raw_ifd.effective_dimensions();
        if w == 0 || h == 0 || w > MAX_DIMENSION || h > MAX_DIMENSION {
            return Err(format!("dimensões RAW inválidas ({w}x{h})"));
        }
        return Ok(orient_dimensions(w, h, parsed.orientation));
    }
    for jpeg in collect_embedded_jpegs(bytes, &parsed) {
        if let Some((w, h)) = jpeg_dimensions(jpeg) {
            return Ok(orient_dimensions(w, h, parsed.orientation));
        }
    }
    Err("arquivo RAW comprimido sem prévia JPEG decodificável; converta para DNG não comprimido ou PNG/TIFF".into())
}

/// Decodes a DNG or camera RAW file into straight-alpha sRGB RGBA8.
pub fn decode_file(path: &Path) -> Result<RawRaster, String> {
    let bytes = std::fs::read(path)
        .map_err(|e| format!("falha ao ler arquivo RAW {}: {e}", path.display()))?;
    decode_bytes(&bytes)
}

/// Decodes a DNG or camera RAW byte buffer into straight-alpha sRGB RGBA8.
pub fn decode_bytes(bytes: &[u8]) -> Result<RawRaster, String> {
    let parsed = parse_container(bytes)?;

    // AsShotNeutral Often lives in IFD0 while sensor pixels live in SubIFD0.
    let shared_neutral: Vec<f64> = parsed
        .ifds
        .iter()
        .find(|i| i.as_shot_neutral.len() >= 3)
        .map(|i| i.as_shot_neutral.clone())
        .unwrap_or_default();

    if let Some(raw_ifd) = select_raw_ifd(&parsed) {
        let img = decode_uncompressed_ifd(bytes, parsed.endian, raw_ifd, &shared_neutral)?;
        let oriented = apply_orientation(img, parsed.orientation);
        let (width, height) = oriented.dimensions();
        return Ok(RawRaster {
            width,
            height,
            rgba: oriented.into_raw(),
        });
    }

    for jpeg in collect_embedded_jpegs(bytes, &parsed) {
        if let Ok(dyn_img) = image::load_from_memory_with_format(jpeg, ImageFormat::Jpeg) {
            let oriented = apply_orientation(dyn_img.into_rgba8(), parsed.orientation);
            let (width, height) = oriented.dimensions();
            return Ok(RawRaster {
                width,
                height,
                rgba: oriented.into_raw(),
            });
        }
    }

    Err("arquivo RAW comprimido sem prévia JPEG decodificável; converta para DNG não comprimido ou PNG/TIFF".into())
}

fn unpack_samples(raw: &[u8], endian: Endian, bits: u16, expected_count: usize) -> Vec<u16> {
    let mut out = Vec::with_capacity(expected_count);
    match bits {
        8 => {
            for &v in raw.iter().take(expected_count) {
                out.push(u16::from(v));
            }
        }
        16 => {
            for i in 0..expected_count {
                let at = i * 2;
                let Some(v) = endian.u16(raw, at) else { break };
                out.push(v);
            }
        }
        10 | 12 | 14 => {
            // DNG specification packs 10/12/14-bit uncompressed samples MSB-first in a bitstream.
            let b_len = usize::from(bits);
            let mask = (1u32 << b_len) - 1;
            let mut bit_pos = 0usize;
            let total_bits = raw.len().saturating_mul(8);
            while out.len() < expected_count && bit_pos + b_len <= total_bits {
                let byte_idx = bit_pos / 8;
                let bit_off = bit_pos % 8;
                let b0 = u32::from(raw.get(byte_idx).copied().unwrap_or(0));
                let b1 = u32::from(raw.get(byte_idx + 1).copied().unwrap_or(0));
                let b2 = u32::from(raw.get(byte_idx + 2).copied().unwrap_or(0));
                let acc = (b0 << 16) | (b1 << 8) | b2;
                let shift = 24 - bit_off - b_len;
                let val = ((acc >> shift) & mask) as u16;
                out.push(val);
                bit_pos += b_len;
            }
        }
        _ => {}
    }
    out.resize(expected_count, 0);
    out
}

fn read_ifd_samples(
    file_bytes: &[u8],
    endian: Endian,
    ifd: &IfdEntry,
    bits: u16,
    spp: usize,
) -> Result<Vec<u16>, String> {
    let w = ifd.width as usize;
    let h = ifd.height as usize;
    let total_samples = w
        .checked_mul(h)
        .and_then(|p| p.checked_mul(spp))
        .ok_or_else(|| "overflow nas dimensões da imagem RAW".to_string())?;
    let mut dst = vec![0u16; total_samples];

    if !ifd.tile_offsets.is_empty() && ifd.tile_width > 0 && ifd.tile_height > 0 {
        let tw = ifd.tile_width as usize;
        let th = ifd.tile_height as usize;
        let tiles_across = w.div_ceil(tw);
        let tile_samples = tw.saturating_mul(th).saturating_mul(spp);
        for (idx, &off) in ifd.tile_offsets.iter().enumerate() {
            let byte_cnt = ifd
                .tile_byte_counts
                .get(idx)
                .copied()
                .unwrap_or(0) as usize;
            let off = off as usize;
            let Some(slice) = file_bytes.get(off..off.saturating_add(byte_cnt)) else {
                return Err("bloco (tile) DNG fora dos limites do arquivo".into());
            };
            let tile_buf = unpack_samples(slice, endian, bits, tile_samples);
            let tx = (idx % tiles_across) * tw;
            let ty = (idx / tiles_across) * th;
            let copy_w = tw.min(w.saturating_sub(tx));
            let copy_h = th.min(h.saturating_sub(ty));
            for row in 0..copy_h {
                let src_start = row * tw * spp;
                let dst_start = ((ty + row) * w + tx) * spp;
                dst[dst_start..dst_start + copy_w * spp]
                    .copy_from_slice(&tile_buf[src_start..src_start + copy_w * spp]);
            }
        }
        return Ok(dst);
    }

    let rows_per_strip = if ifd.rows_per_strip == 0 || ifd.rows_per_strip as usize > h {
        h
    } else {
        ifd.rows_per_strip as usize
    };
    for (idx, &off) in ifd.strip_offsets.iter().enumerate() {
        let y0 = idx.saturating_mul(rows_per_strip);
        if y0 >= h {
            break;
        }
        let rows = rows_per_strip.min(h - y0);
        let expected = rows * w * spp;
        let default_bytes = (expected * usize::from(bits)).div_ceil(8);
        let byte_cnt = ifd
            .strip_byte_counts
            .get(idx)
            .copied()
            .map(|c| c as usize)
            .unwrap_or(default_bytes);
        let off = off as usize;
        let Some(slice) = file_bytes.get(off..off.saturating_add(byte_cnt)) else {
            return Err("faixa (strip) DNG fora dos limites do arquivo".into());
        };
        let unpacked = unpack_samples(slice, endian, bits, expected);
        let dst_start = y0 * w * spp;
        dst[dst_start..dst_start + expected].copy_from_slice(&unpacked);
    }
    Ok(dst)
}

#[inline]
fn linear_to_srgb_u8(v: f32) -> u8 {
    let c = v.clamp(0.0, 1.0);
    let encoded = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (encoded.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

fn wb_gains(as_shot_neutral: &[f64]) -> [f32; 3] {
    if as_shot_neutral.len() >= 3
        && as_shot_neutral[0] > 1e-4
        && as_shot_neutral[1] > 1e-4
        && as_shot_neutral[2] > 1e-4
    {
        let inv = [
            (1.0 / as_shot_neutral[0]) as f32,
            (1.0 / as_shot_neutral[1]) as f32,
            (1.0 / as_shot_neutral[2]) as f32,
        ];
        let base = inv[0].min(inv[1]).min(inv[2]).max(1e-4);
        [
            (inv[0] / base).clamp(0.25, 4.0),
            (inv[1] / base).clamp(0.25, 4.0),
            (inv[2] / base).clamp(0.25, 4.0),
        ]
    } else {
        [1.0, 1.0, 1.0]
    }
}

fn decode_uncompressed_ifd(
    file_bytes: &[u8],
    endian: Endian,
    ifd: &IfdEntry,
    shared_neutral: &[f64],
) -> Result<image::RgbaImage, String> {
    let w = ifd.width as usize;
    let h = ifd.height as usize;
    if w == 0 || h == 0 || ifd.width > MAX_DIMENSION || ifd.height > MAX_DIMENSION || w * h > MAX_PIXELS
    {
        return Err(format!("dimensões DNG não suportadas ({w}x{h})"));
    }
    if ifd.planar_config != 1 && ifd.samples_per_pixel > 1 {
        return Err("configuração planar DNG separada não suportada".into());
    }
    let bits = ifd.bits_per_sample.first().copied().unwrap_or(16);
    if !matches!(bits, 8 | 10 | 12 | 14 | 16) {
        return Err(format!("profundidade DNG de {bits} bits não suportada"));
    }
    let spp = usize::from(ifd.samples_per_pixel.max(1));
    let raw = read_ifd_samples(file_bytes, endian, ifd, bits, spp)?;

    let max_code = ((1u32 << bits) - 1) as f32;
    let black0 = ifd.black_level.first().copied().unwrap_or(0.0) as f32;
    let white = (ifd.white_level.unwrap_or(f64::from(max_code)) as f32).max(black0 + 1.0);
    let scale = 1.0 / (white - black0);

    let neutral = if !ifd.as_shot_neutral.is_empty() {
        &ifd.as_shot_neutral
    } else {
        shared_neutral
    };
    let gains = wb_gains(neutral);

    let [crop_x, crop_y, crop_w, crop_h] = match (ifd.crop_origin, ifd.crop_size) {
        (origin, Some([cw, ch])) => {
            let [ox, oy] = origin.unwrap_or([0, 0]);
            if cw > 0
                && ch > 0
                && (ox as usize) + (cw as usize) <= w
                && (oy as usize) + (ch as usize) <= h
            {
                [ox as usize, oy as usize, cw as usize, ch as usize]
            } else {
                [0, 0, w, h]
            }
        }
        _ => [0, 0, w, h],
    };

    let mut rgba = vec![255u8; crop_w * crop_h * 4];

    if ifd.photometric == 32803 && spp == 1 {
        // Bayer CFA demosaicing (2x2 pattern; defaults to RGGB `[0, 1, 1, 2]`).
        let cfa_rows = usize::from(ifd.cfa_dim[0].max(1));
        let cfa_cols = usize::from(ifd.cfa_dim[1].max(1));
        let pat: &[u8] = if ifd.cfa_pattern.len() >= cfa_rows * cfa_cols {
            &ifd.cfa_pattern
        } else {
            &[0, 1, 1, 2]
        };
        let color_at = |x: usize, y: usize| -> u8 {
            if ifd.cfa_pattern.len() >= cfa_rows * cfa_cols {
                pat[(y % cfa_rows) * cfa_cols + (x % cfa_cols)]
            } else {
                [0u8, 1, 1, 2][(y & 1) * 2 + (x & 1)]
            }
        };

        let norm: Vec<f32> = raw
            .iter()
            .enumerate()
            .map(|(idx, &v)| {
                let x = idx % w;
                let y = idx / w;
                let b_idx = (y & 1) * 2 + (x & 1);
                let blk = ifd
                    .black_level
                    .get(b_idx)
                    .or_else(|| ifd.black_level.first())
                    .copied()
                    .unwrap_or(0.0) as f32;
                let denom = (white - blk).max(1.0);
                ((f32::from(v) - blk) / denom).clamp(0.0, 1.0)
            })
            .collect();

        let sample = |sx: isize, sy: isize| -> f32 {
            let cx = sx.clamp(0, (w - 1) as isize) as usize;
            let cy = sy.clamp(0, (h - 1) as isize) as usize;
            norm[cy * w + cx]
        };

        for dy in 0..crop_h {
            let y = (crop_y + dy) as isize;
            for dx in 0..crop_w {
                let x = (crop_x + dx) as isize;
                let c = color_at(x as usize, y as usize);
                let center = sample(x, y);
                // 3x3 neighborhood accumulation by CFA colour plane (0=R, 1=G, 2=B)
                let mut sums = [0.0f32; 3];
                let mut cnts = [0u8; 3];
                for oy in -1..=1isize {
                    for ox in -1..=1isize {
                        let nx = (x + ox).clamp(0, (w - 1) as isize) as usize;
                        let ny = (y + oy).clamp(0, (h - 1) as isize) as usize;
                        let nc = usize::from(color_at(nx, ny));
                        if nc < 3 {
                            sums[nc] += norm[ny * w + nx];
                            cnts[nc] += 1;
                        }
                    }
                }
                let avg = |plane: usize| -> f32 {
                    if usize::from(c) == plane {
                        center
                    } else if cnts[plane] > 0 {
                        sums[plane] / f32::from(cnts[plane])
                    } else {
                        center
                    }
                };
                let r = avg(0) * gains[0];
                let g = avg(1) * gains[1];
                let b = avg(2) * gains[2];
                let out_idx = (dy * crop_w + dx) * 4;
                rgba[out_idx] = linear_to_srgb_u8(r);
                rgba[out_idx + 1] = linear_to_srgb_u8(g);
                rgba[out_idx + 2] = linear_to_srgb_u8(b);
                rgba[out_idx + 3] = 255;
            }
        }
    } else {
        // LinearRaw (34892), uncompressed RGB (2), or Monochrome (1)
        let apply_gamma = ifd.photometric == 34892 || bits > 8;
        for dy in 0..crop_h {
            let y = crop_y + dy;
            for dx in 0..crop_w {
                let x = crop_x + dx;
                let base = (y * w + x) * spp;
                let norm_ch = |ch: usize| -> f32 {
                    let v = f32::from(raw.get(base + ch).copied().unwrap_or(0));
                    let blk = ifd
                        .black_level
                        .get(ch)
                        .or_else(|| ifd.black_level.first())
                        .copied()
                        .unwrap_or(0.0) as f32;
                    ((v - blk) / (white - blk).max(1.0)).clamp(0.0, 1.0)
                };
                let (r, g, b) = if spp >= 3 {
                    (
                        norm_ch(0) * gains[0],
                        norm_ch(1) * gains[1],
                        norm_ch(2) * gains[2],
                    )
                } else {
                    let l = (f32::from(raw[base]) - black0) * scale;
                    (l, l, l)
                };
                let out_idx = (dy * crop_w + dx) * 4;
                if apply_gamma {
                    rgba[out_idx] = linear_to_srgb_u8(r);
                    rgba[out_idx + 1] = linear_to_srgb_u8(g);
                    rgba[out_idx + 2] = linear_to_srgb_u8(b);
                } else {
                    rgba[out_idx] = (r.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                    rgba[out_idx + 1] = (g.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                    rgba[out_idx + 2] = (b.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                }
                rgba[out_idx + 3] = 255;
            }
        }
    }

    image::RgbaImage::from_raw(crop_w as u32, crop_h as u32, rgba)
        .ok_or_else(|| "falha ao construir imagem RGBA a partir do buffer RAW".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TiffBuilder {
        buf: Vec<u8>,
    }

    impl TiffBuilder {
        fn new_le() -> Self {
            // "II*\0" + placeholder IFD0 offset at byte 8
            let mut buf = vec![0x49, 0x49, 0x2a, 0x00, 8, 0, 0, 0];
            let _ = &mut buf;
            Self { buf }
        }

        fn write_ifd(&mut self, entries: &[(u16, u16, u32, Vec<u8>)]) -> usize {
            while self.buf.len() % 2 != 0 {
                self.buf.push(0);
            }
            let ifd_off = self.buf.len();
            self.buf[4..8].copy_from_slice(&(ifd_off as u32).to_le_bytes());
            self.buf.extend_from_slice(&(entries.len() as u16).to_le_bytes());
            let table_start = self.buf.len();
            self.buf.resize(table_start + entries.len() * 12 + 4, 0);
            for (idx, (tag, ty, count, data)) in entries.iter().enumerate() {
                let slot = table_start + idx * 12;
                self.buf[slot..slot + 2].copy_from_slice(&tag.to_le_bytes());
                self.buf[slot + 2..slot + 4].copy_from_slice(&ty.to_le_bytes());
                self.buf[slot + 4..slot + 8].copy_from_slice(&count.to_le_bytes());
                if data.len() <= 4 {
                    self.buf[slot + 8..slot + 8 + data.len()].copy_from_slice(data);
                } else {
                    while self.buf.len() % 2 != 0 {
                        self.buf.push(0);
                    }
                    let ext_off = self.buf.len() as u32;
                    self.buf.extend_from_slice(data);
                    self.buf[slot + 8..slot + 12].copy_from_slice(&ext_off.to_le_bytes());
                }
            }
            ifd_off
        }
    }

    fn u16_entry(tag: u16, v: u16) -> (u16, u16, u32, Vec<u8>) {
        (tag, 3, 1, v.to_le_bytes().to_vec())
    }

    fn u32_entry(tag: u16, v: u32) -> (u16, u16, u32, Vec<u8>) {
        (tag, 4, 1, v.to_le_bytes().to_vec())
    }

    #[test]
    fn decodes_uncompressed_16bit_bayer_rggb_dng() {
        let (w, h) = (8u32, 8u32);
        // Pure saturated red under RGGB: R (even x, even y) = 4095, G & B = 0
        let mut pixels = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let is_red = (x % 2 == 0) && (y % 2 == 0);
                let sample: u16 = if is_red { 4095 } else { 0 };
                pixels.extend_from_slice(&sample.to_le_bytes());
            }
        }
        let mut tb = TiffBuilder::new_le();
        let pixel_off = 256u32;
        tb.write_ifd(&[
            u32_entry(0x00fe, 0),
            u32_entry(0x0100, w),
            u32_entry(0x0101, h),
            u16_entry(0x0102, 16),
            u16_entry(0x0103, 1),
            u16_entry(0x0106, 32803), // CFA
            u32_entry(0x0111, pixel_off),
            u16_entry(0x0115, 1),
            u32_entry(0x0116, h),
            u32_entry(0x0117, pixels.len() as u32),
            (0x828d, 3, 2, [2u16.to_le_bytes(), 2u16.to_le_bytes()].concat()),
            (0x828e, 1, 4, vec![0, 1, 1, 2]),
            u32_entry(0xc61d, 4095), // WhiteLevel = 4095
        ]);
        if tb.buf.len() < pixel_off as usize {
            tb.buf.resize(pixel_off as usize, 0);
        }
        tb.buf.extend_from_slice(&pixels);

        assert_eq!(probe_bytes(&tb.buf).unwrap(), (8, 8));
        let raster = decode_bytes(&tb.buf).unwrap();
        assert_eq!((raster.width, raster.height), (8, 8));
        // Interior pixel (2, 2): Red = 255, Green = 0, Blue = 0
        let idx = (2 * 8 + 2) * 4;
        assert_eq!(raster.rgba[idx], 255);
        assert_eq!(raster.rgba[idx + 1], 0);
        assert_eq!(raster.rgba[idx + 2], 0);
        assert_eq!(raster.rgba[idx + 3], 255);
    }

    #[test]
    fn decodes_12bit_packed_bayer_and_orientation_swap() {
        // 4x6 sensor rotated 90 CW (Orientation = 6) -> output 6x4
        let (w, h) = (4u32, 6u32);
        let total = (w * h) as usize;
        // Pack 24 samples of 12 bits = 36 bytes, all at full white (4095 = 0xFFF -> all 0xFF bytes)
        let packed = vec![0xffu8; (total * 12) / 8];
        let mut tb = TiffBuilder::new_le();
        let pixel_off = 256u32;
        tb.write_ifd(&[
            u32_entry(0x0100, w),
            u32_entry(0x0101, h),
            u16_entry(0x0102, 12),
            u16_entry(0x0103, 1),
            u16_entry(0x0106, 32803),
            u32_entry(0x0111, pixel_off),
            u16_entry(0x0112, 6), // 90 CW
            u16_entry(0x0115, 1),
            u32_entry(0x0116, h),
            u32_entry(0x0117, packed.len() as u32),
        ]);
        tb.buf.resize(pixel_off as usize, 0);
        tb.buf.extend_from_slice(&packed);

        assert_eq!(probe_bytes(&tb.buf).unwrap(), (6, 4));
        let raster = decode_bytes(&tb.buf).unwrap();
        assert_eq!((raster.width, raster.height), (6, 4));
        assert!(raster.rgba.chunks_exact(4).all(|px| px == [255, 255, 255, 255]));
    }

    #[test]
    fn extracts_embedded_jpeg_from_fujifilm_raf_and_camera_raw() {
        let src = image::RgbaImage::from_pixel(12, 8, image::Rgba([40, 180, 90, 255]));
        let mut jpg = Vec::new();
        image::DynamicImage::ImageRgba8(src)
            .write_to(&mut std::io::Cursor::new(&mut jpg), ImageFormat::Jpeg)
            .unwrap();

        let mut raf = vec![0u8; 96];
        raf[..16].copy_from_slice(b"FUJIFILMCCD-RAW ");
        raf[84..88].copy_from_slice(&96u32.to_be_bytes());
        raf[88..92].copy_from_slice(&(jpg.len() as u32).to_be_bytes());
        raf.extend_from_slice(&jpg);

        assert_eq!(probe_bytes(&raf).unwrap(), (12, 8));
        let decoded = decode_bytes(&raf).unwrap();
        assert_eq!((decoded.width, decoded.height), (12, 8));
        assert!(decoded.rgba[1] > 140);
    }

    #[test]
    fn proprietary_cinema_raw_reports_clear_vendor_sdk_diagnostic() {
        for name in ["clip.r3d", "take01.BRAW", "A001.crm", "scene.ari"] {
            let p = Path::new(name);
            assert!(is_proprietary_raw_path(p), "{name}");
            let msg = proprietary_raw_error(p).expect("diagnostic present");
            assert!(msg.contains("SDK proprietário"), "{msg}");
        }
        assert!(!is_proprietary_raw_path(Path::new("frame_0001.dng")));
    }
}
