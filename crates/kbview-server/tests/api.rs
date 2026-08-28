//! End-to-end tests against the real router.
//!
//! These drive the assembled `axum` app rather than a process, so they cover the layers
//! that unit tests cannot: that the authentication gate is actually mounted in front of
//! every route, and that a save preserves someone else's concurrent edit.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use kbview_core::config::Config;
use kbview_server::auth::store::AuthStore;
use kbview_server::state::AppState;
use std::path::PathBuf;
use tower::ServiceExt;

const PASSWORD: &str = "integration-test-password";

/// The login route rate-limits per client address, so it needs the connect info that a
/// real socket would supply. Driving the router directly means providing it here.
fn login_request(body: serde_json::Value) -> Request<Body> {
    Request::post("/api/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .extension(axum::extract::ConnectInfo(
            "127.0.0.1:40000".parse::<std::net::SocketAddr>().unwrap(),
        ))
        .body(Body::from(body.to_string()))
        .unwrap()
}

struct Harness {
    app: axum::Router,
    root: PathBuf,
}

fn harness(label: &str) -> Harness {
    let base = std::env::temp_dir().join(format!("kbview-it-{label}"));
    let _ = std::fs::remove_dir_all(&base);
    let root = base.join("vault");
    std::fs::create_dir_all(root.join(".obsidian")).unwrap();
    std::fs::create_dir_all(root.join("notes")).unwrap();

    std::fs::write(root.join(".obsidian/app.json"), "{}").unwrap();
    std::fs::write(root.join("index.md"), "# Index\nLink to [[Target]].\n").unwrap();
    std::fs::write(root.join("notes/Target.md"), "# Target\nBody.\n").unwrap();
    std::fs::write(root.join("notes/secret.png"), b"not-really-a-png").unwrap();
    // Genuinely decodable, so the resize path is exercised rather than skipped: one wide
    // enough to have variants offered for it, one too small for any.
    std::fs::write(root.join("notes/wide.png"), opaque_png(1000, 600)).unwrap();
    std::fs::write(root.join("notes/small.png"), opaque_png(120, 90)).unwrap();
    // Compresses well as PNG, so a JPEG variant of it can come out larger. The route must
    // notice and serve the original instead.
    std::fs::write(root.join("notes/gradient.png"), gradient_png(1000, 600)).unwrap();

    let data_dir = base.join("data");
    let config_path = base.join("kbview.config.json");
    std::fs::write(
        &config_path,
        serde_json::json!({
            "host": "127.0.0.1",
            "port": 0,
            "dataDir": data_dir,
            "roots": [{ "id": "kb", "name": "KB", "path": root }],
        })
        .to_string(),
    )
    .unwrap();

    let config = Config::load(&config_path).unwrap();
    let store = AuthStore::open(&config.data_dir).unwrap();
    store.add_user("tester@example.com", PASSWORD).unwrap();

    let canonical_root = config.roots[0].path.clone();
    let state = AppState::new(config, store);
    Harness {
        app: kbview_server::router::build(state),
        root: canonical_root,
    }
}

/// A valid, fully opaque PNG whose pixels do not compress — the shape of a screenshot or
/// a photo, which is what a knowledge base actually holds.
///
/// Opaque matters: an image that merely *has* an alpha channel must still re-encode as
/// JPEG, or the whole size saving is forfeited.
fn opaque_png(width: u32, height: u32) -> Vec<u8> {
    use std::io::Cursor;
    let mut buffer = image::RgbaImage::new(width, height);
    let mut noise: u32 = 0x1234_5678;
    for (_, _, pixel) in buffer.enumerate_pixels_mut() {
        noise = noise.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let [r, g, b, _] = noise.to_le_bytes();
        *pixel = image::Rgba([r, g, b, 255]);
    }
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(buffer)
        .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
        .unwrap();
    out
}

