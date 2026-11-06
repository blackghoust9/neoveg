//! Project Media Pool Manager & 23 Metadata Columns (results.md §12.10).
//!
//! Provides the canonical 23 metadata detail columns, multi-column sorting,
//! real-time search filtering, media replacement (preserving timeline trims/envelopes),
//! missing media relinking, and unused media cleanup.

use neoveg_common::alerts::AlertCode;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The exact 23 detail metadata columns for Project Media detailed list view (results.md §12.10 lines 12789–12824).
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
    AvgTotalBitrate,
    SampleRate,
    BitDepth,
    Channels,
    AvgAudioBitrate,
    AudioFormat,
    Fps,
    FieldOrder,
    Alpha,
    PixelAspect,
    FrameSize,
}

impl MediaDetailColumn {
    pub const ALL: [MediaDetailColumn; 23] = [
        MediaDetailColumn::Name,
        MediaDetailColumn::UseCount,
        MediaDetailColumn::Type,
        MediaDetailColumn::MediaFx,
        MediaDetailColumn::Comment,
        MediaDetailColumn::TimecodeIn,
        MediaDetailColumn::TimecodeOut,
        MediaDetailColumn::Length,
        MediaDetailColumn::TapeName,
        MediaDetailColumn::Size,
        MediaDetailColumn::Modified,
        MediaDetailColumn::Path,
        MediaDetailColumn::AvgTotalBitrate,
        MediaDetailColumn::SampleRate,
        MediaDetailColumn::BitDepth,
        MediaDetailColumn::Channels,
        MediaDetailColumn::AvgAudioBitrate,
        MediaDetailColumn::AudioFormat,
        MediaDetailColumn::Fps,
        MediaDetailColumn::FieldOrder,
        MediaDetailColumn::Alpha,
        MediaDetailColumn::PixelAspect,
        MediaDetailColumn::FrameSize,
    ];

    pub fn label(self) -> &'static str {
        match self {
            MediaDetailColumn::Name => "Name",
            MediaDetailColumn::UseCount => "Use Count",
            MediaDetailColumn::Type => "Type",
            MediaDetailColumn::MediaFx => "Media FX",
            MediaDetailColumn::Comment => "Comment",
            MediaDetailColumn::TimecodeIn => "Timecode In",
            MediaDetailColumn::TimecodeOut => "Timecode Out",
            MediaDetailColumn::Length => "Length",
            MediaDetailColumn::TapeName => "Tape Name",
            MediaDetailColumn::Size => "Size",
            MediaDetailColumn::Modified => "Modified",
            MediaDetailColumn::Path => "Path",
            MediaDetailColumn::AvgTotalBitrate => "Avg. Total Bitrate",
            MediaDetailColumn::SampleRate => "Sample Rate",
            MediaDetailColumn::BitDepth => "Bit Depth",
            MediaDetailColumn::Channels => "Channels",
            MediaDetailColumn::AvgAudioBitrate => "Avg. Audio Bitrate",
            MediaDetailColumn::AudioFormat => "Audio Format",
            MediaDetailColumn::Fps => "Frame Rate",
            MediaDetailColumn::FieldOrder => "Field Order",
            MediaDetailColumn::Alpha => "Alpha",
            MediaDetailColumn::PixelAspect => "Pixel Aspect",
            MediaDetailColumn::FrameSize => "Frame Size",
        }
    }
}

/// Project Media presentation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MediaViewMode {
    List,
    #[default]
    Details,
    Thumbnail,
}

/// Formatted metadata representation for an item in the Project Media pool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaDetailRow {
    pub id: u64,
    pub name: String,
    pub use_count: usize,
    pub media_type: String, // e.g. "Video", "Audio", "Still Image", "Subproject"
    pub has_fx: bool,
    pub comment: String,
    pub timecode_in: String,
    pub timecode_out: String,
    pub length: String,
    pub length_seconds: f64,
    pub tape_name: String,
    pub size_bytes: u64,
    pub size_formatted: String,
    pub modified_timestamp: u64,
    pub modified_formatted: String,
    pub path: PathBuf,
    pub avg_total_bitrate: u64, // bits per second
    pub sample_rate: u32,       // Hz
    pub bit_depth: u32,
    pub channels: u16,
    pub avg_audio_bitrate: u32, // bps
    pub audio_format: String,
    pub fps: f64,
    pub field_order: String,
    pub alpha: String,
    pub pixel_aspect: f64,
    pub frame_size: (u32, u32),
    pub is_missing: bool,
    pub has_proxy: bool,
    pub tags: Vec<String>,
}

