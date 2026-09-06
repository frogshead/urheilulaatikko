use std::env;

use anyhow::{bail, Context, Result};
use base64::Engine;

/// Length of an AES-256 key in bytes.
const ENCRYPTION_KEY_LEN: usize = 32;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub host: String,
    pub port: u16,

    /// Keycloak realm base URL, e.g. `http://keycloak:8080/realms/lock-demo`.
    pub keycloak_issuer: String,
    pub keycloak_client_id: String,
    pub keycloak_client_secret: String,

    /// Value that must appear in the token's `aud` claim. Checking this is
    /// mandatory: without it any token from the realm would be accepted here.
    pub expected_audience: String,

    /// Realm role required by `POST /v1/totp/provision`.
    pub admin_role: String,

    /// Decoded AES-256-GCM key. Never logged.
    pub encryption_key: [u8; ENCRYPTION_KEY_LEN],

    /// Number of time steps the *lock* is expected to tolerate. The server only
    /// ever generates the code for the current step; this value is reported on
    /// `/healthz` so the lock's configuration can be cross-checked.
    pub totp_skew_steps: u8,

    pub rate_limit_max_requests: u32,
    pub rate_limit_window_seconds: u64,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        let encryption_key = parse_encryption_key(&required("TOTP_ENCRYPTION_KEY")?)?;

        let rate_limit_max_requests = optional_parse("RATE_LIMIT_MAX_REQUESTS", 6u32)?;
        if rate_limit_max_requests == 0 {
            bail!("RATE_LIMIT_MAX_REQUESTS must be at least 1");
        }
        let rate_limit_window_seconds = optional_parse("RATE_LIMIT_WINDOW_SECONDS", 300u64)?;
        if rate_limit_window_seconds == 0 {
            bail!("RATE_LIMIT_WINDOW_SECONDS must be at least 1");
        }

        Ok(Config {
            database_url: required("DATABASE_URL")?,
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: optional_parse("PORT", 3000u16)?,
            keycloak_issuer: required("KEYCLOAK_ISSUER")?
                .trim_end_matches('/')
                .to_string(),
            keycloak_client_id: required("TOTP_SERVER_KEYCLOAK_CLIENT_ID")?,
            keycloak_client_secret: required("TOTP_SERVER_KEYCLOAK_CLIENT_SECRET")?,
            expected_audience: env::var("TOTP_EXPECTED_AUDIENCE")
                .unwrap_or_else(|_| "totp-server".to_string()),
            admin_role: env::var("TOTP_ADMIN_ROLE").unwrap_or_else(|_| "totp-admin".to_string()),
            encryption_key,
            totp_skew_steps: optional_parse("TOTP_SKEW_STEPS", 1u8)?,
            rate_limit_max_requests,
            rate_limit_window_seconds,
        })
    }

    pub fn server_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    /// RFC 7662 introspection endpoint for the configured realm.
    pub fn introspection_url(&self) -> String {
        format!(
            "{}/protocol/openid-connect/token/introspect",
            self.keycloak_issuer
        )
    }
}

fn required(key: &str) -> Result<String> {
    env::var(key).with_context(|| format!("{key} must be set"))
}

fn optional_parse<T>(key: &str, default: T) -> Result<T>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    match env::var(key) {
        Err(_) => Ok(default),
        Ok(raw) => raw
            .trim()
            .parse::<T>()
            .map_err(|e| anyhow::anyhow!("{key} is not a valid value: {e}")),
    }
}

/// Decodes the base64 key, tolerating the `base64:` prefix used in `.env.example`.
///
/// Validated at startup rather than on first use so a bad key fails the deploy
/// instead of the first user request.
fn parse_encryption_key(raw: &str) -> Result<[u8; ENCRYPTION_KEY_LEN]> {
    let raw = raw.trim();
    let encoded = raw.strip_prefix("base64:").unwrap_or(raw);

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .context("TOTP_ENCRYPTION_KEY must be valid base64 (optionally `base64:`-prefixed)")?;

    let len = bytes.len();
    bytes.try_into().map_err(|_| {
        anyhow::anyhow!("TOTP_ENCRYPTION_KEY must decode to exactly 32 bytes, got {len}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_32_byte_key_with_and_without_prefix() {
        let key = base64::engine::general_purpose::STANDARD.encode([7u8; 32]);
        assert_eq!(parse_encryption_key(&key).unwrap(), [7u8; 32]);
        assert_eq!(
            parse_encryption_key(&format!("base64:{key}")).unwrap(),
            [7u8; 32]
        );
    }

    #[test]
    fn rejects_a_key_of_the_wrong_length() {
        let short = base64::engine::general_purpose::STANDARD.encode([7u8; 16]);
        let err = parse_encryption_key(&short).unwrap_err().to_string();
        assert!(err.contains("32 bytes"), "unexpected error: {err}");
    }

    #[test]
    fn rejects_a_key_that_is_not_base64() {
        assert!(parse_encryption_key("not base64!!").is_err());
    }
}
