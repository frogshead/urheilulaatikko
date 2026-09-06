use std::time::Duration;

use axum::http::Request;
use governor::middleware::NoOpMiddleware;
use tower_governor::{
    errors::GovernorError,
    governor::{GovernorConfig, GovernorConfigBuilder},
    key_extractor::KeyExtractor,
};

use crate::{auth::AuthenticatedUser, config::Config};

/// Rate-limits per Keycloak subject rather than per IP: the same person may
/// request a PIN from a phone on mobile data and then from a laptop, and two
/// people behind one NAT must not share a budget.
#[derive(Clone)]
pub struct SubjectKeyExtractor;

impl KeyExtractor for SubjectKeyExtractor {
    type Key = String;

    fn name(&self) -> &'static str {
        "authenticated subject"
    }

    fn extract<T>(&self, req: &Request<T>) -> Result<Self::Key, GovernorError> {
        // Present only because the auth middleware runs first — see the layer
        // ordering note in `routes::create_router`.
        req.extensions()
            .get::<AuthenticatedUser>()
            .map(|user| user.subject.clone())
            .ok_or(GovernorError::UnableToExtractKey)
    }

    fn key_name(&self, key: &Self::Key) -> Option<String> {
        Some(key.clone())
    }
}

/// Builds a token bucket holding `RATE_LIMIT_MAX_REQUESTS` tokens that refills
/// one token every `window / max` seconds. Over any window that is the
/// configured budget, while still allowing a short burst.
pub fn config(
    config: &Config,
) -> anyhow::Result<GovernorConfig<SubjectKeyExtractor, NoOpMiddleware>> {
    let refill = Duration::from_millis(
        config.rate_limit_window_seconds * 1000 / u64::from(config.rate_limit_max_requests),
    );

    GovernorConfigBuilder::default()
        .period(refill)
        .burst_size(config.rate_limit_max_requests)
        .key_extractor(SubjectKeyExtractor)
        .finish()
        .ok_or_else(|| anyhow::anyhow!("invalid rate limit configuration"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;

    #[test]
    fn extracts_the_subject_set_by_the_auth_middleware() {
        let mut request = Request::new(Body::empty());
        request.extensions_mut().insert(AuthenticatedUser {
            subject: "user-1".to_string(),
            realm_roles: vec![],
        });

        assert_eq!(
            SubjectKeyExtractor.extract(&request).unwrap(),
            "user-1".to_string()
        );
    }

    #[test]
    fn fails_when_the_request_was_not_authenticated() {
        let request: Request<Body> = Request::new(Body::empty());
        assert!(matches!(
            SubjectKeyExtractor.extract(&request),
            Err(GovernorError::UnableToExtractKey)
        ));
    }
}
