//! Named window layouts saved on disk (manual v21 p.87-88): the arrangement of the docked windows
//! under a name, optionally with a number 1 to 9 that `Alt+D` then that number recalls. One layout is
//! one `.neoveglayout` file (JSON) in the per-user layouts folder, so layouts can be moved between
//! computers by copying the files.
//!
//! This module is plain file handling with tests; the application decides when to call it.

use std::path::{Path, PathBuf};

use egui_dock::DockState;
use serde::{Deserialize, Serialize};

use crate::app::Tab;

/// Version of the file; a newer one is refused with a clear message.
pub const FORMAT_VERSION: u32 = 1;

/// Extension of a layout file.
pub const EXTENSION: &str = "neoveglayout";

/// The layout slots the keyboard reaches.
pub const SLOTS: std::ops::RangeInclusive<u8> = 1..=9;

#[derive(Debug, Serialize, Deserialize)]
pub struct LayoutFile {
    pub format_version: u32,
    pub name: String,
    /// The number that recalls this layout (`Alt+D`, then the number), if any.
    #[serde(default)]
    pub slot: Option<u8>,
    pub dock: DockState<Tab>,
}

/// A layout found in the folder.
#[derive(Debug)]
pub struct SavedLayout {
    pub path: PathBuf,
    pub file: LayoutFile,
}

/// The per-user folder of the layouts.
pub fn dir() -> Option<PathBuf> {
    neoveg_common::paths::app_data_dir().map(|d| d.join("layouts"))
}

/// A file name for `name`: letters, digits, `-` and `_` stay, anything else becomes `_`.
pub fn file_name(name: &str) -> String {
    let stem: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let stem = if stem.is_empty() {
        "layout".to_owned()
    } else {
        stem
    };
    format!("{stem}.{EXTENSION}")
}

/// The dock keeps rectangles that are infinite until the first frame lays them out, and JSON writes
/// an infinite number as `null`. They are recomputed on every frame, so a `null` point reads as zero.
fn zero_unset_points(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for key in ["x", "y"] {
                if map.get(key).is_some_and(serde_json::Value::is_null) {
                    map.insert(key.to_owned(), serde_json::Value::from(0.0));
                }
            }
            map.values_mut().for_each(zero_unset_points);
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(zero_unset_points),
        _ => {}
    }
}

fn read(path: &Path) -> Result<LayoutFile, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("não foi possível ler {}: {e}", path.display()))?;
    let mut value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("layout inválido {}: {e}", path.display()))?;
    zero_unset_points(&mut value);
    let file: LayoutFile = serde_json::from_value(value)
        .map_err(|e| format!("layout inválido {}: {e}", path.display()))?;
    if file.format_version > FORMAT_VERSION {
        return Err(format!(
            "o layout {} foi gravado por uma versão mais nova do Neoveg",
            path.display()
        ));
    }
    if file.dock.iter_all_tabs().next().is_none() {
        return Err(format!(
            "o layout {} não tem nenhuma janela",
            path.display()
        ));
    }
    Ok(file)
}

/// Writes `file` to `path` atomically (a temporary file, then a rename).
fn write(path: &Path, file: &LayoutFile) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("não foi possível criar a pasta {}: {e}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(file)
        .map_err(|e| format!("não foi possível gravar o layout: {e}"))?;
    let temp = path.with_extension(format!("{EXTENSION}.tmp"));
    std::fs::write(&temp, json)
        .map_err(|e| format!("não foi possível gravar {}: {e}", temp.display()))?;
    std::fs::rename(&temp, path).map_err(|e| {
        let _ = std::fs::remove_file(&temp);
        format!("não foi possível gravar {}: {e}", path.display())
    })
}

/// The layouts in `dir`, by name. Files that cannot be read, are broken or come from a newer
/// version are left out (and logged).
pub fn list(dir: &Path) -> Vec<SavedLayout> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<SavedLayout> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == EXTENSION))
        .filter_map(|path| match read(&path) {
            Ok(file) => Some(SavedLayout { path, file }),
            Err(e) => {
                tracing::warn!("[WARN] layout ignored: {e}");
                None
            }
        })
        .collect();
    found.sort_by_key(|l| l.file.name.to_lowercase());
    found
}

/// Saves `dock` as `name` in `dir`, with `slot` when given: the slot is taken away from any other
/// layout that had it. Saving under a name that exists replaces that layout. Returns the file.
pub fn save(
    dir: &Path,
    name: &str,
    slot: Option<u8>,
    dock: &DockState<Tab>,
) -> Result<PathBuf, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("dê um nome ao layout".into());
    }
    if slot.is_some_and(|s| !SLOTS.contains(&s)) {
        return Err("o atalho do layout vai de 1 a 9".into());
    }
    let path = dir.join(file_name(name));
    if let Some(slot) = slot {
        release_slot(dir, slot, Some(&path))?;
    }
    write(
        &path,
        &LayoutFile {
            format_version: FORMAT_VERSION,
            name: name.to_owned(),
            slot,
            dock: dock.clone(),
        },
    )?;
    Ok(path)
}