/// A smooth gradient: the case PNG is genuinely good at, where re-encoding can come out
/// *larger* than the original. Nothing guarantees a resize is a saving, which is why the
/// route compares before it commits.
fn gradient_png(width: u32, height: u32) -> Vec<u8> {
    use std::io::Cursor;
    let mut buffer = image::RgbaImage::new(width, height);
    for (x, y, pixel) in buffer.enumerate_pixels_mut() {
        *pixel = image::Rgba([(x % 256) as u8, (y % 256) as u8, 128, 255]);
    }
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(buffer)
        .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
        .unwrap();
    out
}

impl Harness {
    async fn send(&self, request: Request<Body>) -> (StatusCode, String) {
        let response = self.app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (status, String::from_utf8_lossy(&bytes).to_string())
    }

    /// Bytes and headers rather than a lossy string: an image response is not text.
    async fn send_raw(&self, request: Request<Body>) -> (StatusCode, header::HeaderMap, Vec<u8>) {
        let response = self.app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (status, headers, bytes.to_vec())
    }

    async fn login(&self) -> String {
        let response = self
            .app
            .clone()
            .oneshot(login_request(
                serde_json::json!({ "email": "tester@example.com", "password": PASSWORD }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "login should succeed");
        response
            .headers()
            .get(header::SET_COOKIE)
            .expect("login must set a session cookie")
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string()
    }

    async fn get_authed(&self, cookie: &str, uri: &str) -> (StatusCode, String) {
        self.send(
            Request::get(uri)
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
    }
}

/// Create a document through the API so the in-memory index and the file on disk agree.
/// Writing straight to disk leaves the index stale, and every write route checks the
/// mtime it handed out.
async fn create_document(harness: &Harness, cookie: &str, path: &str, content: &str) -> i64 {
    let (status, _) = harness
        .send(
            Request::post(format!("/api/doc/kb/{path}"))
                .header(header::COOKIE, cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({ "content": content, "baseMtimeMs": 0 }).to_string(),
                ))
                .unwrap(),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "setup: creating {path}");

    let (_, body) = harness
        .get_authed(cookie, &format!("/api/doc/kb/{path}"))
        .await;
    let payload: serde_json::Value = serde_json::from_str(&body).unwrap();
    payload["meta"]["mtimeMs"].as_i64().unwrap()
}

/// Every route under `/api` except login must refuse an anonymous caller. This is the
/// test that would catch a new route being mounted outside the gate.
#[tokio::test]
async fn every_api_route_requires_a_session() {
    let harness = harness("gate");
    let routes = [
        "/api/roots",
        "/api/tree?root=kb",
        "/api/search?root=kb&q=a",
        "/api/doc/kb/index.md",
        "/api/raw/kb/index.md",
        "/api/folder/kb",
        "/api/folder/kb/notes",
        "/api/file/kb/notes/secret.png",
        "/api/auth/session",
        "/api/events",
    ];
    for uri in routes {
        let (status, _) = harness
            .send(Request::get(uri).body(Body::empty()).unwrap())
            .await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "{uri} was reachable without logging in"
        );
    }
}

#[tokio::test]
async fn file_bytes_are_not_public() {
    let harness = harness("filegate");
    let (status, body) = harness
        .send(
            Request::get("/api/file/kb/notes/secret.png")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(
        !body.contains("not-really-a-png"),
        "attachment bytes leaked to an anonymous caller"
    );
}

#[tokio::test]
async fn a_wrong_password_and_an_unknown_email_are_indistinguishable() {
    let harness = harness("enumerate");
    let mut responses = Vec::new();
    for (email, password) in [
        ("tester@example.com", "not-the-password"),
        ("nobody@example.com", "not-the-password"),
    ] {
        let (status, body) = harness
            .send(login_request(
                serde_json::json!({ "email": email, "password": password }),
            ))
            .await;
        responses.push((status, body));
    }
    assert_eq!(responses[0].0, StatusCode::UNAUTHORIZED);
    assert_eq!(
        responses[0], responses[1],
        "the response must not reveal whether an account exists"
    );
}

#[tokio::test]
async fn a_session_unlocks_the_api() {
    let harness = harness("session");
    let cookie = harness.login().await;
    let (status, body) = harness.get_authed(&cookie, "/api/roots").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("\"obsidianMode\":true"), "got {body}");
}

#[tokio::test]
async fn logging_out_invalidates_the_session() {
    let harness = harness("logout");
    let cookie = harness.login().await;
    let (status, _) = harness
        .send(
            Request::post("/api/auth/logout")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, _) = harness.get_authed(&cookie, "/api/roots").await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "the cookie must be dead after logout"
    );
}

