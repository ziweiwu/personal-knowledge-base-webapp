//! Router assembly.

use crate::auth::middleware::require_session;
use crate::routes::{auth, events, files, read, write};
use crate::state::AppState;
use axum::routing::{get, post};
use axum::Router;
use std::sync::Arc;
use tower_http::compression::CompressionLayer;
use tower_http::trace::TraceLayer;

pub fn build(state: Arc<AppState>) -> Router {
    // The gate is applied to the group rather than per route so a route added later is
    // covered by construction. Everything behind it, including file bytes and search,
    // requires a session.
    let protected = protected_routes().route_layer(axum::middleware::from_fn_with_state(
        state.clone(),
        require_session,
    ));

    // The only unauthenticated route in the application.
    let public = Router::new().route("/auth/login", post(auth::login));

    // An unknown /api path must be a JSON 404, not the HTML shell: handing a client
    // `<!doctype html>` where it expected JSON turns a typo into a parse error.
    let api = public.merge(protected).fallback(api_not_found);

    Router::new()
        .nest("/api", api)
        .fallback(crate::assets::serve)
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// The gated routes. Adding one here places it behind the session gate; there is no
/// per-route opt-in to forget.
fn protected_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/auth/logout", post(auth::logout))
        .route("/auth/session", get(auth::session))
        .route("/roots", get(read::roots))
        .route("/tree", get(read::tree))
        .route("/search", get(read::search))
        .route("/tag/{root_id}/{*tag}", get(read::tagged))
        .route("/events", get(events::events))
        .route("/folder/{root_id}", get(read::folder_root))
        .route(
            "/folder/{root_id}/{*path}",
            get(read::folder).post(write::create_folder),
        )
        .route(
            "/doc/{root_id}/{*path}",
            get(read::document)
                .put(write::save)
                .post(write::create)
                .delete(write::delete)
                // axum's 2 MiB default would reject a large note before the route's own
                // limit was ever consulted, with a bare non-JSON 413.
                .layer(axum::extract::DefaultBodyLimit::max(write::MAX_WRITE_BYTES)),
        )
        .route("/raw/{root_id}/{*path}", get(read::raw))
        .route(
            "/file/{root_id}/{*path}",
            get(files::file)
                .post(write::upload)
                .layer(axum::extract::DefaultBodyLimit::max(
                    write::MAX_UPLOAD_BYTES,
                )),
        )
        .route("/docx-media/{root_id}/{*rest}", get(files::docx_media))
        .route("/rename", post(write::rename))
}

async fn api_not_found() -> crate::error::AppError {
    crate::error::AppError::NotFound("endpoint".into())
}
