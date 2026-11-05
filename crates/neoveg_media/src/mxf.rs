//! Clean-room SMPTE ST 377-1 / ST 382 MXF KLV container reader and uncompressed PCM/BWF/AES3
//! audio decoder.
//!
//! Professional broadcast and cinema cameras (XDCAM HD422, XAVC, Canon XF, DNxHD/DNxHR, ProRes in
//! OP1a/OP-Atom MXF) wrap compressed picture streams alongside uncompressed 16/24/32-bit PCM or
//! AES3 sound tracks. Picture decoding uses FFmpeg when `--features ffmpeg` is enabled; this module
//! probes MXF structural metadata (resolution, frame rate, duration, audio tracks) and decodes
//! uncompressed Wave/BWF, AIFF and AES3 sound essence directly in pure Rust.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// SMPTE Universal Label prefix (`06.0E.2B.34`).
const SMPTE_UL_PREFIX: [u8; 4] = [0x06, 0x0e, 0x2b, 0x34];

/// Maximum run-in allowed before the MXF Header Partition Pack by SMPTE ST 377-1 (64 KiB).
const MAX_RUN_IN: usize = 65_536;

/// Safety cap on a single KLV value allocation when reading metadata or sound packets (64 MiB).
const MAX_PACKET_BYTES: u64 = 64 * 1024 * 1024;

/// Summary of an MXF container probed from its Header/Footer Partition structural metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct MxfInfo {
    pub has_video: bool,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub duration_seconds: Option<f64>,
    pub has_audio: bool,
    pub audio_sample_rate: u32,
    pub audio_channels: u16,
    pub audio_bits: u16,
}

/// Decoded interleaved `f32` PCM extracted from an MXF container.
#[derive(Debug, Clone)]
pub struct MxfAudio {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SoundEncoding {
    WaveLe,
    AiffBe,
    Aes3Raw,
    Smpte331m,
}

#[derive(Debug, Clone, Copy)]
struct SoundDesc {
    sample_rate: u32,
    channels: u16,
    bits: u16,
    encoding: SoundEncoding,
}

/// Returns `true` when `data` begins with an MXF Partition Pack key (allowing up to 64 KiB run-in).
pub fn sniff_mxf(data: &[u8]) -> bool {
    find_partition_offset(data).is_some()
}

/// Returns `true` when `path` has extension `.mxf` (case-insensitive).
pub fn is_mxf_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("mxf"))
}

fn find_partition_offset(data: &[u8]) -> Option<usize> {
    let limit = data.len().min(MAX_RUN_IN + 16);
    if limit < 16 {
        return None;
    }
    // Fast path at byte 0 (almost all camera and NLE MXF files have zero run-in).
    if is_partition_pack_key(&data[..16]) {
        return Some(0);
    }
    let end = limit - 16;
    for i in 1..=end {
        if is_partition_pack_key(&data[i..i + 16]) {
            return Some(i);
        }
    }
    None
}

fn is_partition_pack_key(key: &[u8]) -> bool {
    key.len() >= 16
        && key[0..4] == SMPTE_UL_PREFIX
        && key[4] == 0x02
        && key[5] == 0x05
        && key[6] == 0x01
        && key[8] == 0x0d
        && key[9] == 0x01
        && key[10] == 0x02
        && key[11] == 0x01
        && key[12] == 0x01
        && matches!(key[13], 0x02..=0x04)
}

fn read_ber_length<R: Read>(reader: &mut R) -> Result<Option<u64>, String> {
    let mut first = [0u8; 1];
    match reader.read_exact(&mut first) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(format!("falha ao ler comprimento BER no MXF: {e}")),
    }
    let b0 = first[0];
    if b0 < 0x80 {
        return Ok(Some(u64::from(b0)));
    }
    if b0 == 0x80 {
        return Err("comprimento BER indefinido inválido no MXF".into());
    }
    let count = usize::from(b0 & 0x7f);
    if count > 8 {
        return Err(format!("comprimento BER de {count} bytes excede 64 bits no MXF"));
    }
    let mut buf = [0u8; 8];
    reader
        .read_exact(&mut buf[8 - count..])
        .map_err(|e| format!("comprimento BER truncado no MXF: {e}"))?;
    Ok(Some(u64::from_be_bytes(buf)))
}