#[tokio::test]
async fn traversal_attempts_are_refused() {
    let harness = harness("traversal");
    let cookie = harness.login().await;
    for uri in [
        "/api/file/kb/../../../etc/passwd",
        "/api/raw/kb/../../etc/passwd",
        "/api/doc/kb/..%2f..%2fetc%2fpasswd",
        "/api/file/kb/.obsidian/app.json",
    ] {
        let (status, body) = harness.get_authed(&cookie, uri).await;
        assert!(
            status == StatusCode::NOT_FOUND || status == StatusCode::BAD_REQUEST,
            "{uri} returned {status}"
        );
        assert!(!body.contains("root:"), "{uri} leaked file content");
    }
}

/// The property that protects against silent data loss: a save carrying a stale mtime is
/// refused, and the file on disk is untouched.
#[tokio::test]
async fn a_save_with_a_stale_base_mtime_is_refused_and_changes_nothing() {
    let harness = harness("conflict");
    let cookie = harness.login().await;
    let original = std::fs::read_to_string(harness.root.join("notes/Target.md")).unwrap();

    let (status, body) = harness
        .send(
            Request::put("/api/doc/kb/notes/Target.md")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({ "content": "clobbered", "baseMtimeMs": 1 }).to_string(),
                ))
                .unwrap(),
        )
        .await;

    assert_eq!(status, StatusCode::CONFLICT);
    assert!(
        body.contains("\"yourContent\""),
        "the conflict must return both versions: {body}"
    );
    assert!(body.contains("\"diskContent\""));
    assert_eq!(
        std::fs::read_to_string(harness.root.join("notes/Target.md")).unwrap(),
        original,
        "a refused save must not have written anything"
    );
}

#[tokio::test]
async fn a_save_with_the_current_mtime_succeeds() {
    let harness = harness("save");
    let cookie = harness.login().await;

    let (_, body) = harness
        .get_authed(&cookie, "/api/doc/kb/notes/Target.md")
        .await;
    let payload: serde_json::Value = serde_json::from_str(&body).unwrap();
    let mtime = payload["meta"]["mtimeMs"].as_i64().unwrap();

    let (status, _) = harness
        .send(
            Request::put("/api/doc/kb/notes/Target.md")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({ "content": "# Target\nEdited.\n", "baseMtimeMs": mtime })
                        .to_string(),
                ))
                .unwrap(),
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        std::fs::read_to_string(harness.root.join("notes/Target.md"))
            .unwrap()
            .contains("Edited.")
    );
}

