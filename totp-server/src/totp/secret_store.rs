use sqlx::PgPool;

use crate::{crypto::Crypto, error::AppError};

/// Outcome recorded in `request_log` for each PIN request. Kept as a small enum
/// so the strings in the audit trail stay consistent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestResult {
    Issued,
    DeniedInactiveToken,
    DeniedNoSecret,
    RateLimited,
    Provisioned,
}

impl RequestResult {
    pub fn as_str(self) -> &'static str {
        match self {
            RequestResult::Issued => "issued",
            RequestResult::DeniedInactiveToken => "denied_inactive_token",
            RequestResult::DeniedNoSecret => "denied_no_secret",
            RequestResult::RateLimited => "rate_limited",
            RequestResult::Provisioned => "provisioned",
        }
    }
}

/// Storage for TOTP secrets.
///
/// Encryption and decryption happen here, so a plaintext secret only ever exists
/// inside this module's callers for as long as it takes to compute a PIN — and
/// is returned to a client only by the provisioning endpoint.
#[derive(Clone)]
pub struct SecretStore {
    pool: PgPool,
    crypto: Crypto,
}

impl SecretStore {
    pub fn new(pool: PgPool, crypto: Crypto) -> Self {
        SecretStore { pool, crypto }
    }

    /// Returns the decrypted secret for `subject`, or `None` if none is registered.
    pub async fn get(&self, subject: &str) -> Result<Option<Vec<u8>>, AppError> {
        let row: Option<(Vec<u8>, Vec<u8>)> =
            sqlx::query_as("select encrypted_secret, nonce from totp_secrets where subject = $1")
                .bind(subject)
                .fetch_optional(&self.pool)
                .await?;

        match row {
            None => Ok(None),
            Some((encrypted_secret, nonce)) => {
                Ok(Some(self.crypto.decrypt(&encrypted_secret, &nonce)?))
            }
        }
    }

    /// Stores (or replaces) the secret for `subject`, encrypted with a fresh nonce.
    pub async fn upsert(&self, subject: &str, secret: &[u8]) -> Result<(), AppError> {
        let (encrypted_secret, nonce) = self.crypto.encrypt(secret)?;

        sqlx::query(
            r#"
            insert into totp_secrets (subject, encrypted_secret, nonce)
            values ($1, $2, $3)
            on conflict (subject) do update
              set encrypted_secret = excluded.encrypted_secret,
                  nonce            = excluded.nonce,
                  created_at       = now(),
                  last_used_at     = null
            "#,
        )
        .bind(subject)
        .bind(&encrypted_secret)
        .bind(&nonce)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn touch_last_used(&self, subject: &str) -> Result<(), AppError> {
        sqlx::query("update totp_secrets set last_used_at = now() where subject = $1")
            .bind(subject)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Appends an audit row. `subject` is `None` when the token was rejected
    /// before a subject could be established.
    ///
    /// Failures are logged rather than propagated: losing an audit line must not
    /// turn a successful PIN request into a 500.
    pub async fn log_request(&self, subject: Option<&str>, result: RequestResult) {
        let outcome =
            sqlx::query("insert into request_log (id, subject, result) values ($1, $2, $3)")
                .bind(uuid::Uuid::new_v4())
                .bind(subject)
                .bind(result.as_str())
                .execute(&self.pool)
                .await;

        if let Err(err) = outcome {
            tracing::error!(
                subject = subject.unwrap_or("<unauthenticated>"),
                result = result.as_str(),
                error = ?err,
                "failed to write the audit log entry"
            );
        }
    }
}