impl MediaDetailRow {
    pub fn new(id: u64, name: impl Into<String>, path: PathBuf) -> Self {
        Self {
            id,
            name: name.into(),
            use_count: 0,
            media_type: "Video".into(),
            has_fx: false,
            comment: String::new(),
            timecode_in: "00:00:00:00".into(),
            timecode_out: "00:00:00:00".into(),
            length: "00:00:00:00".into(),
            length_seconds: 0.0,
            tape_name: String::new(),
            size_bytes: 0,
            size_formatted: "0 B".into(),
            modified_timestamp: 0,
            modified_formatted: String::new(),
            path,
            avg_total_bitrate: 0,
            sample_rate: 48_000,
            bit_depth: 24,
            channels: 2,
            avg_audio_bitrate: 0,
            audio_format: "None".into(),
            fps: 29.97,
            field_order: "Progressive".into(),
            alpha: "None".into(),
            pixel_aspect: 1.0,
            frame_size: (1920, 1080),
            is_missing: false,
            has_proxy: false,
            tags: Vec::new(),
        }
    }
}

/// Project Media pool manager with filtering, multi-column sorting, and relinking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaPoolManager {
    items: Vec<MediaDetailRow>,
    visible_columns: Vec<MediaDetailColumn>,
    view_mode: MediaViewMode,
    sort_column: MediaDetailColumn,
    sort_ascending: bool,
    search_filter: String,
}

impl Default for MediaPoolManager {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            visible_columns: MediaDetailColumn::ALL.to_vec(),
            view_mode: MediaViewMode::Details,
            sort_column: MediaDetailColumn::Name,
            sort_ascending: true,
            search_filter: String::new(),
        }
    }
}

