# Architectural Exploration & Specification Report: Milestone 5 (Features 24–34)

**Agent**: `explorer_m5_2` (Milestone 5 Codebase Architecture Explorer)  
**Parent Conversation ID**: `c220c328-9551-48e1-a35b-23e05589d782`  
**Working Directory**: `C:\Users\paulomatos\Documents\GitHub\neoveg\.agents\teamwork\explorer_m5_2\`  
**Date**: 2026-11-06  

---

## 1. Observation

### 1.1 Mandatory Reference Documents & Guidelines
- **`ORIGINAL_REQUEST.md`**:
  - Pure clean-room Rust implementation compliant with `MIT OR Apache-2.0` (`deny.toml`), zero GPL/LGPL dependencies, and zero third-party trademarks in runtime strings.
  - Strict 8 GB RAM execution budget: run all Cargo commands sequentially with `-j 1` and `--test-threads=1`. NEVER execute `cargo test -p neoveg_ui` (verify `neoveg_ui` strictly using `cargo check` and `cargo clippy`).
  - Pure software/CPU fallbacks and mock implementations must be provided for all background tasks and hardware acceleration.
  - Obey VEGAS Pro semantics across all workflows, referencing `results.md` and manuals in `manuais/markdown`.
- **`PROJECT.md` Lines 56–66**:
  - Feature 24: Project Properties Setup (Video/audio project settings matching Dialog 826/863).
  - Feature 25: Project Templates (.vqpt) (46 standard project templates across broadcast, web, and disc formats).
  - Feature 26: Media Profiles Rule Matcher (reverse-order rule matching of imported media stream attributes from `profiles.ini`).
  - Feature 27: Project Media Detail Columns (23 detail metadata columns for media pool list/details view).
  - Feature 28: Media Bins & Smart Search Bins (hierarchical bin folders, quick tags, and dynamic query search bins).
  - Feature 29: Unified File Explorer (drive navigation, favorites, media preview player, and scene detection).
  - Feature 30: Make Movie Guided Export Wizard (novice-friendly export wizard flow for web, camera, and local disk targets).
  - Feature 31: Color-Coded Preview Cache Ruler (timeline ruler reflecting frame cache status: Green = RAM/Disk cached, Yellow = Playable, Red = Unrendered).
  - Feature 32: Dynamic RAM & Selective Prerender (memory buffer cache Shift+B and disk prerender cache Shift+M).
  - Feature 33: Standardized Alerts Subsystem (centralized alert subsystem matching §10 error codes and actions).
  - Feature 34: Smart Proxies & Quality Switching (automatic proxy usage on Draft/Preview and original media on Good/Best; original conformed for final render).

### 1.2 Static Reverse Engineering & Manual Citations (`results.md` & `manuais/markdown/`)
- **`results.md` §10 (Alerts, Messages, Errors & Confirmations, lines 6797–6980)**:
  - Documents 240 resource strings categorized into:
    - §10.1: Missing Media / Offline Media (Resource IDs: 1561, 1634, 1830, 16058, 16132, 16926, 17250, 17360, 17361, 17383, 17630).
    - §10.2: Codec / Format / Decoding (Resource IDs: 17359, 17369, 17370, 17381, 17382, 17384, 18322, 18331, 18368, 21452, 33279, 33280, 33501).
    - §10.3: Render / Prerender / Make Movie (Resource IDs: 1578, 1580, 1583, 1584, 1585, 1586, 1881, 1894, 1910, 1911, 3404, 17364, 17365, 17437, 17581, 17628, 18393, 18394, 18405, 18548, 33257, 44720).
    - §10.4: Memory / Disk / System Resources (Resource IDs: 16227, 16522, 18004, 18400, 18548).
    - §10.5: GPU Acceleration (Resource IDs: 16013, 16016, 16017, 16020, 33316).
    - §10.6: Plug-ins / Effects / VST / OFX (Resource IDs: 16092, 16093, 16511, 16555, 16556, 16560, 16564, 16587, 16607, 16907, 16909, 16910, 17101, 18043, 18047, 33413, 44654, 44655).
- **`results.md` §8 (Dialogs 826, 863, 817, 804, 881, 885, 888, 889, 890)**:
  - Dialog 826 (Video tab, lines 6433–6496): Width, Height, HDR Mode, Field Order, Pixel Aspect Ratio, Output Rotation, Frame Rate, Pixel Format (8-bit video, 8-bit full, 32-bit video, 32-bit full), Compositing Gamma (1.000 linear, 2.222 video), ACES Version (1.0, 1.1, 1.2), ACES Color Space (ACES2065-1, ACEScg, ACEScc, ACEScct), View Transform (Rec.709, sRGB, Rec.2020, HLG, Raw), Look Modification Transform (LMT), Render Quality (Draft, Preview, Good, Best), Motion Blur Type (Gaussian, Pyramid, Box, Asymmetric variants), Deinterlace Method, Resample Mode, Adjust Source Media, Prerender Video Override (Format, Template, Prerender Folder, Free Storage).
  - Dialog 863 (Audio tab, lines 6498–6532): Master Bus Mode (Stereo, Surround 5.1), Number of Stereo Busses (0..26), Sample Rate (8,000..192,000 Hz), Bit Depth (8, 16, 24, 32-bit float), Resample & Stretch Quality, Low-Pass Filter on LFE (cutoff Hz, quality), Recorded Files Folder, Free Space.
  - Dialog 817 (Ruler tab, lines 6404–6431): Ruler Time Format (16 formats), Ruler Start Time, BPM (Tempo), Beats Per Measure, Note That Gets One Beat.
  - Dialogs 804, 881, 885, 888, 889, 890, 861 (Make Movie Wizard, lines 3667–3995): Destination Selection (Web: Vimeo/YouTube/MP4; Camera: DV/HDV; Hard Drive; Disc: DVD/Blu-ray), Render Settings (Video/Audio paths, loop region only, stretch video to fill, save markers), Completion dialog.
- **`results.md` §12.10 (Project Media Window & 23 Columns, lines 2980–3020, 12735–12845)**:
  - Identifies the 23 standard detail metadata columns in Project Media: Name, Use Count, Type, Media FX, Comment, Timecode In, Timecode Out, Length, Tape Name, Size, Modified, Path, Avg. Total Data Rate, Sample Rate, Bit Depth, Channels, Avg. Audio Data Rate, Audio Format, Frame Rate, Field Order, Alpha, Pixel Aspect, Frame Size. (Extended columns: Color Depth, Avg. Video Data Rate, Video Format, Date/Time Stamp, Rating, Video Capture Comment, UMID).
  - View modes: List (32903), Details (32904), Thumbnail (32902).
- **`results.md` §12.11 (Unified Explorer, lines 12898–12970)**:
  - Directory navigation, favorite folders, preview transport player with auto-play preview toggle, scene detection integration ("Detect Scenes and create subclips" / "Detect Scenes and add to Timeline").
- **`results.md` §18.3 & §18.4 (Templates & `profiles.ini`, lines 20091–20197)**:
  - Exactly 46 `.vqpt` project templates specified:
    - 3D Blu-ray: `1280x720-50p`, `1280x720-60p`, `1920x1080-24p` (3)
    - 3D Internet: `1920x1080-24p`, `1920x1080-25p`, `1920x1080-30p` (3)
    - AVCHD: `1440x1080-24p`, `1440x1080-50i`, `1440x1080-60i`, `1920x1080-24p`, `1920x1080-50i`, `1920x1080-60i` (6)
    - Blu-ray: `1440x1080-24p`, `1440x1080-50i`, `1440x1080-60i`, `1920x1080-24p`, `1920x1080-50i`, `1920x1080-60i` (6)
    - DV: `DV NTSC`, `DV PAL` (2)
    - DVD: `DVD 24p widescreen`, `DVD 24p`, `DVD NTSC widescreen`, `DVD NTSC`, `DVD PAL widescreen`, `DVD PAL` (6)
    - HDV: `HDV 1280x720-24p`, `1280x720-25p`, `1280x720-30p`, `1440x1080-24p`, `1440x1080-50i`, `1440x1080-60i`, `3840x2160-24p`, `3840x2160-25p`, `3840x2160-30p`, `3840x2160-50p`, `3840x2160-60p` (11)
    - Internet: `Internet 360-24p`, `360-25p`, `360-30p`, `480-24p`, `480-25p`, `480-30p`, `720-24p`, `720-25p`, `720-30p` (9)
    - Total: 3 + 3 + 6 + 6 + 2 + 6 + 11 + 9 = 46 templates.
  - `profiles.ini` rule matcher format:
    - `KeyN = CLSID, "Subformat", Size X, Size Y, Frame Rate, Color Bit Depth`
    - `AttributesN = "Interlace", Pixel Aspect Ratio, "Alpha Channel Interpretation", Gamma`
    - REVERSE search order: searches from highest index down to 1; later rules override earlier ones; 0 / "None" are wildcards.
- **Manuals (`manual_vegaspro21_en.md` p. 213, `Sony_Vegas_Pro_14_Manual.md` pp. 84-85, 110-112, 501-507, `VP12Edit_ReviewersGuide.md` pp. 15-16)**:
  - Smart Proxies: Video proxy `.sfvp0` created alongside original media. Preview quality `Draft` or `Preview` uses proxy; `Good` or `Best` uses original media. Final render ALWAYS conforms to original full-resolution media!
  - Prerender: Dynamic RAM Preview (`Shift+B`) allocates bounded memory cache. Selective Prerender (`Shift+M`) renders to disk in discrete chunks of at most 300 frames (~40 MB).
  - Preview Cache Ruler: Green = cached in RAM/disk; Yellow = real-time playable single-stream; Red = unrendered complex effect/composite requiring prerender.
  - Media Bins: Virtual folders, 10 Quick Tags (`Ctrl+1`..`Ctrl+0`), Smart Search Bins with dynamic multi-condition filters.

### 1.3 Existing Codebase Status
- `crates/neoveg_common/src/project.rs`: contains baseline `ProjectSettings` and `MediaRef`, but lacks ACES/OCIO fields, audio busses, prerender folders, and LFE cutoff.
- `crates/neoveg_common/src/presets.rs`: defines 28 general templates, missing the full 46 `.vqpt` templates, `.vqpt` serialization, and the preset manager.
- `crates/neoveg_common/src/profiles.rs`: does NOT exist yet.
- `crates/neoveg_common/src/alerts.rs`: does NOT exist yet.
- `crates/neoveg_engine/src/`:
  - `media_pool.rs`: does NOT exist yet.
  - `media_bins.rs`: does NOT exist yet.
  - `explorer.rs`: does NOT exist yet.
  - `make_movie.rs`: does NOT exist yet.
  - `cache_ruler.rs`: does NOT exist yet.
  - `prerender.rs`: does NOT exist yet.
  - `proxy.rs`: does NOT exist yet.

---

## 2. Logic Chain

1. **Decoupling Crate Dependencies (Clean-Room Architecture)**:
   - `neoveg_common` cannot depend on `neoveg_video` or `neoveg_engine`.
   - Therefore, core types needed in project settings and media profiles (such as `AcesSpace`, `ViewTransform`, `AcesVersion`, `FieldOrder`, `AlphaMode`) must be defined in `neoveg_common` (or re-exported). `neoveg_video` can reference or convert to these types.
2. **Project Properties & Templates (Features 24 & 25)**:
   - `ProjectSettings` in `neoveg_common::project` must be extended with:
     - ACES configuration (`aces_version`, `aces_color_space`, `view_transform`, `look_transform`).
     - Prerender settings (`override_prerender`, `prerender_format`, `prerender_folder`).
     - Audio busses and surround LFE filter options (`stereo_busses`, `lfe_filter_enabled`, `lfe_cutoff_hz`, `recorded_files_folder`).
   - `neoveg_common::presets` must define the canonical 46 `.vqpt` templates listed in §18.3, plus a parser and writer for `.vqpt` template files (JSON/binary interchange).
3. **Media Profiles Engine (Feature 26)**:
   - A dedicated module `neoveg_common::profiles` must parse INI configuration files into structured rules.
   - Matching logic must iterate backwards from `KeyN` (highest `N`) to `Key1`, matching media properties against rule keys with 0/None wildcards, and applying the matching attributes.
4. **Project Media Window & 23 Metadata Columns (Feature 27)**:
   - In `neoveg_engine::media_pool`, an item in the media pool is represented by `MediaPoolItem` containing the full 23 metadata columns.
   - View modes (`List`, `Details`, `Thumbnail`) and sorting/filtering state must be encapsulated with full support for missing-media detection, replacement, and relinking.
5. **Media Bins & Smart Search Bins (Feature 28)**:
   - In `neoveg_engine::media_bins`, a hierarchical tree of virtual folders (`MediaBin`) organizes media IDs.
   - 10 Quick Tags (`Ctrl+1`..`Ctrl+0`) map to tags and auto-generate tag bins.
   - `SmartSearchBin` evaluates predicates against media item properties dynamically.
6. **Unified File Explorer (Feature 29)**:
   - In `neoveg_engine::explorer`, the filesystem model exposes folder trees, favorites, media preview state (play/pause/stop/scrub with auto-play), and scene detection orchestration.
7. **Make Movie Export Wizard (Feature 30)**:
   - In `neoveg_engine::make_movie`, a state machine handles destination selection (`Web`, `Camera`, `LocalDisk`, `Disc`), analyzes project properties to recommend optimal rendering presets, manages background rendering, and presents completion actions.
8. **Preview Cache Ruler & Prerender Subsystems (Features 31 & 32)**:
   - `neoveg_engine::cache_ruler` tracks timeline status spans: `Green` (cached in RAM or disk), `Yellow` (real-time playable single-stream), `Red` (unrendered composite/effect).
   - `neoveg_engine::prerender` handles Dynamic RAM Preview (`Shift+B`) with memory limits and Selective Disk Prerender (`Shift+M`) in 300-frame chunks. Timeline edits invalidate affected chunks only.
9. **Standardized Alerts Subsystem (Feature 33)**:
   - `neoveg_common::alerts` provides `AlertCode` matching §10 error codes, dual-locale templates (EN and PT-BR), severity levels, and sanitization of any third-party trademarks.
10. **Smart Proxies & Quality Switching (Feature 34)**:
    - `neoveg_engine::proxy` generates `.sfvp0` proxy files, switches between proxy and original media based on preview quality (`Draft`/`Preview` vs `Good`/`Best`), and guarantees that final render conformed to original media.

---

## 3. Data Models & Interface Contracts

Below are the concrete Rust data models, enum variants, and interface contracts specified for implementation across the workspace.

### 3.1 Feature 24: Project Properties Model (`neoveg_common::project`)

```rust
// In crates/neoveg_common/src/project.rs

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::presets::{
    CompositingGamma, Deinterlace, FieldOrder, HdrMode, MasterBusMode, MotionBlurType,
    OutputRotation, PixelFormat, RenderQuality, ResampleMode,
};
use crate::ruler_format::RulerFormat;

