//! Raw file bytes: images, PDFs and downloads.
//!
//! Behind the same authentication gate as everything else — an attachment is document
//! content, and serving it unauthenticated would be a hole straight through the app.

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use kbviewer_core::index::Document;
use kbviewer_core::kinds::DocumentKind;
use kbviewer_core::paths::resolve_in_root;
use serde::Deserialize;
use std::sync::Arc;
use tokio_util::io::ReaderStream;

/// `?w=` asks for a resized copy. Absent, the original is served exactly as before.
#[derive(Debug, Deserialize)]
pub struct FileQuery {
    w: Option<u32>,
}

pub async fn file(
    State(state): State<Arc<AppState>>,
    Path((root_id, path)): Path<(String, String)>,
    Query(query): Query<FileQuery>,
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

    if let Some(width) = requested_width(&query, document) {
        let request = VariantRequest {
            state: &state,
            root_id: &root_id,
            path: &path,
            absolute: &absolute,
            document,
            width,
        };
        return resized(request, &headers).await;
    }

    let etag = format!("\"{}-{}\"", document.mtime_ms, document.size);
    if etag_matches(&headers, &etag) {
        return Ok(StatusCode::NOT_MODIFIED.into_response());
    }

    stream_file(&absolute, document, etag).await
}

/// The width to resize to, or `None` to serve the original untouched.
///
/// Refuses anything but an offered width, and refuses to scale up: a variant wider than
/// the source is strictly more bytes for no more detail.
fn requested_width(query: &FileQuery, document: &Document) -> Option<u32> {
    let width = query.w?;
    if document.kind != DocumentKind::Image || !crate::render::variants::is_offered(width) {
        return None;
    }
    let natural = document.dimensions?.width;
    (natural > width).then_some(width)
}

/// Resize once, and decide whether the result is worth serving.
///
/// A smooth image can compress better as the PNG it already is, so a variant is not
/// guaranteed to be a saving; when it is not, the answer is to serve the original. The
/// verdict is returned rather than recomputed later, because reaching it costs a full
/// decode.
async fn produce_variant(
    absolute: &std::path::Path,
    document: &Document,
    width: u32,
) -> AppResult<crate::render::variants::Variant> {
    use crate::render::variants::Variant;

    let bytes = tokio::fs::read(absolute).await?;
    // Decoding and resampling a several-megapixel image is CPU-bound work and would stall
    // every other request sharing this runtime thread.
    let produced =
        tokio::task::spawn_blocking(move || kbviewer_core::images::variant(&bytes, width))
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(match produced {
        Ok((bytes, _)) if bytes.len() as u64 >= document.size => Variant::NotWorthIt,
        Ok((bytes, format)) => Variant::Resized { bytes, format },
        Err(_) => Variant::NotWorthIt,
    })
}

/// Everything one variant request needs, so the handler is not a list of loose arguments
/// that can be passed in the wrong order.
struct VariantRequest<'a> {
    state: &'a Arc<AppState>,
    root_id: &'a str,
    path: &'a str,
    absolute: &'a std::path::Path,
    document: &'a Document,
    width: u32,
}

async fn resized(request: VariantRequest<'_>, headers: &HeaderMap) -> AppResult<Response> {
    let VariantRequest {
        state,
        root_id,
        path,
        absolute,
        document,
        width,
    } = request;

    let key = crate::render::variants::VariantKey::new(root_id, path, document.mtime_ms, width);
    let etag = key.etag();
    if etag_matches(headers, &etag) {
        return Ok(StatusCode::NOT_MODIFIED.into_response());
    }

    let variant = match state.variants.get(&key) {
        Some(hit) => hit,
        None => {
            let fresh = produce_variant(absolute, document, width).await?;
            state.variants.put(key, fresh.clone());
            fresh
        }
    };

    let crate::render::variants::Variant::Resized { bytes, format } = variant else {
        let etag = format!("\"{}-{}\"", document.mtime_ms, document.size);
        return stream_file(absolute, document, etag).await;
    };
    Ok(variant_response(bytes, format, etag))
}

fn variant_response(
    bytes: Vec<u8>,
    format: kbviewer_core::images::VariantFormat,
    etag: String,
) -> Response {
    (
        [
            (header::CONTENT_TYPE, format.mime().to_string()),
            (header::CONTENT_LENGTH, bytes.len().to_string()),
            (header::ETAG, etag),
            // Derived from an immutable (path, mtime, width): a changed file gets a
            // different key, so this can be cached hard.
            (
                header::CACHE_CONTROL,
                "private, max-age=604800, immutable".to_string(),
            ),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
        ],
        bytes,
    )
        .into_response()
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
    let (data, mime) = kbviewer_docx::extract_media(&bytes, rel_id)
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
