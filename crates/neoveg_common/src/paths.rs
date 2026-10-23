//! Per-user data locations.

use std::path::PathBuf;

/// Per-user application data folder (`%LOCALAPPDATA%\Neoveg`, `~/.local/share/neoveg` or
/// `~/Library/Application Support/neoveg`).
pub fn app_data_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("Neoveg"))
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .map(|h| PathBuf::from(h).join("Library/Application Support/neoveg"))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .map(|p| p.join("neoveg"))
    }
}

/// Folder holding `recovery_*.neoveg` autosave files.
pub fn autosave_dir() -> Option<PathBuf> {
    app_data_dir().map(|d| d.join("autosave"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autosave_dir_lives_under_the_app_data_dir() {
        if let (Some(data), Some(auto)) = (app_data_dir(), autosave_dir()) {
            assert!(auto.starts_with(data));
            assert!(auto.ends_with("autosave"));
        }
    }
}