/// ACES version for project color management.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcesVersion {
    #[default]
    V1_0,
    V1_1,
    V1_2,
}

/// ACES color space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcesColorSpace {
    Aces2065_1,
    #[default]
    AcesCg,
    AcesCc,
    AcesCct,
}

/// Output display view transform for ACES color management.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcesViewTransform {
    #[default]
    Rec709,
    Srgb,
    Rec2020,
    Hlg,
    Raw,
}

/// Low-pass filter quality for LFE channel in 5.1 surround projects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LfeFilterQuality {
    Draft,
    #[default]
    Good,
    Best,
}

/// Comprehensive project settings matching VEGAS Pro Dialogs 826 (Video), 863 (Audio), and 817 (Ruler).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectSettings {
    // --- Video Dimensions & Frame Rate ---
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub pixel_aspect: f64,
    pub field_order: FieldOrder,
    pub output_rotation: OutputRotation,

    // --- Color Management & Pixel Formats ---
    pub pixel_format: PixelFormat, // 8-bit, 32-bit float (video levels), 32-bit float (full range)
    pub compositing_gamma: CompositingGamma, // 1.000 Linear or 2.222 Video
    pub hdr_mode: HdrMode,
    pub aces_version: AcesVersion,
    pub aces_color_space: AcesColorSpace,
    pub view_transform: AcesViewTransform,
    pub look_transform: Option<String>, // LMT preset

    // --- Video Processing & Resampling ---
    pub render_quality: RenderQuality,
    pub motion_blur: MotionBlurType,
    pub deinterlace: Deinterlace,
    pub resample_mode: ResampleMode,
    pub adjust_source_media: bool,

    // --- Prerender Settings ---
    pub override_prerender: bool,
    pub prerender_format: Option<String>,
    pub prerender_template: Option<String>,
    pub prerender_folder: PathBuf,

    // --- Audio Settings (Dialog 863) ---
    pub master_bus: MasterBusMode,
    pub stereo_busses: u32,       // 0 to 26
    pub sample_rate: u32,         // 8,000 to 192,000 Hz
    pub audio_bit_depth: u32,     // 8, 16, 24, 32-bit float
    pub audio_resample_quality: RenderQuality,
    pub lfe_filter_enabled: bool,
    pub lfe_cutoff_hz: f32,       // e.g. 80.0 Hz, 120.0 Hz
    pub lfe_quality: LfeFilterQuality,
    pub recorded_files_folder: PathBuf,

    // --- Ruler & Metronome (Dialog 817) ---
    pub ruler_format: RulerFormat,
    pub ruler_start: f64,
    pub bpm: f32,
    pub beats_per_bar: u32,
    pub note_value_beat: u32, // e.g. 4 for quarter note

    // --- Summary & Timeline Defaults ---
    pub auto_crossfade: bool,
    pub normalize_peak_db: f64,
    pub title: String,
    pub artist: String,
    pub engineer: String,
    pub copyright: String,
    pub comments: String,
}
```

---

### 3.2 Feature 25: Project Templates Subsystem (.vqpt) (`neoveg_common::presets`)

```rust
// In crates/neoveg_common/src/presets.rs

