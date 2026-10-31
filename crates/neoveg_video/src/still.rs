//! Pure-Rust still image and numbered image sequence probing and decoding (`image` crate).

use std::path::{Path, PathBuf};

use image::imageops::FilterType;
use image::ImageReader;
use serde::{Deserialize, Serialize};

use crate::frame::FrameBuffer;
use crate::info::{fit_size, VideoInfo};

/// Still-image file extensions: the first twelve are read in pure Rust, the HEIF family (phone
/// photos and AVIF) needs the `ffmpeg` feature.
pub const IMAGE_EXTENSIONS: [&str; 16] = [
    "png", "jpg", "jpeg", "bmp", "gif", "webp", "tif", "tiff", "tga", "ico", "ppm", "pgm", "heic",
    "heif", "hif", "avif",
];

/// Pictures stored as HEIF items (HEVC or AV1 inside an ISO base media file). FFmpeg reads them,
/// grids of tiles and the rotation included.
const HEIF_EXTENSIONS: [&str; 4] = ["heic", "heif", "hif", "avif"];

fn is_heif_path(path: &Path) -> bool {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .is_some_and(|e| HEIF_EXTENSIONS.contains(&e.as_str()))
}

#[cfg(not(feature = "ffmpeg"))]
const NO_HEIF_DECODER: &str =
    "HEIC, HEIF e AVIF precisam do build com a decodificação por FFmpeg (recurso `ffmpeg`)";

/// JSON manifest extension representing an imported numbered image sequence.
pub const SEQUENCE_EXTENSION: &str = "neovegseq";

pub fn is_image_path(path: &Path) -> bool {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .is_some_and(|e| IMAGE_EXTENSIONS.contains(&e.as_str()))
}

pub fn is_sequence_manifest_path(path: &Path) -> bool {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .is_some_and(|e| e == SEQUENCE_EXTENSION)
}

/// Probes dimensions of a still image from its header without decoding pixel data.
pub fn probe_image(path: &Path, fps: f64, still_seconds: f64) -> Result<VideoInfo, String> {
    if is_heif_path(path) {
        #[cfg(feature = "ffmpeg")]
        {
            let shown = crate::decode::probe(path)?;
            return Ok(still_info(shown.width, shown.height, fps, still_seconds));
        }
        #[cfg(not(feature = "ffmpeg"))]
        return Err(NO_HEIF_DECODER.into());
    }
    let reader = ImageReader::open(path)
        .map_err(|e| format!("falha ao abrir imagem {}: {e}", path.display()))?
        .with_guessed_format()
        .map_err(|e| format!("falha ao identificar formato de {}: {e}", path.display()))?;
    let (width, height) = reader
        .into_dimensions()
        .map_err(|e| format!("cabeçalho de imagem inválido em {}: {e}", path.display()))?;
    Ok(still_info(width, height, fps, still_seconds))
}

/// The metadata of a still picture: it lasts `still_seconds` and has the project's frame rate.
fn still_info(width: u32, height: u32, fps: f64, still_seconds: f64) -> VideoInfo {
    VideoInfo {
        width: width.max(2),
        height: height.max(2),
        fps: if fps.is_finite() && fps > 0.0 {
            fps
        } else {
            29.97
        },
        duration: if still_seconds.is_finite() && still_seconds > 0.0 {
            still_seconds
        } else {
            5.0
        },
    }
}

/// Decodes a still image into an RGBA8 `FrameBuffer`, scaling down to `max_width` (even dimensions)
/// when requested (`0` keeps native resolution rounded down to even dimensions).
pub fn decode_image(path: &Path, max_width: u32) -> Result<FrameBuffer, String> {
    if is_heif_path(path) {
        #[cfg(feature = "ffmpeg")]
        return crate::decode::VideoDecoder::open(path)?.frame_at(0.0, max_width);
        #[cfg(not(feature = "ffmpeg"))]
        return Err(NO_HEIF_DECODER.into());
    }
    let img = ImageReader::open(path)
        .map_err(|e| format!("falha ao abrir imagem {}: {e}", path.display()))?
        .with_guessed_format()
        .map_err(|e| format!("falha ao identificar formato de {}: {e}", path.display()))?
        .decode()
        .map_err(|e| format!("falha ao decodificar imagem {}: {e}", path.display()))?
        .into_rgba8();

    let (src_w, src_h) = img.dimensions();
    let (dst_w, dst_h) = fit_size(src_w, src_h, max_width);
    let rgba = if (dst_w, dst_h) == (src_w, src_h) {
        img.into_raw()
    } else {
        image::imageops::resize(&img, dst_w, dst_h, FilterType::Triangle).into_raw()
    };
    Ok(FrameBuffer {
        width: dst_w,
        height: dst_h,
        rgba,
    })
}