#[tokio::test]
async fn a_cross_origin_write_is_refused() {
    let harness = harness("csrf");
    let cookie = harness.login().await;
    let (status, _) = harness
        .send(
            Request::put("/api/doc/kb/notes/Target.md")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::ORIGIN, "https://evil.example.com")
                .header(header::HOST, "kb.example.ts.net")
                .body(Body::from(
                    serde_json::json!({ "content": "x", "baseMtimeMs": 0 }).to_string(),
                ))
                .unwrap(),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

/// Renaming must carry inbound links with it, or the folder is left full of dead
/// references that the user has to find by hand.
#[tokio::test]
async fn renaming_rewrites_inbound_links() {
    let harness = harness("rename");
    let cookie = harness.login().await;

    let (status, body) = harness
        .send(
            Request::post("/api/rename?root=kb")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "from": "notes/Target.md",
                        "to": "notes/Renamed.md",
                        "updateLinks": true
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body.contains("index.md"),
        "the linking document should be reported as updated: {body}"
    );

    let index = std::fs::read_to_string(harness.root.join("index.md")).unwrap();
    assert!(
        index.contains("[[Renamed]]"),
        "inbound link not rewritten: {index}"
    );
    assert!(!index.contains("[[Target]]"));
    assert!(harness.root.join("notes/Renamed.md").exists());
}

#[tokio::test]
async fn deleting_moves_to_trash_rather_than_destroying() {
    let harness = harness("trash");
    let cookie = harness.login().await;

    let (status, _) = harness
        .send(
            Request::delete("/api/doc/kb/notes/Target.md")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await;

    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(!harness.root.join("notes/Target.md").exists());
    assert!(
        harness.root.join(".trash/notes/Target.md").exists(),
        "a delete over the network must be recoverable"
    );
}

#[tokio::test]
async fn a_folder_without_an_index_still_lists_its_contents() {
    let harness = harness("listing");
    let cookie = harness.login().await;
    let (status, body) = harness.get_authed(&cookie, "/api/folder/kb/notes").await;
    assert_eq!(status, StatusCode::OK);

    let listing: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(listing["index"].is_null(), "notes/ has no index file");
    assert!(
        !listing["entries"].as_array().unwrap().is_empty(),
        "a folder with no index must still be browsable"
    );
}

#[tokio::test]
async fn the_root_index_document_becomes_the_folder_landing_page() {
    let harness = harness("rootindex");
    let cookie = harness.login().await;
    let (status, body) = harness.get_authed(&cookie, "/api/folder/kb").await;
    assert_eq!(status, StatusCode::OK);

    let listing: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(listing["index"]["meta"]["path"], "index.md");
    let entries = listing["entries"].as_array().unwrap();
    assert!(
        !entries.iter().any(|e| e["path"] == "index.md"),
        "the index must not be listed underneath itself"
    );
}

#[tokio::test]
async fn binary_documents_have_no_editable_source() {
    let harness = harness("rawbinary");
    let cookie = harness.login().await;
    let (status, _) = harness
        .get_authed(&cookie, "/api/raw/kb/notes/secret.png")
        .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "an image has no editable text"
    );
}

#[tokio::test]
async fn an_unknown_api_path_returns_json_not_the_html_shell() {
    let harness = harness("apifallback");
    let cookie = harness.login().await;
    let (status, body) = harness.get_authed(&cookie, "/api/does-not-exist").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(
        !body.contains("<!doctype html"),
        "a mistyped API path must not return the app shell: {body}"
    );
    assert!(
        body.contains("\"error\""),
        "expected a JSON error body: {body}"
    );
}

/// A folder rename moves every document beneath it, and each is a separate link target.
/// Planning against the folder's own path finds nothing — the backlink map is keyed by
/// documents — so this used to leave every path-qualified link dangling while bare-name
/// links only appeared to survive, because the resolver re-resolves those at render time.
#[tokio::test]
async fn renaming_a_folder_rewrites_path_qualified_inbound_links() {
    let harness = harness("renamefolder");
    let cookie = harness.login().await;

    std::fs::write(
        harness.root.join("index.md"),
        "# Index\nBare [[Target]] and qualified [[notes/Target|aliased]].\n",
    )
    .unwrap();

    let (status, body) = harness
        .send(
            Request::post("/api/rename?root=kb")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({ "from": "notes", "to": "renamed-notes", "updateLinks": true })
                        .to_string(),
                ))
                .unwrap(),
        )
        .await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(harness.root.join("renamed-notes/Target.md").exists());

    let index = std::fs::read_to_string(harness.root.join("index.md")).unwrap();
    assert!(
        index.contains("[[renamed-notes/Target|aliased]]"),
        "path-qualified link not rewritten after a folder rename: {index}"
    );
    assert!(
        !index.contains("[[notes/Target|aliased]]"),
        "stale folder-prefixed link left dangling: {index}"
    );
}

