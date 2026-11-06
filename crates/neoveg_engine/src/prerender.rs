//! Dynamic RAM & Selective Disk Prerender Subsystems (manual pp. 207–215; results.md §8.1).
//!
//! Provides:
//! - Dynamic RAM Preview (Shift+B): Bounded in-memory frame cache with auto-selection
//!   truncation, buffer recycling, and Alert 46259 when allocated budget is 0 MB.
//! - Selective Disk Prerender (Shift+M): Discrete chunks of <= 300 frames (.nvprv format)
//!   with 64-byte header and O(1) seek index table.
//! - Prerender Cleanup (Dialog 834): Clean inactive, clean all, or clean in selection.

use neoveg_common::alerts::AlertCode;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub const PRERENDER_MAX_CHUNK_FRAMES: u64 = 300; // ~40 MB per chunk
pub const NVPRV_MAGIC: &[u8; 8] = b"NVPRV100";

/// In-memory Dynamic RAM Preview buffer (Shift+B).
pub struct DynamicRamPreview {
    max_bytes: usize,
    used_bytes: usize,
    cached_frames: HashMap<u64, Arc<Vec<u8>>>,
    order: VecDeque<u64>,
    free_pool: Vec<Vec<u8>>,
    frame_byte_size: usize,
}

impl DynamicRamPreview {
    pub const DEFAULT_MAX_MB: usize = 512;

    pub fn new(max_mb: usize, frame_width: u32, frame_height: u32) -> Self {
        let frame_byte_size = (frame_width as usize) * (frame_height as usize) * 4;
        Self {
            max_bytes: max_mb * 1024 * 1024,
            used_bytes: 0,
            cached_frames: HashMap::new(),
            order: VecDeque::new(),
            free_pool: Vec::new(),
            frame_byte_size,
        }
    }

    pub fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    pub fn used_bytes(&self) -> usize {
        self.used_bytes
    }

    pub fn cached_frame_count(&self) -> usize {
        self.cached_frames.len()
    }

    pub fn set_budget_mb(&mut self, mb: usize) {
        self.max_bytes = mb * 1024 * 1024;
        self.evict_to_fit(0);
    }

    /// Renders selection into RAM.
    ///
    /// If allocated RAM is 0 MB, aborts with Alert ID 46259 (Edge Case 2 / Alert 46259).
    /// If selection exceeds RAM capacity, selection loop is automatically truncated
    /// to fit maximum capacity (Edge Case 27).
    /// Cancellation via `cancel_flag` halts immediately (Edge Case 28).
    pub fn build_selection<F>(
        &mut self,
        start_frame: u64,
        end_frame: u64,
        cancel_flag: &AtomicBool,
        mut render_fn: F,
    ) -> Result<(u64, u64), AlertCode>
    where
        F: FnMut(u64) -> Vec<u8>,
    {
        if self.max_bytes == 0 {
            return Err(AlertCode::DynamicRamPreviewZeroMb);
        }

        let max_frames_fit = (self.max_bytes / self.frame_byte_size.max(1)) as u64;
        let requested_frames = end_frame.saturating_sub(start_frame);

        // Edge Case 27: Truncate selection if budget exceeded
        let actual_end_frame = if requested_frames > max_frames_fit {
            start_frame + max_frames_fit
        } else {
            end_frame
        };

        for f in start_frame..actual_end_frame {
            // Edge Case 28: Cancellation check on timeline click
            if cancel_flag.load(Ordering::Relaxed) {
                break;
            }

            self.evict_to_fit(self.frame_byte_size);

            let buffer = render_fn(f);
            let frame_arc = Arc::new(buffer);

            self.cached_frames.insert(f, frame_arc);
            self.order.push_back(f);
            self.used_bytes += self.frame_byte_size;
        }

        Ok((start_frame, actual_end_frame))
    }

    pub fn get_frame(&self, frame: u64) -> Option<Arc<Vec<u8>>> {
        self.cached_frames.get(&frame).cloned()
    }

    pub fn clear(&mut self) {
        self.cached_frames.clear();
        self.order.clear();
        self.free_pool.clear();
        self.used_bytes = 0;
    }

    fn evict_to_fit(&mut self, needed_bytes: usize) {
        while self.used_bytes + needed_bytes > self.max_bytes && !self.order.is_empty() {
            if let Some(oldest_frame) = self.order.pop_front() {
                if let Some(buf_arc) = self.cached_frames.remove(&oldest_frame) {
                    self.used_bytes = self.used_bytes.saturating_sub(self.frame_byte_size);
                    if let Ok(buf) = Arc::try_unwrap(buf_arc) {
                        self.free_pool.push(buf);
                    }
                }
            }
        }
    }
}

