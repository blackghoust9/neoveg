//! Project-level types shared by the engine and the UI.

use crate::presets::{
    CompositingGamma, Deinterlace, FieldOrder, HdrMode, MasterBusMode, MotionBlurType,
    OutputRotation, PixelFormat, RenderQuality, ResampleMode,
};
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
    /// Width of a pixel over its height (`1.0` for computer screens; television formats differ).
    #[serde(default = "default_pixel_aspect")]
    pub pixel_aspect: f64,
    #[serde(default)]
    pub field_order: FieldOrder,
    #[serde(default)]
    pub pixel_format: PixelFormat,
    /// Gamma of the blending (32-bit full range only).
    #[serde(default)]
    pub compositing_gamma: CompositingGamma,
    #[serde(default)]
    pub output_rotation: OutputRotation,
    #[serde(default)]
    pub hdr_mode: HdrMode,
    /// Quality of the final render's scaling.
    #[serde(default)]
    pub render_quality: RenderQuality,
    #[serde(default)]
    pub motion_blur: MotionBlurType,
    #[serde(default)]
    pub deinterlace: Deinterlace,
    /// Default resampling of media whose frame rate is below the project's.
    #[serde(default)]
    pub resample_mode: ResampleMode,
    /// Bit depth of the project audio (`32` is floating point).
    #[serde(default = "default_audio_bit_depth")]
    pub audio_bit_depth: u32,
    #[serde(default)]
    pub master_bus: MasterBusMode,
}

fn default_pixel_aspect() -> f64 {
    1.0
}

fn default_audio_bit_depth() -> u32 {
    24
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
            pixel_aspect: 1.0,
            field_order: FieldOrder::default(),
            pixel_format: PixelFormat::default(),
            compositing_gamma: CompositingGamma::default(),
            output_rotation: OutputRotation::default(),
            hdr_mode: HdrMode::default(),
            render_quality: RenderQuality::default(),
            motion_blur: MotionBlurType::default(),
            deinterlace: Deinterlace::default(),
            resample_mode: ResampleMode::default(),
            audio_bit_depth: 24,
            master_bus: MasterBusMode::default(),
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
    /// What the file was like when it was loaded, so a replacement found later can be checked
    /// against it (older projects have none).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<MediaFormat>,
}

/// The traits of a media file that a replacement has to keep: the number of sound channels and the
/// frame rate of the picture (the duration is `MediaRef::duration_seconds`).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct MediaFormat {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channels: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fps: Option<f64>,
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

/// Colours a note can take (the index is stored in the project).
pub const NOTE_COLORS: [(&str, [u8; 3]); 6] = [
    ("Amarelo", [0xF1, 0xC4, 0x0F]),
    ("Vermelho", [0xE7, 0x4C, 0x3C]),
    ("Verde", [0x2E, 0xCC, 0x71]),
    ("Azul", [0x34, 0x98, 0xDB]),
    ("Roxo", [0x9B, 0x59, 0xB6]),
    ("Cinza", [0x95, 0xA5, 0xA6]),
];

/// A note pinned to a moment of the timeline: a reminder for oneself or a comment for a
/// collaborator, sorted by time, colour-coded, with a label, and marked as resolved when dealt with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectNote {
    pub id: u64,
    /// Timeline position in seconds.
    pub time: f64,
    pub text: String,
    #[serde(default)]
    pub label: String,
    /// Index into [`NOTE_COLORS`].
    #[serde(default)]
    pub color: u8,
    #[serde(default)]
    pub resolved: bool,
}

impl ProjectNote {
    /// The colour of the note (the first one when the stored index is out of range).
    pub fn rgb(&self) -> [u8; 3] {
        NOTE_COLORS[usize::from(self.color).min(NOTE_COLORS.len() - 1)].1
    }
}

/// The notes in time order, optionally without the resolved ones.
pub fn notes_in_order(notes: &[ProjectNote], hide_resolved: bool) -> Vec<&ProjectNote> {
    let mut list: Vec<&ProjectNote> = notes
        .iter()
        .filter(|n| !(hide_resolved && n.resolved))
        .collect();
    list.sort_by(|a, b| a.time.total_cmp(&b.time).then(a.id.cmp(&b.id)));
    list
}

/// An id no note has yet.
pub fn next_note_id(notes: &[ProjectNote]) -> u64 {
    notes.iter().map(|n| n.id).max().map_or(1, |m| m + 1)
}

/// Top-level project metadata. Neoveg starts with an empty project (no media).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub name: String,
    pub settings: ProjectSettings,
    pub media_pool: Vec<MediaRef>,
    /// Notes pinned to the timeline (Exibir > Notas do projeto).
    #[serde(default)]
    pub notes: Vec<ProjectNote>,
}

impl ProjectInfo {
    pub fn create_empty() -> Self {
        Self {
            name: "Untitled".into(),
            settings: ProjectSettings::default(),
            media_pool: Vec::new(),
            notes: Vec::new(),
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
            format: None,
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

    fn note(id: u64, time: f64, resolved: bool) -> ProjectNote {
        ProjectNote {
            id,
            time,
            text: format!("nota {id}"),
            label: String::new(),
            color: 0,
            resolved,
        }
    }

    #[test]
    fn notes_come_in_time_order_and_resolved_ones_can_be_hidden() {
        let notes = vec![note(1, 9.0, false), note(2, 2.0, true), note(3, 2.0, false)];
        let all: Vec<u64> = notes_in_order(&notes, false).iter().map(|n| n.id).collect();
        assert_eq!(all, vec![2, 3, 1], "same time: the older note first");
        let open: Vec<u64> = notes_in_order(&notes, true).iter().map(|n| n.id).collect();
        assert_eq!(open, vec![3, 1]);
        assert_eq!(next_note_id(&notes), 4);
        assert_eq!(next_note_id(&[]), 1);
    }

    #[test]
    fn a_note_colour_outside_the_palette_falls_back_to_the_last_one() {
        let mut n = note(1, 0.0, false);
        n.color = 200;
        assert_eq!(n.rgb(), NOTE_COLORS[NOTE_COLORS.len() - 1].1);
        n.color = 1;
        assert_eq!(n.rgb(), NOTE_COLORS[1].1);
    }

    #[test]
    fn the_recorded_format_of_a_media_file_round_trips_and_old_projects_have_none() {
        let old: MediaRef =
            serde_json::from_str(r#"{"id":1,"path":"a.wav","duration_seconds":2.5}"#).unwrap();
        assert!(old.format.is_none());
        assert!(
            !serde_json::to_string(&old).unwrap().contains("format"),
            "nothing recorded, nothing written"
        );
        let with = MediaRef {
            format: Some(MediaFormat {
                channels: Some(2),
                fps: Some(29.97),
            }),
            ..old
        };
        let back: MediaRef = serde_json::from_str(&serde_json::to_string(&with).unwrap()).unwrap();
        assert_eq!(back, with);
        let channels_only: MediaRef = serde_json::from_str(
            r#"{"id":1,"path":"a.wav","duration_seconds":2.5,"format":{"channels":1}}"#,
        )
        .unwrap();
        assert_eq!(
            channels_only.format,
            Some(MediaFormat {
                channels: Some(1),
                fps: None
            })
        );
    }
}