/// Contiguous numbered image sequence on disk (`prefix` + zero-padded `digits` + `.extension`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageSequence {
    pub directory: PathBuf,
    pub prefix: String,
    pub digits: usize,
    pub start_number: u64,
    pub frame_count: usize,
    pub extension: String,
    pub fps: f64,
    pub width: u32,
    pub height: u32,
}

impl ImageSequence {
    pub fn duration(&self) -> f64 {
        let rate = if self.fps.is_finite() && self.fps > 0.0 {
            self.fps
        } else {
            29.97
        };
        (self.frame_count.max(1) as f64) / rate
    }

    pub fn video_info(&self) -> VideoInfo {
        VideoInfo {
            width: self.width,
            height: self.height,
            fps: if self.fps.is_finite() && self.fps > 0.0 {
                self.fps
            } else {
                29.97
            },
            duration: self.duration(),
        }
    }

    pub fn format_filename(&self, number: u64) -> String {
        format!(
            "{}{:0width$}.{}",
            self.prefix,
            number,
            self.extension,
            width = self.digits
        )
    }

    pub fn frame_path(&self, frame_index: usize) -> PathBuf {
        let clamped = frame_index.min(self.frame_count.saturating_sub(1));
        let num = self.start_number + clamped as u64;
        self.directory.join(self.format_filename(num))
    }

    pub fn frame_index_at(&self, seconds: f64) -> usize {
        if self.frame_count == 0 {
            return 0;
        }
        let rate = if self.fps.is_finite() && self.fps > 0.0 {
            self.fps
        } else {
            29.97
        };
        let t = if seconds.is_finite() {
            seconds.max(0.0)
        } else {
            0.0
        };
        let raw = (t * rate + 1e-9).floor() as usize;
        raw.min(self.frame_count - 1)
    }

    pub fn frame_path_at_seconds(&self, seconds: f64) -> PathBuf {
        self.frame_path(self.frame_index_at(seconds))
    }

    pub fn display_stem(&self) -> String {
        let end_num = self.start_number + self.frame_count.saturating_sub(1) as u64;
        format!(
            "{}{:0w$}-{:0w$}.{}",
            self.prefix,
            self.start_number,
            end_num,
            self.extension,
            w = self.digits
        )
    }

    pub fn save_manifest(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("falha ao criar pasta {}: {e}", parent.display()))?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("falha ao serializar sequência: {e}"))?;
        std::fs::write(path, json)
            .map_err(|e| format!("falha ao gravar manifesto {}: {e}", path.display()))
    }

    pub fn load_manifest(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("falha ao ler manifesto {}: {e}", path.display()))?;
        serde_json::from_str(&text)
            .map_err(|e| format!("manifesto de sequência inválido {}: {e}", path.display()))
    }
}

/// Inspects `first_path` for trailing digits in its file stem and counts contiguous numbered frames
/// in the same folder. Returns `Ok(Some(seq))` when at least 2 contiguous frames exist.
pub fn detect_image_sequence(first_path: &Path, fps: f64) -> Result<Option<ImageSequence>, String> {
    if !is_image_path(first_path) {
        return Ok(None);
    }
    let Some(directory) = first_path.parent() else {
        return Ok(None);
    };
    let Some(stem) = first_path.file_stem().and_then(|s| s.to_str()) else {
        return Ok(None);
    };
    let Some(ext) = first_path.extension().and_then(|e| e.to_str()) else {
        return Ok(None);
    };

    let digit_bytes = stem
        .bytes()
        .rev()
        .take_while(|b| b.is_ascii_digit())
        .count();
    if digit_bytes == 0 {
        return Ok(None);
    }
    let split_at = stem.len() - digit_bytes;
    let prefix = &stem[..split_at];
    let digits_str = &stem[split_at..];
    let Ok(start_number) = digits_str.parse::<u64>() else {
        return Ok(None);
    };

    let mut frame_count = 0usize;
    while let Some(num) = start_number.checked_add(frame_count as u64) {
        let candidate = directory.join(format!(
            "{}{:0width$}.{}",
            prefix,
            num,
            ext,
            width = digit_bytes
        ));
        if candidate.is_file() {
            frame_count += 1;
        } else {
            break;
        }
    }

    if frame_count < 2 {
        return Ok(None);
    }

    let info = probe_image(first_path, fps, 5.0)?;
    Ok(Some(ImageSequence {
        directory: directory.to_path_buf(),
        prefix: prefix.to_string(),
        digits: digit_bytes,
        start_number,
        frame_count,
        extension: ext.to_string(),
        fps: info.fps,
        width: info.width,
        height: info.height,
    }))
}

