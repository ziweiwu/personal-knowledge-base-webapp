//! Shared server state: the loaded config, one index per root, and the change channel.

use arc_swap::ArcSwap;
use dashmap::DashMap;
use kbview_core::config::{Config, RootConfig};
use kbview_core::index::Index;
use kbview_core::model::ChangeEvent;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::auth::store::AuthStore;
use crate::render::cache::RenderCache;
use crate::render::variants::VariantCache;

pub struct AppState {
    pub config: Config,
    /// One index per root, swapped wholesale when the watcher sees a change. Readers take
    /// a consistent snapshot without locking, so a rebuild never blocks a request.
    indexes: HashMap<String, ArcSwap<Index>>,
    pub renders: RenderCache,
    /// Resized images, keyed by path, mtime and width.
    pub variants: VariantCache,
    pub auth: AuthStore,
    pub changes: tokio::sync::broadcast::Sender<ChangeEvent>,

    /// Writes this server made, so the watcher's echo of them can be attributed.
    ///
    /// A save emits one change event immediately, then the filesystem watcher notices the
    /// same write a moment later and would emit a second one with no origin. The editing
    /// tab would read that as someone else changing the file underneath it and warn about
    /// its own save. Recording the write lets the echo carry the same tag.
    ///
    /// The recorded **mtime** is what makes this safe. Matching on path alone would also
    /// swallow a genuine external edit that happened to land within the window — exactly
    /// the event the author most needs to see. An external write changes the mtime, so it
    /// no longer matches and is correctly reported as external.
    recent_writes: DashMap<(String, String), RecentWrite>,
}

#[derive(Clone)]
struct RecentWrite {
    origin: String,
    /// The mtime the file had immediately after we wrote it.
    mtime_ms: i64,
    at: Instant,
}

/// How long a write stays attributable. Comfortably longer than the watcher debounce,
/// short enough that a genuine external edit moments later is still reported as external.
const WRITE_ATTRIBUTION_WINDOW: Duration = Duration::from_secs(5);

/// Buffered change events per subscriber. A client that falls further behind than this is
/// told it lagged and refetches, which is cheaper than holding an unbounded queue.
const CHANGE_CHANNEL_CAPACITY: usize = 256;

/// A path that no longer exists reports 0, which still distinguishes it from any write.
fn mtime_of(index: &Index, path: &str) -> i64 {
    index
        .get(path)
        .map(|document| document.mtime_ms)
        .unwrap_or(0)
}

impl AppState {
    pub fn new(config: Config, auth: AuthStore) -> Arc<Self> {
        let indexes = config
            .roots
            .iter()
            .map(|root| {
                let index = Index::build(root);
                tracing::info!(
                    root = %root.id,
                    documents = index.documents.len(),
                    wikilinks = index.wikilinks,
                    "indexed root"
                );
                (root.id.clone(), ArcSwap::from_pointee(index))
            })
            .collect();

        let (changes, _) = tokio::sync::broadcast::channel(CHANGE_CHANNEL_CAPACITY);
        Arc::new(Self {
            config,
            indexes,
            renders: RenderCache::new(),
            variants: VariantCache::new(),
            auth,
            changes,
            recent_writes: DashMap::new(),
        })
    }

    pub fn index(&self, root_id: &str) -> Option<Arc<Index>> {
        self.indexes.get(root_id).map(|slot| slot.load_full())
    }

    pub fn root(&self, root_id: &str) -> Option<&RootConfig> {
        self.config.root(root_id)
    }

    /// Rebuild one root's index and tell connected clients what changed.
    ///
    /// `origin` identifies the client that asked for the change, or `None` when the change
    /// came from outside this server (Obsidian, a sync client, an editor).
    pub fn reindex(&self, root_id: &str, paths: Vec<String>, origin: Option<String>) {
        let Some(root) = self.config.root(root_id) else {
            return;
        };
        let Some(slot) = self.indexes.get(root_id) else {
            return;
        };

        let rebuilt = Arc::new(Index::build(root));
        slot.store(rebuilt.clone());
        self.renders.invalidate_root(root_id);
        self.variants.invalidate_root(root_id);

        let origin = match origin {
            Some(origin) => {
                let written = paths
                    .iter()
                    .map(|path| (path.clone(), mtime_of(&rebuilt, path)))
                    .collect();
                self.remember_write(root_id, &origin, written);
                Some(origin)
            }
            // No caller means the watcher saw it. Attribute it to our own recent write
            // only if the file still holds exactly what we wrote.
            None => self.attribute_to_recent_write(root_id, &paths, &rebuilt),
        };

        let _ = self.changes.send(ChangeEvent {
            root_id: root_id.to_string(),
            paths,
            origin,
        });
    }

    /// Record what we just wrote, with the mtime it left behind, so the watcher's echo of
    /// our own write can be attributed to the client that caused it.
    fn remember_write(&self, root_id: &str, origin: &str, written: Vec<(String, i64)>) {
        let now = Instant::now();
        for (path, mtime_ms) in written {
            self.recent_writes.insert(
                (root_id.to_string(), path),
                RecentWrite {
                    origin: origin.to_string(),
                    mtime_ms,
                    at: now,
                },
            );
        }
        self.recent_writes
            .retain(|_, write| now.duration_since(write.at) < WRITE_ATTRIBUTION_WINDOW);
    }

