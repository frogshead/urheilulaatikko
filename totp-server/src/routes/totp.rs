use axum::{extract::State, Extension, Json};
use serde::Serialize;

use crate::{
    auth::AuthenticatedUser,
    error::{AppError, AppResult},
    state::AppState,
    totp,
    totp::secret_store::RequestResult,
};

#[derive(Serialize)]
pub struct PinResponse {
    pin: String,
    valid_for_seconds: u64,
}

/// `POST /v1/totp/request` — issues the current PIN for the authenticated user.
///
/// The response carries the PIN and nothing else: the secret it was derived from
/// never leaves the server after provisioning, and is never logged.
pub async fn request_pin(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
) -> AppResult<Json<PinResponse>> {
    let Some(secret) = state.secrets.get(&user.subject).await? else {
        state
            .secrets
            .log_request(Some(&user.subject), RequestResult::DeniedNoSecret)
            .await;
        return Err(AppError::NotFound);
    };

    let generated = totp::generate(&secret)?;

    state.secrets.touch_last_used(&user.subject).await?;
    state
        .secrets
        .log_request(Some(&user.subject), RequestResult::Issued)
        .await;

    tracing::info!(subject = %user.subject, "issued a PIN");

    Ok(Json(PinResponse {
        pin: generated.pin,
        valid_for_seconds: generated.valid_for_seconds,
    }))
}