fn read_klv_header<R: Read>(reader: &mut R) -> Result<Option<([u8; 16], u64)>, String> {
    let mut key = [0u8; 16];
    let mut filled = 0usize;
    while filled < 16 {
        match reader.read(&mut key[filled..]) {
            Ok(0) if filled == 0 => return Ok(None),
            Ok(0) => return Err("chave KLV truncada no MXF".into()),
            Ok(n) => filled += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(format!("erro de leitura no MXF: {e}")),
        }
    }
    if key[0..4] != SMPTE_UL_PREFIX {
        return Err("fluxo KLV fora de sincronia no MXF (prefixo SMPTE ausente)".into());
    }
    let Some(len) = read_ber_length(reader)? else {
        return Err("comprimento KLV ausente no MXF".into());
    };
    Ok(Some((key, len)))
}

fn seek_to_first_partition<R: Read + Seek>(reader: &mut R) -> Result<u64, String> {
    reader
        .seek(SeekFrom::Start(0))
        .map_err(|e| format!("falha ao posicionar início do MXF: {e}"))?;
    let mut head = vec![0u8; MAX_RUN_IN + 16];
    let mut read_total = 0usize;
    while read_total < head.len() {
        match reader.read(&mut head[read_total..]) {
            Ok(0) => break,
            Ok(n) => read_total += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(format!("falha ao inspecionar cabeçalho MXF: {e}")),
        }
    }
    let offset = find_partition_offset(&head[..read_total])
        .ok_or_else(|| "arquivo não contém cabeçalho MXF SMPTE ST 377 válido".to_string())?
        as u64;
    reader
        .seek(SeekFrom::Start(offset))
        .map_err(|e| format!("falha ao posicionar partição MXF: {e}"))?;
    Ok(offset)
}

fn parse_rational_be(bytes: &[u8]) -> Option<f64> {
    if bytes.len() < 8 {
        return None;
    }
    let num = i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let den = i32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    if num > 0 && den > 0 {
        Some(f64::from(num) / f64::from(den))
    } else {
        None
    }
}

fn parse_u32_be(bytes: &[u8]) -> Option<u32> {
    if bytes.len() < 4 {
        return None;
    }
    Some(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn parse_i64_be(bytes: &[u8]) -> Option<i64> {
    if bytes.len() < 8 {
        return None;
    }
    Some(i64::from_be_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ]))
}

fn is_local_set_key(key: &[u8; 16]) -> bool {
    key[4] == 0x02
        && key[5] == 0x53
        && key[8] == 0x0d
        && key[9] == 0x01
        && key[10] == 0x01
        && key[11] == 0x01
}

fn is_sound_essence_key(key: &[u8; 16]) -> Option<([u8; 4], SoundEncoding)> {
    if key[4] != 0x01
        || key[5] != 0x02
        || key[6] != 0x01
        || key[8] != 0x0d
        || key[9] != 0x01
        || key[10] != 0x03
        || key[11] != 0x01
    {
        return None;
    }
    let track_id = [key[12], key[13], key[14], key[15]];
    match key[12] {
        // CP Sound Item (SMPTE 326M / 331M D-10 AES3)
        0x06 => Some((track_id, SoundEncoding::Smpte331m)),
        // GC Sound Item (SMPTE ST 382 Wave / AIFF / AES3)
        0x16 => {
            let enc = match key[14] {
                0x01 | 0x02 => SoundEncoding::WaveLe,
                0x03 | 0x04 => SoundEncoding::AiffBe,
                0x08..=0x0a => SoundEncoding::Aes3Raw,
                0x10 => SoundEncoding::Smpte331m,
                // Compressed sound in MXF (e.g. MPEG/Dolby E) is not raw PCM
                _ => return None,
            };
            Some((track_id, enc))
        }
        _ => None,
    }
}