    fn attribute_to_recent_write(
        &self,
        root_id: &str,
        paths: &[String],
        index: &Index,
    ) -> Option<String> {
        let now = Instant::now();
        for path in paths {
            let key = (root_id.to_string(), path.clone());
            let Some(entry) = self.recent_writes.get(&key).map(|e| e.value().clone()) else {
                continue;
            };
            if now.duration_since(entry.at) >= WRITE_ATTRIBUTION_WINDOW {
                self.recent_writes.remove(&key);
                continue;
            }
            // Content changed since our write, so someone else touched it. Drop the record
            // so a later echo cannot inherit this origin either.
            if mtime_of(index, path) != entry.mtime_ms {
                self.recent_writes.remove(&key);
                continue;
            }
            return Some(entry.origin);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kbview_core::config::{Config, RootConfig};
    use std::path::PathBuf;

    fn state(label: &str) -> (Arc<AppState>, PathBuf) {
        let base = std::env::temp_dir().join(format!("kbview-state-{label}"));
        let _ = std::fs::remove_dir_all(&base);
        let root = base.join("vault");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("a.md"), "# A\n").unwrap();

        let config = Config {
            host: "127.0.0.1".into(),
            port: 0,
            data_dir: base.join("data"),
            roots: vec![RootConfig {
                id: "kb".into(),
                name: "kb".into(),
                path: root.canonicalize().unwrap(),
                index_names: vec!["index.md".into()],
                wikilinks: Some(true),
                folder_notes: false,
                read_only: false,
            }],
        };
        let auth = AuthStore::open(&config.data_dir).unwrap();
        let root = config.roots[0].path.clone();
        (AppState::new(config, auth), root)
    }

    /// Long enough for a coarse filesystem timestamp to advance between two writes.
    const MTIME_GRANULARITY_PAUSE: std::time::Duration = std::time::Duration::from_millis(20);

    /// Writing through the app bumps mtime; pausing keeps the two writes distinguishable
    /// on filesystems with coarse timestamps.
    fn write_and_reindex(
        state: &AppState,
        root: &std::path::Path,
        body: &str,
        origin: Option<&str>,
    ) {
        std::thread::sleep(MTIME_GRANULARITY_PAUSE);
        std::fs::write(root.join("a.md"), body).unwrap();
        state.reindex("kb", vec!["a.md".into()], origin.map(str::to_string));
    }

    fn last_origin(receiver: &mut tokio::sync::broadcast::Receiver<ChangeEvent>) -> Option<String> {
        let mut seen = None;
        while let Ok(event) = receiver.try_recv() {
            seen = Some(event.origin);
        }
        seen.flatten()
    }

    #[test]
    fn the_watcher_echo_of_our_own_write_is_attributed_to_its_author() {
        let (state, root) = state("echo");
        let mut rx = state.changes.subscribe();

        write_and_reindex(&state, &root, "# A\nmine\n", Some("tabX"));
        assert_eq!(last_origin(&mut rx).as_deref(), Some("tabX"));

        // The watcher notices the same write moments later, with no caller.
        state.reindex("kb", vec!["a.md".into()], None);
        assert_eq!(
            last_origin(&mut rx).as_deref(),
            Some("tabX"),
            "the author must not be warned about their own save"
        );
    }

    /// The bug this guards against: matching on path alone made any external edit within
    /// the attribution window inherit the previous author's id, so the client silently
    /// discarded the very event it most needed to see.
    #[test]
    fn a_genuine_external_edit_soon_after_our_write_is_not_attributed_to_us() {
        let (state, root) = state("external");
        let mut rx = state.changes.subscribe();

        write_and_reindex(&state, &root, "# A\nmine\n", Some("tabX"));
        let _ = last_origin(&mut rx);

        // Someone else edits the file immediately afterwards, well inside the window.
        write_and_reindex(&state, &root, "# A\nmine\nsomeone else\n", None);
        assert_eq!(
            last_origin(&mut rx),
            None,
            "an edit by another program must be reported as external, not as our own echo"
        );
    }

    #[test]
    fn a_later_echo_cannot_inherit_an_origin_after_an_external_edit_broke_the_match() {
        let (state, root) = state("poison");
        let mut rx = state.changes.subscribe();

        write_and_reindex(&state, &root, "# A\nmine\n", Some("tabX"));
        let _ = last_origin(&mut rx);
        write_and_reindex(&state, &root, "# A\nexternal\n", None);
        let _ = last_origin(&mut rx);

        state.reindex("kb", vec!["a.md".into()], None);
        assert_eq!(
            last_origin(&mut rx),
            None,
            "the stale record must have been dropped, not left to tag later events"
        );
    }

    #[test]
    fn an_unrelated_path_is_never_attributed() {
        let (state, root) = state("unrelated");
        let mut rx = state.changes.subscribe();

        write_and_reindex(&state, &root, "# A\nmine\n", Some("tabX"));
        let _ = last_origin(&mut rx);

        std::fs::write(root.join("b.md"), "# B\n").unwrap();
        state.reindex("kb", vec!["b.md".into()], None);
        assert_eq!(last_origin(&mut rx), None);
    }
}
