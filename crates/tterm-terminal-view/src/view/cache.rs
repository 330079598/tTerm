//! The per-row layout cache.
//!
//! The key is [`super::row::row_hash`]: the row's characters, attributes,
//! colors and the style epoch. The snapshot version is not part of the key,
//! because it changes every frame; it only provides the fast path where a
//! blink redraw reuses the whole previous frame.
//!
//! The cache survives scrolling: when the screen moves up by one line, every
//! remaining row keeps its content hash and therefore its shaped layout. On
//! a burst that replaces every visible row at once there is nothing to reuse,
//! which is why the report lists the measured hit rate per scenario instead
//! of assuming one.

use std::collections::HashMap;
use std::sync::Arc;

use super::row::RowLayout;

/// Counters for one cache instance.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CacheStats {
    /// Rows found in the cache.
    pub hits: u64,
    /// Rows that had to be planned and shaped.
    pub misses: u64,
    /// Rows whose layout was reused from the previous frame because Alacritty
    /// did not damage them.
    pub damage_reuses: u64,
}

struct Entry {
    layout: Arc<RowLayout>,
    last_used: u64,
}

/// A bounded cache of shaped rows, keyed by content.
pub struct RowCache {
    entries: HashMap<u64, Entry>,
    capacity: usize,
    stats: CacheStats,
}

impl RowCache {
    /// A cache holding at most `capacity` rows.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: HashMap::new(),
            capacity: capacity.max(1),
            stats: CacheStats::default(),
        }
    }

    /// Returns the cached layout, or builds and stores one.
    pub fn get_or_insert_with(
        &mut self,
        key: u64,
        frame: u64,
        build: impl FnOnce() -> RowLayout,
    ) -> (Arc<RowLayout>, bool) {
        if let Some(entry) = self.entries.get_mut(&key) {
            entry.last_used = frame;
            self.stats.hits += 1;
            return (Arc::clone(&entry.layout), true);
        }
        let layout = Arc::new(build());
        self.entries.insert(
            key,
            Entry {
                layout: Arc::clone(&layout),
                last_used: frame,
            },
        );
        self.stats.misses += 1;
        self.evict(frame);
        (layout, false)
    }

    /// Records a row reused because Alacritty did not damage it.
    pub fn record_damage_reuse(&mut self) {
        self.stats.damage_reuses += 1;
    }

    fn evict(&mut self, frame: u64) {
        if self.entries.len() <= self.capacity {
            return;
        }
        // Keep rows that were used in the last two frames; those are the ones
        // a scroll is most likely to reuse.
        let cutoff = frame.saturating_sub(2);
        self.entries.retain(|_, entry| entry.last_used >= cutoff);
        while self.entries.len() > self.capacity {
            let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(key, _)| *key)
            else {
                break;
            };
            self.entries.remove(&oldest);
        }
    }

    /// The counters so far.
    #[cfg(test)]
    pub fn stats(&self) -> CacheStats {
        self.stats
    }

    /// The number of rows currently held.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache is empty.
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Drops every row, for a font, palette or metrics change.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_keys_hit_and_changed_keys_miss() {
        let mut cache = RowCache::new(16);
        let (_, hit) = cache.get_or_insert_with(1, 0, || RowLayout::empty(80));
        assert!(!hit);
        let (_, hit) = cache.get_or_insert_with(1, 1, || RowLayout::empty(80));
        assert!(hit);
        let (_, hit) = cache.get_or_insert_with(2, 1, || RowLayout::empty(80));
        assert!(!hit);
        assert_eq!(cache.stats().hits, 1);
        assert_eq!(cache.stats().misses, 2);
    }

    #[test]
    fn a_scrolled_screen_reuses_every_remaining_row() {
        let mut cache = RowCache::new(64);
        let mut rows: Vec<u64> = (0..24).collect();
        for (frame, key) in rows.iter().enumerate() {
            cache.get_or_insert_with(*key, frame as u64, || RowLayout::empty(80));
        }
        let before = cache.stats();
        assert_eq!(before.misses, 24);
        assert_eq!(before.hits, 0);

        // One line of output: rows 1..24 move up and row 24 is new.
        rows.remove(0);
        rows.push(24);
        for (frame, key) in rows.iter().enumerate() {
            cache.get_or_insert_with(*key, 24 + frame as u64, || RowLayout::empty(80));
        }
        let after = cache.stats();
        assert_eq!(after.hits, 23);
        assert_eq!(after.misses, 25);
        assert!(after.hits > after.misses / 2);
    }

    #[test]
    fn the_cache_stays_bounded() {
        let mut cache = RowCache::new(4);
        for key in 0..20 {
            cache.get_or_insert_with(key, key, || RowLayout::empty(80));
        }
        assert!(cache.len() <= 4);
    }

    #[test]
    fn clearing_drops_every_row() {
        let mut cache = RowCache::new(8);
        cache.get_or_insert_with(7, 0, || RowLayout::empty(80));
        assert_eq!(cache.len(), 1);
        cache.clear();
        assert!(cache.is_empty());
    }
}
