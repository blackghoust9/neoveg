//! The individual layers of a `.psd`: each one with its name, its position on the canvas, its
//! opacity and visibility and its own RGBA8 picture (straight alpha), so every layer can go to a
//! track of its own. Reads 8 and 16 bits per channel, grayscale or RGB, raw or PackBits RLE.

use super::{be16, be32, err, parse_header, section, unpack, unsupported, Header};

/// One layer of a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsdLayer {
    pub name: String,
    /// Position of the top-left corner on the canvas (may be negative: the layer hangs off it).
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
    /// 0 to 255.
    pub opacity: u8,
    pub visible: bool,
    /// `width * height * 4` bytes, straight alpha.
    pub rgba: Vec<u8>,
}

struct Record {
    top: i32,
    left: i32,
    bottom: i32,
    right: i32,
    /// `(channel id, data length)` of each channel.
    channels: Vec<(i16, usize)>,
    opacity: u8,
    visible: bool,
    name: String,
}

fn i32_at(b: &[u8], at: usize) -> Option<i32> {
    be32(b, at).map(|v| v as i32)
}

/// Reads one layer record at `pos`; returns it with the position after it.
fn record(name: &str, b: &[u8], mut pos: usize) -> Result<(Record, usize), String> {
    let trunc = || err(name, "registro de camada truncado");
    let top = i32_at(b, pos).ok_or_else(trunc)?;
    let left = i32_at(b, pos + 4).ok_or_else(trunc)?;
    let bottom = i32_at(b, pos + 8).ok_or_else(trunc)?;
    let right = i32_at(b, pos + 12).ok_or_else(trunc)?;
    let count = usize::from(be16(b, pos + 16).ok_or_else(trunc)?);
    pos += 18;
    let mut channels = Vec::with_capacity(count);
    for _ in 0..count {
        let id = be16(b, pos).ok_or_else(trunc)? as i16;
        let len = be32(b, pos + 2).ok_or_else(trunc)? as usize;
        channels.push((id, len));
        pos += 6;
    }
    if b.get(pos..pos + 4) != Some(b"8BIM".as_slice()) {
        return Err(err(name, "assinatura de mesclagem da camada ausente"));
    }
    // blend key (4), opacity, clipping, flags, filler
    let opacity = *b.get(pos + 8).ok_or_else(trunc)?;
    let flags = *b.get(pos + 10).ok_or_else(trunc)?;
    let extra_len = be32(b, pos + 12).ok_or_else(trunc)? as usize;
    let extra = pos + 16;
    let end = extra.checked_add(extra_len).ok_or_else(trunc)?;
    if end > b.len() {
        return Err(trunc());
    }
    // Layer mask data, blending ranges, then the Pascal name padded to a multiple of 4
    let mask_len = be32(b, extra).ok_or_else(trunc)? as usize;
    let ranges_at = extra.checked_add(4 + mask_len).ok_or_else(trunc)?;
    let ranges_len = be32(b, ranges_at).ok_or_else(trunc)? as usize;
    let name_at = ranges_at.checked_add(4 + ranges_len).ok_or_else(trunc)?;
    let name_len = usize::from(*b.get(name_at).ok_or_else(trunc)?);
    let pascal = b
        .get(name_at + 1..name_at + 1 + name_len)
        .ok_or_else(trunc)?;
    let mut layer_name = String::from_utf8_lossy(pascal).into_owned();
    // Additional blocks: the Unicode name (`luni`) beats the Pascal one
    let mut p = name_at + ((1 + name_len + 3) & !3);
    while p + 12 <= end {
        let key = &b[p + 4..p + 8];
        let len = be32(b, p + 8).ok_or_else(trunc)? as usize;
        let data = p + 12;
        if key == b"luni" {
            if let Some(chars) = be32(b, data) {
                let units: Vec<u16> = (0..chars as usize)
                    .map_while(|i| be16(b, data + 4 + i * 2))
                    .collect();
                let text = String::from_utf16_lossy(&units);
                let text = text.trim_end_matches('\0');
                if !text.is_empty() {
                    layer_name = text.to_owned();
                }
            }
        }
        p = data.saturating_add((len + 1) & !1);
    }
    Ok((
        Record {
            top,
            left,
            bottom,
            right,
            channels,
            opacity,
            // Bit 1 of the flags set means hidden
            visible: flags & 2 == 0,
            name: layer_name,
        },
        end,
    ))
}

