//! Write routes: save, create, rename, delete, upload.
//!
//! Two properties matter more than anything else here, because both failure modes destroy
//! the user's data silently:
//!   1. A save must never overwrite a change made elsewhere (Obsidian may have the same
//!      file open), so every save carries the mtime it was based on.
//!   2. A rename must rewrite inbound links, or it quietly breaks references across the
//!      whole folder.

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::Json;
use kbview_core::links::{rewrite_wikilinks, RenameContext};
use kbview_core::model::{DocumentMeta, RenameRequest, RenameResult, SaveConflict, SaveRequest};
use kbview_core::paths::resolve_in_root;
use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

/// Files above this size are refused rather than buffered into memory.
pub const MAX_WRITE_BYTES: usize = 16 * 1024 * 1024;

/// Identifies the browser tab that made a change.
///
/// It is echoed back on the change event so the tab that caused it can ignore its own
/// echo, while every *other* connected client still refreshes. Without it a save would
/// bounce straight back as an external change and fight the editor.
const ORIGIN_HEADER: &str = "x-kbview-origin";

/// Uploads are streamed into memory, so this bounds what one request can allocate.
pub const MAX_UPLOAD_BYTES: usize = 64 * 1024 * 1024;

/// A tab identifier longer than this is a client bug or an attempt to bloat the map of
/// recent writes, so it is truncated rather than trusted.
const MAX_ORIGIN_CHARS: usize = 64;

fn origin_of(headers: &HeaderMap) -> Option<String> {
    headers
        .get(ORIGIN_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.chars().take(MAX_ORIGIN_CHARS).collect::<String>())
        .filter(|value| !value.is_empty())
}

fn writable_root<'a>(
    state: &'a AppState,
    root_id: &str,
) -> AppResult<&'a kbview_core::config::RootConfig> {
    let root = state
        .root(root_id)
        .ok_or(AppError::NotFound("folder".into()))?;
    if root.read_only {
        return Err(AppError::ReadOnly);
    }
    Ok(root)
}

/// Refuse to write where the indexer will not look.
///
/// `.obsidian/`, `.trash/`, `@eaDir` and dotfiles are excluded from the index, so writing
/// one succeeds on disk and then vanishes: `create` returns 404 for a file it just wrote,
/// and `create_folder` returns 201 for a folder that never appears. Rejecting up front
/// makes the refusal explicit instead of silently producing an invisible file.
fn reject_excluded(path: &str) -> AppResult<()> {
    if kbview_core::paths::is_excluded(FsPath::new(path)) {
        return Err(AppError::BadRequest(
            "that name is reserved: paths beginning with a dot, and .obsidian/.trash/@eaDir, are not shown".into(),
        ));
    }
    Ok(())
}

fn mtime_ms(path: &FsPath) -> i64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Write via a temporary file in the same directory, then rename.
///
/// A partial write would leave a truncated note behind, and this folder is also being
/// watched and synced; `rename` within a directory is atomic, so readers see either the
/// old file or the new one and never a half-written one.
fn write_atomic(absolute: &FsPath, contents: &str) -> AppResult<()> {
    let parent = absolute
        .parent()
        .ok_or_else(|| AppError::BadRequest("invalid path".into()))?;
    std::fs::create_dir_all(parent)?;

    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    std::io::Write::write_all(&mut temp, contents.as_bytes())?;
    temp.as_file().sync_all()?;
    temp.persist(absolute)
        .map_err(|e| AppError::Internal(e.error.to_string()))?;
    Ok(())
}

pub async fn save(
    State(state): State<Arc<AppState>>,
    Path((root_id, path)): Path<(String, String)>,
    headers: HeaderMap,
    Json(body): Json<SaveRequest>,
) -> AppResult<Json<DocumentMeta>> {
    let root = writable_root(&state, &root_id)?;
    if body.content.len() > MAX_WRITE_BYTES {
        return Err(AppError::BadRequest("document too large".into()));
    }

    reject_uneditable(&state, &root_id, &path)?;

    let absolute = resolve_in_root(&root.path, &path)?;
    let current = mtime_ms(&absolute);

    // The precondition. Without it, whoever saves last wins and the other edit is gone
    // with no trace and no warning.
    if current != body.base_mtime_ms {
        let disk_content = std::fs::read_to_string(&absolute).unwrap_or_default();
        return Err(AppError::Conflict(Box::new(SaveConflict {
            path: path.clone(),
            your_content: body.content,
            disk_content,
            disk_mtime_ms: current,
        })));
    }

    write_atomic(&absolute, &body.content)?;
    state.reindex(&root_id, vec![path.clone()], origin_of(&headers));
    indexed_meta(&state, &root_id, &path)
}

/// A save may only touch a document the index already holds, in a format editable here.
fn reject_uneditable(state: &AppState, root_id: &str, path: &str) -> AppResult<()> {
    let index = state
        .index(root_id)
        .ok_or(AppError::NotFound("folder".into()))?;
    let document = index
        .get(path)
        .ok_or(AppError::NotFound("document".into()))?;
    if !document.kind.is_editable() {
        return Err(AppError::BadRequest(
            "this file type cannot be edited here".into(),
        ));
    }
    Ok(())
}