fn inspect_metadata_set(
    key: &[u8; 16],
    payload: &[u8],
    info: &mut MxfInfo,
    sound_descs: &mut Vec<SoundDesc>,
    picture_edit_rate: &mut Option<f64>,
    picture_duration_units: &mut Option<i64>,
    audio_duration_units: &mut Option<(f64, i64)>,
) {
    let mut sample_rate_rational: Option<f64> = None;
    let mut container_duration: Option<i64> = None;
    let mut stored_w: Option<u32> = None;
    let mut stored_h: Option<u32> = None;
    let mut display_w: Option<u32> = None;
    let mut display_h: Option<u32> = None;
    let mut audio_rate: Option<f64> = None;
    let mut channels: Option<u32> = None;
    let mut quant_bits: Option<u32> = None;

    let mut pos = 0usize;
    while pos + 4 <= payload.len() {
        let tag = u16::from_be_bytes([payload[pos], payload[pos + 1]]);
        let len = usize::from(u16::from_be_bytes([payload[pos + 2], payload[pos + 3]]));
        pos += 4;
        if pos + len > payload.len() {
            break;
        }
        let val = &payload[pos..pos + len];
        pos += len;
        match tag {
            0x3001 => sample_rate_rational = parse_rational_be(val),
            0x3002 => container_duration = parse_i64_be(val).filter(|&d| d > 0),
            0x3203 => stored_w = parse_u32_be(val).filter(|&v| v > 0),
            0x3202 => stored_h = parse_u32_be(val).filter(|&v| v > 0),
            0x3209 => display_w = parse_u32_be(val).filter(|&v| v > 0),
            0x3208 => display_h = parse_u32_be(val).filter(|&v| v > 0),
            0x3d03 => audio_rate = parse_rational_be(val),
            0x3d07 => channels = parse_u32_be(val).filter(|&v| v > 0),
            0x3d01 => quant_bits = parse_u32_be(val).filter(|&v| v > 0),
            _ => {}
        }
    }

    let class_byte = key[14];
    let is_picture_desc = (key[12] == 0x01
        && key[13] == 0x01
        && matches!(class_byte, 0x24..=0x29 | 0x51 | 0x5c))
        || (stored_w.is_some() && stored_h.is_some());

    if is_picture_desc {
        let w = display_w.or(stored_w).unwrap_or(0);
        let h = display_h.or(stored_h).unwrap_or(0);
        if w > 0 && h > 0 {
            info.has_video = true;
            info.width = w;
            info.height = h;
        }
        if let Some(rate) = sample_rate_rational {
            info.fps = rate;
            *picture_edit_rate = Some(rate);
        }
        if let Some(dur) = container_duration {
            *picture_duration_units = Some(dur);
        }
    }

    let is_sound_desc = (key[12] == 0x01
        && key[13] == 0x01
        && matches!(class_byte, 0x42 | 0x47 | 0x48))
        || audio_rate.is_some()
        || quant_bits.is_some();

    if is_sound_desc {
        let sr = audio_rate
            .or(sample_rate_rational)
            .map(|r| r.round() as u32)
            .unwrap_or(48_000);
        let ch = channels.unwrap_or(1).min(64) as u16;
        let bits = quant_bits.unwrap_or(16).min(32) as u16;
        let encoding = if class_byte == 0x47 {
            SoundEncoding::Aes3Raw
        } else {
            SoundEncoding::WaveLe
        };
        sound_descs.push(SoundDesc {
            sample_rate: sr,
            channels: ch,
            bits,
            encoding,
        });
        info.has_audio = true;
        if info.audio_sample_rate == 0 {
            info.audio_sample_rate = sr;
        }
        if info.audio_bits == 0 {
            info.audio_bits = bits;
        }
        if let (Some(edit_rate), Some(dur)) = (sample_rate_rational.or(audio_rate), container_duration)
        {
            if audio_duration_units.is_none() {
                *audio_duration_units = Some((edit_rate, dur));
            }
        }
    }
}

