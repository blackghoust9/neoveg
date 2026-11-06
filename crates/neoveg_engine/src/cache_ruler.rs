//! Color-Coded Preview Cache Ruler Subsystem (manual pp. 207–215; R2.6).
//!
//! Tracks timeline frame cache states:
//! - Green: cached in Dynamic RAM (Shift+B) or Disk Prerender (Shift+M).
//! - Yellow: real-time playable single-stream / uncomplex composite on CPU.
//! - Red: unrendered complex composite or heavy VFX requiring prerender.
//!
//! Provides strictly O(1) amortized queries during linear playback via monotonic cursor
//! tracking, fast binary search on scrub, and granular range invalidation upon timeline edits.

use serde::{Deserialize, Serialize};

/// Color-coded cache status for a frame or span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameCacheStatus {
    /// Green: frame is cached in RAM or on disk. Zero decode cost during playback.
    GreenCached,
    /// Yellow: playable in real-time on CPU without prerendering.
    YellowRealtime,
    /// Red: complex unrendered section requiring prerender to prevent frame drops.
    #[default]
    RedUnrendered,
}

impl FrameCacheStatus {
    pub fn hex_color(self) -> &'static str {
        match self {
            FrameCacheStatus::GreenCached => "#2ECC71",
            FrameCacheStatus::YellowRealtime => "#F1C40F",
            FrameCacheStatus::RedUnrendered => "#E74C3C",
        }
    }
}

/// A contiguous interval of frames sharing the same cache status: `[start_frame, end_frame)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheSpan {
    pub start_frame: u64,
    pub end_frame: u64, // Exclusive
    pub status: FrameCacheStatus,
}

impl CacheSpan {
    pub fn len(&self) -> u64 {
        self.end_frame.saturating_sub(self.start_frame)
    }

    pub fn is_empty(&self) -> bool {
        self.start_frame >= self.end_frame
    }
}

/// Color-coded preview cache ruler maintaining coalesced disjoint intervals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreviewCacheRuler {
    total_frames: u64,
    fps: f64,
    spans: Vec<CacheSpan>,
    cursor_idx: usize,
}

impl PreviewCacheRuler {
    pub fn new(total_frames: u64, fps: f64) -> Self {
        let spans = if total_frames > 0 {
            vec![CacheSpan {
                start_frame: 0,
                end_frame: total_frames,
                status: FrameCacheStatus::RedUnrendered,
            }]
        } else {
            Vec::new()
        };

        Self {
            total_frames,
            fps,
            spans,
            cursor_idx: 0,
        }
    }

    pub fn total_frames(&self) -> u64 {
        self.total_frames
    }

    pub fn fps(&self) -> f64 {
        self.fps
    }

    pub fn all_spans(&self) -> &[CacheSpan] {
        &self.spans
    }

    /// Resizes ruler to accommodate changed project duration.
    pub fn resize(&mut self, new_total_frames: u64) {
        if new_total_frames == self.total_frames {
            return;
        }
        if new_total_frames > self.total_frames {
            // Extend with RedUnrendered
            self.mark_span(
                self.total_frames,
                new_total_frames,
                FrameCacheStatus::RedUnrendered,
            );
        } else {
            // Truncate spans to new_total_frames
            self.spans.retain_mut(|span| {
                if span.start_frame >= new_total_frames {
                    false
                } else {
                    if span.end_frame > new_total_frames {
                        span.end_frame = new_total_frames;
                    }
                    true
                }
            });
        }
        self.total_frames = new_total_frames;
        self.cursor_idx = 0;
    }

    /// Marks a frame range `[start_frame, end_frame)` with the given cache status.
    ///
    /// Automatically coalesces adjacent spans with identical status.
    pub fn mark_span(&mut self, start: u64, end: u64, status: FrameCacheStatus) {
        if start >= end || start >= self.total_frames {
            return;
        }
        let end = end.min(self.total_frames);

        let mut new_spans = Vec::new();

        for span in &self.spans {
            if span.end_frame <= start || span.start_frame >= end {
                // No overlap
                new_spans.push(span.clone());
            } else {
                // Overlaps: preserve non-overlapping parts
                if span.start_frame < start {
                    new_spans.push(CacheSpan {
                        start_frame: span.start_frame,
                        end_frame: start,
                        status: span.status,
                    });
                }
                if span.end_frame > end {
                    new_spans.push(CacheSpan {
                        start_frame: end,
                        end_frame: span.end_frame,
                        status: span.status,
                    });
                }
            }
        }

        new_spans.push(CacheSpan {
            start_frame: start,
            end_frame: end,
            status,
        });

        // Sort by start_frame and coalesce
        new_spans.sort_by_key(|s| s.start_frame);

        let mut coalesced: Vec<CacheSpan> = Vec::new();
        for span in new_spans {
            if let Some(last) = coalesced.last_mut() {
                if last.end_frame == span.start_frame && last.status == span.status {
                    last.end_frame = span.end_frame;
                    continue;
                }
            }
            coalesced.push(span);
        }

        self.spans = coalesced;
        self.cursor_idx = 0;
    }