/// The document's metadata as the index holds it after the write.
fn indexed_meta(state: &AppState, root_id: &str, path: &str) -> AppResult<Json<DocumentMeta>> {
    let index = state
        .index(root_id)
        .ok_or(AppError::NotFound("folder".into()))?;
    let document = index
        .get(path)
        .ok_or(AppError::NotFound("document".into()))?;
    Ok(Json(document.meta()))
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Path((root_id, path)): Path<(String, String)>,
    headers: HeaderMap,
    Json(body): Json<SaveRequest>,
) -> AppResult<(StatusCode, Json<DocumentMeta>)> {
    let root = writable_root(&state, &root_id)?;
    reject_excluded(&path)?;
    let absolute = resolve_in_root(&root.path, &path)?;

    if absolute.exists() {
        return Err(AppError::AlreadyExists(path));
    }
    write_atomic(&absolute, &body.content)?;
    state.reindex(&root_id, vec![path.clone()], origin_of(&headers));
    Ok((StatusCode::CREATED, indexed_meta(&state, &root_id, &path)?))
}

pub async fn create_folder(
    State(state): State<Arc<AppState>>,
    Path((root_id, path)): Path<(String, String)>,
    headers: HeaderMap,
) -> AppResult<StatusCode> {
    let root = writable_root(&state, &root_id)?;
    reject_excluded(&path)?;
    let absolute = resolve_in_root(&root.path, &path)?;

    if absolute.exists() {
        return Err(AppError::AlreadyExists(path));
    }
    std::fs::create_dir_all(&absolute)?;
    state.reindex(&root_id, vec![path], origin_of(&headers));
    Ok(StatusCode::CREATED)
}

/// Move a document to the folder's `.trash/`, matching what Obsidian does by default.
///
/// Deleting over a network, from a phone, with no undo is a bad combination; a recoverable
/// delete costs nothing and `.trash/` is already excluded from the index.
pub async fn delete(
    State(state): State<Arc<AppState>>,
    Path((root_id, path)): Path<(String, String)>,
    headers: HeaderMap,
) -> AppResult<StatusCode> {
    let root = writable_root(&state, &root_id)?;
    let absolute = resolve_in_root(&root.path, &path)?;
    if !absolute.exists() {
        return Err(AppError::NotFound("document".into()));
    }

    let trashed = trash_destination(&root.path, &path);
    if let Some(parent) = trashed.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::rename(&absolute, &trashed)?;

    state.reindex(&root_id, vec![path], origin_of(&headers));
    Ok(StatusCode::NO_CONTENT)
}