/// Probes structural metadata of an MXF file without decoding compressed video frames.
pub fn probe_file(path: &Path) -> Result<MxfInfo, String> {
    let mut file = File::open(path).map_err(|e| format!("falha ao abrir MXF {}: {e}", path.display()))?;
    probe_reader(&mut file)
}

/// Probes structural metadata from any seekable reader.
pub fn probe_reader<R: Read + Seek>(reader: &mut R) -> Result<MxfInfo, String> {
    seek_to_first_partition(reader)?;
    let mut info = MxfInfo {
        has_video: false,
        width: 0,
        height: 0,
        fps: 29.97,
        duration_seconds: None,
        has_audio: false,
        audio_sample_rate: 0,
        audio_channels: 0,
        audio_bits: 0,
    };
    let mut sound_descs = Vec::new();
    let mut picture_edit_rate = None;
    let mut picture_duration_units = None;
    let mut audio_duration_units = None;
    let mut essence_packets_seen = 0u32;

    while let Some((key, len)) = read_klv_header(reader)? {
        if is_local_set_key(&key) && len <= MAX_PACKET_BYTES {
            let mut payload = vec![0u8; len as usize];
            reader
                .read_exact(&mut payload)
                .map_err(|e| format!("conjunto de metadados MXF truncado: {e}"))?;
            inspect_metadata_set(
                &key,
                &payload,
                &mut info,
                &mut sound_descs,
                &mut picture_edit_rate,
                &mut picture_duration_units,
                &mut audio_duration_units,
            );
        } else {
            if is_sound_essence_key(&key).is_some() {
                info.has_audio = true;
                essence_packets_seen += 1;
            } else if key[4] == 0x01 && key[5] == 0x02 && key[6] == 0x01 {
                essence_packets_seen += 1;
            }
            reader
                .seek(SeekFrom::Current(
                    i64::try_from(len).map_err(|_| "bloco KLV grande demais no MXF".to_string())?,
                ))
                .map_err(|e| format!("falha ao avançar bloco KLV no MXF: {e}"))?;
            // Stop scanning once we are well into the body essence if header metadata was already read
            if essence_packets_seen >= 32 && (info.has_video || !sound_descs.is_empty()) {
                break;
            }
        }
    }

    if !sound_descs.is_empty() {
        let total_mono_tracks = sound_descs.iter().filter(|d| d.channels == 1).count();
        info.audio_channels = if sound_descs[0].channels > 1 {
            sound_descs[0].channels
        } else if total_mono_tracks >= 2 {
            2
        } else {
            1
        };
    } else if info.has_audio && info.audio_sample_rate == 0 {
        info.audio_sample_rate = 48_000;
        info.audio_channels = 2;
        info.audio_bits = 16;
    }

    info.duration_seconds = match (picture_edit_rate, picture_duration_units) {
        (Some(rate), Some(dur)) if rate > 0.0 && dur > 0 => Some(dur as f64 / rate),
        _ => audio_duration_units
            .and_then(|(rate, dur)| (rate > 0.0 && dur > 0).then_some(dur as f64 / rate)),
    };

    Ok(info)
}

/// Decodes uncompressed PCM / BWF / AIFF / AES3 audio from an MXF container into interleaved `f32`.
///
/// Multi-mono camera MXFs (e.g. Ch1 + Ch2 recorded as separate mono WaveAudio tracks in XDCAM/XAVC)
/// automatically pair their first two mono tracks into stereo (`L, R`); multi-channel single-track
/// MXFs preserve their native channel layout.
pub fn decode_audio_file(path: &Path) -> Result<MxfAudio, String> {
    let mut file = File::open(path).map_err(|e| format!("falha ao abrir MXF {}: {e}", path.display()))?;
    decode_audio_reader(&mut file)
}

