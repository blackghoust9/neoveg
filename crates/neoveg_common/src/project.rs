//! Project-level types shared by the engine and the UI.

use serde::{Deserialize, Serialize};

/// Video and audio properties of a project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectSettings {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub sample_rate: u32,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self { width: 1920, height: 1080, fps: 29.97, sample_rate: 48_000 }
    }
}

/// Reference to a media file in the project pool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaRef {
    pub id: u64,
    pub path: String,
    pub duration_seconds: f64,
}

/// Top-level project metadata. Neoveg starts with an empty project (no media).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub name: String,
    pub settings: ProjectSettings,
    pub media_pool: Vec<MediaRef>,
}

impl ProjectInfo {
    pub fn create_empty() -> Self {
        Self { name: "Untitled".into(), settings: ProjectSettings::default(), media_pool: Vec::new() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_project_has_no_media() {
        assert!(ProjectInfo::create_empty().media_pool.is_empty());
    }
}
