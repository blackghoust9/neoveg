//! User preferences persisted as JSON.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::keymap::KeyMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    /// UI language tag (default Brazilian Portuguese).
    pub language: String,
    /// Fade in/out applied automatically on media import (disabled by default).
    pub auto_fade_on_import: bool,
    /// Default timeline duration in seconds assigned to imported still images.
    pub still_image_seconds: f64,
    pub show_welcome_on_startup: bool,
    /// Minutes between automatic recovery saves (0 disables autosave).
    pub autosave_minutes: u32,
    /// Built-in theme id, or `custom` to use `theme_file`.
    pub theme: String,
    /// Path of the user's theme `.json` (used when `theme` is `custom`).
    pub theme_file: String,
    /// Keyboard shortcuts that differ from the defaults: command id -> shortcuts (empty = unbound).
    pub keymap: BTreeMap<String, Vec<String>>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            language: "pt-BR".into(),
            auto_fade_on_import: false,
            still_image_seconds: 5.0,
            show_welcome_on_startup: false,
            autosave_minutes: 2,
            theme: "modern-carbon-dark".into(),
            theme_file: String::new(),
            keymap: BTreeMap::new(),
        }
    }
}

impl Preferences {
    /// The theme to show: the user's file when selected and readable, else the chosen built-in
    /// (Modern Carbon Dark when unknown). The second value is a warning for a broken file.
    pub fn resolve_theme(&self) -> (crate::theme::Theme, Option<String>) {
        use crate::theme::{Theme, CUSTOM_THEME_ID};
        if self.theme == CUSTOM_THEME_ID {
            return match Theme::load(Path::new(&self.theme_file)) {
                Ok(theme) => (theme, None),
                Err(e) => (Theme::default(), Some(e)),
            };
        }
        (Theme::builtin(&self.theme).unwrap_or_default(), None)
    }

    /// The effective keyboard map (defaults plus this user's overrides) and any warnings about
    /// entries that could not be applied.
    pub fn key_map(&self) -> (KeyMap, Vec<String>) {
        KeyMap::from_overrides(&self.keymap)
    }

    /// Per-user preferences file (`%APPDATA%`, `~/.config` or `~/Library/Application Support`).
    pub fn default_path() -> Option<PathBuf> {
        let base = if cfg!(windows) {
            std::env::var_os("APPDATA").map(PathBuf::from)
        } else if cfg!(target_os = "macos") {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
        } else {
            std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        };
        base.map(|b| b.join("neoveg").join("preferences.json"))
    }

    /// Loads preferences, falling back to defaults when the file is missing or invalid.
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_spec() {
        let p = Preferences::default();
        assert_eq!(p.language, "pt-BR");
        assert!(!p.auto_fade_on_import);
        assert_eq!(p.still_image_seconds, 5.0);
        assert_eq!(p.autosave_minutes, 2);
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let p: Preferences = serde_json::from_str(r#"{"language":"en-US"}"#).unwrap();
        assert_eq!(p.language, "en-US");
        assert!(!p.show_welcome_on_startup);
        // Files written before the keymap existed keep the default shortcuts
        assert!(p.keymap.is_empty());
    }

    #[test]
    fn theme_resolves_builtins_and_falls_back_on_a_broken_file() {
        let mut p = Preferences::default();
        assert_eq!(p.resolve_theme().0.name, "Modern Carbon Dark");
        p.theme = "classic-silver".into();
        assert!(!p.resolve_theme().0.dark);
        p.theme = "unknown".into();
        assert_eq!(p.resolve_theme().0.name, "Modern Carbon Dark");
        p.theme = "custom".into();
        p.theme_file = "/missing/theme.json".into();
        let (theme, warning) = p.resolve_theme();
        assert_eq!(theme.name, "Modern Carbon Dark");
        assert!(warning.is_some());
        let old: Preferences = serde_json::from_str(r#"{"language":"pt-BR"}"#).unwrap();
        assert_eq!(old.theme, "modern-carbon-dark");
    }

    #[test]
    fn keymap_overrides_survive_a_save_and_load() {
        let mut p = Preferences::default();
        p.keymap
            .insert("edit.split".into(), vec!["F10".into(), "Ctrl+K".into()]);
        let json = serde_json::to_string(&p).unwrap();
        let back: Preferences = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
        let (map, warnings) = back.key_map();
        assert!(warnings.is_empty());
        let k = "F10".parse().unwrap();
        assert_eq!(
            map.command_for(&k),
            Some(crate::commands::Command::EditSplit)
        );
    }
}
