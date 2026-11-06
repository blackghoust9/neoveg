//! Gallery of reference frames: frames kept while grading (with their correction already applied)
//! and compared with the current frame by a wipe or side by side. The composition is pure; the
//! storage is a folder of PNG files plus an `index.json`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::frame::FrameBuffer;

/// How the reference and the current frame are laid out together.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CompareMode {
    /// Vertical wipe: the reference shows left of `position` (`0..=1` of the width), the current
    /// frame right of it, with a thin divider line.
    WipeVertical { position: f32 },
    /// Horizontal wipe: the reference shows above `position` (`0..=1` of the height).
    WipeHorizontal { position: f32 },
    /// Reference on the left half, current frame on the right half, both squeezed to fit.
    SideBySide,
}

/// Nearest-neighbour resize (cheap and exact for integer ratios).
pub fn resize_nearest(src: &FrameBuffer, w: u32, h: u32) -> FrameBuffer {
    if src.width == w && src.height == h {
        return src.clone();
    }
    let mut out = FrameBuffer::solid(w, h, [0, 0, 0, 255]);
    if src.width == 0 || src.height == 0 || w == 0 || h == 0 {
        return out;
    }
    for y in 0..h {
        let sy = ((u64::from(y) * 2 + 1) * u64::from(src.height) / (u64::from(h) * 2)) as u32;
        for x in 0..w {
            let sx = ((u64::from(x) * 2 + 1) * u64::from(src.width) / (u64::from(w) * 2)) as u32;
            let s = ((sy.min(src.height - 1) * src.width + sx.min(src.width - 1)) * 4) as usize;
            let d = ((y * w + x) * 4) as usize;
            out.rgba[d..d + 4].copy_from_slice(&src.rgba[s..s + 4]);
        }
    }
    out
}

/// Composes the comparison; the output has the size of `current`.
pub fn compare(current: &FrameBuffer, reference: &FrameBuffer, mode: CompareMode) -> FrameBuffer {
    let (w, h) = (current.width, current.height);
    if w == 0 || h == 0 {
        return current.clone();
    }
    let clamp = |p: f32| if p.is_finite() { p.clamp(0.0, 1.0) } else { 0.5 };
    match mode {
        CompareMode::SideBySide => {
            let half = (w / 2).max(1);
            let left = resize_nearest(reference, half, h);
            let right = resize_nearest(current, w - half.min(w - 1), h);
            let mut out = FrameBuffer::solid(w, h, [0, 0, 0, 255]);
            for y in 0..h {
                let row = (y * w * 4) as usize;
                let l = (y * left.width * 4) as usize;
                out.rgba[row..row + left.width as usize * 4]
                    .copy_from_slice(&left.rgba[l..l + left.width as usize * 4]);
                let r = (y * right.width * 4) as usize;
                let at = row + half as usize * 4;
                let n = (right.width.min(w - half)) as usize * 4;
                out.rgba[at..at + n].copy_from_slice(&right.rgba[r..r + n]);
            }
            out
        }
        CompareMode::WipeVertical { position } => {
            let reference = resize_nearest(reference, w, h);
            let edge = (clamp(position) * w as f32).round() as u32;
            let mut out = current.clone();
            for y in 0..h {
                for x in 0..edge.min(w) {
                    let i = ((y * w + x) * 4) as usize;
                    out.rgba[i..i + 4].copy_from_slice(&reference.rgba[i..i + 4]);
                }
                if edge > 0 && edge < w {
                    let i = ((y * w + edge) * 4) as usize;
                    out.rgba[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
                }
            }
            out
        }
        CompareMode::WipeHorizontal { position } => {
            let reference = resize_nearest(reference, w, h);
            let edge = (clamp(position) * h as f32).round() as u32;
            let mut out = current.clone();
            let row = (w * 4) as usize;
            let upto = edge.min(h) as usize * row;
            out.rgba[..upto].copy_from_slice(&reference.rgba[..upto]);
            if edge > 0 && edge < h {
                for px in out.rgba[edge as usize * row..(edge as usize + 1) * row].chunks_exact_mut(4)
                {
                    px.copy_from_slice(&[255, 255, 255, 255]);
                }
            }
            out
        }
    }
}

/// Entry of the gallery index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GalleryEntry {
    pub id: u64,
    pub name: String,
    /// File name of the PNG, inside the gallery folder.
    pub file: String,
}