use crate::project::ProjectSettings;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Project template matching .vqpt format.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectTemplate {
    pub name: String,
    pub category: TemplateCategory,
    pub is_builtin: bool,
    pub settings: ProjectSettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateCategory {
    Broadcast3d,
    Avchd,
    Bluray,
    Dv,
    Dvd,
    Hdv,
    Internet,
    UserCustom,
}

/// Returns the 46 canonical standard templates specified in §18.3.
pub fn canonical_46_templates() -> Vec<ProjectTemplate> {
    // Generates the exact 46 templates:
    // - 3D Blu-ray (3), 3D Internet (3), AVCHD (6), Blu-ray (6),
    // - DV (2), DVD (6), HDV (11), Internet (9)
    // ...
}

/// Template subsystem manager for loading, saving, and exporting .vqpt template files.
pub struct TemplateSubsystem {
    builtin: Vec<ProjectTemplate>,
    custom_dir: PathBuf,
}

impl TemplateSubsystem {
    pub fn new(custom_dir: PathBuf) -> Self;
    pub fn all_templates(&self) -> Vec<ProjectTemplate>;
    pub fn find_by_name(&self, name: &str) -> Option<ProjectTemplate>;
    pub fn save_custom(&self, template: ProjectTemplate) -> Result<(), String>;
    pub fn delete_custom(&self, name: &str) -> Result<(), String>;
    pub fn export_vqpt(&self, template: &ProjectTemplate, dest_path: &Path) -> Result<(), String>;
    pub fn import_vqpt(&self, src_path: &Path) -> Result<ProjectTemplate, String>;
}
```

---

### 3.3 Feature 26: Media Profiles Rule Engine (`neoveg_common::profiles`)

```rust
// In crates/neoveg_common/src/profiles.rs

use crate::media_props::{AlphaMode, FieldOrder};
use std::path::Path;

/// Matching criteria from KeyN in profiles.ini.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaProfileKey {
    pub clsid: String,       // "0" or GUID "{...}"
    pub subformat: String,   // "None" or custom string
    pub size_x: u32,         // 0 = wildcard
    pub size_y: u32,         // 0 = wildcard
    pub frame_rate: f64,     // 0.0 = wildcard
    pub bit_depth: u32,      // 0 = wildcard
}

