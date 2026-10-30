//! Fonts for the languages the bundled fonts cannot draw (Japanese and Chinese).
//!
//! The fonts that come with egui have no ideographs, and embedding a CJK font would add many megabytes
//! and a licence to ship, so the program borrows the one the operating system already has. The file is
//! read on a worker thread (it can be 20 MB), and until it arrives the text shows empty boxes for a
//! moment.

use eframe::egui;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::time::Duration;

/// Language tags that need a system font.
fn needs_system_font(tag: &str) -> bool {
    matches!(tag, "ja-JP" | "zh-CN")
}

/// Font files to try for `tag`, best first; empty for a language the default fonts draw. Windows is
/// listed first (the development platform), then macOS and the usual Linux packages.
pub fn candidates(tag: &str) -> Vec<PathBuf> {
    if !needs_system_font(tag) {
        return Vec::new();
    }
    let windir = std::env::var_os("WINDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    let windows = |name: &str| windir.join("Fonts").join(name);
    let paths: Vec<PathBuf> = match tag {
        "ja-JP" => vec![
            windows("YuGothR.ttc"),
            windows("meiryo.ttc"),
            windows("msgothic.ttc"),
            "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc".into(),
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc".into(),
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc".into(),
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc".into(),
            "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc".into(),
        ],
        _ => vec![
            windows("msyh.ttc"),
            windows("simsun.ttc"),
            "/System/Library/Fonts/PingFang.ttc".into(),
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc".into(),
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc".into(),
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc".into(),
            "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc".into(),
            "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc".into(),
        ],
    };
    paths
}

/// The bytes of the first font of [`candidates`] that can be read.
fn read_first(tag: &str) -> Option<Vec<u8>> {
    candidates(tag)
        .into_iter()
        .find_map(|path| std::fs::read(path).ok())
}

/// Vertical adjustment of the system font. The Japanese font of Windows (Yu Gothic) has taller
/// metrics than the fonts that come with egui, so its glyphs sat above the Latin ones in a line.
fn tweak_for(tag: &str) -> egui::FontTweak {
    match tag {
        "ja-JP" => egui::FontTweak {
            y_offset_factor: 0.18,
            ..Default::default()
        },
        _ => egui::FontTweak::default(),
    }
}

/// The default fonts of egui plus `bytes` as the last resort of every family.
fn definitions_with(bytes: Vec<u8>, tag: &str) -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "system-cjk".into(),
        egui::FontData::from_owned(bytes).tweak(tweak_for(tag)),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        if let Some(list) = fonts.families.get_mut(&family) {
            list.push("system-cjk".into());
        }
    }
    fonts
}

/// Installs the font of `tag` right now, reading the file on this thread: for the offscreen
/// screenshots, which have no UI thread to protect. Returns whether a font was found.
pub fn install_blocking(ctx: &egui::Context, tag: &str) -> bool {
    match read_first(tag) {
        Some(bytes) => {
            ctx.set_fonts(definitions_with(bytes, tag));
            true
        }
        None => false,
    }
}

/// Follows the interface language: loads the system font when Japanese or Chinese is chosen and
/// drops it when another language comes back.
#[derive(Default)]
pub struct CjkFonts {
    /// The language whose font the context has now.
    installed: Option<String>,
    /// A font being read on a worker thread, with the language it is for.
    pending: Option<(String, Receiver<Option<Vec<u8>>>)>,
    /// A language whose font was not found, so the search is not repeated every frame.
    missing: Option<String>,
    /// Definitions waiting for the end of the frame (see [`CjkFonts::apply`]).
    to_apply: Option<egui::FontDefinitions>,
}

impl CjkFonts {
    /// Called every frame with the language of the preference; cheap when nothing changes.
    pub fn update(&mut self, ctx: &egui::Context, tag: &str) {
        if !needs_system_font(tag) {
            self.pending = None;
            self.missing = None;
            if self.installed.take().is_some() {
                self.to_apply = Some(egui::FontDefinitions::default());
                ctx.request_repaint();
            }
            return;
        }
        if self.installed.as_deref() == Some(tag) || self.missing.as_deref() == Some(tag) {
            return;
        }
        if let Some((for_tag, receiver)) = &self.pending {
            if for_tag != tag {
                // the language changed again before the font arrived
                self.pending = None;
            } else {
                match receiver.try_recv() {
                    Ok(Some(bytes)) => {
                        self.to_apply = Some(definitions_with(bytes, tag));
                        ctx.request_repaint();
                        self.installed = Some(tag.to_owned());
                        self.pending = None;
                    }
                    Ok(None) | Err(TryRecvError::Disconnected) => {
                        tracing::warn!("[WARN] no system font with ideographs found for {tag}");
                        self.missing = Some(tag.to_owned());
                        self.pending = None;
                    }
                    Err(TryRecvError::Empty) => {
                        ctx.request_repaint_after(Duration::from_millis(100))
                    }
                }
                return;
            }
        }
        let (sender, receiver) = channel();
        let wanted = tag.to_owned();
        std::thread::spawn(move || {
            let _ = sender.send(read_first(&wanted));
        });
        self.pending = Some((tag.to_owned(), receiver));
        ctx.request_repaint_after(Duration::from_millis(100));
    }

    /// Hands a new font to egui. Called once, at the very end of the frame: egui drops the fonts of
    /// every pixel density when the next pass takes new definitions, and a second window on a monitor
    /// of another scale starts such a pass in the middle of the frame, so a font set earlier made the
    /// first window panic ("No fonts available") the next time it measured a text.
    pub fn apply(&mut self, ctx: &egui::Context) {
        if let Some(definitions) = self.to_apply.take() {
            ctx.set_fonts(definitions);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_japanese_and_chinese_ask_for_a_system_font() {
        for tag in [
            "pt-BR", "en-US", "de-DE", "es-ES", "fr-FR", "ru-RU", "klingon",
        ] {
            assert!(candidates(tag).is_empty(), "{tag}");
        }
        assert!(!candidates("ja-JP").is_empty());
        assert!(!candidates("zh-CN").is_empty());
    }

    #[test]
    fn each_language_prefers_its_own_windows_font() {
        let name = |tag: &str| {
            candidates(tag)[0]
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default()
        };
        assert_eq!(name("ja-JP"), "yugothr.ttc");
        assert_eq!(name("zh-CN"), "msyh.ttc");
    }

    #[test]
    fn a_new_font_waits_for_the_end_of_the_frame() {
        let ctx = egui::Context::default();
        let mut fonts = CjkFonts {
            to_apply: Some(egui::FontDefinitions::default()),
            ..CjkFonts::default()
        };
        // Nothing reaches egui until `apply`, and it is handed over only once
        assert!(fonts.to_apply.is_some());
        fonts.apply(&ctx);
        assert!(fonts.to_apply.is_none());
        fonts.apply(&ctx);
    }

    #[test]
    fn a_language_without_a_font_does_not_touch_the_fonts() {
        let ctx = egui::Context::default();
        let mut fonts = CjkFonts::default();
        fonts.update(&ctx, "pt-BR");
        fonts.update(&ctx, "en-US");
        assert!(fonts.installed.is_none() && fonts.pending.is_none());
    }
}
