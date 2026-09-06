use axum::{extract::State, Extension, Json};
use serde::{Deserialize, Serialize};

use crate::{
    auth::AuthenticatedUser,
    error::{AppError, AppResult},
    state::AppState,
    totp,
    totp::secret_store::RequestResult,
};

#[derive(Deserialize)]
pub struct ProvisionRequest {
    subject: String,
}

#[derive(Serialize)]
pub struct ProvisionResponse {
    secret_base32: String,
    otpauth_url: String,
}

/// `POST /v1/totp/provision` — registers a TOTP secret for a subject.
///
/// Requires the admin realm role. The plaintext secret is returned **once**, in
/// this response only: it is stored encrypted, no other endpoint exposes it, and
/// it is never written to a log line.
pub async fn provision(
    State(state): State<AppState>,
    Extension(admin): Extension<AuthenticatedUser>,
    Json(payload): Json<ProvisionRequest>,
) -> AppResult<Json<ProvisionResponse>> {
    if !admin.has_role(&state.config.admin_role) {
        tracing::warn!(
            subject = %admin.subject,
            "rejected a provisioning attempt from a caller without the admin role"
        );
        return Err(AppError::Forbidden);
    }

    let subject = payload.subject.trim();
    if subject.is_empty() {
        return Err(AppError::Forbidden);
    }

    let provisioned = totp::provision(subject)?;
    state.secrets.upsert(subject, &provisioned.raw).await?;
    state
        .secrets
        .log_request(Some(subject), RequestResult::Provisioned)
        .await;

    // Subject and actor only — the secret and the otpauth URL stay out of the log.
    tracing::info!(
        subject = %subject,
        provisioned_by = %admin.subject,
        "provisioned a TOTP secret"
    );

    Ok(Json(ProvisionResponse {
        secret_base32: provisioned.secret_base32,
        otpauth_url: provisioned.otpauth_url,
    }))
}