/// Takes `slot` away from the layouts that hold it, except the one at `keep`.
fn release_slot(dir: &Path, slot: u8, keep: Option<&Path>) -> Result<(), String> {
    for mut other in list(dir) {
        if other.file.slot == Some(slot) && keep != Some(other.path.as_path()) {
            other.file.slot = None;
            write(&other.path, &other.file)?;
        }
    }
    Ok(())
}

/// Gives the layout at `path` the shortcut number `slot` (or none).
pub fn set_slot(dir: &Path, path: &Path, slot: Option<u8>) -> Result<(), String> {
    if slot.is_some_and(|s| !SLOTS.contains(&s)) {
        return Err("o atalho do layout vai de 1 a 9".into());
    }
    if let Some(slot) = slot {
        release_slot(dir, slot, Some(path))?;
    }
    let mut file = read(path)?;
    file.slot = slot;
    write(path, &file)
}

/// Deletes the layout file.
pub fn remove(path: &Path) -> Result<(), String> {
    std::fs::remove_file(path)
        .map_err(|e| format!("não foi possível excluir {}: {e}", path.display()))
}

/// The layout that holds `slot`.
pub fn by_slot(layouts: &[SavedLayout], slot: u8) -> Option<&SavedLayout> {
    layouts.iter().find(|l| l.file.slot == Some(slot))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dock(tabs: Vec<Tab>) -> DockState<Tab> {
        DockState::new(tabs)
    }

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("neoveg_layouts_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_layout_survives_the_round_trip_with_its_name_slot_and_windows() {
        let dir = temp("roundtrip");
        let saved = dock(vec![Tab::Preview, Tab::Timeline]);
        let path = save(&dir, "Mixagem de áudio", Some(4), &saved).unwrap();
        assert_eq!(path.extension().unwrap(), EXTENSION);
        let found = list(&dir);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file.name, "Mixagem de áudio");
        assert_eq!(found[0].file.slot, Some(4));
        assert!(found[0].file.dock.find_tab(&Tab::Timeline).is_some());
        assert!(found[0].file.dock.find_tab(&Tab::Preview).is_some());
        assert!(found[0].file.dock.find_tab(&Tab::Media).is_none());
        assert!(by_slot(&found, 4).is_some() && by_slot(&found, 5).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_slot_belongs_to_one_layout_and_saving_again_replaces_the_layout() {
        let dir = temp("slots");
        let a = dock(vec![Tab::Preview]);
        let b = dock(vec![Tab::Timeline]);
        let first = save(&dir, "A", Some(2), &a).unwrap();
        save(&dir, "B", Some(2), &b).unwrap();
        let found = list(&dir);
        assert_eq!(found.len(), 2);
        assert_eq!(by_slot(&found, 2).unwrap().file.name, "B", "B took slot 2");
        assert_eq!(
            found.iter().find(|l| l.path == first).unwrap().file.slot,
            None
        );
        // The same name again replaces it, not a third file
        save(&dir, "B", None, &a).unwrap();
        let found = list(&dir);
        assert_eq!(found.len(), 2);
        assert!(found
            .iter()
            .find(|l| l.file.name == "B")
            .unwrap()
            .file
            .dock
            .find_tab(&Tab::Preview)
            .is_some());
        // A slot given later moves like that
        set_slot(&dir, &first, Some(2)).unwrap();
        let found = list(&dir);
        assert_eq!(by_slot(&found, 2).unwrap().file.name, "A");
        set_slot(&dir, &first, None).unwrap();
        assert!(by_slot(&list(&dir), 2).is_none());
        // Delete
        remove(&first).unwrap();
        assert_eq!(list(&dir).len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bad_input_is_an_error_and_bad_files_are_skipped() {
        let dir = temp("bad");
        let d = dock(vec![Tab::Preview]);
        assert!(save(&dir, "  ", None, &d).is_err(), "no name");
        assert!(save(&dir, "X", Some(0), &d).is_err() && save(&dir, "X", Some(10), &d).is_err());
        assert!(set_slot(&dir, &dir.join("nada.neoveglayout"), Some(10)).is_err());
        save(&dir, "Boa", None, &d).unwrap();
        std::fs::write(dir.join("quebrada.neoveglayout"), "{ não é json").unwrap();
        // A file from a newer version, and a file with no windows at all
        let newer = LayoutFile {
            format_version: FORMAT_VERSION + 1,
            name: "Futuro".into(),
            slot: None,
            dock: d.clone(),
        };
        std::fs::write(
            dir.join("futuro.neoveglayout"),
            serde_json::to_string(&newer).unwrap(),
        )
        .unwrap();
        let found = list(&dir);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file.name, "Boa");
        // A folder that does not exist has no layouts
        assert!(list(&dir.join("nao_existe")).is_empty());
        // No temporary file is left behind
        assert!(std::fs::read_dir(&dir).unwrap().all(|e| !e
            .unwrap()
            .path()
            .to_string_lossy()
            .ends_with(".tmp")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn file_names_are_safe() {
        assert_eq!(
            file_name("Cor e Texto!"),
            format!("Cor_e_Texto_.{EXTENSION}")
        );
        assert_eq!(file_name("../../x"), format!("______x.{EXTENSION}"));
        assert_eq!(file_name("   "), format!("layout.{EXTENSION}"));
    }
}
