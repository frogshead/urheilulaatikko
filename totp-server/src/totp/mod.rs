pub mod secret_store;

use std::time::{SystemTime, UNIX_EPOCH};

use totp_rs::{Algorithm, Builder, Secret, Totp};

use crate::error::AppError;

/// RFC 6238 parameters. SHA-1 / 6 digits / 30 s is the Google Authenticator
/// profile, which is what the `otpauth://` URL handed out at provisioning time
/// must match.
pub const ALGORITHM: Algorithm = Algorithm::SHA1;
pub const DIGITS: u8 = 6;
pub const STEP_SECONDS: u64 = 30;

/// Issuer shown by an authenticator app for provisioned secrets.
const ISSUER: &str = "urheilulaatikko";

/// A PIN plus how long the current time step still has to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedPin {
    pub pin: String,
    pub valid_for_seconds: u64,
}

/// Builds the generator for a raw (decrypted) secret.
///
/// `account_name` only affects the `otpauth://` URL, never the PIN.
fn generator(secret: &[u8], account_name: &str) -> Result<Totp, AppError> {
    Builder::new()
        .with_algorithm(ALGORITHM)
        .with_digits(DIGITS)
        .with_step_duration(STEP_SECONDS)
        .with_secret(secret.to_vec())
        .with_issuer(Some(ISSUER))
        .with_account_name(account_name)
        .build()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("invalid TOTP parameters: {e}")))
}

/// Generates the PIN for the current time step.
///
/// The server deliberately produces only the *current* code. Clock drift is
/// absorbed on the lock's side, which accepts `TOTP_SKEW_STEPS` steps either
/// way — see the `TOTP_SKEW_STEPS` note in the README.
pub fn generate(secret: &[u8]) -> Result<GeneratedPin, AppError> {
    generate_at(secret, unix_now())
}

fn generate_at(secret: &[u8], now: u64) -> Result<GeneratedPin, AppError> {
    // The account name is irrelevant to the code itself; a fixed placeholder
    // keeps the generator's builder happy without leaking a subject in here.
    let totp = generator(secret, "user")?;

    Ok(GeneratedPin {
        pin: totp.generate(now).to_string(),
        valid_for_seconds: STEP_SECONDS - (now % STEP_SECONDS),
    })
}

/// A freshly minted secret, returned to the caller exactly once at provisioning
/// time. `secret_base32` is plaintext and must never be logged or persisted
/// unencrypted.
pub struct ProvisionedSecret {
    pub raw: Vec<u8>,
    pub secret_base32: String,
    pub otpauth_url: String,
}

pub fn provision(account_name: &str) -> Result<ProvisionedSecret, AppError> {
    let secret = Secret::generate();
    let raw = secret.as_bytes().to_vec();

    let totp = generator(&raw, account_name)?;
    let otpauth_url = totp
        .to_url()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("could not build the otpauth URL: {e}")))?;

    Ok(ProvisionedSecret {
        secret_base32: secret.to_base32(),
        otpauth_url,
        raw,
    })
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is before the Unix epoch")
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 6238 Appendix B, HMAC-SHA1: the ASCII secret "12345678901234567890"
    /// at T = 59 yields 94287082 for 8 digits; the low 6 digits are 287082.
    #[test]
    fn matches_the_rfc_6238_test_vector() {
        let secret = b"12345678901234567890";
        assert_eq!(generate_at(secret, 59).unwrap().pin, "287082");
    }

    /// A second vector from the same table: T = 1111111109 → 07081804.
    #[test]
    fn matches_a_later_rfc_6238_test_vector() {
        let secret = b"12345678901234567890";
        assert_eq!(generate_at(secret, 1111111109).unwrap().pin, "081804");
    }

    #[test]
    fn reports_the_time_left_in_the_current_step() {
        let secret = b"12345678901234567890";
        assert_eq!(generate_at(secret, 59).unwrap().valid_for_seconds, 1);
        assert_eq!(generate_at(secret, 60).unwrap().valid_for_seconds, 30);
        assert_eq!(generate_at(secret, 75).unwrap().valid_for_seconds, 15);
    }

    #[test]
    fn holds_the_pin_steady_within_a_step_and_changes_at_the_boundary() {
        let secret = b"12345678901234567890";
        let start = generate_at(secret, 60).unwrap().pin;

        assert_eq!(generate_at(secret, 89).unwrap().pin, start);
        assert_ne!(generate_at(secret, 90).unwrap().pin, start);
    }

    #[test]
    fn always_pads_the_pin_to_six_digits() {
        // A PIN whose numeric value is small must still be six characters, or
        // the lock's fixed-width entry would reject it.
        for t in 0..500u64 {
            let pin = generate_at(b"12345678901234567890", t * 30).unwrap().pin;
            assert_eq!(pin.len(), DIGITS as usize, "short PIN at t={t}: {pin}");
        }
    }

    #[test]
    fn provisions_a_usable_secret_and_url() {
        let provisioned = provision("testuser").unwrap();

        assert!(!provisioned.secret_base32.is_empty());
        assert!(provisioned
            .otpauth_url
            .starts_with("otpauth://totp/urheilulaatikko:testuser?"));
        assert!(provisioned.otpauth_url.contains("issuer=urheilulaatikko"));
        // SHA-1 / 6 digits / 30 s are the Key URI defaults, so they are omitted
        // from the URL rather than spelled out.
        assert!(!provisioned.otpauth_url.contains("algorithm="));

        // The base32 string and the raw bytes must describe the same secret.
        let parsed = Secret::try_from_base32(&provisioned.secret_base32).unwrap();
        assert_eq!(parsed.as_bytes(), provisioned.raw.as_slice());

        assert_eq!(
            generate(&provisioned.raw).unwrap().pin.len(),
            DIGITS as usize
        );
    }

    #[test]
    fn different_secrets_give_different_pins() {
        let a = generate_at(b"12345678901234567890", 59).unwrap().pin;
        let b = generate_at(b"09876543210987654321", 59).unwrap().pin;
        assert_ne!(a, b);
    }
}
