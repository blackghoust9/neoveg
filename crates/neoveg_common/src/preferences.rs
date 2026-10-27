//! User preferences persisted as JSON.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::keymap::KeyMap;
use crate::presets::ProjectTemplate;
use crate::project::ProjectSettings;

/// Most recent projects kept, whatever the user asks for.
pub const MAX_RECENT_PROJECTS: u32 = 30;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    /// UI language tag (default Brazilian Portuguese).
    pub language: String,
    /// Fade in/out applied automatically on media import (disabled by default).
    pub auto_fade_on_import: bool,
    /// Default timeline duration in seconds assigned to imported still images.
    pub still_image_seconds: f64,
    /// Show the welcome screen (recent projects, new project) when the program starts.
    pub show_welcome_on_startup: bool,
    /// Project files opened or saved lately, newest first.
    pub recent_projects: Vec<String>,
    /// How many recent projects to remember and list (`0` turns the list off).
    pub recent_projects_max: u32,
    /// Settings every new project starts with, when the user chose "start all new projects with
    /// these settings" in the project properties.
    pub new_project_settings: Option<ProjectSettings>,
    /// Templates the user saved from the project properties.
    pub custom_templates: Vec<ProjectTemplate>,
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
            show_welcome_on_startup: true,
            recent_projects: Vec::new(),
            recent_projects_max: 10,
            new_project_settings: None,
            custom_templates: Vec::new(),
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

    /// Puts `path` at the top of the recent projects (without repeating it) and trims the list to
    /// the size the user chose.
    pub fn add_recent_project(&mut self, path: &Path) {
        let text = path.to_string_lossy().into_owned();
        self.recent_projects.retain(|p| p != &text);
        self.recent_projects.insert(0, text);
        self.trim_recent_projects();
    }

    /// Forgets one recent project (a file that no longer exists, or one the user removed).
    pub fn remove_recent_project(&mut self, path: &str) {
        self.recent_projects.retain(|p| p != path);
    }

    /// Cuts the list to `recent_projects_max` (never more than [`MAX_RECENT_PROJECTS`]).
    pub fn trim_recent_projects(&mut self) {
        let keep = self.recent_projects_max.min(MAX_RECENT_PROJECTS) as usize;
        self.recent_projects.truncate(keep);
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
        assert!(
            p.show_welcome_on_startup,
            "the welcome screen is on unless turned off"
        );
        assert!(p.recent_projects.is_empty() && p.new_project_settings.is_none());
        // Files written before the keymap existed keep the default shortcuts
        assert!(p.keymap.is_empty());
    }

    #[test]
    fn recent_projects_keep_the_newest_first_without_repeats_and_respect_the_limit() {
        let mut p = Preferences {
            recent_projects_max: 3,
            ..Preferences::default()
        };
        for name in ["a.neoveg", "b.neoveg", "c.neoveg", "a.neoveg", "d.neoveg"] {
            p.add_recent_project(Path::new(name));
        }
        assert_eq!(p.recent_projects, vec!["d.neoveg", "a.neoveg", "c.neoveg"]);
        p.remove_recent_project("a.neoveg");
        assert_eq!(p.recent_projects, vec!["d.neoveg", "c.neoveg"]);
        p.recent_projects_max = 0;
        p.trim_recent_projects();
        assert!(p.recent_projects.is_empty(), "0 turns the list off");
        p.recent_projects_max = 1_000;
        for i in 0..100 {
            p.add_recent_project(Path::new(&format!("p{i}.neoveg")));
        }
        assert_eq!(p.recent_projects.len() as u32, MAX_RECENT_PROJECTS);
    }

    #[test]
    fn new_project_settings_and_templates_survive_a_save_and_load() {
        let dir = std::env::temp_dir().join(format!("neoveg_prefs_tpl_{}", std::process::id()));
        let path = dir.join("preferences.json");
        let mut p = Preferences::default();
        let settings = ProjectSettings {
            width: 3840,
            height: 2160,
            ..ProjectSettings::default()
        };
        p.new_project_settings = Some(settings.clone());
        p.custom_templates.push(ProjectTemplate {
            name: "Meu 4K".into(),
            settings,
        });
        p.add_recent_project(Path::new("x.neoveg"));
        p.save(&path).unwrap();
        let back = Preferences::load(&path);
        assert_eq!(back, p);
        let _ = std::fs::remove_dir_all(dir);
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
