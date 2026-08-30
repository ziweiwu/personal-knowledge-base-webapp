//! Login, logout and session inspection.

use crate::auth::middleware::{CurrentUser, SESSION_COOKIE};
use crate::auth::store::User;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::{Extension, Json};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use kbviewer_core::model::SessionInfo;
use serde::Deserialize;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

/// Login reads no cookie of its own, so it takes no `CookieJar` extractor: the jar it
/// returns carries only the session cookie it just minted, which is all axum writes out.
pub async fn login(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(body): Json<LoginRequest>,
) -> AppResult<impl IntoResponse> {
    let email = kbviewer_core::config::normalise_email_for_login(&body.email);
    let keys = vec![
        format!("email:{email}"),
        format!("addr:{}", client_address(&headers, Some(peer))),
    ];

    // Checked before hashing: Argon2 is deliberately expensive, so an unlimited endpoint
    // would be both a guessing oracle and a way to exhaust memory.
    if let crate::auth::rate_limit::Decision::Deny(wait) = state.auth.limiter.check(&keys) {
        return Err(AppError::RateLimited(wait.as_secs().max(1)));
    }

    let Some(user) = authenticate_off_runtime(&state, &email, &body.password).await? else {
        state.auth.limiter.record_failure(&keys);
        // Same response whether the email is unknown or the password is wrong.
        return Err(AppError::Unauthorized);
    };

    state.auth.limiter.record_success(&keys);
    let session = state
        .auth
        .create_session(&user.id)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let jar = CookieJar::new().add(session_cookie(session.id, &headers));
    Ok((jar, Json(SessionInfo { email: user.email })))
}

/// Argon2 blocks for tens of milliseconds; keep it off the async runtime.
async fn authenticate_off_runtime(
    state: &Arc<AppState>,
    email: &str,
    password: &str,
) -> AppResult<Option<User>> {
    let store = state.clone();
    let attempt_email = email.to_string();
    let attempt_password = password.to_string();
    tokio::task::spawn_blocking(move || store.auth.authenticate(&attempt_email, &attempt_password))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))
}

pub async fn logout(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    headers: HeaderMap,
) -> AppResult<impl IntoResponse> {
    if let Some(cookie) = jar.get(SESSION_COOKIE) {
        state.auth.destroy_session(cookie.value()).ok();
    }
    let mut removal = session_cookie(String::new(), &headers);
    removal.make_removal();
    Ok((jar.add(removal), StatusCode::NO_CONTENT))
}

pub async fn session(Extension(CurrentUser(user)): Extension<CurrentUser>) -> Json<SessionInfo> {
    Json(SessionInfo { email: user.email })
}

const SESSION_LIFETIME: Duration = Duration::from_secs(30 * 24 * 60 * 60);

fn session_cookie(value: String, headers: &HeaderMap) -> Cookie<'static> {
    let mut cookie = Cookie::new(SESSION_COOKIE, value);
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_path("/");
    cookie.set_max_age(time::Duration::seconds(SESSION_LIFETIME.as_secs() as i64));
    // `tailscale serve` terminates TLS and forwards plain HTTP to localhost, so the
    // scheme has to come from the forwarded header. Marking the cookie Secure over a
    // plain-HTTP dev server would stop it being stored at all.
    cookie.set_secure(is_https(headers));
    cookie
}

fn is_https(headers: &HeaderMap) -> bool {
    headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .map(|proto| proto.eq_ignore_ascii_case("https"))
        .unwrap_or(false)
}

/// The address to rate-limit against. Behind `tailscale serve` every peer address is
/// localhost, so the forwarded header is what distinguishes callers.
///
/// The **last** entry is used, not the first. A proxy appends the address it observed, so
/// the last value is the one our own proxy vouched for; the earlier entries are whatever
/// the caller chose to send. Taking the first would let anyone pick their own rate-limit
/// key and rotate it per request, which is the entire limiter defeated.
fn client_address(headers: &HeaderMap, peer: Option<SocketAddr>) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.rsplit(',').next())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| {
            peer.map(|p| p.ip().to_string())
                .unwrap_or_else(|| "unknown".into())
        })
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

    fn peer() -> Option<SocketAddr> {
        Some("127.0.0.1:1234".parse().unwrap())
    }

    #[test]
    fn the_cookie_is_not_reachable_from_javascript() {
        let cookie = session_cookie("abc".into(), &HeaderMap::new());
        assert_eq!(cookie.http_only(), Some(true));
        assert_eq!(cookie.same_site(), Some(SameSite::Lax));
    }

    #[test]
    fn the_cookie_is_secure_only_when_the_connection_is() {
        let over_https = session_cookie("abc".into(), &headers(&[("x-forwarded-proto", "https")]));
        assert_eq!(over_https.secure(), Some(true));

        let over_http = session_cookie("abc".into(), &HeaderMap::new());
        assert_ne!(
            over_http.secure(),
            Some(true),
            "a Secure cookie would not be stored over plain http"
        );
    }

    #[test]
    fn the_forwarded_address_is_preferred_over_the_proxy_peer() {
        let map = headers(&[("x-forwarded-for", "100.64.0.9")]);
        assert_eq!(client_address(&map, peer()), "100.64.0.9");
    }

    /// A caller can put anything at the front of the header; only the trailing entry was
    /// actually observed by our proxy.
    #[test]
    fn a_spoofed_leading_forwarded_entry_does_not_choose_the_rate_limit_key() {
        let map = headers(&[("x-forwarded-for", "1.2.3.4, 100.64.0.9")]);
        assert_eq!(
            client_address(&map, peer()),
            "100.64.0.9",
            "trusting the caller's value lets them rotate their own limiter key"
        );
    }

    #[test]
    fn falls_back_to_the_peer_address() {
        assert_eq!(client_address(&HeaderMap::new(), peer()), "127.0.0.1");
        assert_eq!(client_address(&HeaderMap::new(), None), "unknown");
    }

    #[test]
    fn an_empty_forwarded_header_falls_back() {
        let map = headers(&[("x-forwarded-for", "")]);
        assert_eq!(client_address(&map, peer()), "127.0.0.1");
    }
}