/// Decodes one channel (`compression` word included in `data`) of a `width` x `height` layer into
/// 8-bit samples.
fn channel(
    name: &str,
    data: &[u8],
    width: usize,
    height: usize,
    bytes: usize,
) -> Result<Vec<u8>, String> {
    let compression = be16(data, 0).ok_or_else(|| err(name, "canal de camada truncado"))?;
    let body = &data[2..];
    let row_bytes = width * bytes;
    let mut raw = vec![0u8; row_bytes * height];
    match compression {
        0 => {
            let src = body
                .get(..raw.len())
                .ok_or_else(|| err(name, "canal de camada truncado"))?;
            raw.copy_from_slice(src);
        }
        1 => {
            let mut at = height * 2;
            for y in 0..height {
                let len =
                    usize::from(be16(body, y * 2).ok_or_else(|| err(name, "tabela RLE truncada"))?);
                let src = body
                    .get(at..at + len)
                    .ok_or_else(|| err(name, "canal de camada truncado"))?;
                unpack(name, src, &mut raw[y * row_bytes..(y + 1) * row_bytes])?;
                at += len;
            }
        }
        2 | 3 => return Err(unsupported(name, "compressão ZIP")),
        c => return Err(err(name, &format!("compressão desconhecida {c}"))),
    }
    // 16 bits: round to 8
    Ok(if bytes == 2 {
        raw.as_chunks::<2>()
            .0
            .iter()
            .map(|s| {
                let v = u32::from(u16::from_be_bytes([s[0], s[1]]));
                ((v * 255 + 32_767) / 65_535) as u8
            })
            .collect()
    } else {
        raw
    })
}

/// Every layer of the document with pixels, bottom layer first (the order of the file). Empty
/// layers (groups, dividers) are left out.
pub fn decode_layers_bytes(name: &str, b: &[u8]) -> Result<Vec<PsdLayer>, String> {
    let hdr: Header = parse_header(name, b)?;
    let (_, pos) = section(name, b, 26)?;
    let (_, pos) = section(name, b, pos)?;
    let (mask_section, _) = section(name, b, pos)?;
    // Layer info: length, signed count
    let info_len =
        be32(mask_section, 0).ok_or_else(|| err(name, "seção de camadas truncada"))? as usize;
    if info_len < 2 {
        return Ok(Vec::new());
    }
    let info = mask_section
        .get(4..4usize.saturating_add(info_len))
        .ok_or_else(|| err(name, "seção de camadas truncada"))?;
    let count = (be16(info, 0).ok_or_else(|| err(name, "seção de camadas truncada"))? as i16)
        .unsigned_abs() as usize;
    if count > 8_192 {
        return Err(err(name, "camadas demais"));
    }
    let mut records = Vec::with_capacity(count);
    let mut at = 2;
    for _ in 0..count {
        let (r, next) = record(name, info, at)?;
        records.push(r);
        at = next;
    }
    let mut layers = Vec::new();
    for r in records {
        let (w, h) = (
            i64::from(r.right) - i64::from(r.left),
            i64::from(r.bottom) - i64::from(r.top),
        );
        let total: usize = r.channels.iter().map(|c| c.1).sum();
        let data = info
            .get(at..at.saturating_add(total))
            .ok_or_else(|| err(name, "dados de camada truncados"))?;
        at += total;
        if w <= 0 || h <= 0 {
            continue;
        }
        let (w, h) = (w as usize, h as usize);
        if w.checked_mul(h).is_none_or(|n| n > super::MAX_PIXELS) {
            return Err(err(name, "camada grande demais"));
        }
        let mut planes: Vec<(i16, Vec<u8>)> = Vec::new();
        let mut offset = 0;
        for &(id, len) in &r.channels {
            let part = &data[offset..offset + len];
            offset += len;
            if matches!(id, -1..=2) {
                planes.push((id, channel(name, part, w, h, hdr.bytes_per_sample)?));
            }
        }
        let plane = |id: i16| planes.iter().find(|p| p.0 == id).map(|p| &p.1);
        let colour = if hdr.colours == 1 {
            [plane(0), plane(0), plane(0)]
        } else {
            [plane(0), plane(1), plane(2)]
        };
        if colour.iter().any(Option::is_none) {
            return Err(err(name, "camada sem os canais de cor"));
        }
        let alpha = plane(-1);
        let mut rgba = vec![255u8; w * h * 4];
        for (i, px) in rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            for (c, p) in colour.iter().enumerate() {
                px[c] = p.map_or(0, |p| p[i]);
            }
            if let Some(a) = alpha {
                px[3] = a[i];
            }
        }
        layers.push(PsdLayer {
            name: r.name,
            left: r.left,
            top: r.top,
            width: w as u32,
            height: h as u32,
            opacity: r.opacity,
            visible: r.visible,
            rgba,
        });
    }
    Ok(layers)
}

/// Reads the layers of a `.psd` file from disk.
pub fn decode_layers_file(path: &std::path::Path) -> Result<Vec<PsdLayer>, String> {
    let bytes =
        std::fs::read(path).map_err(|e| format!("falha ao ler PSD {}: {e}", path.display()))?;
    decode_layers_bytes(&path.display().to_string(), &bytes)
}

