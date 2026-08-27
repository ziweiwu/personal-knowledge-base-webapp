//! The authentication gate.
//!
//! Registered once on the `/api` router so that every route beneath it is covered,
//! including routes added later. A route cannot opt out by forgetting to add a check;
//! it would have to be deliberately mounted outside the gated tree.

use crate::auth::store::User;
use crate::error::AppError;
use crate::state::AppState;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, Method};
use axum::middleware::Next;
use axum::response::Response;
use axum_extra::extract::cookie::CookieJar;
use std::sync::Arc;

pub const SESSION_COOKIE: &str = "kbv_session";

pub async fn require_session(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    // The session cookie is already restricted to same-site sends on state-changing
    // methods; checking Origin as well means a browser that mishandles that restriction
    // is still covered.
    if is_mutating(request.method()) && !origin_is_same_site(request.headers()) {
        return Err(AppError::Forbidden("Cross-origin request refused".into()));
    }

    let session_id = jar
        .get(SESSION_COOKIE)
        .map(|cookie| cookie.value().to_string())
        .ok_or(AppError::Unauthorized)?;

    let user = state
        .auth
        .session_user(&session_id)
        .ok_or(AppError::Unauthorized)?;

    request.extensions_mut().insert(CurrentUser(user));
    Ok(next.run(request).await)
}

#[derive(Clone)]
pub struct CurrentUser(pub User);

fn is_mutating(method: &Method) -> bool {
    !matches!(method, &Method::GET | &Method::HEAD | &Method::OPTIONS)
}

/// Accept a request whose `Origin` host matches the `Host` it was sent to, or that sends
/// no `Origin` at all (non-browser clients such as curl, which are not subject to CSRF).
fn origin_is_same_site(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get("origin").and_then(|v| v.to_str().ok()) else {
        return true;
    };
    let Some(host) = headers.get("host").and_then(|v| v.to_str().ok()) else {
        return false;
    };
    origin_host(origin).map(|o| o == host).unwrap_or(false)
}

fn origin_host(origin: &str) -> Option<&str> {
    origin
        .strip_prefix("https://")
        .or_else(|| origin.strip_prefix("http://"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (key, value) in pairs {
            let name = axum::http::HeaderName::from_bytes(key.as_bytes()).unwrap();
            map.insert(name, HeaderValue::from_str(value).unwrap());
        }
        map
    }

    #[test]
    fn only_state_changing_methods_are_origin_checked() {
        assert!(!is_mutating(&Method::GET));
        assert!(!is_mutating(&Method::HEAD));
        assert!(is_mutating(&Method::POST));
        assert!(is_mutating(&Method::PUT));
        assert!(is_mutating(&Method::DELETE));
    }

    #[test]
    fn a_matching_origin_is_accepted() {
        assert!(origin_is_same_site(&headers(&[
            ("origin", "https://kb.example.ts.net"),
            ("host", "kb.example.ts.net"),
        ])));
    }

    #[test]
    fn a_foreign_origin_is_refused() {
        assert!(!origin_is_same_site(&headers(&[
            ("origin", "https://evil.example.com"),
            ("host", "kb.example.ts.net"),
        ])));
    }

    #[test]
    fn an_origin_that_merely_contains_the_host_is_refused() {
        assert!(
            !origin_is_same_site(&headers(&[
                ("origin", "https://kb.example.ts.net.evil.com"),
                ("host", "kb.example.ts.net"),
            ])),
            "suffix matching would let an attacker register a lookalike domain"
        );
    }

    #[test]
    fn a_request_with_no_origin_is_allowed() {
        assert!(
            origin_is_same_site(&headers(&[("host", "kb.example.ts.net")])),
            "non-browser clients send no Origin and cannot be CSRF'd"
        );
    }

    #[test]
    fn an_origin_with_no_host_header_is_refused() {
        assert!(!origin_is_same_site(&headers(&[(
            "origin",
            "https://x.example.com"
        )])));
    }
}
