//! Trash routes: list what `delete` moved into `.trash/`, and put one file back.
//!
//! Delete is recoverable by construction (INV-12), but recovering meant a shell on the
//! NAS, which from a phone means never. Restore is the same move as delete run in
//! reverse, under the same write gate and the same excluded-path rules, so it adds no
//! new way to write into a root.

use crate::error::{AppError, AppResult};
use crate::routes::read::RootQuery;
use crate::routes::write::{
    move_file, mtime_ms, one_writer, origin_of, reject_excluded, writable_root,
};
use crate::state::AppState;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use kbviewer_core::model::{RestoreRequest, TrashEntry};
use kbviewer_core::paths::{is_excluded, resolve_in_root};
use std::path::Path as FsPath;
use std::sync::Arc;

/// Where `delete` puts files, matching Obsidian's default.
const TRASH_DIR: &str = ".trash";

/// What `trash_destination` puts between a stem and its counter: `name (2).md`.
const COUNTER_OPEN: &str = " (";
const COUNTER_CLOSE: char = ')';
/// The counter `trash_destination` falls back to after a thousand repeats.
const OVERFLOW_COUNTER: &str = "overflow";

/// Every file in the root's `.trash/`, newest edit first.
///
/// Read straight from disk rather than the index, which deliberately never sees this
/// directory. Junk the sync client or DSM drops in here (`.DS_Store`, `@eaDir`) is
/// skipped by the same rule that hides it everywhere else.
pub async fn list(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RootQuery>,
) -> AppResult<Json<Vec<TrashEntry>>> {
    let root = writable_root(&state, &query.root)?;
    let bin = root.path.join(TRASH_DIR);
    if !bin.is_dir() {
        return Ok(Json(Vec::new()));
    }

    let mut entries = Vec::new();
    for item in walkdir::WalkDir::new(&bin)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !item.file_type().is_file() {
            continue;
        }
        let Ok(relative) = item.path().strip_prefix(&bin) else {
            continue;
        };
        if is_excluded(relative) {
            continue;
        }
        let trash_path = relative.to_string_lossy().to_string();
        entries.push(entry_for(&bin, item.path(), &trash_path));
    }
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.mtime_ms));
    Ok(Json(entries))
}

fn entry_for(bin: &FsPath, absolute: &FsPath, trash_path: &str) -> TrashEntry {
    let name = trash_path
        .rsplit('/')
        .next()
        .unwrap_or(trash_path)
        .to_string();
    let size = std::fs::metadata(absolute).map(|m| m.len()).unwrap_or(0);
    TrashEntry {
        trash_path: trash_path.to_string(),
        original_path: original_path(bin, trash_path),
        name,
        size,
        mtime_ms: mtime_ms(absolute),
    }
}

/// Move one trashed file back to where it was deleted from.
///
/// A file already at the destination is left alone and reported as a conflict: the user
/// may have rewritten the note since, and a restore that overwrote it would be a second
/// data loss dressed up as recovery.
pub async fn restore(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<RestoreRequest>,
) -> AppResult<StatusCode> {
    let root = writable_root(&state, &body.root_id)?;
    reject_excluded(&body.trash_path)?;
    let bin = root.path.join(TRASH_DIR);
    // `.trash/` is a root of its own here, so a request cannot name a file outside it,
    // and the destination is checked against the real root like every other write.
    let source = resolve_in_root(&bin, &body.trash_path)?;
    let original = original_path(&bin, &body.trash_path);
    reject_excluded(&original)?;
    let destination = resolve_in_root(&root.path, &original)?;
    let _one_writer = one_writer(&state, &body.root_id)?;

    if !source.is_file() {
        return Err(AppError::NotFound("document".into()));
    }
    if destination.exists() {
        return Err(AppError::AlreadyExists(original));
    }
    move_file(&source, &destination)?;

    state.reindex(&body.root_id, vec![original], origin_of(&headers));
    Ok(StatusCode::NO_CONTENT)
}

