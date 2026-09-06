use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
};

use crate::{
    auth::AuthenticatedUser, error::AppError, state::AppState, totp::secret_store::RequestResult,
};

/// Verifies the bearer token with Keycloak and attaches the resulting
/// [`AuthenticatedUser`](crate::auth::AuthenticatedUser) as a request extension.
///
/// Layer ordering matters: in axum the *last* `.layer()` added runs first, so
/// this must be layered after (i.e. outside) the rate limiter — the limiter
/// keys on the subject this middleware inserts.
pub async fn require_authentication(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let Some(token) = bearer_token(&request) else {
        state
            .secrets
            .log_request(None, RequestResult::DeniedInactiveToken)
            .await;
        return Err(AppError::Unauthorized);
    };

    let user = match state.introspection.authenticate(&token).await {
        Ok(user) => user,
        Err(err) => {
            // A rejected token carries no usable subject, so the audit row is
            // written without one. 503s are not recorded here: they say
            // something about Keycloak, not about the caller.
            if matches!(err, AppError::Unauthorized | AppError::Forbidden) {
                state
                    .secrets
                    .log_request(None, RequestResult::DeniedInactiveToken)
                    .await;
            }
            return Err(err);
        }
    };

    tracing::debug!(subject = %user.subject, "request authenticated");
    request.extensions_mut().insert(user);

    Ok(next.run(request).await)
}

/// Records rate-limited requests in the audit trail.
///
/// `tower_governor` answers 429 itself, so the only way to see one is to sit
/// outside it and inspect the status. This must be layered *inside* the
/// authentication middleware so the subject is already in the extensions.
pub async fn audit_rate_limited(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let subject = request
        .extensions()
        .get::<AuthenticatedUser>()
        .map(|user| user.subject.clone());

    let response = next.run(request).await;

    if response.status() == StatusCode::TOO_MANY_REQUESTS {
        tracing::warn!(
            subject = subject.as_deref().unwrap_or("<unknown>"),
            "rate limit exceeded"
        );
        state
            .secrets
            .log_request(subject.as_deref(), RequestResult::RateLimited)
            .await;
    }

    response
}

/// Extracts the credential from an `Authorization: Bearer <token>` header.
/// The scheme comparison is case-insensitive per RFC 7235.
fn bearer_token(request: &Request) -> Option<String> {
    let value = request
        .headers()
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    let (scheme, token) = value.split_once(' ')?;

    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }

    let token = token.trim();
    (!token.is_empty()).then(|| token.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;

    fn request_with_auth(value: &str) -> Request {
        Request::builder()
            .header(header::AUTHORIZATION, value)
            .body(Body::empty())
            .unwrap()
    }

    #[test]
    fn extracts_a_bearer_token() {
        assert_eq!(
            bearer_token(&request_with_auth("Bearer abc.def.ghi")).as_deref(),
            Some("abc.def.ghi")
        );
    }

    #[test]
    fn accepts_any_casing_of_the_scheme() {
        assert_eq!(
            bearer_token(&request_with_auth("bEaReR abc")).as_deref(),
            Some("abc")
        );
    }

    #[test]
    fn rejects_other_schemes_and_empty_tokens() {
        assert!(bearer_token(&request_with_auth("Basic abc")).is_none());
        assert!(bearer_token(&request_with_auth("Bearer   ")).is_none());
        assert!(bearer_token(&request_with_auth("abc")).is_none());
    }

    #[test]
    fn rejects_a_request_without_the_header() {
        let request = Request::builder().body(Body::empty()).unwrap();
        assert!(bearer_token(&request).is_none());
    }
}
