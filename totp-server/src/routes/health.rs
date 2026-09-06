use axum::{extract::State, Json};
use serde::Serialize;

use crate::{state::AppState, totp};

#[derive(Serialize)]
pub struct HealthResponse {
    status: &'static str,
    algorithm: &'static str,
    digits: u8,
    step_seconds: u64,
    /// How many steps of drift the *lock* is expected to tolerate. Exposed so a
    /// lock's configuration can be cross-checked against the server's without
    /// reading either one's environment.
    skew_steps: u8,
}

/// Unauthenticated liveness probe, used by the Docker healthcheck.
pub async fn healthz(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        algorithm: "SHA1",
        digits: totp::DIGITS,
        step_seconds: totp::STEP_SECONDS,
        skew_steps: state.config.totp_skew_steps,
    })
}
