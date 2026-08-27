//! Raw file bytes: images, PDFs and downloads.
//!
//! Behind the same authentication gate as everything else — an attachment is document
//! content, and serving it unauthenticated would be a hole straight through the app.

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use kbview_core::index::Document;
use kbview_core::kinds::DocumentKind;
use kbview_core::paths::resolve_in_root;
use std::sync::Arc;
use tokio_util::io::ReaderStream;

pub async fn file(
    State(state): State<Arc<AppState>>,
    Path((root_id, path)): Path<(String, String)>,
    headers: HeaderMap,
) -> AppResult<Response> {
    let index = state
        .index(&root_id)
        .ok_or(AppError::NotFound("folder".into()))?;
    let root = state
        .root(&root_id)
        .ok_or(AppError::NotFound("folder".into()))?;

    // Serve only what the index knows about. That keeps the excluded directories
    // (`.obsidian`, `.trash`, `@eaDir`) genuinely unreachable rather than merely hidden.
    let document = index.get(&path).ok_or(AppError::NotFound("file".into()))?;
    let absolute = resolve_in_root(&root.path, &path)?;

    let etag = format!("\"{}-{}\"", document.mtime_ms, document.size);
    if etag_matches(&headers, &etag) {
        return Ok(StatusCode::NOT_MODIFIED.into_response());
    }

    stream_file(&absolute, document, etag).await
}

fn etag_matches(headers: &HeaderMap, etag: &str) -> bool {
    headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .map(|value| value == etag)
        .unwrap_or(false)
}

/// Only formats the browser renders safely are shown inline. Anything else downloads, so
/// an HTML or SVG file in the folder cannot execute in the app's origin.
fn content_disposition(document: &Document) -> String {
    let disposition = match document.kind {
        DocumentKind::Image | DocumentKind::Pdf => "inline",
        _ => "attachment",
    };
    let filename = document.name.replace('"', "");
    format!("{disposition}; filename=\"{filename}\"")
}

async fn stream_file(
    absolute: &std::path::Path,
    document: &Document,
    etag: String,
) -> AppResult<Response> {
    let mime = mime_guess::from_path(absolute)
        .first_or_octet_stream()
        .to_string();

    let handle = tokio::fs::File::open(absolute).await?;
    // Length must come from the file we are about to stream, not from the index snapshot.
    // The index lags an external write by the watcher debounce, and a mismatched
    // Content-Length makes hyper abort the connection or truncate the body silently.
    let length = handle.metadata().await?.len();
    let body = Body::from_stream(ReaderStream::new(handle));

    let mut response = Response::builder()
        .header(header::CONTENT_TYPE, mime)
        .header(header::CONTENT_LENGTH, length)
        .header(header::CONTENT_DISPOSITION, content_disposition(document))
        .header(header::ETAG, etag)
        // `private` because this is one user's content behind auth: no shared cache
        // should ever hold it.
        .header(header::CACHE_CONTROL, "private, max-age=3600")
        .header(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        )
        .body(body)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    deny_scripts_in_viewed_media(&mut response, document.kind);
    Ok(response)
}

/// Belt and braces for SVG, which can carry script when rendered as a document.
fn deny_scripts_in_viewed_media(response: &mut Response, kind: DocumentKind) {
    if kind != DocumentKind::Image && kind != DocumentKind::Pdf {
        return;
    }
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("default-src 'none'; img-src 'self'; style-src 'unsafe-inline'"),
    );
}

/// Serve one image embedded inside a `.docx`.
///
/// The converter emits `{media_base}/{rel_id}`, so the final path segment is the
/// relationship id and everything before it is the document's path. Media is extracted
/// from the zip on demand rather than unpacked to disk, so there is no cache directory to
/// invalidate when the document changes.
pub async fn docx_media(
    State(state): State<Arc<AppState>>,
    Path((root_id, rest)): Path<(String, String)>,
) -> AppResult<Response> {
    let (path, rel_id) = rest
        .rsplit_once('/')
        .ok_or_else(|| AppError::BadRequest("missing relationship id".into()))?;

    let index = state
        .index(&root_id)
        .ok_or(AppError::NotFound("folder".into()))?;
    let root = state
        .root(&root_id)
        .ok_or(AppError::NotFound("folder".into()))?;

    let document = index
        .get(path)
        .ok_or(AppError::NotFound("document".into()))?;
    if document.kind != DocumentKind::Docx {
        return Err(AppError::NotFound("document".into()));
    }
    let absolute = resolve_in_root(&root.path, path)?;

    let bytes = tokio::fs::read(&absolute).await?;
    let (data, mime) = kbview_docx::extract_media(&bytes, rel_id)
        .map_err(|_| AppError::NotFound("embedded image".into()))?;

    Ok((
        [
            (header::CONTENT_TYPE, mime),
            (header::CACHE_CONTROL, "private, max-age=3600".to_string()),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
        ],
        data,
    )
        .into_response())
}