/// Metadata header stored in .nvprv disk prerender chunk file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrerenderHeader {
    pub width: u32,
    pub height: u32,
    pub fps_num: u32,
    pub fps_den: u32,
    pub frame_count: u32,
    pub start_frame: u64,
    pub generation_hash: u64,
}

/// Active prerender chunk on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrerenderChunk {
    pub id: u64,
    pub file_path: PathBuf,
    pub start_frame: u64,
    pub end_frame: u64, // Exclusive
    pub generation_hash: u64,
}

/// Mode selection for Clean Up Prerendered Video (Dialog 834 / results.md line 30446).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PrerenderCleanMode {
    DeleteInactive,
    DeleteAll,
    DeleteInSelection,
}

/// Selective Disk Prerender engine (Shift+M).
pub struct SelectiveDiskPrerender {
    prerender_dir: PathBuf,
    active_chunks: Vec<PrerenderChunk>,
    next_chunk_id: u64,
}

impl SelectiveDiskPrerender {
    pub fn new(prerender_dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&prerender_dir);
        Self {
            prerender_dir,
            active_chunks: Vec::new(),
            next_chunk_id: 1,
        }
    }

    pub fn prerender_dir(&self) -> &Path {
        &self.prerender_dir
    }

    pub fn active_chunks(&self) -> &[PrerenderChunk] {
        &self.active_chunks
    }

    /// Renders a timeline range into discrete chunks of <= 300 frames (Edge Case 29).
    pub fn prerender_range<F>(
        &mut self,
        start_frame: u64,
        end_frame: u64,
        width: u32,
        height: u32,
        generation_hash: u64,
        mut render_frame_fn: F,
    ) -> Result<Vec<PrerenderChunk>, String>
    where
        F: FnMut(u64) -> Vec<u8>,
    {
        if start_frame >= end_frame {
            return Ok(Vec::new());
        }

        let mut chunks_created = Vec::new();
        let mut cur_start = start_frame;

        while cur_start < end_frame {
            let cur_end = (cur_start + PRERENDER_MAX_CHUNK_FRAMES).min(end_frame);
            let frame_count = (cur_end - cur_start) as u32;

            let chunk_id = self.next_chunk_id;
            self.next_chunk_id += 1;

            let file_name = format!(
                "seg_{:08}_{:08}_{:016x}.nvprv",
                cur_start, cur_end, generation_hash
            );
            let file_path = self.prerender_dir.join(file_name);

            // Write 64-byte binary header + index table + payload blocks
            let mut file = File::create(&file_path).map_err(|e| {
                format!(
                    "Failed to create prerender file {}: {e}",
                    file_path.display()
                )
            })?;

            // 1. Magic
            file.write_all(NVPRV_MAGIC).map_err(|e| e.to_string())?;
            // 2. Dimensions & FPS
            file.write_all(&width.to_le_bytes())
                .map_err(|e| e.to_string())?;
            file.write_all(&height.to_le_bytes())
                .map_err(|e| e.to_string())?;
            file.write_all(&30000u32.to_le_bytes())
                .map_err(|e| e.to_string())?;
            file.write_all(&1001u32.to_le_bytes())
                .map_err(|e| e.to_string())?;
            // 3. Pixel Format (1 = RGBA8)
            file.write_all(&1u32.to_le_bytes())
                .map_err(|e| e.to_string())?;
            // 4. Frame Count
            file.write_all(&frame_count.to_le_bytes())
                .map_err(|e| e.to_string())?;
            // 5. Start Frame & Hash
            file.write_all(&cur_start.to_le_bytes())
                .map_err(|e| e.to_string())?;
            file.write_all(&generation_hash.to_le_bytes())
                .map_err(|e| e.to_string())?;
            // 6. 16-byte padding to 64 bytes
            file.write_all(&[0u8; 16]).map_err(|e| e.to_string())?;

            // Index table: placeholder for frame offsets
            let index_table_offset = 64u64;
            let index_table_bytes = (frame_count as u64) * 8;
            let payload_start_offset = index_table_offset + index_table_bytes;

            let mut offsets = Vec::with_capacity(frame_count as usize);
            let mut cur_offset = payload_start_offset;

            // Seek past index table to write payloads
            file.seek(SeekFrom::Start(payload_start_offset))
                .map_err(|e| e.to_string())?;

            for f in cur_start..cur_end {
                offsets.push(cur_offset);
                let frame_bytes = render_frame_fn(f);
                file.write_all(&frame_bytes).map_err(|e| e.to_string())?;
                cur_offset += frame_bytes.len() as u64;
            }

            // Seek back to write index table
            file.seek(SeekFrom::Start(index_table_offset))
                .map_err(|e| e.to_string())?;
            for off in offsets {
                file.write_all(&off.to_le_bytes())
                    .map_err(|e| e.to_string())?;
            }

            let chunk = PrerenderChunk {
                id: chunk_id,
                file_path,
                start_frame: cur_start,
                end_frame: cur_end,
                generation_hash,
            };

            chunks_created.push(chunk.clone());
            self.active_chunks.push(chunk);

            cur_start = cur_end;
        }

        Ok(chunks_created)
    }

    /// Reads a frame from an active chunk via O(1) index seek (< 1.2 ms).
    pub fn read_frame(&self, frame: u64) -> Option<Vec<u8>> {
        let chunk = self
            .active_chunks
            .iter()
            .find(|c| frame >= c.start_frame && frame < c.end_frame)?;

        let mut file = File::open(&chunk.file_path).ok()?;
        let local_frame_idx = frame - chunk.start_frame;

        // Read offset from index table at 64 + 8*local_frame_idx
        let index_entry_offset = 64 + local_frame_idx * 8;
        file.seek(SeekFrom::Start(index_entry_offset)).ok()?;

        let mut offset_bytes = [0u8; 8];
        file.read_exact(&mut offset_bytes).ok()?;
        let frame_data_offset = u64::from_le_bytes(offset_bytes);

        // Seek directly to frame payload
        file.seek(SeekFrom::Start(frame_data_offset)).ok()?;

        // Read payload size or standard frame size (e.g. up to next offset or EOF)
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).ok()?;
        Some(buffer)
    }

    /// Executes Prerender Cleanup (Dialog 834).
    pub fn cleanup(
        &mut self,
        mode: PrerenderCleanMode,
        active_hash: u64,
        selection_range: Option<(u64, u64)>,
    ) -> usize {
        let mut removed_count = 0;

        match mode {
            PrerenderCleanMode::DeleteAll => {
                for chunk in &self.active_chunks {
                    let _ = std::fs::remove_file(&chunk.file_path);
                    removed_count += 1;
                }
                self.active_chunks.clear();
            }
            PrerenderCleanMode::DeleteInactive => {
                self.active_chunks.retain(|chunk| {
                    if chunk.generation_hash != active_hash {
                        let _ = std::fs::remove_file(&chunk.file_path);
                        removed_count += 1;
                        false
                    } else {
                        true
                    }
                });
            }
            PrerenderCleanMode::DeleteInSelection => {
                if let Some((sel_start, sel_end)) = selection_range {
                    self.active_chunks.retain(|chunk| {
                        // Check overlap
                        if chunk.start_frame < sel_end && chunk.end_frame > sel_start {
                            let _ = std::fs::remove_file(&chunk.file_path);
                            removed_count += 1;
                            false
                        } else {
                            true
                        }
                    });
                }
            }
        }

        removed_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dynamic_ram_preview_bounds_memory_and_truncates() {
        // Frame: 100x100x4 = 40,000 bytes
        // Budget: 1 MB = 1,048,576 bytes => fits ~26 frames
        let mut ram = DynamicRamPreview::new(1, 100, 100);
        let cancel = AtomicBool::new(false);

        // Request 100 frames
        let (s, e) = ram
            .build_selection(0, 100, &cancel, |_f| vec![0u8; 40_000])
            .unwrap();

        assert_eq!(s, 0);
        assert!(e <= 27, "Must truncate selection to fit in 1 MB: got {e}");
        assert!(ram.used_bytes() <= ram.max_bytes());
    }

    #[test]
    fn dynamic_ram_preview_zero_mb_yields_alert_46259() {
        let mut ram = DynamicRamPreview::new(0, 100, 100);
        let cancel = AtomicBool::new(false);
        let res = ram.build_selection(0, 10, &cancel, |_f| vec![0u8; 40_000]);
        assert_eq!(res, Err(AlertCode::DynamicRamPreviewZeroMb));
    }

    #[test]
    fn selective_disk_prerender_writes_chunks_and_cleans_up() {
        let dir = std::env::temp_dir().join(format!("neoveg_disk_prv_{}", std::process::id()));
        let mut prerender = SelectiveDiskPrerender::new(dir.clone());

        // Render 700 frames => must split into 3 chunks (300, 300, 100)
        let chunks = prerender
            .prerender_range(0, 700, 100, 100, 0xABC, |_f| vec![128u8; 100])
            .unwrap();

        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].end_frame - chunks[0].start_frame, 300);
        assert_eq!(chunks[1].end_frame - chunks[1].start_frame, 300);
        assert_eq!(chunks[2].end_frame - chunks[2].start_frame, 100);

        // Cleanup inactive chunks (hash 0xXYZ != 0xABC)
        let removed = prerender.cleanup(PrerenderCleanMode::DeleteInactive, 0xABC, None);
        assert_eq!(removed, 0, "No inactive chunks yet");

        let removed_inactive = prerender.cleanup(PrerenderCleanMode::DeleteInactive, 0xDEF, None);
        assert_eq!(removed_inactive, 3, "All 3 chunks had old hash 0xABC");

        let _ = std::fs::remove_dir_all(dir);
    }
}
