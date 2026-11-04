//! Named keyboard maps: the changes a user made to the shortcuts, kept under a name so they can be
//! saved as, loaded, deleted, exported to a file and imported from one.
//!
//! A map is the same `command id -> [shortcuts]` table that `preferences.json` keeps for the
//! active shortcuts; only what differs from the defaults is stored. Maps live one to a file
//! (`<name>.keymap.json`) in a folder next to the preferences, and every function takes that folder
//! so it can be tested on a temporary one.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::keymap::KeyMap;
use crate::preferences::Preferences;

/// Ending of the file of a map.
const SUFFIX: &str = ".keymap.json";
/// Longest name of a map, in characters.
pub const MAX_NAME: usize = 40;

/// A keyboard map with its name.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct NamedKeymap {
    /// A file without a name takes the name of the file when it is imported.
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub overrides: BTreeMap<String, Vec<String>>,
}

/// The folder where the maps are kept: `keymaps` next to `preferences.json`.
pub fn default_dir() -> Option<PathBuf> {
    Preferences::default_path().and_then(|p| p.parent().map(|d| d.join("keymaps")))
}

/// The name as it is stored: trimmed, no control characters, at most [`MAX_NAME`] characters, and
/// nothing that is not allowed in a file name. `None` when nothing usable is left.
pub fn clean_name(name: &str) -> Option<String> {
    let cleaned: String = name
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_NAME)
        .collect();
    let cleaned = cleaned.trim().trim_end_matches('.').to_string();
    let forbidden = |c: char| matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|');
    (!cleaned.is_empty() && !cleaned.chars().any(forbidden)).then_some(cleaned)
}

fn path_for(dir: &Path, name: &str) -> Option<PathBuf> {
    Some(dir.join(format!("{}{SUFFIX}", clean_name(name)?)))
}

/// The names of the maps in `dir`, in alphabetical order (an empty list when the folder is missing).
pub fn list(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let file = entry.file_name().to_string_lossy().into_owned();
            file.strip_suffix(SUFFIX).map(str::to_string)
        })
        .filter(|name| clean_name(name).is_some())
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    names
}

/// Saves `overrides` as the map `name` (replacing a map with that name); returns the name as stored.
pub fn save(
    dir: &Path,
    name: &str,
    overrides: &BTreeMap<String, Vec<String>>,
) -> Result<String, String> {
    let clean = clean_name(name).ok_or_else(|| {
        "nome inválido: use até 40 letras, números ou espaços, sem / \\ : * ? \" < > |".to_string()
    })?;
    let set = NamedKeymap {
        name: clean.clone(),
        overrides: overrides.clone(),
    };
    std::fs::create_dir_all(dir).map_err(|e| format!("não foi possível criar a pasta: {e}"))?;
    let path = path_for(dir, &clean).ok_or("nome inválido")?;
    write(&path, &set)?;
    Ok(clean)
}

fn write(path: &Path, set: &NamedKeymap) -> Result<(), String> {
    let json = serde_json::to_string_pretty(set).map_err(|e| e.to_string())?;
    std::fs::write(path, json)
        .map_err(|e| format!("não foi possível gravar {}: {e}", path.display()))
}

fn read(path: &Path) -> Result<NamedKeymap, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("não foi possível ler {}: {e}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|e| format!("{} não é um mapa de teclado: {e}", path.display()))
}

/// The map `name` from `dir`.
pub fn load(dir: &Path, name: &str) -> Result<NamedKeymap, String> {
    let path = path_for(dir, name).ok_or("nome inválido")?;
    let mut set = read(&path)?;
    set.name = clean_name(name).unwrap_or(set.name);
    Ok(set)
}

/// Deletes the map `name` from `dir`.
pub fn delete(dir: &Path, name: &str) -> Result<(), String> {
    let path = path_for(dir, name).ok_or("nome inválido")?;
    std::fs::remove_file(&path)
        .map_err(|e| format!("não foi possível excluir {}: {e}", path.display()))
}

