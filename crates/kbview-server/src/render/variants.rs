//! Cache of resized image variants.
//!
//! Unlike the render cache this one is bounded by *bytes*, not entries: a rendered page is
//! kilobytes and a decoded screenshot is megabytes, so counting entries would be counting
//! the wrong thing. When the budget is exceeded the whole cache is dropped rather than
//! evicted one entry at a time — regenerating a variant costs one resize, the corpus this
//! app is built for never comes close to the budget, and an LRU is a data structure whose
//! bugs are silent.

use dashmap::DashMap;
use kbview_core::images::VariantFormat;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Roughly a hundred resized screenshots. Deliberately modest: this also runs on a NAS
/// where RAM is the scarce resource and the disk is not.
const BUDGET_BYTES: usize = 64 * 1024 * 1024;

/// The outcome of resizing one image at one width.
#[derive(Clone)]
pub enum Variant {
    Resized {
        bytes: Vec<u8>,
        format: VariantFormat,
    },
    /// Resizing produced something no smaller than the source, so the source is the better
    /// answer. Cached as a result in its own right: without it every request would redo
    /// the decode to reach the same conclusion.
    NotWorthIt,
}

impl Variant {
    fn size(&self) -> usize {
        match self {
            Self::Resized { bytes, .. } => bytes.len(),
            Self::NotWorthIt => 0,
        }
    }
}

/// One version of one image at one width. The mtime is part of the identity, so an edited
/// image cannot collide with its own previous variant.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct VariantKey {
    root_id: String,
    path: String,
    mtime_ms: i64,
    width: u32,
}

impl VariantKey {
    pub fn new(root_id: &str, path: &str, mtime_ms: i64, width: u32) -> Self {
        Self {
            root_id: root_id.to_string(),
            path: path.to_string(),
            mtime_ms,
            width,
        }
    }

    /// Stable enough to use as an ETag: it names the file, its version and the width.
    pub fn etag(&self) -> String {
        format!("\"{}-{}-w{}\"", self.mtime_ms, self.path.len(), self.width)
    }
}

#[derive(Default)]
pub struct VariantCache {
    entries: DashMap<VariantKey, Variant>,
    bytes: AtomicUsize,
}

impl VariantCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, key: &VariantKey) -> Option<Variant> {
        self.entries.get(key).map(|entry| entry.clone())
    }

    pub fn put(&self, key: VariantKey, variant: Variant) {
        let size = variant.size();
        if size > BUDGET_BYTES {
            return;
        }
        if self.bytes.load(Ordering::Relaxed) + size > BUDGET_BYTES {
            self.entries.clear();
            self.bytes.store(0, Ordering::Relaxed);
        }
        if self.entries.insert(key, variant).is_none() {
            self.bytes.fetch_add(size, Ordering::Relaxed);
        }
    }

    pub fn invalidate_root(&self, root_id: &str) {
        self.entries.retain(|key, _| key.root_id != root_id);
        // Recount rather than subtract: a `retain` that removed several entries leaves no
        // per-entry hook to decrement from, and an over-count would evict early forever.
        let total = self.entries.iter().map(|entry| entry.size()).sum();
        self.bytes.store(total, Ordering::Relaxed);
    }
}

/// The widths a variant may be requested at.
///
/// A whitelist rather than any integer: an open parameter lets one caller fill the cache
/// with a thousand near-identical renderings of the same screenshot.
pub const WIDTHS: [u32; 4] = [400, 800, 1200, 1600];

pub fn is_offered(width: u32) -> bool {
    WIDTHS.contains(&width)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two offered widths, so a test can say "a different width" without a bare number.
    const A_WIDTH: u32 = 800;
    const ANOTHER_WIDTH: u32 = 400;
    /// Deliberately not in `WIDTHS`.
    const AN_UNOFFERED_WIDTH: u32 = 801;
    const SOME_BYTES: usize = 10;
    const MORE_BYTES: usize = 20;

    fn variant(size: usize) -> Variant {
        Variant::Resized {
            bytes: vec![0; size],
            format: VariantFormat::Jpeg,
        }
    }

    #[test]
    fn a_variant_round_trips() {
        let cache = VariantCache::new();
        let key = VariantKey::new("kb", "a.png", 1, A_WIDTH);
        cache.put(key.clone(), variant(SOME_BYTES));
        assert!(
            matches!(cache.get(&key), Some(Variant::Resized { bytes, .. }) if bytes.len() == SOME_BYTES)
        );
    }

    #[test]
    fn a_different_mtime_is_a_different_entry() {
        let cache = VariantCache::new();
        cache.put(
            VariantKey::new("kb", "a.png", 1, A_WIDTH),
            variant(SOME_BYTES),
        );
        assert!(
            cache
                .get(&VariantKey::new("kb", "a.png", 2, A_WIDTH))
                .is_none(),
            "an edited image must not serve its previous variant"
        );
    }

    #[test]
    fn a_different_width_is_a_different_entry() {
        let cache = VariantCache::new();
        cache.put(
            VariantKey::new("kb", "a.png", 1, A_WIDTH),
            variant(SOME_BYTES),
        );
        assert!(cache
            .get(&VariantKey::new("kb", "a.png", 1, ANOTHER_WIDTH))
            .is_none());
    }

    #[test]
    fn exceeding_the_budget_clears_rather_than_growing_without_bound() {
        let cache = VariantCache::new();
        cache.put(
            VariantKey::new("kb", "a.png", 1, A_WIDTH),
            variant(BUDGET_BYTES / 2),
        );
        cache.put(
            VariantKey::new("kb", "b.png", 1, A_WIDTH),
            variant(BUDGET_BYTES / 2),
        );
        cache.put(
            VariantKey::new("kb", "c.png", 1, A_WIDTH),
            variant(BUDGET_BYTES / 2),
        );
        assert!(cache.bytes.load(Ordering::Relaxed) <= BUDGET_BYTES);
        assert!(
            cache
                .get(&VariantKey::new("kb", "c.png", 1, A_WIDTH))
                .is_some(),
            "the entry that triggered the clear must still be stored"
        );
    }

    #[test]
    fn invalidating_a_root_leaves_the_others_and_recounts() {
        let cache = VariantCache::new();
        cache.put(
            VariantKey::new("kb", "a.png", 1, A_WIDTH),
            variant(SOME_BYTES),
        );
        cache.put(
            VariantKey::new("other", "b.png", 1, A_WIDTH),
            variant(MORE_BYTES),
        );
        cache.invalidate_root("kb");
        assert!(cache
            .get(&VariantKey::new("kb", "a.png", 1, A_WIDTH))
            .is_none());
        assert!(cache
            .get(&VariantKey::new("other", "b.png", 1, A_WIDTH))
            .is_some());
        assert_eq!(cache.bytes.load(Ordering::Relaxed), MORE_BYTES);
    }

    /// A negative result is worth caching too: it costs a full decode to establish.
    #[test]
    fn a_not_worth_it_result_is_remembered() {
        let cache = VariantCache::new();
        let key = VariantKey::new("kb", "flat.png", 1, ANOTHER_WIDTH);
        cache.put(key.clone(), Variant::NotWorthIt);
        assert!(matches!(cache.get(&key), Some(Variant::NotWorthIt)));
    }

    #[test]
    fn only_whitelisted_widths_are_offered() {
        assert!(is_offered(A_WIDTH));
        assert!(
            !is_offered(AN_UNOFFERED_WIDTH),
            "an open width parameter would let the cache be filled"
        );
    }
}
