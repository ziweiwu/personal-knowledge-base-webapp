//! Filesystem watching.
//!
//! The folder is expected to change underneath the server: Obsidian writes to it, and in
//! the reference setup Synology Drive syncs it. Both write by creating a temporary file
//! and renaming over the target, which produces bursts of remove/create events rather
//! than a single modify. Debouncing collapses each burst into one rebuild.

use crate::state::AppState;
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
    tracing::debug!(root = %root_id, count = paths.len(), "reindexing after change");
    // `origin: None` marks this as an external change, which is what tells the client it
    // did not cause it and should refresh.
    state.reindex(root_id, paths, None);
}

/// Relative paths worth reacting to, with excluded directories filtered out.
///
/// Without this filter a Synology `@eaDir` thumbnail refresh would rebuild the index
/// repeatedly for content the app never shows.
fn changed_paths(
    root: &std::path::Path,
    events: &[notify_debouncer_full::DebouncedEvent],
) -> Vec<String> {
    let mut paths: Vec<String> = events
        .iter()
        .flat_map(|event| event.paths.iter())
        .filter_map(|path| relative_if_relevant(root, path))
        .collect();
    paths.sort();
    paths.dedup();
    paths
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

    #[test]
    fn ignores_paths_outside_the_root() {
        assert!(relative_if_relevant(
            Path::new("/vault"),
            &std::path::PathBuf::from("/elsewhere/a.md")
        )
        .is_none());
    }
}
