//! Project-level types shared by the engine and the UI.

use serde::{Deserialize, Serialize};

/// Video and audio properties of a project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectSettings {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub sample_rate: u32,
    /// Overlapping events on the same track crossfade automatically.
    #[serde(default = "default_true")]
    pub auto_crossfade: bool,
    /// Tempo of the metronome and of the beat grid, beats per minute.
    #[serde(default = "default_bpm")]
    pub bpm: f32,
    /// Beats in a bar (the metronome accents the first one).
    #[serde(default = "default_beats_per_bar")]
    pub beats_per_bar: u32,
    /// Peak level (dBFS) an event reaches with the Normalize switch.
    #[serde(default)]
    pub normalize_peak_db: f64,
}

fn default_bpm() -> f32 {
    120.0
}

fn default_beats_per_bar() -> u32 {
    4
}

fn default_true() -> bool {
    true
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: 29.97,
            sample_rate: 48_000,
            auto_crossfade: true,
            bpm: default_bpm(),
            beats_per_bar: default_beats_per_bar(),
            normalize_peak_db: 0.0,
        }
    }
}

/// Reference to a media file in the project pool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaRef {
    pub id: u64,
    pub path: String,
    pub duration_seconds: f64,
    /// `true` when `path` is another `.neoveg` project played as one clip (a nested sequence).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub subproject: bool,
}

/// Where a media pool entry comes from.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MediaSource {
    /// An audio, video or image file.
    File(std::path::PathBuf),
    /// A `.neoveg` project nested as one clip.
    SubProject(std::path::PathBuf),
}

impl MediaSource {
    pub fn path(&self) -> &std::path::Path {
        match self {
            MediaSource::File(p) | MediaSource::SubProject(p) => p,
        }
    }

    pub fn is_subproject(&self) -> bool {
        matches!(self, MediaSource::SubProject(_))
    }
}

impl MediaRef {
    /// The kind of media this entry is, with its path.
    pub fn source(&self) -> MediaSource {
        let path = std::path::PathBuf::from(&self.path);
        if self.subproject {
            MediaSource::SubProject(path)
        } else {
            MediaSource::File(path)
        }
    }
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
        Self {
            name: "Untitled".into(),
            settings: ProjectSettings::default(),
            media_pool: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_media_ref_knows_whether_it_is_a_subproject_and_old_files_load() {
        let old: MediaRef =
            serde_json::from_str(r#"{"id":1,"path":"a.wav","duration_seconds":2.0}"#).unwrap();
        assert!(!old.subproject);
        assert_eq!(old.source(), MediaSource::File("a.wav".into()));
        let sub = MediaRef {
            subproject: true,
            ..old.clone()
        };
        assert!(sub.source().is_subproject());
        assert_eq!(sub.source().path(), std::path::Path::new("a.wav"));
        // A plain file does not write the flag
        assert!(!serde_json::to_string(&old).unwrap().contains("subproject"));
        assert!(serde_json::to_string(&sub)
            .unwrap()
            .contains("\"subproject\":true"));
    }

    use super::*;

    #[test]
    fn empty_project_has_no_media() {
        assert!(ProjectInfo::create_empty().media_pool.is_empty());
    }
}
