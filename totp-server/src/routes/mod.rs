pub mod health;
pub mod provision;
pub mod totp;

use std::sync::Arc;

use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use tower_governor::GovernorLayer;

use crate::{
    auth::middleware::{audit_rate_limited, require_authentication},
    rate_limit,
    state::AppState,
};

pub fn create_router(state: AppState) -> anyhow::Result<Router> {
    let governor = Arc::new(rate_limit::config(&state.config)?);

    // Layer ordering: axum runs the *last* added layer first, so these execute
    // outermost-first as authentication -> audit -> rate limit -> handler.
    // Authentication must come first because the limiter keys on the subject it
    // puts into the request extensions, and the audit layer must wrap the
    // limiter to observe the 429 it returns.
    let protected = Router::new()
        .route("/v1/totp/request", post(totp::request_pin))
        .route("/v1/totp/provision", post(provision::provision))
        .layer(GovernorLayer::new(governor))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            audit_rate_limited,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            require_authentication,
        ));

    Ok(Router::new()
        .route("/healthz", get(health::healthz))
        .merge(protected)
        .with_state(state))
}