/// Preserve the original layout inside `.trash/`, disambiguating a repeat delete of the
/// same path rather than overwriting what is already in the bin.
fn trash_destination(root: &FsPath, path: &str) -> PathBuf {
    let base = root.join(".trash").join(path);
    if !base.exists() {
        return base;
    }
    let stem = FsPath::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".into());
    let extension = FsPath::new(path)
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();

    let parent = base
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join(".trash"));
    for attempt in 1..1000 {
        let candidate = parent.join(format!("{stem} ({attempt}){extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    parent.join(format!("{stem} (overflow){extension}"))
}

/// Upload a file's raw bytes.
///
/// Separate from the JSON create route so neither has to guess at its body shape from a
/// content type. Binary content never goes through the markdown path.
pub async fn upload(
    State(state): State<Arc<AppState>>,
    Path((root_id, path)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<StatusCode> {
    let root = writable_root(&state, &root_id)?;
    reject_excluded(&path)?;
    if body.len() > MAX_UPLOAD_BYTES {
        return Err(AppError::BadRequest("file too large".into()));
    }

    let absolute = resolve_in_root(&root.path, &path)?;
    if absolute.exists() {
        return Err(AppError::AlreadyExists(path));
    }
    if let Some(parent) = absolute.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut temp = tempfile::NamedTempFile::new_in(
        absolute
            .parent()
            .ok_or_else(|| AppError::BadRequest("invalid path".into()))?,
    )?;
    std::io::Write::write_all(&mut temp, &body)?;
    temp.as_file().sync_all()?;
    temp.persist(&absolute)
        .map_err(|e| AppError::Internal(e.error.to_string()))?;

    state.reindex(&root_id, vec![path], origin_of(&headers));
    Ok(StatusCode::CREATED)
}

pub async fn rename(
    State(state): State<Arc<AppState>>,
    Query(query): Query<crate::routes::read::RootQuery>,
    headers: HeaderMap,
    Json(body): Json<RenameRequest>,
) -> AppResult<Json<RenameResult>> {
    let root_id = query.root;
    let root = writable_root(&state, &root_id)?;
    let from_absolute = resolve_in_root(&root.path, &body.from)?;
    let to_absolute = resolve_in_root(&root.path, &body.to)?;

    if !from_absolute.exists() {
        return Err(AppError::NotFound("document".into()));
    }
    reject_occupied_destination(&from_absolute, &to_absolute, &body.to)?;

    // Work out the rewrites first, but do not apply them until the move has actually
    // happened. Rewriting first would point every backlink at a path that does not exist
    // yet, and a failed move would then leave the whole folder referring to a missing
    // file while the original sat untouched — broken, and invisibly so.
    let pending = if body.update_links {
        planned_link_rewrites(&state, &root_id, root, &body)?
    } else {
        Vec::new()
    };

    move_file(&from_absolute, &to_absolute)?;
    let updated = apply_link_rewrites(root, pending)?;

    let mut touched = vec![body.from.clone(), body.to.clone()];
    touched.extend(updated.iter().cloned());
    state.reindex(&root_id, touched, origin_of(&headers));

    Ok(Json(RenameResult {
        from: body.from,
        to: body.to,
        updated,
    }))
}

/// On a case-insensitive filesystem (the macOS default) `note.md` and `Note.md` are the
/// same file, so a plain existence check would refuse the very rename that fixes a
/// capitalisation mistake.
fn reject_occupied_destination(
    from_absolute: &FsPath,
    to_absolute: &FsPath,
    destination: &str,
) -> AppResult<()> {
    if !to_absolute.exists() {
        return Ok(());
    }
    let renaming_in_place = from_absolute
        .canonicalize()
        .ok()
        .zip(to_absolute.canonicalize().ok())
        .map(|(from, to)| from == to)
        .unwrap_or(false);
    if renaming_in_place {
        return Ok(());
    }
    Err(AppError::AlreadyExists(destination.to_string()))
}

fn move_file(source: &FsPath, destination: &FsPath) -> AppResult<()> {
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::rename(source, destination)?;
    Ok(())
}

/// Apply the rewrites planned before the move, reporting which documents were updated.
fn apply_link_rewrites(
    root: &kbview_core::config::RootConfig,
    pending: Vec<(String, String)>,
) -> AppResult<Vec<String>> {
    let mut updated = Vec::new();
    for (source_path, rewritten) in pending {
        let absolute = resolve_in_root(&root.path, &source_path)?;
        // The move already succeeded; a link that fails to rewrite is a visible broken
        // link, which is recoverable, so report it rather than failing the whole rename.
        match write_atomic(&absolute, &rewritten) {
            Ok(()) => updated.push(source_path),
            Err(error) => tracing::warn!(%source_path, ?error, "could not rewrite inbound link"),
        }
    }
    Ok(updated)
}

/// Compute, without writing, the new contents of every document linking to the old path.
fn planned_link_rewrites(
    state: &AppState,
    root_id: &str,
    root: &kbview_core::config::RootConfig,
    request: &RenameRequest,
) -> AppResult<Vec<(String, String)>> {
    if !root.uses_wikilinks() {
        return Ok(Vec::new());
    }
    let index = state
        .index(root_id)
        .ok_or(AppError::NotFound("folder".into()))?;

    let mut planned = Vec::new();
    for source_path in index
        .backlinks(&request.from)
        .into_iter()
        .map(|link| link.path)
    {
        let Some(document) = index.get(&source_path) else {
            continue;
        };
        let Some(content) = &document.content else {
            continue;
        };
        let Some(rewritten) = rewrite_wikilinks(
            content,
            RenameContext {
                from_path: &source_path,
                old_path: &request.from,
                new_path: &request.to,
                resolver: &index.resolver,
            },
        ) else {
            continue;
        };
        planned.push((source_path, rewritten));
    }
    Ok(planned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trash_keeps_the_original_folder_layout() {
        let root = std::env::temp_dir().join("kbview-trash-layout");
        let _ = std::fs::remove_dir_all(&root);
        let destination = trash_destination(&root, "notes/deep/a.md");
        assert!(
            destination.ends_with(".trash/notes/deep/a.md"),
            "got {destination:?}"
        );
    }

    #[test]
    fn deleting_the_same_path_twice_does_not_overwrite_the_first() {
        let root = std::env::temp_dir().join("kbview-trash-collide");
        let _ = std::fs::remove_dir_all(&root);
        let first = trash_destination(&root, "a.md");
        std::fs::create_dir_all(first.parent().unwrap()).unwrap();
        std::fs::write(&first, "first").unwrap();

        let second = trash_destination(&root, "a.md");
        assert_ne!(
            first, second,
            "the second delete must not clobber the first"
        );
        assert!(second.to_string_lossy().contains("(1)"));
    }

    #[test]
    fn an_atomic_write_replaces_content_completely() {
        let dir = std::env::temp_dir().join("kbview-atomic");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.md");

        write_atomic(&file, "first version, quite long").unwrap();
        write_atomic(&file, "short").unwrap();
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "short",
            "no leftover tail from the longer write"
        );
    }

    #[test]
    fn an_atomic_write_creates_missing_directories() {
        let dir = std::env::temp_dir().join("kbview-atomic-mkdir");
        let _ = std::fs::remove_dir_all(&dir);
        let file = dir.join("deep/nested/a.md");
        write_atomic(&file, "content").unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "content");
    }
}