/// A document inside the renamed folder has itself moved by the time the rewrites are
/// applied, so writing it back at its old path would either fail or resurrect the folder.
#[tokio::test]
async fn a_document_inside_a_renamed_folder_is_rewritten_at_its_new_path() {
    let harness = harness("renameinside");
    let cookie = harness.login().await;

    // Created through the API so the index knows about it — a file written straight to
    // disk would not be in the link graph until the watcher noticed it.
    let (status, _) = harness
        .send(
            Request::post("/api/doc/kb/notes/sibling.md")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "content": "# Sibling\nPoints at [[notes/Target|the target]].\n",
                        "baseMtimeMs": 0
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, body) = harness
        .send(
            Request::post("/api/rename?root=kb")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({ "from": "notes", "to": "moved", "updateLinks": true })
                        .to_string(),
                ))
                .unwrap(),
        )
        .await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        !harness.root.join("notes").exists(),
        "the old folder must be gone"
    );

    let sibling = std::fs::read_to_string(harness.root.join("moved/sibling.md")).unwrap();
    assert!(
        sibling.contains("[[moved/Target|the target]]"),
        "a link between two documents that moved together was not retargeted: {sibling}"
    );
}

/// The other write routes refuse a dotted destination; rename did not. It hid the file
/// from the index and rewrote every inbound link to point at a path no longer served,
/// reporting both as success.
#[tokio::test]
async fn renaming_into_an_excluded_path_is_refused() {
    let harness = harness("renameexcluded");
    let cookie = harness.login().await;

    for destination in ["notes/.hidden.md", ".trash/gone.md", ".obsidian/sneaky.md"] {
        let (status, _) = harness
            .send(
                Request::post("/api/rename?root=kb")
                    .header(header::COOKIE, &cookie)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::json!({ "from": "notes/Target.md", "to": destination })
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "{destination} should be refused"
        );
    }
    assert!(
        harness.root.join("notes/Target.md").exists(),
        "a refused rename must leave the file where it was"
    );
}

/// A bare wikilink must not be rewritten into a name that resolves to a different
/// document. This is the failure the module is explicitly designed to avoid: not a broken
/// link the reader can see, but a plausible link to the wrong note.
#[tokio::test]
async fn a_rename_never_repoints_a_link_at_a_different_document() {
    let harness = harness("renameambiguous");
    let cookie = harness.login().await;

    std::fs::create_dir_all(harness.root.join("other")).unwrap();
    std::fs::write(harness.root.join("other/Renamed.md"), "# Decoy\n").unwrap();
    std::fs::write(
        harness.root.join("other/linker.md"),
        "# Linker\nPoints at [[Target]].\n",
    )
    .unwrap();

    // Reindex through the API so the new files are in the link graph.
    let (status, _) = harness
        .send(
            Request::post("/api/doc/kb/other/touch.md")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({ "content": "# Touch\n", "baseMtimeMs": 0 }).to_string(),
                ))
                .unwrap(),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, body) = harness
        .send(
            Request::post("/api/rename?root=kb")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "from": "notes/Target.md",
                        "to": "notes/Renamed.md",
                        "updateLinks": true
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let linker = std::fs::read_to_string(harness.root.join("other/linker.md")).unwrap();
    assert!(
        !linker.contains("[[Renamed]]"),
        "a bare [[Renamed]] resolves to other/Renamed.md, a different document: {linker}"
    );
    assert!(
        linker.contains("[[notes/Renamed]]"),
        "the link should have been qualified to stay unambiguous: {linker}"
    );
}

