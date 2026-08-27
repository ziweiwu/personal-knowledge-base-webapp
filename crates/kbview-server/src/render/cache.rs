//! Rendered-HTML cache, keyed by path and modification time.
//!
//! A stale entry would show the reader a document that no longer matches the file, so the
//! mtime is part of the key rather than something checked separately: a changed file
//! cannot collide with its own previous entry.

use dashmap::DashMap;

#[derive(Clone)]
pub struct CachedRender {
    pub html: String,
    pub headings: Vec<kbview_core::model::Heading>,
    pub warning: Option<String>,
}

/// Identifies one version of one document. The mtime is part of the identity rather than
/// something checked separately, so a changed file cannot collide with its own entry.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct RenderKey {
    root_id: String,
    path: String,
    mtime_ms: i64,
}

impl RenderKey {
    pub fn new(root_id: &str, path: &str, mtime_ms: i64) -> Self {
        Self {
            root_id: root_id.to_string(),
            path: path.to_string(),
            mtime_ms,
        }
    }
}

#[derive(Default)]
pub struct RenderCache {
    entries: DashMap<RenderKey, CachedRender>,
}

impl RenderCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, key: &RenderKey) -> Option<CachedRender> {
        self.entries.get(key).map(|entry| entry.clone())
    }

    pub fn put(&self, key: RenderKey, render: CachedRender) {
        self.entries.insert(key, render);
    }

    pub fn invalidate_root(&self, root_id: &str) {
        self.entries.retain(|key, _| key.root_id != root_id);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(html: &str) -> CachedRender {
        CachedRender {
            html: html.into(),
            headings: Vec::new(),
            warning: None,
        }
    }

    #[test]
    fn stores_and_returns_a_render() {
        let cache = RenderCache::new();
        cache.put(RenderKey::new("kb", "a.md", 100), render("<p>a</p>"));
        assert_eq!(
            cache.get(&RenderKey::new("kb", "a.md", 100)).unwrap().html,
            "<p>a</p>"
        );
    }

    #[test]
    fn a_changed_mtime_misses_rather_than_returning_stale_html() {
        const EDITED_MTIME_MS: i64 = 101;
        let cache = RenderCache::new();
        cache.put(RenderKey::new("kb", "a.md", 100), render("<p>old</p>"));
        assert!(
            cache
                .get(&RenderKey::new("kb", "a.md", EDITED_MTIME_MS))
                .is_none(),
            "an edited file must not hit the cache"
        );
    }

    #[test]
    fn the_same_path_in_two_roots_does_not_collide() {
        let cache = RenderCache::new();
        cache.put(RenderKey::new("one", "index.md", 1), render("<p>one</p>"));
        cache.put(RenderKey::new("two", "index.md", 1), render("<p>two</p>"));
        assert_eq!(
            cache
                .get(&RenderKey::new("one", "index.md", 1))
                .unwrap()
                .html,
            "<p>one</p>"
        );
        assert_eq!(
            cache
                .get(&RenderKey::new("two", "index.md", 1))
                .unwrap()
                .html,
            "<p>two</p>"
        );
    }

    #[test]
    fn invalidating_a_root_leaves_other_roots_alone() {
        let cache = RenderCache::new();
        cache.put(RenderKey::new("one", "a.md", 1), render("<p>one</p>"));
        cache.put(RenderKey::new("two", "a.md", 1), render("<p>two</p>"));
        cache.invalidate_root("one");
        assert!(cache.get(&RenderKey::new("one", "a.md", 1)).is_none());
        assert!(cache.get(&RenderKey::new("two", "a.md", 1)).is_some());
    }
}
