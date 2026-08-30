//! Read-only routes: roots, tree, folders, documents, source text and search.

use crate::error::{AppError, AppResult};
use crate::render::document::build_payload;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::Json;
use kbviewer_core::index::Index;
use kbviewer_core::model::{DocumentPayload, FolderListing, RootInfo, SearchHit, TreeNode};
use serde::Deserialize;
use std::sync::Arc;

const DEFAULT_SEARCH_LIMIT: usize = 40;
const MAX_SEARCH_LIMIT: usize = 200;

pub async fn roots(State(state): State<Arc<AppState>>) -> Json<Vec<RootInfo>> {
    Json(
        state
            .config
            .roots
            .iter()
            .map(|root| RootInfo {
                id: root.id.clone(),
                name: root.name.clone(),
                obsidian_mode: root.uses_wikilinks(),
                read_only: root.read_only,
            })
            .collect(),
    )
}

#[derive(Debug, Deserialize)]
pub struct RootQuery {
    pub root: String,
}

pub async fn tree(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RootQuery>,
) -> AppResult<Json<Vec<TreeNode>>> {
    let index = state
        .index(&query.root)
        .ok_or(AppError::NotFound("folder".into()))?;
    Ok(Json(index.tree()))
}

pub async fn document(
    State(state): State<Arc<AppState>>,
    Path((root_id, path)): Path<(String, String)>,
) -> AppResult<Json<DocumentPayload>> {
    let index = state
        .index(&root_id)
        .ok_or(AppError::NotFound("folder".into()))?;
    let document = index
        .get(&path)
        .ok_or(AppError::NotFound("document".into()))?;
    Ok(Json(build_payload(&state, &root_id, &index, document)))
}

/// The source text behind a document, for the editor.
pub async fn raw(
    State(state): State<Arc<AppState>>,
    Path((root_id, path)): Path<(String, String)>,
) -> AppResult<String> {
    let index = state
        .index(&root_id)
        .ok_or(AppError::NotFound("folder".into()))?;
    let document = index
        .get(&path)
        .ok_or(AppError::NotFound("document".into()))?;

    if !document.kind.is_editable() {
        return Err(AppError::BadRequest(format!(
            "{} files have no editable source",
            document.kind.as_str()
        )));
    }
    document
        .content
        .clone()
        .ok_or(AppError::NotFound("document source".into()))
}

pub async fn folder(
    State(state): State<Arc<AppState>>,
    Path((root_id, path)): Path<(String, String)>,
) -> AppResult<Json<FolderListing>> {
    folder_listing(state, root_id, path).await
}

/// The root folder, which has no trailing path segment.
pub async fn folder_root(
    State(state): State<Arc<AppState>>,
    Path(root_id): Path<String>,
) -> AppResult<Json<FolderListing>> {
    folder_listing(state, root_id, String::new()).await
}

async fn folder_listing(
    state: Arc<AppState>,
    root_id: String,
    path: String,
) -> AppResult<Json<FolderListing>> {
    let index = state
        .index(&root_id)
        .ok_or(AppError::NotFound("folder".into()))?;
    let root = state
        .root(&root_id)
        .ok_or(AppError::NotFound("folder".into()))?;

    let path = path.trim_end_matches('/').to_string();
    if !index.has_directory(&path) {
        return Err(AppError::NotFound("folder".into()));
    }

    let index_path = index.index_document(&path, root);
    let index_payload = index_path
        .as_ref()
        .and_then(|p| index.get(p))
        .map(|document| build_payload(&state, &root_id, &index, document));

    // The landing page is rendered above the listing, so listing it again would show the
    // same document twice.
    let entries = index
        .folder_entries(&path)
        .into_iter()
        .filter(|entry| Some(&entry.path) != index_path.as_ref())
        .collect();

    Ok(Json(FolderListing {
        name: folder_name(&path, &index),
        path,
        index: index_payload,
        entries,
    }))
}

fn folder_name(path: &str, index: &Index) -> String {
    if path.is_empty() {
        return index.root_name.clone();
    }
    path.rsplit('/').next().unwrap_or(path).to_string()
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub root: String,
    pub q: String,
    pub limit: Option<usize>,
}

pub async fn search(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
) -> AppResult<Json<Vec<SearchHit>>> {
    let index = state
        .index(&query.root)
        .ok_or(AppError::NotFound("folder".into()))?;
    let limit = query
        .limit
        .unwrap_or(DEFAULT_SEARCH_LIMIT)
        .clamp(1, MAX_SEARCH_LIMIT);
    Ok(Json(kbviewer_core::search::search(&index, &query.q, limit)))
}

/// Documents carrying a given tag.
pub async fn tagged(
    State(state): State<Arc<AppState>>,
    Path((root_id, tag)): Path<(String, String)>,
) -> AppResult<Json<Vec<kbviewer_core::model::DocumentMeta>>> {
    let index = state
        .index(&root_id)
        .ok_or(AppError::NotFound("folder".into()))?;

    let wanted = tag.trim_start_matches('#').to_lowercase();
    let mut matches: Vec<kbviewer_core::model::DocumentMeta> = index
        .documents
        .values()
        .filter(|document| {
            document
                .tags
                .iter()
                // Nested tags: `#area/work` is matched by `#area`, as Obsidian does.
                .any(|t| {
                    let t = t.to_lowercase();
                    t == wanted || t.starts_with(&format!("{wanted}/"))
                })
        })
        .map(|document| document.meta())
        .collect();
    matches.sort_by_key(|a| a.title.to_lowercase());
    Ok(Json(matches))
}