pub fn probe_sequence_manifest(path: &Path) -> Result<VideoInfo, String> {
    let seq = ImageSequence::load_manifest(path)?;
    let first = seq.frame_path(0);
    if !first.is_file() {
        return Err(format!(
            "primeiro quadro da sequência ausente: {}",
            first.display()
        ));
    }
    Ok(seq.video_info())
}

pub fn decode_sequence_at(
    manifest_path: &Path,
    seconds: f64,
    max_width: u32,
) -> Result<FrameBuffer, String> {
    let seq = ImageSequence::load_manifest(manifest_path)?;
    let frame_path = seq.frame_path_at_seconds(seconds);
    decode_image(&frame_path, max_width)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};

    fn write_solid_png(path: &Path, w: u32, h: u32, color: [u8; 4]) {
        let img = ImageBuffer::from_pixel(w, h, Rgba(color));
        img.save(path).unwrap();
    }

    #[test]
    fn single_still_image_probes_and_decodes_with_downscale() {
        let dir = std::env::temp_dir().join(format!("neoveg_still_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let png = dir.join("poster.PNG");
        write_solid_png(&png, 320, 180, [220, 40, 90, 255]);

        assert!(is_image_path(&png));
        let info = probe_image(&png, 24.0, 5.0).unwrap();
        assert_eq!((info.width, info.height), (320, 180));
        assert_eq!(info.fps, 24.0);
        assert_eq!(info.duration, 5.0);

        let decoded = decode_image(&png, 160).unwrap();
        assert_eq!((decoded.width, decoded.height), (160, 90));
        assert_eq!(decoded.pixel(80, 45), [220, 40, 90, 255]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn numbered_sequence_detects_saves_manifest_and_decodes_frame_accurately() {
        let dir = std::env::temp_dir().join(format!("neoveg_seq_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let colors = [
            [255, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 0, 255, 255],
            [255, 255, 255, 255],
        ];
        for (idx, c) in colors.iter().enumerate() {
            let p = dir.join(format!("shot_{:04}.png", idx + 1));
            write_solid_png(&p, 64, 48, *c);
        }

        let first = dir.join("shot_0001.png");
        let seq = detect_image_sequence(&first, 25.0)
            .unwrap()
            .expect("must detect 4-frame sequence");
        assert_eq!(seq.prefix, "shot_");
        assert_eq!(seq.digits, 4);
        assert_eq!(seq.start_number, 1);
        assert_eq!(seq.frame_count, 4);
        assert!((seq.duration() - 0.16).abs() < 1e-6);

        let manifest = dir.join("shot_0001-0004.neovegseq");
        seq.save_manifest(&manifest).unwrap();
        assert!(is_sequence_manifest_path(&manifest));

        let info = probe_sequence_manifest(&manifest).unwrap();
        assert_eq!((info.width, info.height), (64, 48));
        assert!((info.duration - 0.16).abs() < 1e-6);

        for (idx, expected) in colors.iter().enumerate() {
            let t = idx as f64 / 25.0 + 0.005;
            let frame = decode_sequence_at(&manifest, t, 0).unwrap();
            assert_eq!(frame.pixel(10, 10), *expected, "frame {idx} mismatch");
        }

        // Clamps past end of sequence cleanly to the final frame.
        let past_end = decode_sequence_at(&manifest, 10.0, 0).unwrap();
        assert_eq!(past_end.pixel(10, 10), colors[3]);

        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod heif_tests {
    use super::*;

    const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

    /// `(r, g, b)` of the pixel at `(x, y)`.
    fn pixel(frame: &FrameBuffer, x: u32, y: u32) -> (u8, u8, u8) {
        let i = ((y * frame.width + x) * 4) as usize;
        (frame.rgba[i], frame.rgba[i + 1], frame.rgba[i + 2])
    }

    /// The fixtures are red in the top left quarter, green in the bottom right and blue elsewhere.
    fn assert_the_three_colours(frame: &FrameBuffer, what: &str) {
        let (w, h) = (frame.width, frame.height);
        let near = |got: (u8, u8, u8), want: (u8, u8, u8)| {
            [
                got.0 as i32 - want.0 as i32,
                got.1 as i32 - want.1 as i32,
                got.2 as i32 - want.2 as i32,
            ]
            .iter()
            .all(|d| d.abs() < 40)
        };
        assert!(
            near(pixel(frame, w / 8, h / 8), (255, 0, 0)),
            "{what}: top left {:?}",
            pixel(frame, w / 8, h / 8)
        );
        assert!(
            near(pixel(frame, w * 7 / 8, h * 7 / 8), (0, 255, 0)),
            "{what}: bottom right"
        );
        assert!(
            near(pixel(frame, w * 7 / 8, h / 8), (30, 60, 200)),
            "{what}: top right"
        );
    }

    /// What the conversion window gives a build without the FFmpeg decoder: a PNG, which the pure Rust
    /// reader opens. Skipped without the `ffmpeg` command line.
    #[test]
    fn a_heic_picture_converted_to_png_reads_without_ffmpeg_decoding() {
        use crate::convert::{convert, output_path, target_for, Target};
        let source = Path::new(FIXTURES).join("picture_320x180.heic");
        assert_eq!(target_for(&source), Target::Image);
        let dir = std::env::temp_dir().join(format!("neoveg_heic_png_{}", std::process::id()));
        let out = output_path(&source, &dir, Target::Image, Path::exists);
        if convert(&source, &out, Target::Image).is_err() {
            eprintln!("skipped: the ffmpeg command line is not installed");
            return;
        }
        assert_eq!(out.extension().unwrap(), "png");
        let frame = decode_image(&out, 0).unwrap();
        assert_eq!((frame.width, frame.height), (320, 180));
        assert_the_three_colours(&frame, "converted PNG");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_heif_family_counts_as_still_pictures() {
        for name in ["a.heic", "a.HEIF", "a.hif", "a.avif"] {
            assert!(is_image_path(Path::new(name)), "{name}");
        }
    }

    #[cfg(feature = "ffmpeg")]
    #[test]
    fn heic_and_avif_pictures_decode_probe_and_scale() {
        for name in ["picture_320x180.heic", "picture_320x180.avif"] {
            let path = Path::new(FIXTURES).join(name);
            let info = probe_image(&path, 25.0, 4.0).unwrap();
            assert_eq!(
                (info.width, info.height, info.fps, info.duration),
                (320, 180, 25.0, 4.0),
                "{name}"
            );
            let frame = decode_image(&path, 0).unwrap();
            assert_eq!((frame.width, frame.height), (320, 180), "{name}");
            assert_the_three_colours(&frame, name);
            let small = decode_image(&path, 160).unwrap();
            assert_eq!((small.width, small.height), (160, 90), "{name}");
        }
    }

    #[cfg(feature = "ffmpeg")]
    #[test]
    fn a_large_heic_photo_made_of_tiles_decodes_whole() {
        let path = Path::new(FIXTURES).join("picture_1200x800.heic");
        let frame = decode_image(&path, 0).unwrap();
        assert_eq!((frame.width, frame.height), (1200, 800));
        assert_the_three_colours(&frame, "1200x800");
    }

    #[cfg(not(feature = "ffmpeg"))]
    #[test]
    fn without_ffmpeg_a_heic_picture_says_what_is_missing() {
        let path = Path::new(FIXTURES).join("picture_320x180.heic");
        let error = decode_image(&path, 0).unwrap_err();
        assert!(error.contains("FFmpeg"), "{error}");
        assert!(probe_image(&path, 25.0, 4.0).is_err());
        let _ = pixel;
    }
}
