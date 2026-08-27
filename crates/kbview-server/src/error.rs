//! One error type for every route, so failures are reported consistently.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use kbview_core::model::{ApiError, SaveConflict};

#[derive(Debug)]
pub enum AppError {
    Unauthorized,
    NotFound(String),
    BadRequest(String),
    Forbidden(String),
    /// A write would have overwritten someone else's change. Carries both versions.
    Conflict(Box<SaveConflict>),
    /// The same precondition failing where there is no edited buffer to hand back — a
    /// checkbox click. Same 409 a save gets, because the cause is the same; no body,
    /// because there is nothing to choose between.
    Stale(String),
    AlreadyExists(String),
    RateLimited(u64),
    ReadOnly,
    Internal(String),
}

impl AppError {
    fn parts(&self) -> (StatusCode, &'static str, String) {
        let (status, code) = self.status_and_code();
        (status, code, self.message())
    }

    fn status_and_code(&self) -> (StatusCode, &'static str) {
        match self {
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized"),
            Self::NotFound(_) => (StatusCode::NOT_FOUND, "not_found"),
            Self::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad_request"),
            Self::Forbidden(_) => (StatusCode::FORBIDDEN, "forbidden"),
            Self::Conflict(_) => (StatusCode::CONFLICT, "conflict"),
            Self::Stale(_) => (StatusCode::CONFLICT, "stale"),
            Self::AlreadyExists(_) => (StatusCode::CONFLICT, "already_exists"),
            Self::RateLimited(_) => (StatusCode::TOO_MANY_REQUESTS, "rate_limited"),
            Self::ReadOnly => (StatusCode::FORBIDDEN, "read_only"),
            Self::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),
        }
    }

    /// Deliberately uniform: never reveal whether an email exists or a path is real.
    fn message(&self) -> String {
        match self {
            Self::Unauthorized => "Authentication required".into(),
            Self::NotFound(what) => format!("No such {what}"),
            Self::BadRequest(why) => why.clone(),
            Self::Forbidden(why) => why.clone(),
            Self::Conflict(_) => "This file changed on disk since you opened it".into(),
            Self::Stale(why) => why.clone(),
            Self::AlreadyExists(path) => format!("{path} already exists"),
            Self::RateLimited(seconds) => {
                format!("Too many attempts. Try again in {seconds} seconds.")
            }
            Self::ReadOnly => "This folder is configured as read-only".into(),
            Self::Internal(detail) => {
                // Logged in full, but not returned: internal detail is not the caller's business.
                tracing::error!(detail, "internal error");
                "Something went wrong".into()
            }
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error, message) = self.parts();

        if let Self::Conflict(conflict) = self {
            return (status, Json(*conflict)).into_response();
        }
        (
            status,
            Json(ApiError {
                error: error.to_string(),
                message,
            }),
        )
            .into_response()
    }
}

impl From<kbview_core::paths::PathError> for AppError {
    fn from(error: kbview_core::paths::PathError) -> Self {
        // A traversal attempt is logged but answered as a plain 404, so probing the
        // allowlist tells an attacker nothing about what exists outside it.
        tracing::warn!(%error, "rejected path outside root");
        Self::NotFound("document".into())
    }
}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        match error.kind() {
            std::io::ErrorKind::NotFound => Self::NotFound("file".into()),
            std::io::ErrorKind::PermissionDenied => Self::Forbidden("Permission denied".into()),
            _ => Self::Internal(error.to_string()),
        }
    }
}

pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    /// Generous enough for any error body; a cap is required by `to_bytes`.
    const MAX_ERROR_BODY_BYTES: usize = 64 * 1024;

    async fn body_of(error: AppError) -> (StatusCode, serde_json::Value) {
        let response = error.into_response();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), MAX_ERROR_BODY_BYTES)
            .await
            .unwrap();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
        )
    }

    #[tokio::test]
    async fn unauthorized_does_not_explain_itself() {
        let (status, body) = body_of(AppError::Unauthorized).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let message = body["message"].as_str().unwrap();
        assert!(!message.contains("email") && !message.contains("password"));
    }

    #[tokio::test]
    async fn a_path_escape_is_reported_as_a_plain_not_found() {
        let error: AppError =
            kbview_core::paths::PathError::OutsideRoot("../../etc/passwd".into()).into();
        let (status, body) = body_of(error).await;
        assert_eq!(
            status,
            StatusCode::NOT_FOUND,
            "must not confirm that the path was rejected as an escape"
        );
        assert!(!body["message"].as_str().unwrap().contains("etc/passwd"));
    }

    #[tokio::test]
    async fn internal_errors_do_not_leak_detail() {
        let (status, body) =
            body_of(AppError::Internal("connection string with secret".into())).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(!body["message"].as_str().unwrap().contains("secret"));
    }

    #[tokio::test]
    async fn a_conflict_returns_both_versions_for_the_user_to_choose() {
        let conflict = SaveConflict {
            path: "a.md".into(),
            your_content: "mine".into(),
            disk_content: "theirs".into(),
            disk_mtime_ms: 42,
        };
        let (status, body) = body_of(AppError::Conflict(Box::new(conflict))).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["yourContent"], "mine");
        assert_eq!(body["diskContent"], "theirs");
    }

    #[tokio::test]
    async fn rate_limiting_says_how_long_to_wait() {
        const RETRY_AFTER_SECONDS: u64 = 90;
        let (status, body) = body_of(AppError::RateLimited(RETRY_AFTER_SECONDS)).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert!(body["message"].as_str().unwrap().contains("90"));
    }

    #[tokio::test]
    async fn a_missing_file_maps_to_not_found() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "nope");
        let (status, _) = body_of(io.into()).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}
