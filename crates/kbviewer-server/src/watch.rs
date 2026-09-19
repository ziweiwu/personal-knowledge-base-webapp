//! Filesystem watching.
//!
//! The folder is expected to change underneath the server: Obsidian writes to it, and in
//! the reference setup Synology Drive syncs it. Both write by creating a temporary file
//! and renaming over the target, which produces bursts of remove/create events rather
//! than a single modify. Debouncing collapses each burst into one rebuild.

use crate::state::AppState;
use notify::event::{AccessKind, AccessMode, EventKind};
use notify::RecursiveMode;
use notify_debouncer_full::{new_debouncer, DebounceEventResult};
use std::sync::Arc;
use std::time::Duration;

/// Long enough to absorb a rename burst, short enough that an edit in Obsidian appears in
/// the browser without the user wondering whether it worked.
const DEBOUNCE: Duration = Duration::from_millis(400);

pub fn spawn(state: Arc<AppState>) -> anyhow::Result<Vec<impl Sized>> {
    let mut debouncers = Vec::new();

    for root in &state.config.roots {
        let root_id = root.id.clone();
        let root_path = root.path.clone();
        let watcher_state = state.clone();

        if !root_path.exists() {
            tracing::warn!(root = %root_id, path = %root_path.display(), "root does not exist; not watching");
            continue;
        }

        let mut debouncer = new_debouncer(DEBOUNCE, None, move |result: DebounceEventResult| {
            reindex_changed_paths(&watcher_state, &root_id, &root_path, result);
        })?;

        debouncer.watch(&root.path, RecursiveMode::Recursive)?;
        tracing::info!(root = %root.id, path = %root.path.display(), "watching for changes");
        debouncers.push(debouncer);
    }
    Ok(debouncers)
}

fn reindex_changed_paths(
    state: &Arc<AppState>,
    root_id: &str,
    root_path: &std::path::Path,
    result: DebounceEventResult,
) {
    let events = match result {
        Ok(events) => events,
        Err(errors) => {
            for error in errors {
                tracing::warn!(%error, "watch error");
            }
            return;
        }
    };

    let paths = changed_paths(root_path, &events);
    if paths.is_empty() {
        return;
    }
    // Info, not debug: one line per edit burst is cheap, and a stream of them with nobody
    // editing is the only visible sign of a rebuild feeding itself.
    tracing::info!(root = %root_id, count = paths.len(), "reindexing after change");
    // `origin: None` marks this as an external change, which is what tells the client it
    // did not cause it and should refresh.
    state.reindex(root_id, paths, None);
}

/// Relative paths worth reacting to, with read-only events and excluded directories
/// filtered out.
///
/// Without the directory filter a Synology `@eaDir` thumbnail refresh would rebuild the
/// index repeatedly for content the app never shows.
fn changed_paths(
    root: &std::path::Path,
    events: &[notify_debouncer_full::DebouncedEvent],
) -> Vec<String> {
    let mut paths: Vec<String> = events
        .iter()
        .filter(|event| is_change(&event.kind))
        .flat_map(|event| event.paths.iter())
        .filter_map(|path| relative_if_relevant(root, path))
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

/// Whether an event can mean the file's content or name changed.
///
/// On Linux, notify subscribes to inotify's `IN_OPEN` and `IN_CLOSE_NOWRITE`, so merely
/// reading a file is reported. Rebuilding the index reads every file, so without this
/// filter each rebuild triggered the next: an endless reindex loop that held a core and
/// grew the NAS container to 9 GiB within two hours. macOS reports no reads, which is why
/// it never showed up there.
fn is_change(kind: &EventKind) -> bool {
    match kind {
        EventKind::Access(AccessKind::Close(AccessMode::Write)) => true,
        EventKind::Access(_) => false,
        _ => true,
    }
}

fn relative_if_relevant(root: &std::path::Path, path: &std::path::Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    if kbviewer_core::paths::is_excluded(relative) {
        return None;
    }
    Some(relative.to_string_lossy().replace('\\', "/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn ignores_changes_in_excluded_directories() {
        let root = Path::new("/vault");
        assert!(
            relative_if_relevant(root, &std::path::PathBuf::from("/vault/@eaDir/thumb.jpg"))
                .is_none()
        );
        assert!(relative_if_relevant(
            root,
            &std::path::PathBuf::from("/vault/.obsidian/workspace.json")
        )
        .is_none());
        assert!(
            relative_if_relevant(root, &std::path::PathBuf::from("/vault/.trash/old.md")).is_none()
        );
    }

    #[test]
    fn reports_ordinary_documents() {
        let root = Path::new("/vault");
        assert_eq!(
            relative_if_relevant(root, &std::path::PathBuf::from("/vault/notes/a.md")).as_deref(),
            Some("notes/a.md")
        );
    }

    fn event(kind: EventKind, path: &str) -> notify_debouncer_full::DebouncedEvent {
        notify_debouncer_full::DebouncedEvent::new(
            notify::Event::new(kind).add_path(std::path::PathBuf::from(path)),
            std::time::Instant::now(),
        )
    }

    /// The reindex loop: rebuilding the index opens every file, and on Linux each open
    /// arrives as an event. Reads must never count as changes.
    #[test]
    fn ignores_reads() {
        let root = Path::new("/vault");
        let reads = [
            event(
                EventKind::Access(AccessKind::Open(AccessMode::Any)),
                "/vault/a.md",
            ),
            event(
                EventKind::Access(AccessKind::Close(AccessMode::Read)),
                "/vault/b.md",
            ),
        ];
        assert!(changed_paths(root, &reads).is_empty());
    }

    #[test]
    fn reports_writes() {
        let root = Path::new("/vault");
        let writes = [
            event(
                EventKind::Access(AccessKind::Close(AccessMode::Write)),
                "/vault/a.md",
            ),
            event(
                EventKind::Modify(notify::event::ModifyKind::Any),
                "/vault/b.md",
            ),
        ];
        assert_eq!(changed_paths(root, &writes), vec!["a.md", "b.md"]);
    }

    #[test]
    fn ignores_paths_outside_the_root() {
        assert!(relative_if_relevant(
            Path::new("/vault"),
            &std::path::PathBuf::from("/elsewhere/a.md")
        )
        .is_none());
    }
}