/// Lays `layer` on a transparent canvas of `canvas` size, clipped to it: the picture that goes on
/// the layer's own track.
pub fn on_canvas(layer: &PsdLayer, canvas: (u32, u32)) -> Vec<u8> {
    let mut out = vec![0u8; canvas.0 as usize * canvas.1 as usize * 4];
    for y in 0..i64::from(layer.height) {
        let cy = y + i64::from(layer.top);
        if cy < 0 || cy >= i64::from(canvas.1) {
            continue;
        }
        for x in 0..i64::from(layer.width) {
            let cx = x + i64::from(layer.left);
            if cx < 0 || cx >= i64::from(canvas.0) {
                continue;
            }
            let from = (y as usize * layer.width as usize + x as usize) * 4;
            let to = (cy as usize * canvas.0 as usize + cx as usize) * 4;
            out[to..to + 4].copy_from_slice(&layer.rgba[from..from + 4]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    struct L<'a> {
        name: &'a str,
        unicode: Option<&'a str>,
        /// top, left, bottom, right
        rect: (i32, i32, i32, i32),
        opacity: u8,
        hidden: bool,
        rgb: [u8; 3],
        alpha: u8,
        rle: bool,
    }

    fn packbits_literal(row: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        for chunk in row.chunks(128) {
            out.push((chunk.len() - 1) as u8);
            out.extend_from_slice(chunk);
        }
        out
    }

    fn channel_data(value: u8, w: usize, h: usize, rle: bool) -> Vec<u8> {
        if !rle {
            let mut d = vec![0, 0];
            d.extend(std::iter::repeat_n(value, w * h));
            return d;
        }
        let rows: Vec<Vec<u8>> = (0..h).map(|_| packbits_literal(&vec![value; w])).collect();
        let mut d = vec![0, 1];
        for r in &rows {
            d.extend((r.len() as u16).to_be_bytes());
        }
        for r in &rows {
            d.extend(r);
        }
        d
    }

    /// An 8-bit RGB document of `canvas` size whose layers are built in memory.
    fn build(canvas: (u32, u32), layers: &[L]) -> Vec<u8> {
        let mut b = b"8BPS".to_vec();
        b.extend(1u16.to_be_bytes());
        b.extend([0; 6]);
        b.extend(3u16.to_be_bytes());
        b.extend(canvas.1.to_be_bytes());
        b.extend(canvas.0.to_be_bytes());
        b.extend(8u16.to_be_bytes());
        b.extend(3u16.to_be_bytes());
        b.extend(0u32.to_be_bytes()); // colour mode data
        b.extend(0u32.to_be_bytes()); // image resources
        let mut records = Vec::new();
        let mut pixels = Vec::new();
        for l in layers {
            let (w, h) = (
                (l.rect.3 - l.rect.1).max(0) as usize,
                (l.rect.2 - l.rect.0).max(0) as usize,
            );
            let chans: Vec<(i16, Vec<u8>)> = if w * h == 0 {
                vec![]
            } else {
                vec![
                    (-1, channel_data(l.alpha, w, h, l.rle)),
                    (0, channel_data(l.rgb[0], w, h, l.rle)),
                    (1, channel_data(l.rgb[1], w, h, l.rle)),
                    (2, channel_data(l.rgb[2], w, h, l.rle)),
                ]
            };
            for v in [l.rect.0, l.rect.1, l.rect.2, l.rect.3] {
                records.extend(v.to_be_bytes());
            }
            records.extend((chans.len() as u16).to_be_bytes());
            for (id, d) in &chans {
                records.extend(id.to_be_bytes());
                records.extend((d.len() as u32).to_be_bytes());
            }
            records.extend(b"8BIMnorm");
            records.extend([l.opacity, 0, if l.hidden { 2 } else { 0 }, 0]);
            let mut extra = Vec::new();
            extra.extend(0u32.to_be_bytes()); // mask
            extra.extend(0u32.to_be_bytes()); // blending ranges
            let mut pascal = vec![l.name.len() as u8];
            pascal.extend(l.name.as_bytes());
            while pascal.len() % 4 != 0 {
                pascal.push(0);
            }
            extra.extend(pascal);
            if let Some(u) = l.unicode {
                let units: Vec<u16> = u.encode_utf16().collect();
                let mut block = (units.len() as u32).to_be_bytes().to_vec();
                for unit in &units {
                    block.extend(unit.to_be_bytes());
                }
                if block.len() % 2 == 1 {
                    block.push(0);
                }
                extra.extend(b"8BIMluni");
                extra.extend((block.len() as u32).to_be_bytes());
                extra.extend(block);
            }
            records.extend((extra.len() as u32).to_be_bytes());
            records.extend(extra);
            for (_, d) in chans {
                pixels.extend(d);
            }
        }
        let mut info = (layers.len() as i16).to_be_bytes().to_vec();
        info.extend(records);
        info.extend(pixels);
        if info.len() % 2 == 1 {
            info.push(0);
        }
        let mut section = (info.len() as u32).to_be_bytes().to_vec();
        section.extend(info);
        section.extend(0u32.to_be_bytes()); // global mask
        b.extend((section.len() as u32).to_be_bytes());
        b.extend(section);
        b
    }

    fn layer(name: &str, rect: (i32, i32, i32, i32), rgb: [u8; 3]) -> L<'_> {
        L {
            name,
            unicode: None,
            rect,
            opacity: 255,
            hidden: false,
            rgb,
            alpha: 255,
            rle: false,
        }
    }

    #[test]
    fn each_layer_comes_out_with_its_name_offset_size_and_pixels() {
        let psd = build(
            (8, 6),
            &[
                layer("Fundo", (0, 0, 6, 8), [10, 20, 30]),
                L {
                    opacity: 128,
                    alpha: 200,
                    rle: true,
                    ..layer("Logo", (1, 2, 3, 5), [200, 100, 50])
                },
            ],
        );
        let layers = decode_layers_bytes("teste.psd", &psd).unwrap();
        assert_eq!(layers.len(), 2);
        let bg = &layers[0];
        assert_eq!(
            (bg.name.as_str(), bg.left, bg.top, bg.width, bg.height),
            ("Fundo", 0, 0, 8, 6)
        );
        assert_eq!(bg.rgba.len(), 8 * 6 * 4);
        assert_eq!(bg.rgba[..4], [10, 20, 30, 255]);
        let logo = &layers[1];
        assert_eq!(
            (
                logo.name.as_str(),
                logo.left,
                logo.top,
                logo.width,
                logo.height
            ),
            ("Logo", 2, 1, 3, 2)
        );
        assert_eq!(logo.opacity, 128);
        assert!(logo.visible);
        assert_eq!(logo.rgba[..4], [200, 100, 50, 200]);
        assert_eq!(logo.rgba.len(), 3 * 2 * 4);
    }

    #[test]
    fn unicode_names_hidden_layers_and_empty_groups_are_handled() {
        let psd = build(
            (4, 4),
            &[
                L {
                    unicode: Some("Título ção"),
                    hidden: true,
                    ..layer("Titulo", (0, 0, 2, 2), [1, 2, 3])
                },
                layer("</Layer group>", (0, 0, 0, 0), [0, 0, 0]),
                layer("Depois", (2, 2, 4, 4), [4, 5, 6]),
            ],
        );
        let layers = decode_layers_bytes("teste.psd", &psd).unwrap();
        assert_eq!(layers.len(), 2, "the empty group is left out");
        assert_eq!(layers[0].name, "Título ção");
        assert!(!layers[0].visible);
        assert_eq!(layers[1].name, "Depois");
        assert_eq!(
            layers[1].rgba[..3],
            [4, 5, 6],
            "pixels stay with their layer after a skipped one"
        );
    }

    #[test]
    fn a_layer_is_laid_on_the_canvas_and_clipped_at_its_edges() {
        let psd = build((4, 4), &[layer("Meio", (-1, 3, 2, 6), [9, 9, 9])]);
        let layer = &decode_layers_bytes("t.psd", &psd).unwrap()[0];
        assert_eq!((layer.left, layer.top), (3, -1));
        let canvas = on_canvas(layer, (4, 4));
        // Only column 3, rows 0..2 are inside
        let px = |x: usize, y: usize| canvas[(y * 4 + x) * 4..(y * 4 + x) * 4 + 4].to_vec();
        assert_eq!(px(3, 0), [9, 9, 9, 255]);
        assert_eq!(px(3, 1), [9, 9, 9, 255]);
        assert_eq!(px(3, 2), [0, 0, 0, 0]);
        assert_eq!(px(2, 0), [0, 0, 0, 0]);
    }

    #[test]
    fn documents_without_layers_and_hostile_cuts_never_panic() {
        let none = build((4, 4), &[]);
        assert!(decode_layers_bytes("t.psd", &none).unwrap().is_empty());
        let psd = build(
            (8, 6),
            &[
                layer("A", (0, 0, 6, 8), [1, 2, 3]),
                L {
                    rle: true,
                    ..layer("B", (0, 0, 2, 2), [1, 2, 3])
                },
            ],
        );
        for cut in 0..psd.len() {
            let _ = decode_layers_bytes("cortado.psd", &psd[..cut]);
        }
    }
}
