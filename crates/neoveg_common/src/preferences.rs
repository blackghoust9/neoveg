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
    pub show_welcome_on_startup: bool,
    /// Minutes between automatic recovery saves (0 disables autosave).
    pub autosave_minutes: u32,
    /// Keyboard shortcuts that differ from the defaults: command id -> shortcuts (empty = unbound).
    pub keymap: BTreeMap<String, Vec<String>>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            language: "pt-BR".into(),
            auto_fade_on_import: false,
            show_welcome_on_startup: false,
            autosave_minutes: 2,
            keymap: BTreeMap::new(),
        }
    }
}

impl Preferences {
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
    fn keymap_overrides_survive_a_save_and_load() {
        let mut p = Preferences::default();
        p.keymap
            .insert("edit.split".into(), vec!["K".into(), "Ctrl+K".into()]);
        let json = serde_json::to_string(&p).unwrap();
        let back: Preferences = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
        let (map, warnings) = back.key_map();
        assert!(warnings.is_empty());
        let k = "K".parse().unwrap();
        assert_eq!(
            map.command_for(&k),
            Some(crate::commands::Command::EditSplit)
        );
    }
}
