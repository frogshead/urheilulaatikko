use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

/// Errors that can be turned into an HTTP response.
///
/// The `Display` text of each variant is what the client sees. Anything more
/// detailed (database errors, upstream response bodies) is logged instead — it
/// must never reach the caller, since it can leak internal topology.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// Token missing, malformed, expired, or introspection said `active: false`.
    #[error("unauthorized")]
    Unauthorized,

    /// Token is valid but lacks the audience or role required for this route.
    #[error("forbidden")]
    Forbidden,

    /// No TOTP secret is registered for this subject.
    #[error("no TOTP secret registered for this user")]
    NotFound,

    #[error("too many requests")]
    RateLimited,

    /// Keycloak could not be reached or answered with an error. Never fail open:
    /// without a positive introspection result the request is rejected.
    #[error("token introspection is temporarily unavailable")]
    IntrospectionUnavailable(#[source] anyhow::Error),

    #[error("internal server error")]
    Internal(#[source] anyhow::Error),
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match self {
            AppError::Unauthorized => StatusCode::UNAUTHORIZED,
            AppError::Forbidden => StatusCode::FORBIDDEN,
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            AppError::IntrospectionUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        // Log the underlying cause; return only the opaque message.
        match &self {
            AppError::IntrospectionUnavailable(source) => {
                tracing::error!(error = ?source, "token introspection failed")
            }
            AppError::Internal(source) => tracing::error!(error = ?source, "internal error"),
            other => tracing::debug!(error = %other, "request rejected"),
        }

        let body = ErrorBody {
            error: self.to_string(),
        };
        (status, Json(body)).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        AppError::Internal(anyhow::Error::new(err))
    }
}

pub type AppResult<T> = Result<T, AppError>;
