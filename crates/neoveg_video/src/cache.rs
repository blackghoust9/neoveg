//! Byte-bounded LRU cache for decoded and composited `FrameBuffer`s.

use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::sync::Arc;

use crate::frame::FrameBuffer;

/// Default frame cache budget (`128 MiB`).
pub const DEFAULT_CACHE_MAX_BYTES: usize = 128 * 1024 * 1024;

/// Generic byte-bounded LRU cache storing `Arc<FrameBuffer>` values.
#[derive(Debug)]
pub struct LruFrameCache<K: Eq + Hash + Clone> {
    max_bytes: usize,
    used_bytes: usize,
    stamp: u64,
    hits: u64,
    misses: u64,
    entries: HashMap<K, (u64, Arc<FrameBuffer>)>,
    order: VecDeque<(u64, K)>,
}

impl<K: Eq + Hash + Clone> Default for LruFrameCache<K> {
    fn default() -> Self {
        Self::new(DEFAULT_CACHE_MAX_BYTES)
    }
}

impl<K: Eq + Hash + Clone> LruFrameCache<K> {
    pub fn new(max_bytes: usize) -> Self {
        Self {
            max_bytes,
            used_bytes: 0,
            stamp: 0,
            hits: 0,
            misses: 0,
            entries: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    pub fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    pub fn used_bytes(&self) -> usize {
        self.used_bytes
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn hits(&self) -> u64 {
        self.hits
    }

    pub fn misses(&self) -> u64 {
        self.misses
    }

    /// Non-mutating membership check used by playback lookahead prefetch.
    pub fn contains(&self, key: &K) -> bool {
        self.entries.contains_key(key)
    }

    pub fn get(&mut self, key: &K) -> Option<Arc<FrameBuffer>> {
        if let Some((entry_stamp, frame)) = self.entries.get_mut(key) {
            self.hits += 1;
            self.stamp += 1;
            *entry_stamp = self.stamp;
            let frame_arc = Arc::clone(frame);
            self.order.push_back((self.stamp, key.clone()));
            self.compact_order_if_needed();
            Some(frame_arc)
        } else {
            self.misses += 1;
            None
        }
    }

    pub fn insert(&mut self, key: K, frame: Arc<FrameBuffer>) {
        let frame_bytes = frame.rgba.len();
        if frame_bytes > self.max_bytes {
            return;
        }
        if let Some((_, prev)) = self.entries.remove(&key) {
            self.used_bytes = self.used_bytes.saturating_sub(prev.rgba.len());
        }
        while self.used_bytes + frame_bytes > self.max_bytes {
            if !self.evict_oldest() {
                break;
            }
        }
        self.stamp += 1;
        self.used_bytes += frame_bytes;
        self.order.push_back((self.stamp, key.clone()));
        self.entries.insert(key, (self.stamp, frame));
        self.compact_order_if_needed();
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
        self.used_bytes = 0;
    }

    pub fn retain(&mut self, mut predicate: impl FnMut(&K) -> bool) {
        let mut kept_bytes = 0usize;
        self.entries.retain(|k, (_, f)| {
            let keep = predicate(k);
            if keep {
                kept_bytes += f.rgba.len();
            }
            keep
        });
        self.used_bytes = kept_bytes;
        let entries = &self.entries;
        self.order
            .retain(|(stamp, k)| entries.get(k).is_some_and(|(s, _)| s == stamp));
    }

    fn evict_oldest(&mut self) -> bool {
        while let Some((queued_stamp, key)) = self.order.pop_front() {
            let stale = match self.entries.get(&key) {
                Some((live_stamp, _)) => *live_stamp != queued_stamp,
                None => true,
            };
            if stale {
                continue;
            }
            if let Some((_, evicted)) = self.entries.remove(&key) {
                self.used_bytes = self.used_bytes.saturating_sub(evicted.rgba.len());
                return true;
            }
        }
        false
    }

    fn compact_order_if_needed(&mut self) {
        if self.order.len() > self.entries.len().saturating_mul(2) + 32 {
            let entries = &self.entries;
            self.order
                .retain(|(stamp, k)| entries.get(k).is_some_and(|(s, _)| s == stamp));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lru_evicts_least_recently_used_and_tracks_bytes_and_hits() {
        // Each 4x4 RGBA8 frame is 64 bytes; budget of 128 bytes fits at most 2 frames.
        let mut cache = LruFrameCache::<u32>::new(128);
        let f1 = Arc::new(FrameBuffer::solid(4, 4, [10, 0, 0, 255]));
        let f2 = Arc::new(FrameBuffer::solid(4, 4, [20, 0, 0, 255]));
        let f3 = Arc::new(FrameBuffer::solid(4, 4, [30, 0, 0, 255]));

        cache.insert(1, Arc::clone(&f1));
        cache.insert(2, Arc::clone(&f2));
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.used_bytes(), 128);

        // Touch key 1 so key 2 becomes the oldest entry.
        assert_eq!(cache.get(&1).unwrap().pixel(0, 0)[0], 10);
        assert_eq!(cache.hits(), 1);

        // Inserting key 3 must evict key 2, keeping key 1 and key 3.
        cache.insert(3, Arc::clone(&f3));
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.used_bytes(), 128);
        assert!(cache.get(&2).is_none());
        assert_eq!(cache.misses(), 1);
        assert_eq!(cache.get(&1).unwrap().pixel(0, 0)[0], 10);
        assert_eq!(cache.get(&3).unwrap().pixel(0, 0)[0], 30);
    }

    #[test]
    fn oversize_frame_is_ignored_without_evicting_existing_entries() {
        let mut cache = LruFrameCache::<u32>::new(100);
        cache.insert(1, Arc::new(FrameBuffer::solid(4, 4, [1, 2, 3, 255])));
        assert_eq!(cache.len(), 1);

        // 16x16 RGBA8 = 1024 bytes > 100 bytes max budget.
        cache.insert(2, Arc::new(FrameBuffer::solid(16, 16, [9, 9, 9, 255])));
        assert_eq!(cache.len(), 1);
        assert!(cache.contains(&1));
        assert!(!cache.contains(&2));

        cache.retain(|k| *k != 1);
        assert!(cache.is_empty());
        assert_eq!(cache.used_bytes(), 0);
    }
}