/// A phone screenshot is several megabytes of PNG shown in a column a few hundred pixels
/// wide. The variant is what closes that gap, so these pin its edges.
#[tokio::test]
async fn a_resized_variant_is_smaller_than_the_original_and_cached() {
    let harness = harness("variant");
    let cookie = harness.login().await;

    let original = std::fs::read(harness.root.join("notes/wide.png")).unwrap();

    let (status, headers, body) = harness
        .send_raw(
            Request::get("/api/file/kb/notes/wide.png?w=400")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        body.len() < original.len(),
        "a variant that is not smaller is pure cost: {} vs {}",
        body.len(),
        original.len()
    );
    assert_eq!(
        headers.get(header::CONTENT_TYPE).unwrap(),
        "image/jpeg",
        "an opaque image should not stay a PNG"
    );

    // Served from the cache the second time, byte for byte.
    let (_, _, again) = harness
        .send_raw(
            Request::get("/api/file/kb/notes/wide.png?w=400")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(again, body);
}

/// The invariant that makes the whole feature safe to turn on: whatever the content, a
/// request for a variant never returns more bytes than simply asking for the file would.
/// Re-encoding is not guaranteed to be a saving — a smooth image can compress better as
/// the PNG it already is — so the route compares and keeps the smaller answer.
#[tokio::test]
async fn a_variant_is_never_larger_than_the_original() {
    let harness = harness("variantnevergrows");
    let cookie = harness.login().await;

    for name in ["gradient.png", "wide.png", "small.png"] {
        let original = std::fs::read(harness.root.join("notes").join(name)).unwrap();
        for width in [400, 800, 1200, 1600] {
            let (status, _, body) = harness
                .send_raw(
                    Request::get(format!("/api/file/kb/notes/{name}?w={width}"))
                        .header(header::COOKIE, &cookie)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await;
            assert_eq!(status, StatusCode::OK, "{name} at {width}");
            assert!(
                body.len() <= original.len(),
                "{name} at w={width} returned {} bytes against an original of {}",
                body.len(),
                original.len()
            );
        }
    }
}

#[tokio::test]
async fn a_width_that_is_not_offered_serves_the_original_untouched() {
    let harness = harness("variantwidth");
    let cookie = harness.login().await;
    let original = std::fs::read(harness.root.join("notes/wide.png")).unwrap();

    for query in ["?w=401", "?w=0", "?w=99999"] {
        let (status, _, body) = harness
            .send_raw(
                Request::get(format!("/api/file/kb/notes/wide.png{query}"))
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{query}");
        assert_eq!(body, original, "{query} must not be resized");
    }
}

/// Scaling up is strictly more bytes for no more detail.
#[tokio::test]
async fn a_variant_wider_than_the_source_serves_the_original() {
    let harness = harness("variantupscale");
    let cookie = harness.login().await;
    let original = std::fs::read(harness.root.join("notes/small.png")).unwrap();

    let (status, _, body) = harness
        .send_raw(
            Request::get("/api/file/kb/notes/small.png?w=1600")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, original);
}

#[tokio::test]
async fn a_rendered_image_carries_its_dimensions_and_a_srcset() {
    let harness = harness("variantsrcset");
    let cookie = harness.login().await;
    create_document(
        &harness,
        &cookie,
        "gallery.md",
        "# Gallery\n\n![wide](notes/wide.png)\n",
    )
    .await;

    let (status, body) = harness
        .send(
            Request::get("/api/doc/kb/gallery.md")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let html = serde_json::from_str::<serde_json::Value>(&body).unwrap()["html"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        html.contains("width=\"1000\""),
        "space must be reserved: {html}"
    );
    assert!(html.contains("height=\"600\""), "{html}");
    assert!(html.contains("?w=400 400w"), "{html}");
    assert!(html.contains("?w=800 800w"), "{html}");
    assert!(
        !html.contains("?w=1200"),
        "no candidate may be wider than the original: {html}"
    );
    assert!(html.contains("loading=\"lazy\""), "{html}");
}

/// Ticking a checkbox is a write, so it carries the same precondition a save does and
/// changes exactly one character.
#[tokio::test]
async fn toggling_a_task_edits_only_the_checkbox() {
    let harness = harness("tasktoggle");
    let cookie = harness.login().await;

    let original = "# Tasks\n\n- [ ] first\n- [x] second\n";
    let mtime = create_document(&harness, &cookie, "tasks.md", original).await;

    let (status, body) = harness
        .send(
            Request::post("/api/task/kb/tasks.md")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({ "line": 3, "checked": true, "baseMtimeMs": mtime })
                        .to_string(),
                ))
                .unwrap(),
        )
        .await;

    assert_eq!(status, StatusCode::OK, "{body}");
    let after = std::fs::read_to_string(harness.root.join("tasks.md")).unwrap();
    assert_eq!(after, "# Tasks\n\n- [x] first\n- [x] second\n");
    assert_eq!(after.len(), original.len(), "only one character may change");
}

/// The line number a checkbox carries has to address the file on disk, not the prepared
/// source the renderer parsed. Frontmatter is stripped and a callout expands into HTML
/// before parsing, so a document with both used to hand the client a line number several
/// off, and the click silently ticked an unrelated task further down.
#[tokio::test]
async fn a_checkbox_after_frontmatter_and_a_callout_toggles_the_task_it_belongs_to() {
    let harness = harness("taskoffset");
    let cookie = harness.login().await;

    let original = concat!(
        "---\n",
        "title: Shifted\n",
        "---\n",
        "\n",
        "> [!note] A callout\n",
        "> body one\n",
        "> body two\n",
        "\n",
        "- [ ] the one that was clicked\n",
        "- [ ] filler\n",
        "- [ ] filler\n",
        "- [ ] filler\n",
        "- [ ] must stay unchecked\n",
    );
    let mtime = create_document(&harness, &cookie, "shifted.md", original).await;

    let (status, body) = harness
        .send(
            Request::get("/api/doc/kb/shifted.md")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let html = serde_json::from_str::<serde_json::Value>(&body).unwrap()["html"]
        .as_str()
        .unwrap()
        .to_string();
    let marker = "data-task-line=\"";
    let at = html
        .find(marker)
        .expect("the first task should be clickable");
    let rest = &html[at + marker.len()..];
    let line: usize = rest[..rest.find('"').unwrap()].parse().unwrap();
    assert_eq!(line, 9, "the first checkbox stands for line 9 of the file");

    let (status, body) = harness
        .send(
            Request::post("/api/task/kb/shifted.md")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({ "line": line, "checked": true, "baseMtimeMs": mtime })
                        .to_string(),
                ))
                .unwrap(),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let after = std::fs::read_to_string(harness.root.join("shifted.md")).unwrap();
    assert!(
        after.contains("- [x] the one that was clicked"),
        "the clicked task should be ticked: {after}"
    );
    assert!(
        after.contains("- [ ] must stay unchecked"),
        "no other task may be touched: {after}"
    );
}

#[tokio::test]
async fn toggling_a_task_with_a_stale_mtime_is_refused() {
    let harness = harness("taskstale");
    let cookie = harness.login().await;

    let original = "# Tasks\n\n- [ ] first\n";
    std::fs::write(harness.root.join("notes/Target.md"), original).unwrap();

    let (status, _) = harness
        .send(
            Request::post("/api/task/kb/notes/Target.md")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({ "line": 3, "checked": true, "baseMtimeMs": 1 }).to_string(),
                ))
                .unwrap(),
        )
        .await;

    // The same 409 a save gets: the precondition that failed is the same one.
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        std::fs::read_to_string(harness.root.join("notes/Target.md")).unwrap(),
        original,
        "a refused toggle must not have written anything"
    );
}

/// The route may only ever flip a checkbox: pointed at a line that is not a task, it
/// refuses rather than editing whatever happens to be there.
#[tokio::test]
async fn toggling_a_line_that_is_not_a_task_is_refused() {
    let harness = harness("tasknotatask");
    let cookie = harness.login().await;

    let original = "# Tasks\n\nJust prose.\n";
    let mtime = create_document(&harness, &cookie, "prose.md", original).await;

    let (status, _) = harness
        .send(
            Request::post("/api/task/kb/prose.md")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({ "line": 3, "checked": true, "baseMtimeMs": mtime })
                        .to_string(),
                ))
                .unwrap(),
        )
        .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        std::fs::read_to_string(harness.root.join("prose.md")).unwrap(),
        original
    );
}

#[tokio::test]
async fn task_toggling_requires_a_session() {
    let harness = harness("taskauth");
    let (status, _) = harness
        .send(
            Request::post("/api/task/kb/notes/Target.md")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({ "line": 1, "checked": true, "baseMtimeMs": 0 }).to_string(),
                ))
                .unwrap(),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
