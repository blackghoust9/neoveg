//! *Exportação rápida*: render the project to a ready destination without opening the dialog.
//!
//! Each destination is a small fixed recipe (container, quality, size limit, whether there is
//! picture at all). The interface turns the recipe into its own render request; here live the
//! recipes and the rule that picks the output file, so both are testable without a window.

use std::path::{Path, PathBuf};

/// Where the quick export is going.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuickExportTarget {
    /// A light file for the web: H.264 MP4, no taller than 1080 lines, moderate quality.
    Web,
    /// A high quality file at the project's own size.
    File,
    /// Sound only, as a WAV.
    AudioOnly,
}

/// The recipe of a destination.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuickExportRecipe {
    /// Container and codec family, as the key the render dialog uses for its format list.
    pub container: Container,
    /// Constant rate factor (lower is better); unused for audio.
    pub crf: u8,
    /// The picture is scaled down (never up) to at most this many lines. `None`: project size.
    pub max_height: Option<u32>,
    pub has_video: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Container {
    Mp4H264,
    WavPcm16,
}

impl QuickExportTarget {
    pub const ALL: [Self; 3] = [Self::Web, Self::File, Self::AudioOnly];

    pub fn label(self) -> &'static str {
        match self {
            Self::Web => "Exportação rápida: web",
            Self::File => "Exportação rápida: arquivo",
            Self::AudioOnly => "Exportação rápida: só áudio",
        }
    }

    /// Stable id for settings and command lines.
    pub fn id(self) -> &'static str {
        match self {
            Self::Web => "web",
            Self::File => "arquivo",
            Self::AudioOnly => "audio",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.id() == id)
    }

    pub fn recipe(self) -> QuickExportRecipe {
        match self {
            Self::Web => QuickExportRecipe {
                container: Container::Mp4H264,
                crf: 23,
                max_height: Some(1080),
                has_video: true,
            },
            Self::File => QuickExportRecipe {
                container: Container::Mp4H264,
                crf: 16,
                max_height: None,
                has_video: true,
            },
            Self::AudioOnly => QuickExportRecipe {
                container: Container::WavPcm16,
                crf: 0,
                max_height: None,
                has_video: false,
            },
        }
    }

    /// Suffix added to the project name so the three destinations never overwrite each other.
    fn suffix(self) -> &'static str {
        match self {
            Self::Web => "_web",
            Self::File => "",
            Self::AudioOnly => "_audio",
        }
    }

    fn extension(self) -> &'static str {
        match self.recipe().container {
            Container::Mp4H264 => "mp4",
            Container::WavPcm16 => "wav",
        }
    }
}

/// The size to render at: the project's, scaled down to the recipe's height limit and kept even.
pub fn output_size(recipe: &QuickExportRecipe, width: u32, height: u32) -> (u32, u32) {
    let (w, h) = (width.max(2), height.max(2));
    match recipe.max_height {
        Some(max) if h > max => {
            let scale = max as f64 / h as f64;
            let nw = ((w as f64 * scale).round() as u32).max(2) & !1;
            (nw, max & !1)
        }
        _ => (w & !1, h & !1),
    }
}

/// The file the quick export writes: next to the project (or in `fallback_dir` for an unsaved
/// one), named after the project. An existing file is never overwritten: `name (2).mp4`, ...
pub fn output_path(
    target: QuickExportTarget,
    project_name: &str,
    project_dir: Option<&Path>,
    fallback_dir: &Path,
    exists: impl Fn(&Path) -> bool,
) -> PathBuf {
    let dir = project_dir.unwrap_or(fallback_dir);
    let base: String = project_name
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let base = base.trim().trim_matches('.');
    let base = if base.is_empty() { "Projeto" } else { base };
    let (suffix, ext) = (target.suffix(), target.extension());
    let first = dir.join(format!("{base}{suffix}.{ext}"));
    if !exists(&first) {
        return first;
    }
    (2..)
        .map(|n| dir.join(format!("{base}{suffix} ({n}).{ext}")))
        .find(|p| !exists(p))
        .expect("an unbounded range always yields a free name")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_target_has_its_own_recipe() {
        let web = QuickExportTarget::Web.recipe();
        assert_eq!(web.max_height, Some(1080));
        assert!(web.has_video);
        let audio = QuickExportTarget::AudioOnly.recipe();
        assert!(!audio.has_video);
        assert_eq!(audio.container, Container::WavPcm16);
        assert!(QuickExportTarget::File.recipe().crf < web.crf, "file is better");
        for t in QuickExportTarget::ALL {
            assert_eq!(QuickExportTarget::from_id(t.id()), Some(t));
        }
        assert_eq!(QuickExportTarget::from_id("x"), None);
    }

    #[test]
    fn web_scales_down_but_never_up() {
        let web = QuickExportTarget::Web.recipe();
        assert_eq!(output_size(&web, 3840, 2160), (1920, 1080));
        assert_eq!(output_size(&web, 1280, 720), (1280, 720));
        assert_eq!(output_size(&web, 1080, 1920), (608, 1080));
        let file = QuickExportTarget::File.recipe();
        assert_eq!(output_size(&file, 3840, 2160), (3840, 2160));
        assert_eq!(output_size(&file, 1921, 1081), (1920, 1080), "even sizes");
    }

    #[test]
    fn the_output_goes_next_to_the_project_and_never_overwrites() {
        let dir = Path::new("C:/p");
        let fallback = Path::new("C:/Videos");
        let p = output_path(QuickExportTarget::Web, "Meu: filme", Some(dir), fallback, |_| false);
        assert_eq!(p, dir.join("Meu_ filme_web.mp4"));
        let taken = [dir.join("Filme.mp4"), dir.join("Filme (2).mp4")];
        let p = output_path(QuickExportTarget::File, "Filme", Some(dir), fallback, |p| {
            taken.iter().any(|t| t == p)
        });
        assert_eq!(p, dir.join("Filme (3).mp4"));
        let p = output_path(QuickExportTarget::AudioOnly, "  ", None, fallback, |_| false);
        assert_eq!(p, fallback.join("Projeto_audio.wav"));
    }
}