/// Where a trashed file came from: its path inside `.trash/` with the ` (n)` counter a
/// repeat delete added to the file name removed.
///
/// The counter is only removed when the un-numbered file is also in the trash, since that
/// is the only way `trash_destination` ever adds one; a note whose real name ends in
/// ` (2)` has no such sibling and goes back under the name it was written with.
///
/// Only the last component is un-numbered. A deleted *folder* that collided keeps its
/// counter, because restoring its files one at a time into the un-numbered folder would
/// silently merge two deletions of different folders.
fn original_path(bin: &FsPath, trash_path: &str) -> String {
    let (dir, name) = match trash_path.rsplit_once('/') {
        Some((dir, name)) => (Some(dir), name),
        None => (None, trash_path),
    };
    let (stem, extension) = match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => (stem, Some(extension)),
        _ => (name, None),
    };
    let mut restored = strip_counter(stem).to_string();
    if let Some(extension) = extension {
        restored.push('.');
        restored.push_str(extension);
    }
    let restored_path = match dir {
        Some(dir) => format!("{dir}/{restored}"),
        None => restored,
    };
    if restored_path != trash_path && !bin.join(&restored_path).exists() {
        return trash_path.to_string();
    }
    restored_path
}

fn strip_counter(stem: &str) -> &str {
    let Some(open) = stem.rfind(COUNTER_OPEN) else {
        return stem;
    };
    let Some(counter) = stem[open + COUNTER_OPEN.len()..].strip_suffix(COUNTER_CLOSE) else {
        return stem;
    };
    let numbered = !counter.is_empty() && counter.bytes().all(|byte| byte.is_ascii_digit());
    if numbered || counter == OVERFLOW_COUNTER {
        &stem[..open]
    } else {
        stem
    }
}

#[cfg(test)]
mod tests {
    use super::original_path;
    use std::path::Path;

    /// A trash holding the files named, so a counter has the sibling that justifies it.
    fn trash_with(files: &[&str]) -> tempfile::TempDir {
        let bin = tempfile::tempdir().unwrap();
        for file in files {
            let path = bin.path().join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "").unwrap();
        }
        bin
    }

    #[test]
    fn a_counter_added_by_a_repeat_delete_is_removed() {
        let bin = trash_with(&["notes/Target.md", "Target.md"]);
        assert_eq!(
            original_path(bin.path(), "notes/Target (1).md"),
            "notes/Target.md"
        );
        assert_eq!(original_path(bin.path(), "Target (12).md"), "Target.md");
        assert_eq!(
            original_path(bin.path(), "Target (overflow).md"),
            "Target.md"
        );
    }

    #[test]
    fn a_first_deletion_and_a_name_with_brackets_are_left_alone() {
        let bin = trash_with(&["Meeting.md"]);
        assert_eq!(
            original_path(bin.path(), "notes/Target.md"),
            "notes/Target.md"
        );
        assert_eq!(
            original_path(bin.path(), "Meeting (draft).md"),
            "Meeting (draft).md"
        );
        assert_eq!(original_path(bin.path(), "Meeting ().md"), "Meeting ().md");
        assert_eq!(original_path(bin.path(), "notes/README"), "notes/README");
    }

    #[test]
    fn a_name_that_genuinely_ends_in_a_number_keeps_it() {
        let bin = trash_with(&["Chapter (2).md"]);
        assert_eq!(
            original_path(bin.path(), "Chapter (2).md"),
            "Chapter (2).md"
        );
        assert_eq!(
            original_path(Path::new("/nonexistent"), "Chapter (2).md"),
            "Chapter (2).md"
        );
    }

    #[test]
    fn a_folder_that_collided_keeps_its_counter() {
        let bin = trash_with(&["notes/a.md", "notes (1)/a.md"]);
        assert_eq!(
            original_path(bin.path(), "notes (1)/a.md"),
            "notes (1)/a.md"
        );
    }
}