/// Writes a map to any file, to give it to someone else.
pub fn export(path: &Path, set: &NamedKeymap) -> Result<(), String> {
    write(path, set)
}

/// Reads a map from any file. A map with no name takes the name of the file.
pub fn import(path: &Path) -> Result<NamedKeymap, String> {
    let mut set = read(path)?;
    if clean_name(&set.name).is_none() {
        let stem = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| {
                n.strip_suffix(SUFFIX)
                    .or_else(|| n.strip_suffix(".json"))
                    .unwrap_or(n)
            })
            .unwrap_or("importado");
        set.name = clean_name(stem).unwrap_or_else(|| "importado".into());
    } else {
        set.name = clean_name(&set.name).unwrap_or_default();
    }
    Ok(set)
}

/// What the map would change that cannot be applied (commands that no longer exist, shortcuts that
/// do not parse), as sentences for the user.
pub fn problems(overrides: &BTreeMap<String, Vec<String>>) -> Vec<String> {
    KeyMap::from_overrides(overrides).1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("neoveg_keymaps_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn sample() -> BTreeMap<String, Vec<String>> {
        BTreeMap::from([(
            "edit.split".to_string(),
            vec!["Ctrl+Alt+Shift+F11".to_string()],
        )])
    }

    #[test]
    fn names_are_cleaned_and_unsafe_ones_are_refused() {
        assert_eq!(clean_name("  Meu mapa  ").as_deref(), Some("Meu mapa"));
        assert_eq!(
            clean_name("Edição rápida").as_deref(),
            Some("Edição rápida")
        );
        assert_eq!(clean_name("nome.").as_deref(), Some("nome"));
        for bad in [
            "", "   ", "a/b", "a\\b", "c:", "x*y", "q?", "<t>", "a|b", "..",
        ] {
            assert_eq!(clean_name(bad), None, "{bad:?}");
        }
        assert_eq!(
            clean_name(&"x".repeat(100)).unwrap().chars().count(),
            MAX_NAME
        );
    }

    #[test]
    fn a_map_is_saved_listed_loaded_and_deleted() {
        let dir = temp("cycle");
        assert!(list(&dir).is_empty(), "a missing folder is an empty list");
        assert_eq!(save(&dir, "  Zeta ", &sample()).unwrap(), "Zeta");
        save(&dir, "alfa", &BTreeMap::new()).unwrap();
        assert_eq!(list(&dir), ["alfa", "Zeta"], "alphabetical, ignoring case");
        let loaded = load(&dir, "Zeta").unwrap();
        assert_eq!(loaded.name, "Zeta");
        assert_eq!(loaded.overrides, sample());
        // Saving under the same name replaces it
        save(&dir, "Zeta", &BTreeMap::new()).unwrap();
        assert!(load(&dir, "Zeta").unwrap().overrides.is_empty());
        delete(&dir, "Zeta").unwrap();
        assert_eq!(list(&dir), ["alfa"]);
        assert!(delete(&dir, "Zeta").is_err());
        assert!(load(&dir, "Zeta").is_err());
        assert!(save(&dir, "a/b", &sample()).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_map_travels_through_an_exported_file_and_a_wrong_file_is_refused() {
        let dir = temp("export");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("para-um-amigo.json");
        let set = NamedKeymap {
            name: "Edição".into(),
            overrides: sample(),
        };
        export(&file, &set).unwrap();
        assert_eq!(import(&file).unwrap(), set);
        // A map without a name takes the file's name
        std::fs::write(&file, r#"{"overrides":{"edit.split":["S"]}}"#).unwrap();
        assert_eq!(import(&file).unwrap().name, "para-um-amigo");
        std::fs::write(&file, "isto não é json").unwrap();
        assert!(import(&file)
            .unwrap_err()
            .contains("não é um mapa de teclado"));
        assert!(import(&dir.join("nada.json")).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_map_with_commands_that_do_not_exist_is_reported() {
        assert!(problems(&sample()).is_empty());
        let wrong = BTreeMap::from([("nao.existe".to_string(), vec!["X".to_string()])]);
        assert_eq!(problems(&wrong).len(), 1);
    }
}