/// Decodes uncompressed PCM / BWF / AIFF / AES3 audio from a seekable MXF stream.
pub fn decode_audio_reader<R: Read + Seek>(reader: &mut R) -> Result<MxfAudio, String> {
    seek_to_first_partition(reader)?;
    let mut dummy_info = MxfInfo {
        has_video: false,
        width: 0,
        height: 0,
        fps: 29.97,
        duration_seconds: None,
        has_audio: false,
        audio_sample_rate: 0,
        audio_channels: 0,
        audio_bits: 0,
    };
    let mut sound_descs: Vec<SoundDesc> = Vec::new();
    let mut pic_rate = None;
    let mut pic_dur = None;
    let mut aud_dur = None;
    let mut track_samples: BTreeMap<[u8; 4], Vec<f32>> = BTreeMap::new();
    let mut track_order: Vec<[u8; 4]> = Vec::new();

    while let Some((key, len)) = read_klv_header(reader)? {
        if is_local_set_key(&key) && len <= MAX_PACKET_BYTES {
            let mut payload = vec![0u8; len as usize];
            reader
                .read_exact(&mut payload)
                .map_err(|e| format!("conjunto de metadados MXF truncado: {e}"))?;
            inspect_metadata_set(
                &key,
                &payload,
                &mut dummy_info,
                &mut sound_descs,
                &mut pic_rate,
                &mut pic_dur,
                &mut aud_dur,
            );
        } else if let Some((track_key, key_enc)) = is_sound_essence_key(&key) {
            if len > MAX_PACKET_BYTES {
                return Err("pacote de áudio MXF excede o limite de segurança de 64 MiB".into());
            }
            let track_idx = match track_order.iter().position(|k| *k == track_key) {
                Some(idx) => idx,
                None => {
                    track_order.push(track_key);
                    track_order.len() - 1
                }
            };
            // Decode at most the first track (if multichannel) or first two mono tracks (L + R).
            let first_channels = sound_descs.first().map_or(1, |d| d.channels);
            let max_tracks = if first_channels > 1 { 1 } else { 2 };
            if track_idx >= max_tracks {
                reader
                    .seek(SeekFrom::Current(
                        i64::try_from(len)
                            .map_err(|_| "bloco KLV grande demais no MXF".to_string())?,
                    ))
                    .map_err(|e| format!("falha ao pular faixa extra no MXF: {e}"))?;
                continue;
            }

            let desc = sound_descs
                .get(track_idx)
                .or_else(|| sound_descs.first())
                .copied()
                .unwrap_or(SoundDesc {
                    sample_rate: 48_000,
                    channels: 1,
                    bits: 16,
                    encoding: key_enc,
                });
            let encoding = if key_enc == SoundEncoding::Smpte331m
                || key_enc == SoundEncoding::AiffBe
                || key_enc == SoundEncoding::Aes3Raw
            {
                key_enc
            } else {
                desc.encoding
            };
            let mut buf = vec![0u8; len as usize];
            reader
                .read_exact(&mut buf)
                .map_err(|e| format!("pacote de áudio MXF truncado: {e}"))?;
            let dst = track_samples.entry(track_key).or_default();
            decode_sound_packet(&buf, desc.channels, desc.bits, encoding, dst);
        } else {
            reader
                .seek(SeekFrom::Current(
                    i64::try_from(len).map_err(|_| "bloco KLV grande demais no MXF".to_string())?,
                ))
                .map_err(|e| format!("falha ao avançar bloco KLV no MXF: {e}"))?;
        }
    }

    if track_order.is_empty() {
        return Err("nenhuma faixa de áudio PCM/BWF/AES3 encontrada no MXF".into());
    }

    let primary_desc = sound_descs.first().copied().unwrap_or(SoundDesc {
        sample_rate: 48_000,
        channels: 1,
        bits: 16,
        encoding: SoundEncoding::WaveLe,
    });
    let sample_rate = primary_desc.sample_rate.max(8_000);

    if primary_desc.channels > 1 || track_order.len() == 1 {
        let key0 = track_order[0];
        let samples = track_samples.remove(&key0).unwrap_or_default();
        if samples.is_empty() {
            return Err("faixa de áudio MXF não contém amostras válidas".into());
        }
        let channels = if primary_desc.encoding == SoundEncoding::Smpte331m
            && primary_desc.channels == 1
        {
            1
        } else {
            primary_desc.channels.max(1)
        };
        Ok(MxfAudio {
            sample_rate,
            channels,
            samples,
        })
    } else {
        let left = track_samples.remove(&track_order[0]).unwrap_or_default();
        let right = track_samples.remove(&track_order[1]).unwrap_or_default();
        let frames = left.len().max(right.len());
        if frames == 0 {
            return Err("faixas mono do MXF não contêm amostras válidas".into());
        }
        let mut stereo = Vec::with_capacity(frames * 2);
        for i in 0..frames {
            stereo.push(left.get(i).copied().unwrap_or(0.0));
            stereo.push(right.get(i).copied().unwrap_or(0.0));
        }
        Ok(MxfAudio {
            sample_rate,
            channels: 2,
            samples: stereo,
        })
    }
}