impl MediaPoolManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn items(&self) -> &[MediaDetailRow] {
        &self.items
    }

    pub fn items_mut(&mut self) -> &mut [MediaDetailRow] {
        &mut self.items
    }

    pub fn view_mode(&self) -> MediaViewMode {
        self.view_mode
    }

    pub fn set_view_mode(&mut self, mode: MediaViewMode) {
        self.view_mode = mode;
    }

    pub fn visible_columns(&self) -> &[MediaDetailColumn] {
        &self.visible_columns
    }

    pub fn toggle_column(&mut self, column: MediaDetailColumn, visible: bool) {
        if visible {
            if !self.visible_columns.contains(&column) {
                self.visible_columns.push(column);
            }
        } else {
            self.visible_columns.retain(|c| *c != column);
        }
    }

    pub fn set_sort(&mut self, column: MediaDetailColumn, ascending: bool) {
        self.sort_column = column;
        self.sort_ascending = ascending;
    }

    pub fn set_search_filter(&mut self, filter: &str) {
        self.search_filter = filter.trim().to_lowercase();
    }

    /// Adds a media item row to the pool.
    pub fn add_item(&mut self, row: MediaDetailRow) -> u64 {
        let id = row.id;
        self.items.push(row);
        id
    }

    /// Removes an item from the media pool.
    ///
    /// If the item is actively referenced on the timeline (`use_count > 0`) and
    /// `allow_timeline_remove` is false, returns `Err(AlertCode::TimelineMediaDeleteConfirm)` (ID 17264).
    pub fn remove_item(&mut self, id: u64, allow_timeline_remove: bool) -> Result<(), AlertCode> {
        if let Some(pos) = self.items.iter().position(|item| item.id == id) {
            if self.items[pos].use_count > 0 && !allow_timeline_remove {
                return Err(AlertCode::TimelineMediaDeleteConfirm);
            }
            self.items.remove(pos);
            Ok(())
        } else {
            Ok(())
        }
    }

    /// Replaces media file reference while preserving timeline event trims and envelopes.
    pub fn replace_media(
        &mut self,
        id: u64,
        new_path: PathBuf,
        new_data: Option<MediaDetailRow>,
    ) -> Result<(), String> {
        let pos = self
            .items
            .iter()
            .position(|item| item.id == id)
            .ok_or_else(|| format!("Media item with ID {id} not found in pool"))?;

        let old_item = &mut self.items[pos];
        old_item.path = new_path;
        old_item.is_missing = false;

        if let Some(data) = new_data {
            old_item.size_bytes = data.size_bytes;
            old_item.size_formatted = data.size_formatted;
            old_item.sample_rate = data.sample_rate;
            old_item.bit_depth = data.bit_depth;
            old_item.channels = data.channels;
            old_item.fps = data.fps;
            old_item.frame_size = data.frame_size;
            old_item.audio_format = data.audio_format;
        }

        Ok(())
    }

    /// Relinks missing media items by searching given directories for matching filenames.
    pub fn relink_missing(&mut self, search_dirs: &[PathBuf]) -> Vec<(u64, PathBuf)> {
        let mut relinked = Vec::new();

        for item in &mut self.items {
            if !item.is_missing {
                continue;
            }
            let target_name = match item.path.file_name() {
                Some(name) => name.to_string_lossy().to_lowercase(),
                None => continue,
            };

            for dir in search_dirs {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file() {
                            if let Some(name) = path.file_name() {
                                if name.to_string_lossy().to_lowercase() == target_name {
                                    item.path = path.clone();
                                    item.is_missing = false;
                                    relinked.push((item.id, path));
                                    break;
                                }
                            }
                        }
                    }
                }
                if !item.is_missing {
                    break;
                }
            }
        }

        relinked
    }

    /// Removes all media pool items that are not referenced by any timeline events (`use_count == 0`).
    pub fn clean_unused(&mut self) -> usize {
        let initial_len = self.items.len();
        self.items.retain(|item| item.use_count > 0);
        initial_len - self.items.len()
    }

    /// Returns sorted and search-filtered rows for table presentation.
    pub fn filtered_rows(&self) -> Vec<&MediaDetailRow> {
        let mut result: Vec<&MediaDetailRow> = self
            .items
            .iter()
            .filter(|item| {
                if self.search_filter.is_empty() {
                    return true;
                }
                item.name.to_lowercase().contains(&self.search_filter)
                    || item.media_type.to_lowercase().contains(&self.search_filter)
                    || item.comment.to_lowercase().contains(&self.search_filter)
                    || item.tape_name.to_lowercase().contains(&self.search_filter)
                    || item
                        .path
                        .to_string_lossy()
                        .to_lowercase()
                        .contains(&self.search_filter)
                    || item
                        .audio_format
                        .to_lowercase()
                        .contains(&self.search_filter)
                    || item
                        .tags
                        .iter()
                        .any(|t| t.to_lowercase().contains(&self.search_filter))
            })
            .collect();

        result.sort_by(|a, b| {
            let ordering = match self.sort_column {
                MediaDetailColumn::Name => a.name.cmp(&b.name),
                MediaDetailColumn::UseCount => a.use_count.cmp(&b.use_count),
                MediaDetailColumn::Type => a.media_type.cmp(&b.media_type),
                MediaDetailColumn::MediaFx => a.has_fx.cmp(&b.has_fx),
                MediaDetailColumn::Comment => a.comment.cmp(&b.comment),
                MediaDetailColumn::TimecodeIn => a.timecode_in.cmp(&b.timecode_in),
                MediaDetailColumn::TimecodeOut => a.timecode_out.cmp(&b.timecode_out),
                MediaDetailColumn::Length => a.length_seconds.total_cmp(&b.length_seconds),
                MediaDetailColumn::TapeName => a.tape_name.cmp(&b.tape_name),
                MediaDetailColumn::Size => a.size_bytes.cmp(&b.size_bytes),
                MediaDetailColumn::Modified => a.modified_timestamp.cmp(&b.modified_timestamp),
                MediaDetailColumn::Path => a.path.cmp(&b.path),
                MediaDetailColumn::AvgTotalBitrate => a.avg_total_bitrate.cmp(&b.avg_total_bitrate),
                MediaDetailColumn::SampleRate => a.sample_rate.cmp(&b.sample_rate),
                MediaDetailColumn::BitDepth => a.bit_depth.cmp(&b.bit_depth),
                MediaDetailColumn::Channels => a.channels.cmp(&b.channels),
                MediaDetailColumn::AvgAudioBitrate => a.avg_audio_bitrate.cmp(&b.avg_audio_bitrate),
                MediaDetailColumn::AudioFormat => a.audio_format.cmp(&b.audio_format),
                MediaDetailColumn::Fps => a.fps.total_cmp(&b.fps),
                MediaDetailColumn::FieldOrder => a.field_order.cmp(&b.field_order),
                MediaDetailColumn::Alpha => a.alpha.cmp(&b.alpha),
                MediaDetailColumn::PixelAspect => a.pixel_aspect.total_cmp(&b.pixel_aspect),
                MediaDetailColumn::FrameSize => {
                    let a_area = (a.frame_size.0 as u64) * (a.frame_size.1 as u64);
                    let b_area = (b.frame_size.0 as u64) * (b.frame_size.1 as u64);
                    a_area.cmp(&b_area)
                }
            };
            if self.sort_ascending {
                ordering
            } else {
                ordering.reverse()
            }
        });

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twenty_three_columns_coverage() {
        assert_eq!(MediaDetailColumn::ALL.len(), 23);
        for col in MediaDetailColumn::ALL {
            assert!(!col.label().is_empty());
        }
    }

    #[test]
    fn sorting_numeric_and_text_columns() {
        let mut mgr = MediaPoolManager::new();
        let mut row1 = MediaDetailRow::new(1, "Zebra", PathBuf::from("zebra.mp4"));
        row1.size_bytes = 1000;
        let mut row2 = MediaDetailRow::new(2, "Apple", PathBuf::from("apple.mp4"));
        row2.size_bytes = 5000;

        mgr.add_item(row1);
        mgr.add_item(row2);

        // Sort by name ascending
        mgr.set_sort(MediaDetailColumn::Name, true);
        let rows = mgr.filtered_rows();
        assert_eq!(rows[0].name, "Apple");
        assert_eq!(rows[1].name, "Zebra");

        // Sort by size ascending (numeric sort, not alphabetical!)
        mgr.set_sort(MediaDetailColumn::Size, true);
        let rows = mgr.filtered_rows();
        assert_eq!(rows[0].size_bytes, 1000);
        assert_eq!(rows[1].size_bytes, 5000);

        // Sort by size descending
        mgr.set_sort(MediaDetailColumn::Size, false);
        let rows = mgr.filtered_rows();
        assert_eq!(rows[0].size_bytes, 5000);
        assert_eq!(rows[1].size_bytes, 1000);
    }

    #[test]
    fn remove_protection_for_timeline_in_use_media() {
        let mut mgr = MediaPoolManager::new();
        let mut row = MediaDetailRow::new(1, "ActiveClip", PathBuf::from("active.mp4"));
        row.use_count = 2; // In use on timeline
        mgr.add_item(row);

        // Attempting to remove without confirmation must return AlertCode 17264
        let res = mgr.remove_item(1, false);
        assert_eq!(res, Err(AlertCode::TimelineMediaDeleteConfirm));
        assert_eq!(mgr.items().len(), 1);

        // With confirmation, remove succeeds
        let res = mgr.remove_item(1, true);
        assert_eq!(res, Ok(()));
        assert_eq!(mgr.items().len(), 0);
    }

    #[test]
    fn search_filtering_by_keyword() {
        let mut mgr = MediaPoolManager::new();
        let mut row1 = MediaDetailRow::new(1, "Summer Vacation", PathBuf::from("summer.mp4"));
        row1.comment = "Beach clips".into();
        let mut row2 = MediaDetailRow::new(2, "Winter Holiday", PathBuf::from("winter.mp4"));
        row2.comment = "Snow mountains".into();

        mgr.add_item(row1);
        mgr.add_item(row2);

        mgr.set_search_filter("beach");
        let filtered = mgr.filtered_rows();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, 1);
    }

    #[test]
    fn clean_unused_retains_only_used_items() {
        let mut mgr = MediaPoolManager::new();
        let mut row1 = MediaDetailRow::new(1, "Used", PathBuf::from("used.mp4"));
        row1.use_count = 1;
        let mut row2 = MediaDetailRow::new(2, "Unused", PathBuf::from("unused.mp4"));
        row2.use_count = 0;

        mgr.add_item(row1);
        mgr.add_item(row2);

        let removed = mgr.clean_unused();
        assert_eq!(removed, 1);
        assert_eq!(mgr.items().len(), 1);
        assert_eq!(mgr.items()[0].name, "Used");
    }
}
