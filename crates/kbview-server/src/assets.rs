//! The built frontend, embedded in the binary.
//!
//! Embedding is what makes deployment a single file: no static directory to copy
//! alongside the executable, and no chance of the binary and the assets drifting apart.

use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../web/dist"]
struct Frontend;

pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let candidate = if path.is_empty() { "index.html" } else { path };

    if let Some(file) = Frontend::get(candidate) {
        return respond(candidate, file);
    }

    // A miss under the build's own asset directory is a missing script or stylesheet.
    // Returning the HTML shell there would hand the browser a page where it expected
    // JavaScript, turning a 404 into a parse error.
    if is_build_asset(candidate) {
        return (StatusCode::NOT_FOUND, "Not found").into_response();
    }

    // Client-side routing: any other unknown path is a deep link into the app, so serve
    // the shell and let the router work out what to show.
    match Frontend::get("index.html") {
        Some(index) => respond("index.html", index),
        None => (
            StatusCode::NOT_FOUND,
            "Frontend not built. Run `npm run build` in web/.",
        )
            .into_response(),
    }
}

/// Vite emits every fingerprinted bundle under `assets/`.
///
/// Deliberately narrow: app routes legitimately end in a file extension — a document
/// lives at `/n/<root>/notes/a.md` — so "the last segment has a dot" would 404 every
/// direct link to a document.
fn is_build_asset(path: &str) -> bool {
    path.starts_with("assets/")
}

fn respond(path: &str, file: rust_embed::EmbeddedFile) -> Response {
    let mime = mime_guess::from_path(path).first_or_octet_stream();

    // Vite fingerprints asset filenames, so they can be cached indefinitely; the shell
    // must not be, or a deploy would never reach an open browser.
    let cache = if path == "index.html" {
        "no-cache"
    } else {
        "public, max-age=31536000, immutable"
    };

    (
        [
            (header::CONTENT_TYPE, mime.as_ref()),
            (header::CACHE_CONTROL, cache),
        ],
        file.data.into_owned(),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_assets_are_recognised() {
        assert!(is_build_asset("assets/index-a1b2.js"));
        assert!(is_build_asset("assets/index-a1b2.css"));
    }

    /// The regression this guards: document routes end in a file extension, so any
    /// "looks like a filename" rule 404s every direct link to a note.
    #[test]
    fn document_deep_links_are_app_routes_even_though_they_end_in_an_extension() {
        assert!(!is_build_asset("n/kb/notes/a.md"));
        assert!(!is_build_asset("n/kb/references/知识管理.md"));
        assert!(!is_build_asset("f/kb/some.folder/child"));
        assert!(!is_build_asset("login"));
        assert!(!is_build_asset(""));
    }
}