fn decode_sound_packet(
    bytes: &[u8],
    channels: u16,
    bits: u16,
    encoding: SoundEncoding,
    out: &mut Vec<f32>,
) {
    match encoding {
        SoundEncoding::Smpte331m => {
            if bytes.len() < 4 {
                return;
            }
            let sample_count = usize::from(u16::from_le_bytes([bytes[1], bytes[2]]));
            let valid_mask = bytes[3];
            let active_ch = usize::from(channels.max(1)).min(8);
            let mut pos = 4usize;
            let frames = if sample_count > 0 {
                sample_count
            } else {
                (bytes.len() - 4) / 32
            };
            for _ in 0..frames {
                if pos + 32 > bytes.len() {
                    break;
                }
                let mut emitted = 0usize;
                for ch in 0..8usize {
                    let sub = u32::from_le_bytes([
                        bytes[pos + ch * 4],
                        bytes[pos + ch * 4 + 1],
                        bytes[pos + ch * 4 + 2],
                        bytes[pos + ch * 4 + 3],
                    ]);
                    if valid_mask == 0 || (valid_mask & (1 << ch)) != 0 {
                        if emitted < active_ch {
                            let s24 = (((sub >> 4) as i32) << 8) >> 8;
                            out.push((s24 as f32 / 8_388_608.0).clamp(-1.0, 1.0));
                            emitted += 1;
                        }
                    }
                }
                while emitted < active_ch {
                    out.push(0.0);
                    emitted += 1;
                }
                pos += 32;
            }
        }
        SoundEncoding::Aes3Raw => {
            for chunk in bytes.chunks_exact(4) {
                let sub = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                let s24 = (((sub >> 4) as i32) << 8) >> 8;
                out.push((s24 as f32 / 8_388_608.0).clamp(-1.0, 1.0));
            }
        }
        SoundEncoding::WaveLe => match bits {
            1..=8 => {
                for &b in bytes {
                    out.push(((f32::from(b) - 128.0) / 128.0).clamp(-1.0, 1.0));
                }
            }
            9..=16 => {
                for chunk in bytes.chunks_exact(2) {
                    let s = i16::from_le_bytes([chunk[0], chunk[1]]);
                    out.push((f32::from(s) / 32_768.0).clamp(-1.0, 1.0));
                }
            }
            17..=24 => {
                for chunk in bytes.chunks_exact(3) {
                    let s = ((i32::from(chunk[2]) << 24)
                        | (i32::from(chunk[1]) << 16)
                        | (i32::from(chunk[0]) << 8))
                        >> 8;
                    out.push((s as f32 / 8_388_608.0).clamp(-1.0, 1.0));
                }
            }
            _ => {
                for chunk in bytes.chunks_exact(4) {
                    let s = i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                    out.push(((f64::from(s) / 2_147_483_648.0) as f32).clamp(-1.0, 1.0));
                }
            }
        },
        SoundEncoding::AiffBe => match bits {
            1..=8 => {
                for &b in bytes {
                    out.push((f32::from(b as i8) / 128.0).clamp(-1.0, 1.0));
                }
            }
            9..=16 => {
                for chunk in bytes.chunks_exact(2) {
                    let s = i16::from_be_bytes([chunk[0], chunk[1]]);
                    out.push((f32::from(s) / 32_768.0).clamp(-1.0, 1.0));
                }
            }
            17..=24 => {
                for chunk in bytes.chunks_exact(3) {
                    let s = ((i32::from(chunk[0]) << 24)
                        | (i32::from(chunk[1]) << 16)
                        | (i32::from(chunk[2]) << 8))
                        >> 8;
                    out.push((s as f32 / 8_388_608.0).clamp(-1.0, 1.0));
                }
            }
            _ => {
                for chunk in bytes.chunks_exact(4) {
                    let s = i32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                    out.push(((f64::from(s) / 2_147_483_648.0) as f32).clamp(-1.0, 1.0));
                }
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn push_klv(buf: &mut Vec<u8>, key: [u8; 16], value: &[u8]) {
        buf.extend_from_slice(&key);
        let len = value.len() as u64;
        if len < 0x80 {
            buf.push(len as u8);
        } else {
            buf.push(0x84);
            buf.extend_from_slice(&(len as u32).to_be_bytes());
        }
        buf.extend_from_slice(value);
    }

    fn push_tag(set: &mut Vec<u8>, tag: u16, value: &[u8]) {
        set.extend_from_slice(&tag.to_be_bytes());
        set.extend_from_slice(&(value.len() as u16).to_be_bytes());
        set.extend_from_slice(value);
    }

    fn header_partition_key() -> [u8; 16] {
        [
            0x06, 0x0e, 0x2b, 0x34, 0x02, 0x05, 0x01, 0x01, 0x0d, 0x01, 0x02, 0x01, 0x01, 0x02,
            0x04, 0x00,
        ]
    }

    fn cdci_descriptor_key() -> [u8; 16] {
        [
            0x06, 0x0e, 0x2b, 0x34, 0x02, 0x53, 0x01, 0x01, 0x0d, 0x01, 0x01, 0x01, 0x01, 0x01,
            0x28, 0x00,
        ]
    }

    fn wave_descriptor_key() -> [u8; 16] {
        [
            0x06, 0x0e, 0x2b, 0x34, 0x02, 0x53, 0x01, 0x01, 0x0d, 0x01, 0x01, 0x01, 0x01, 0x01,
            0x48, 0x00,
        ]
    }

    fn build_synthetic_camera_mxf_dual_mono_24bit() -> Vec<u8> {
        let mut mxf = Vec::new();
        // Header Partition Pack
        push_klv(&mut mxf, header_partition_key(), &[0u8; 88]);

        // Picture descriptor: 1920x1080 @ 24000/1001, 48 frames
        let mut pic = Vec::new();
        let mut rate = Vec::new();
        rate.extend_from_slice(&24_000i32.to_be_bytes());
        rate.extend_from_slice(&1_001i32.to_be_bytes());
        push_tag(&mut pic, 0x3001, &rate);
        push_tag(&mut pic, 0x3002, &48i64.to_be_bytes());
        push_tag(&mut pic, 0x3203, &1920u32.to_be_bytes());
        push_tag(&mut pic, 0x3202, &1080u32.to_be_bytes());
        push_klv(&mut mxf, cdci_descriptor_key(), &pic);

        // Two mono 24-bit 48 kHz WaveAudio descriptors (like Sony XDCAM / XAVC)
        for _ in 0..2 {
            let mut wav = Vec::new();
            let mut ar = Vec::new();
            ar.extend_from_slice(&48_000i32.to_be_bytes());
            ar.extend_from_slice(&1i32.to_be_bytes());
            push_tag(&mut wav, 0x3d03, &ar);
            push_tag(&mut wav, 0x3d07, &1u32.to_be_bytes());
            push_tag(&mut wav, 0x3d01, &24u32.to_be_bytes());
            push_klv(&mut mxf, wave_descriptor_key(), &wav);
        }

        // Ch1 (+0.5 full-scale = +4_194_304) and Ch2 (-0.25 full-scale = -2_097_152), 4 samples each
        let ch1_key = [
            0x06, 0x0e, 0x2b, 0x34, 0x01, 0x02, 0x01, 0x01, 0x0d, 0x01, 0x03, 0x01, 0x16, 0x01,
            0x01, 0x01,
        ];
        let ch2_key = [
            0x06, 0x0e, 0x2b, 0x34, 0x01, 0x02, 0x01, 0x01, 0x0d, 0x01, 0x03, 0x01, 0x16, 0x01,
            0x01, 0x02,
        ];
        let pos_half = 4_194_304i32.to_le_bytes();
        let neg_quarter = (-2_097_152i32).to_le_bytes();
        let mut pkt1 = Vec::new();
        let mut pkt2 = Vec::new();
        for _ in 0..4 {
            pkt1.extend_from_slice(&pos_half[..3]);
            pkt2.extend_from_slice(&neg_quarter[..3]);
        }
        push_klv(&mut mxf, ch1_key, &pkt1);
        push_klv(&mut mxf, ch2_key, &pkt2);
        mxf
    }

    #[test]
    fn probes_and_decodes_dual_mono_24bit_camera_mxf() {
        let bytes = build_synthetic_camera_mxf_dual_mono_24bit();
        assert!(sniff_mxf(&bytes));
        let info = probe_reader(&mut Cursor::new(&bytes)).unwrap();
        assert!(info.has_video);
        assert_eq!((info.width, info.height), (1920, 1080));
        assert!((info.fps - 24_000.0 / 1_001.0).abs() < 1e-4);
        assert!(info.duration_seconds.unwrap() > 2.0);
        assert!(info.has_audio);
        assert_eq!(info.audio_sample_rate, 48_000);
        assert_eq!(info.audio_channels, 2);
        assert_eq!(info.audio_bits, 24);

        let audio = decode_audio_reader(&mut Cursor::new(&bytes)).unwrap();
        assert_eq!(audio.sample_rate, 48_000);
        assert_eq!(audio.channels, 2);
        assert_eq!(audio.samples.len(), 8);
        assert!((audio.samples[0] - 0.5).abs() < 1e-5);
        assert!((audio.samples[1] + 0.25).abs() < 1e-5);
    }

    #[test]
    fn decodes_aiff_be_16bit_op_atom_audio_mxf_with_run_in() {
        let mut mxf = vec![0xaa; 128]; // 128-byte run-in before Header Partition Pack
        push_klv(&mut mxf, header_partition_key(), &[0u8; 88]);
        let mut desc = Vec::new();
        let mut ar = Vec::new();
        ar.extend_from_slice(&48_000i32.to_be_bytes());
        ar.extend_from_slice(&1i32.to_be_bytes());
        push_tag(&mut desc, 0x3d03, &ar);
        push_tag(&mut desc, 0x3d07, &2u32.to_be_bytes());
        push_tag(&mut desc, 0x3d01, &16u32.to_be_bytes());
        push_tag(&mut desc, 0x3002, &2i64.to_be_bytes());
        push_klv(&mut mxf, wave_descriptor_key(), &desc);

        // AIFF big-endian clip-wrapped stereo (0x16, _, 0x04, _)
        let aiff_key = [
            0x06, 0x0e, 0x2b, 0x34, 0x01, 0x02, 0x01, 0x01, 0x0d, 0x01, 0x03, 0x01, 0x16, 0x01,
            0x04, 0x01,
        ];
        let mut pcm = Vec::new();
        pcm.extend_from_slice(&16_384i16.to_be_bytes());
        pcm.extend_from_slice(&(-16_384i16).to_be_bytes());
        push_klv(&mut mxf, aiff_key, &pcm);

        assert!(sniff_mxf(&mxf));
        let info = probe_reader(&mut Cursor::new(&mxf)).unwrap();
        assert!(!info.has_video);
        assert!(info.has_audio);
        assert_eq!(info.audio_channels, 2);

        let audio = decode_audio_reader(&mut Cursor::new(&mxf)).unwrap();
        assert_eq!(audio.channels, 2);
        assert_eq!(audio.samples.len(), 2);
        assert!((audio.samples[0] - 0.5).abs() < 1e-4);
        assert!((audio.samples[1] + 0.5).abs() < 1e-4);
    }
}