/// Stream attribute overrides from AttributesN in profiles.ini.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaProfileAttributes {
    pub interlace: FieldOrder,
    pub pixel_aspect: f64,
    pub alpha: AlphaMode,
    pub gamma: f64,
}

/// One numbered rule pairing KeyN and AttributesN.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaProfileRule {
    pub index: usize,
    pub key: MediaProfileKey,
    pub attributes: MediaProfileAttributes,
}

/// Reverse-order rule matcher for media profiles.
#[derive(Debug, Default, Clone)]
pub struct MediaProfileMatcher {
    rules: Vec<MediaProfileRule>, // Sorted ascending by index 1..N
}

impl MediaProfileMatcher {
    pub fn parse_ini(ini_content: &str) -> Result<Self, String>;
    pub fn load_file(path: &Path) -> Result<Self, String>;
    pub fn default_built_in() -> Self;

    /// Evaluates stream properties against rules in REVERSE order (highest index to 1).
    /// First matching rule wins.
    pub fn match_stream(
        &self,
        clsid: Option<&str>,
        subformat: Option<&str>,
        width: u32,
        height: u32,
        fps: f64,
        bit_depth: u32,
    ) -> Option<MediaProfileAttributes>;
}
```

---

### 3.4 Feature 27: Project Media Window Model (`neoveg_engine::media_pool`)

```rust
// In crates/neoveg_engine/src/media_pool.rs