    /// Invalidates a timeline region: reverts cached frames in `[start_frame, end_frame)` to `RedUnrendered`.
    ///
    /// Adjacent unmodified spans remain Green (Edge Case 26).
    pub fn invalidate_region(&mut self, start_frame: u64, end_frame: u64) {
        self.mark_span(start_frame, end_frame, FrameCacheStatus::RedUnrendered);
    }

    /// Queries the cache status at a specific frame index.
    ///
    /// Strictly O(1) amortized during linear playback ($F \to F+1 \to F+2$) by maintaining
    /// an active cursor pointer. Falls back to binary search upon random scrub/seek.
    pub fn status_at(&mut self, frame: u64) -> FrameCacheStatus {
        if self.spans.is_empty() || frame >= self.total_frames {
            return FrameCacheStatus::RedUnrendered;
        }

        // Fast-path: check cursor
        if self.cursor_idx < self.spans.len() {
            let cur = &self.spans[self.cursor_idx];
            if frame >= cur.start_frame && frame < cur.end_frame {
                return cur.status;
            }
            // Monotonic next check
            if self.cursor_idx + 1 < self.spans.len() {
                let next = &self.spans[self.cursor_idx + 1];
                if frame >= next.start_frame && frame < next.end_frame {
                    self.cursor_idx += 1;
                    return next.status;
                }
            }
        }

        // Fallback: binary search
        match self.spans.binary_search_by(|span| {
            if frame < span.start_frame {
                std::cmp::Ordering::Greater
            } else if frame >= span.end_frame {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        }) {
            Ok(idx) => {
                self.cursor_idx = idx;
                self.spans[idx].status
            }
            Err(_) => FrameCacheStatus::RedUnrendered,
        }
    }

    /// Returns the percentage (0.0 to 100.0) of total frames cached in RAM or on disk.
    pub fn percent_cached(&self) -> f32 {
        if self.total_frames == 0 {
            return 0.0;
        }
        let cached_count: u64 = self
            .spans
            .iter()
            .filter(|s| s.status == FrameCacheStatus::GreenCached)
            .map(|s| s.len())
            .sum();

        ((cached_count as f64 / self.total_frames as f64) * 100.0) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_state_is_all_red() {
        let ruler = PreviewCacheRuler::new(1000, 30.0);
        assert_eq!(ruler.all_spans().len(), 1);
        assert_eq!(ruler.all_spans()[0].status, FrameCacheStatus::RedUnrendered);
        assert_eq!(ruler.percent_cached(), 0.0);
    }

    #[test]
    fn mark_span_coalesces_and_updates_percent() {
        let mut ruler = PreviewCacheRuler::new(1000, 30.0);
        ruler.mark_span(100, 200, FrameCacheStatus::GreenCached);
        assert_eq!(ruler.percent_cached(), 10.0);

        // Adjacent span with same status coalesces
        ruler.mark_span(200, 300, FrameCacheStatus::GreenCached);
        assert_eq!(ruler.percent_cached(), 20.0);
        assert!(ruler
            .all_spans()
            .iter()
            .any(|s| s.start_frame == 100 && s.end_frame == 300));
    }

    #[test]
    fn invalidation_turns_cached_to_red_preserving_adjacent() {
        let mut ruler = PreviewCacheRuler::new(1000, 30.0);
        ruler.mark_span(0, 500, FrameCacheStatus::GreenCached);

        // Timeline edit at frames 100..200 invalidates only those frames
        ruler.invalidate_region(100, 200);

        assert_eq!(ruler.status_at(50), FrameCacheStatus::GreenCached);
        assert_eq!(ruler.status_at(150), FrameCacheStatus::RedUnrendered);
        assert_eq!(ruler.status_at(250), FrameCacheStatus::GreenCached);
    }

    #[test]
    fn status_at_linear_query_and_random_seek() {
        let mut ruler = PreviewCacheRuler::new(300, 30.0);
        ruler.mark_span(0, 100, FrameCacheStatus::GreenCached);
        ruler.mark_span(100, 200, FrameCacheStatus::YellowRealtime);
        ruler.mark_span(200, 300, FrameCacheStatus::RedUnrendered);

        // Monotonic progression (O(1))
        for f in 0..100 {
            assert_eq!(ruler.status_at(f), FrameCacheStatus::GreenCached);
        }
        for f in 100..200 {
            assert_eq!(ruler.status_at(f), FrameCacheStatus::YellowRealtime);
        }

        // Random jump
        assert_eq!(ruler.status_at(250), FrameCacheStatus::RedUnrendered);
        assert_eq!(ruler.status_at(10), FrameCacheStatus::GreenCached);
    }
}