/// Why a gallery operation failed.
#[derive(Debug)]
pub enum GalleryError {
    Io(std::io::Error),
    Image(String),
    Index(String),
    Missing(u64),
}

impl std::fmt::Display for GalleryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "Erro de arquivo na galeria: {e}"),
            Self::Image(e) => write!(f, "Imagem inválida na galeria: {e}"),
            Self::Index(e) => write!(f, "Índice da galeria ilegível: {e}"),
            Self::Missing(id) => write!(f, "Quadro {id} não existe na galeria"),
        }
    }
}

impl std::error::Error for GalleryError {}

impl From<std::io::Error> for GalleryError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// A folder holding reference frames.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceGallery {
    dir: PathBuf,
    entries: Vec<GalleryEntry>,
}

impl ReferenceGallery {
    const INDEX: &'static str = "index.json";

    /// Opens (or creates) the gallery in `dir`.
    pub fn open(dir: impl AsRef<Path>) -> Result<Self, GalleryError> {
        let dir = dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir)?;
        let index = dir.join(Self::INDEX);
        let entries = if index.exists() {
            serde_json::from_str(&std::fs::read_to_string(&index)?)
                .map_err(|e| GalleryError::Index(e.to_string()))?
        } else {
            Vec::new()
        };
        Ok(Self { dir, entries })
    }

    pub fn entries(&self) -> &[GalleryEntry] {
        &self.entries
    }

    fn save_index(&self) -> Result<(), GalleryError> {
        let text = serde_json::to_string_pretty(&self.entries)
            .map_err(|e| GalleryError::Index(e.to_string()))?;
        std::fs::write(self.dir.join(Self::INDEX), text)?;
        Ok(())
    }

    /// Stores `frame` under `name` and returns its id.
    pub fn add(&mut self, name: &str, frame: &FrameBuffer) -> Result<u64, GalleryError> {
        let id = self.entries.iter().map(|e| e.id).max().map_or(1, |m| m + 1);
        let file = format!("ref_{id:04}.png");
        let img = image::RgbaImage::from_raw(frame.width, frame.height, frame.rgba.clone())
            .ok_or_else(|| GalleryError::Image("tamanho do quadro não confere".into()))?;
        img.save(self.dir.join(&file))
            .map_err(|e| GalleryError::Image(e.to_string()))?;
        let name = name.trim();
        let name = if name.is_empty() {
            format!("Referência {id}")
        } else {
            name.to_string()
        };
        self.entries.push(GalleryEntry { id, name, file });
        self.save_index()?;
        Ok(id)
    }

    /// Loads the frame `id`.
    pub fn load(&self, id: u64) -> Result<FrameBuffer, GalleryError> {
        let entry = self
            .entries
            .iter()
            .find(|e| e.id == id)
            .ok_or(GalleryError::Missing(id))?;
        let img = image::open(self.dir.join(&entry.file))
            .map_err(|e| GalleryError::Image(e.to_string()))?
            .to_rgba8();
        Ok(FrameBuffer {
            width: img.width(),
            height: img.height(),
            rgba: img.into_raw(),
        })
    }

    /// Removes the frame `id` and its file.
    pub fn remove(&mut self, id: u64) -> Result<(), GalleryError> {
        let at = self
            .entries
            .iter()
            .position(|e| e.id == id)
            .ok_or(GalleryError::Missing(id))?;
        let entry = self.entries.remove(at);
        let _ = std::fs::remove_file(self.dir.join(entry.file));
        self.save_index()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn half_and_half(w: u32, h: u32, a: [u8; 4], b: [u8; 4]) -> FrameBuffer {
        let mut f = FrameBuffer::solid(w, h, a);
        for y in 0..h {
            for x in w / 2..w {
                let i = ((y * w + x) * 4) as usize;
                f.rgba[i..i + 4].copy_from_slice(&b);
            }
        }
        f
    }

    #[test]
    fn a_vertical_wipe_shows_the_reference_on_the_left_and_the_current_on_the_right() {
        let cur = FrameBuffer::solid(100, 40, [200, 0, 0, 255]);
        let refr = FrameBuffer::solid(100, 40, [0, 0, 200, 255]);
        let out = compare(&cur, &refr, CompareMode::WipeVertical { position: 0.25 });
        assert_eq!(out.pixel(10, 5), [0, 0, 200, 255]);
        assert_eq!(out.pixel(25, 5), [255, 255, 255, 255], "divider");
        assert_eq!(out.pixel(80, 5), [200, 0, 0, 255]);
        // At the extremes the wipe is entirely one side
        assert_eq!(compare(&cur, &refr, CompareMode::WipeVertical { position: 0.0 }), cur);
        let all_ref = compare(&cur, &refr, CompareMode::WipeVertical { position: 1.0 });
        assert_eq!(all_ref, refr);
    }

    #[test]
    fn a_horizontal_wipe_and_a_side_by_side_take_the_right_halves() {
        let cur = FrameBuffer::solid(60, 60, [10, 200, 10, 255]);
        let refr = FrameBuffer::solid(30, 30, [200, 10, 10, 255]); // other size is resized
        let out = compare(&cur, &refr, CompareMode::WipeHorizontal { position: 0.5 });
        assert_eq!(out.pixel(5, 5), [200, 10, 10, 255]);
        assert_eq!(out.pixel(5, 55), [10, 200, 10, 255]);
        let sbs = compare(&cur, &refr, CompareMode::SideBySide);
        assert_eq!((sbs.width, sbs.height), (60, 60));
        assert_eq!(sbs.pixel(5, 30), [200, 10, 10, 255]);
        assert_eq!(sbs.pixel(55, 30), [10, 200, 10, 255]);
        // The side-by-side keeps the structure of the reference (squeezed to half the width)
        let two = half_and_half(40, 10, [255, 0, 0, 255], [0, 0, 255, 255]);
        let sbs2 = compare(&FrameBuffer::solid(40, 10, [0, 0, 0, 255]), &two, CompareMode::SideBySide);
        assert_eq!(sbs2.pixel(2, 5), [255, 0, 0, 255]);
        assert_eq!(sbs2.pixel(17, 5), [0, 0, 255, 255]);
    }

    #[test]
    fn the_gallery_stores_loads_and_removes_frames_across_reopenings() {
        let dir = std::env::temp_dir().join(format!("neoveg_gallery_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let frame = half_and_half(8, 4, [1, 2, 3, 255], [250, 240, 230, 255]);
        let id = {
            let mut g = ReferenceGallery::open(&dir).unwrap();
            let a = g.add("Cena 1", &frame).unwrap();
            let b = g.add("  ", &frame).unwrap();
            assert_ne!(a, b);
            assert_eq!(g.entries()[1].name, format!("Referência {b}"));
            a
        };
        let mut g = ReferenceGallery::open(&dir).unwrap();
        assert_eq!(g.entries().len(), 2);
        assert_eq!(g.load(id).unwrap(), frame, "PNG is lossless");
        g.remove(id).unwrap();
        assert!(matches!(g.load(id), Err(GalleryError::Missing(_))));
        assert_eq!(ReferenceGallery::open(&dir).unwrap().entries().len(), 1);
        // Ids do not get reused while a bigger one exists
        let c = g.add("Nova", &frame).unwrap();
        assert!(c > id);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