use neoveg_common::media_props::MediaProps;
use neoveg_common::project::MediaRef;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The 23 detail metadata columns for Project Media detailed list view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MediaDetailColumn {
    Name,
    UseCount,
    Type,
    MediaFx,
    Comment,
    TimecodeIn,
    TimecodeOut,
    Length,
    TapeName,
    Size,
    Modified,
    Path,
    AvgTotalDataRate,
    SampleRate,
    BitDepth,
    Channels,
    AvgAudioDataRate,
    AudioFormat,
    FrameRate,
    FieldOrder,
    Alpha,
    PixelAspect,
    FrameSize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MediaViewMode {
    List,
    #[default]
    Details,
    Thumbnail,
}

/// Formatted metadata representation for an item in the Project Media list.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaDetailRow {
    pub id: u64,
    pub path: PathBuf,
    pub name: String,
    pub use_count: usize,
    pub kind_label: String,
    pub has_fx: bool,
    pub comment: String,
    pub timecode_in: String,
    pub timecode_out: String,
    pub length: String,
    pub tape_name: String,
    pub file_size_bytes: u64,
    pub file_size_formatted: String,
    pub modified_timestamp: u64,
    pub modified_formatted: String,
    pub avg_total_bitrate: u64,
    pub sample_rate: u32,
    pub bit_depth: u32,
    pub channels: u16,
    pub avg_audio_bitrate: u32,
    pub audio_format: String,
    pub frame_rate: f64,
    pub field_order: String,
    pub alpha_mode: String,
    pub pixel_aspect: f64,
    pub frame_size: (u32, u32),
    pub is_missing: bool,
    pub has_proxy: bool,
}

/// Project media pool manager with filtering, sorting, relinking, and replacement.
pub struct MediaPoolManager {
    items: Vec<MediaDetailRow>,
    visible_columns: Vec<MediaDetailColumn>,
    view_mode: MediaViewMode,
    sort_column: MediaDetailColumn,
    sort_ascending: bool,
    search_filter: String,
}

impl MediaPoolManager {
    pub fn new() -> Self;
    pub fn set_view_mode(&mut self, mode: MediaViewMode);
    pub fn set_sort(&mut self, column: MediaDetailColumn, ascending: bool);
    pub fn set_search_filter(&mut self, filter: &str);
    pub fn toggle_column(&mut self, column: MediaDetailColumn, visible: bool);
    pub fn filtered_rows(&self) -> Vec<&MediaDetailRow>;
    pub fn replace_media(&mut self, id: u64, new_path: PathBuf) -> Result<(), String>;
    pub fn relink_missing(&mut self, search_dirs: &[PathBuf]) -> Vec<(u64, PathBuf)>;
    pub fn clean_unused(&mut self) -> usize; // Removes items with use_count == 0
}
```

---

### 3.5 Feature 28: Media Bins & Smart Search Bins (`neoveg_engine::media_bins`)

```rust
// In crates/neoveg_engine/src/media_bins.rs

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub type BinId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SearchField {
    Name,
    Type,
    Comment,
    TapeName,
    Path,
    AudioFormat,
    VideoFormat,
    Tag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SearchCondition {
    Contains,
    DoesNotContain,
    StartsWith,
    EndsWith,
    Equals,
    NotEquals,
    IsEmpty,
    IsNotEmpty,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchCriterion {
    pub field: SearchField,
    pub condition: SearchCondition,
    pub phrase: String,
}

/// Virtual folder representation in the media bin hierarchy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaBin {
    pub id: BinId,
    pub parent_id: Option<BinId>,
    pub name: String,
    pub is_root: bool,
    pub is_smart: bool,
    pub search_criteria: Vec<SearchCriterion>,
    pub media_ids: HashSet<u64>,
}

/// Hierarchical container managing media bins, quick tags (Ctrl+1..0), and smart queries.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MediaBinTree {
    root_id: BinId,
    tags_root_id: BinId,
    bins: HashMap<BinId, MediaBin>,
    quick_tags: [String; 10], // Shortcuts Ctrl+1 through Ctrl+0 (Dialog 871)
    next_id: u64,
}

impl MediaBinTree {
    pub fn new() -> Self;
    pub fn create_bin(&mut self, parent: Option<BinId>, name: &str) -> Result<BinId, String>;
    pub fn create_smart_bin(&mut self, parent: Option<BinId>, name: &str, criteria: Vec<SearchCriterion>) -> Result<BinId, String>;
    pub fn rename_bin(&mut self, id: BinId, new_name: &str) -> Result<(), String>;
    pub fn delete_bin(&mut self, id: BinId) -> Result<(), String>;
    pub fn set_quick_tag(&mut self, slot_index: usize, tag: &str);
    pub fn apply_quick_tag(&mut self, slot_index: usize, media_id: u64);
    pub fn evaluate_smart_bins(&mut self, pool_items: &[crate::media_pool::MediaDetailRow]);
}
```

---

### 3.6 Feature 29: Unified File Explorer (`neoveg_engine::explorer`)

```rust
// In crates/neoveg_engine/src/explorer.rs

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct FileItem {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub size_bytes: u64,
    pub modified: u64,
    pub is_media: bool,
    pub is_favorite: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportState {
    Stopped,
    Playing,
    Paused,
}

pub struct ExplorerModel {
    current_dir: PathBuf,
    favorites: Vec<PathBuf>,
    auto_preview: bool,
    transport_state: TransportState,
    preview_position_sec: f64,
    preview_volume: f32, // 0.0 to 1.0
}

impl ExplorerModel {
    pub fn new() -> Self;
    pub fn navigate_to(&mut self, path: &Path) -> Result<Vec<FileItem>, String>;
    pub fn add_favorite(&mut self, path: PathBuf);
    pub fn remove_favorite(&mut self, path: &Path);
    pub fn set_auto_preview(&mut self, enable: bool);
    pub fn play_preview(&mut self, media_path: &Path) -> Result<(), String>;
    pub fn pause_preview(&mut self);
    pub fn stop_preview(&mut self);
    pub fn scrub_preview(&mut self, position_sec: f64);
    /// Scene detection: analyzes media file, detects shot cut boundaries, creates subclips.
    pub fn detect_scenes(&self, media_path: &Path, threshold: f32) -> Result<Vec<(f64, f64)>, String>;
}
```

---

### 3.7 Feature 30: Make Movie Guided Export Wizard (`neoveg_engine::make_movie`)

```rust
// In crates/neoveg_engine/src/make_movie.rs

use neoveg_common::project::ProjectSettings;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportDestination {
    WebYoutube,
    WebVimeo,
    WebGenericMp4,
    CameraDv,
    CameraHdv,
    LocalDisk,
    DiscDvd,
    DiscBluray,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MakeMovieOptions {
    pub destination: ExportDestination,
    pub recommended_template_name: String,
    pub output_video_path: PathBuf,
    pub output_audio_path: Option<PathBuf>, // When separate audio stream required
    pub render_loop_region_only: bool,
    pub stretch_video_fill: bool,
    pub save_markers: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WizardStep {
    SelectDestination,
    ConfigureSettings(MakeMovieOptions),
    RenderingProgress { progress_fraction: f32, eta_seconds: f64 },
    Completed { output_path: PathBuf },
    Failed(String),
}

pub struct MakeMovieWizard {
    current_step: WizardStep,
    project_settings: ProjectSettings,
}

impl MakeMovieWizard {
    pub fn new(project: &ProjectSettings) -> Self;
    pub fn choose_destination(&mut self, dest: ExportDestination);
    pub fn recommend_preset(&self, dest: ExportDestination) -> String;
    pub fn update_options(&mut self, options: MakeMovieOptions);
    pub fn start_render(&mut self) -> Result<(), String>;
    pub fn cancel_render(&mut self);
}
```

---

### 3.8 Feature 31: Color-Coded Preview Cache Ruler (`neoveg_engine::cache_ruler`)

```rust
// In crates/neoveg_engine/src/cache_ruler.rs

use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameCacheStatus {
    /// Green: Rendered frame is stored in RAM cache or disk prerender chunk. Near-zero playback latency.
    GreenCached,
    /// Yellow: Playable in real time without dropping frames (simple cuts, single stream, uncomplex VFX).
    YellowRealtime,
    /// Red: Unrendered complex section (compositing, 3D transition, optical flow, nested sequence) requiring prerender.
    RedUnrendered,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CacheSpan {
    pub start_frame: u64,
    pub end_frame: u64,
    pub status: FrameCacheStatus,
}

/// Timeline cache ruler maintaining disjoint coalesced status spans.
pub struct PreviewCacheRuler {
    spans: BTreeMap<u64, CacheSpan>, // Keyed by start_frame
    total_frames: u64,
    fps: f64,
}

impl PreviewCacheRuler {
    pub fn new(total_frames: u64, fps: f64) -> Self;
    pub fn mark_span(&mut self, start_frame: u64, end_frame: u64, status: FrameCacheStatus);
    pub fn invalidate_region(&mut self, start_frame: u64, end_frame: u64); // Reverts cached to Red/Yellow
    pub fn status_at(&self, frame: u64) -> FrameCacheStatus;
    pub fn all_spans(&self) -> Vec<CacheSpan>;
    pub fn percent_cached(&self) -> f32;
}
```

---

### 3.9 Feature 32: Dynamic RAM & Selective Prerender (`neoveg_engine::prerender`)

```rust
// In crates/neoveg_engine/src/prerender.rs

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub const PRERENDER_MAX_CHUNK_FRAMES: u64 = 300; // ~40 MB per chunk

/// In-memory Dynamic RAM Preview buffer (Shift+B).
pub struct DynamicRamPreview {
    max_bytes: usize,
    used_bytes: usize,
    cached_frames: HashMap<u64, Arc<Vec<u8>>>, // frame_index -> RGBA buffer
    selection_range: Option<(u64, u64)>,
}

impl DynamicRamPreview {
    pub fn new(max_bytes: usize) -> Self;
    pub fn set_budget_mb(&mut self, mb: usize);
    pub fn build_selection(&mut self, start: u64, end: u64, render_frame_fn: impl Fn(u64) -> Vec<u8>) -> (u64, u64);
    pub fn get_frame(&self, frame: u64) -> Option<Arc<Vec<u8>>>;
    pub fn clear(&mut self);
}

/// Selective Disk Prerender engine (Shift+M).
pub struct SelectiveDiskPrerender {
    prerender_dir: PathBuf,
    active_chunks: Vec<PrerenderChunk>,
}

#[derive(Debug, Clone)]
pub struct PrerenderChunk {
    pub id: u64,
    pub start_frame: u64,
    pub frame_count: u64,
    pub file_path: PathBuf,
}

impl SelectiveDiskPrerender {
    pub fn new(prerender_dir: PathBuf) -> Self;
    pub fn prerender_range(&mut self, start_frame: u64, end_frame: u64) -> Result<(), String>;
    pub fn clean_all(&mut self) -> Result<usize, String>;
    pub fn clean_inactive(&mut self, timeline_spans: &[(u64, u64)]) -> Result<usize, String>;
}
```

---

### 3.10 Feature 33: Standardized Alerts Subsystem (`neoveg_common::alerts`)

```rust
// In crates/neoveg_common/src/alerts.rs

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AlertCategory {
    MissingMedia,
    Codec,
    Render,
    DiskMemory,
    Gpu,
    Plugin,
    Script,
    General,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AlertSeverity {
    Information,
    Warning,
    Error,
    Confirmation,
}

/// Exact alert codes matching results.md §10.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AlertCode {
    // Missing media (§10.1)
    MissingProjectMediaElements = 1561,
    MediaIsOffline = 1634,
    FindMissingFile = 16132,
    MediaFileNotFound = 16926,
    OfflineMediaFile = 17250,
    CannotSaveMarkersMediaOffline = 17361,
    CannotOpenInSoundEditorMediaOffline = 17363,

    // Codecs & formats (§10.2)
    FileCorrupted = 1648,
    StreamAttributesCouldNotBeDetermined = 17359,
    MissingVideoCodec = 17369,
    CorruptVideoStream = 17370,
    MissingAudioCodec = 17381,
    UnknownStreamError = 17382,
    CorruptAudioStream = 17384,
    UnsupportedAudioChannelCount = 33501,

    // Render & Make Movie (§10.3)
    EmptyProjectRender = 3404,
    NoTemplatesSupportMedia = 1910,
    RenderFileAlreadyExists = 1911,
    Over80PercentHasToBeRendered = 1881,
    RenderFormatIncompatibleWithDevice = 18331,
    CancelRenderConfirmation = 44720,

    // Memory & disk resources (§10.4)
    ClearEditHistoryConfirmation = 16227,
    DiskFullDvdImage = 18548,
    ParametersInvalid = 18400,

    // GPU acceleration (§10.5)
    GpuAccelerationError = 33316,
    GpuFatalError = 16020,

    // Plugins & effects (§10.6)
    PluginChainFull = 16511,
    PluginFailedToLoad = 16555,
    FxSendFull = 16587,
    VideoEffectsUiInitFailed = 33413,
}

/// Standard alert specification with localized templates and sanitized trademark strings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StandardAlert {
    pub code: AlertCode,
    pub category: AlertCategory,
    pub severity: AlertSeverity,
    pub en_us_template: &'static str,
    pub pt_br_template: &'static str,
    pub default_actions: Vec<AlertAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertAction {
    Ok,
    Cancel,
    Yes,
    No,
    Browse,
    Relink,
    Retry,
    Ignore,
}

pub struct AlertRegistry;

impl AlertRegistry {
    pub fn get(code: AlertCode) -> StandardAlert;
    pub fn format_message(code: AlertCode, args: &[&str], locale: &str) -> String;
}
```

---

### 3.11 Feature 34: Smart Proxies & Quality Switching (`neoveg_engine::proxy`)

```rust
// In crates/neoveg_engine/src/proxy.rs

use neoveg_common::presets::RenderQuality;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyState {
    None,
    Building { progress_pct: f32 },
    Available,
    Offline,
}

#[derive(Debug, Clone)]
pub struct SmartProxyEntry {
    pub original_path: PathBuf,
    pub proxy_path: PathBuf, // Suffix .sfvp0
    pub state: ProxyState,
    pub width: u32,
    pub height: u32,
}

pub struct SmartProxyManager {
    entries: Vec<SmartProxyEntry>,
}

impl SmartProxyManager {
    pub fn new() -> Self;
    pub fn get_proxy_path_for(original: &Path) -> PathBuf {
        let mut p = original.to_path_buf();
        let name = format!("{}.sfvp0", p.file_name().unwrap().to_str().unwrap());
        p.set_file_name(name);
        p
    }

    /// Determines whether the proxy file or the original high-resolution file should be read.
    /// - Draft & Preview: use proxy file (if available).
    /// - Good & Best: use original file.
    /// - Final Render: ALWAYS conforms to original file.
    pub fn resolve_read_path<'a>(
        &'a self,
        original_path: &'a Path,
        quality: RenderQuality,
        is_final_render: bool,
    ) -> &'a Path {
        if is_final_render {
            return original_path;
        }
        match quality {
            RenderQuality::Draft | RenderQuality::Preview => {
                if let Some(entry) = self.entries.iter().find(|e| e.original_path == original_path) {
                    if entry.state == ProxyState::Available && entry.proxy_path.exists() {
                        return &entry.proxy_path;
                    }
                }
                original_path
            }
            RenderQuality::Good | RenderQuality::Best => original_path,
        }
    }

    pub fn trigger_create_proxy(&mut self, original: &Path) -> Result<(), String>;
    pub fn relink_proxies(&mut self, folder: &Path) -> usize;
}
```

---

## 4. Caveats

1. **Sequential Build & CI Constraint**:
   - All workspace builds and tests must execute sequentially with `-j 1` and `--test-threads=1` to abide by the strict 8 GB memory ceiling. `neoveg_ui` must never be run through `cargo test -p neoveg_ui`.
2. **Headless & Hardware Agnosticism**:
   - Proxy generation, disk prerendering, and preview rendering in tests must use software synthetic frames or mock loopbacks to guarantee deterministic test execution without relying on GPU hardware or external encoders.
3. **Trademark Sanitization**:
   - In accordance with repository compliance rules, third-party brand names (e.g. Sony, Vegas) must never appear in user-visible alert dialogs, UI labels, or runtime strings. Generic terms such as `$(AppName)`, `Neoveg`, or neutral descriptors are strictly enforced.
4. **File Format Backward Compatibility**:
   - The native `.neoveg` ZIP project container stores `project.json` and `media_pool.json`. All extensions to `ProjectSettings` and `MediaRef` must utilize serde defaults to prevent deserialization regressions with existing project files.

---

## 5. Conclusion

Milestone 5 architecture is completely scoped, designed, and grounded in the reverse-engineered VEGAS Pro specifications (`results.md` §10, §8, §12, §18) and user manuals.
The blueprints established here cleanly decouple metadata models in `neoveg_common`, business logic in `neoveg_engine`, and presentation in `neoveg_ui`.
Implementations can proceed directly against the specified struct definitions, enum variants, and interface contracts.

---

## 6. Verification Method

To independently verify this specification and its downstream implementation:

1. **Compilation Check**:
   ```powershell
   cargo check -j 1 --workspace
   cargo clippy -j 1 --workspace
   ```
2. **Unit & Integration Test Execution**:
   ```powershell
   cargo test -j 1 --test-threads=1 --workspace --exclude neoveg_ui
   ```
3. **Parity & I18N Check**:
   ```powershell
   python scripts/parity_matrix.py --check
   ```
4. **License Compliance**:
   ```powershell
   cargo deny check
   ```
5. **Concrete Test Scenarios for Milestone 5**:
   - Verify `profiles.ini` rule matching in reverse order (highest rule index overrides earlier rules; wildcards match correctly).
   - Verify 46 standard project templates match §18.3 names and dimensions.
   - Verify Project Media 23 detail metadata column formatting and sorting.
   - Verify Media Bins hierarchy, Quick Tags (Ctrl+1..0), and Smart Search Bin predicate evaluation.
   - Verify Dynamic RAM (`Shift+B`) memory bounding and Selective Prerender (`Shift+M`) 300-frame chunking.
   - Verify Preview Cache Ruler transitions between Green, Yellow, and Red upon edits.
   - Verify Standard Alerts (§10) format localized strings without third-party trademarks.
   - Verify Smart Proxy switches to proxy on Draft/Preview, to original on Good/Best, and always conforms to original on final render.
