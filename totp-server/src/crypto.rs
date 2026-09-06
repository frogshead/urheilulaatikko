use aes_gcm::{
    aead::{Aead, Generate, KeyInit, Nonce},
    Aes256Gcm, Key,
};

use crate::error::AppError;

/// AES-256-GCM encryption of TOTP secrets at rest.
///
/// The key comes from `TOTP_ENCRYPTION_KEY` and is validated at startup. In
/// production it belongs in a vault/KMS rather than an environment variable —
/// see the README.
#[derive(Clone)]
pub struct Crypto {
    cipher: Aes256Gcm,
}

impl std::fmt::Debug for Crypto {
    /// Deliberately opaque so the key can never reach a log line via `{:?}`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Crypto(<redacted>)")
    }
}

impl Crypto {
    pub fn new(key: &[u8; 32]) -> Self {
        let key = Key::<Aes256Gcm>::from(*key);
        Crypto {
            cipher: Aes256Gcm::new(&key),
        }
    }

    /// Encrypts `plaintext`, returning `(ciphertext, nonce)`.
    ///
    /// A fresh random 96-bit nonce is drawn per call — reusing one under the
    /// same key would break GCM's confidentiality *and* its authentication.
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<(Vec<u8>, Vec<u8>), AppError> {
        let nonce = Nonce::<Aes256Gcm>::generate();
        let ciphertext = self
            .cipher
            .encrypt(&nonce, plaintext)
            .map_err(|_| AppError::Internal(anyhow::anyhow!("failed to encrypt secret")))?;
        Ok((ciphertext, nonce.to_vec()))
    }

    pub fn decrypt(&self, ciphertext: &[u8], nonce: &[u8]) -> Result<Vec<u8>, AppError> {
        let nonce: &Nonce<Aes256Gcm> = nonce.try_into().map_err(|_| {
            AppError::Internal(anyhow::anyhow!(
                "stored nonce has the wrong length: {}",
                nonce.len()
            ))
        })?;

        self.cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| AppError::Internal(anyhow::anyhow!("failed to decrypt stored secret")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crypto() -> Crypto {
        Crypto::new(&[42u8; 32])
    }

    #[test]
    fn roundtrips_a_secret() {
        let crypto = crypto();
        let secret = b"JBSWY3DPEHPK3PXP";

        let (ciphertext, nonce) = crypto.encrypt(secret).unwrap();
        assert_ne!(ciphertext.as_slice(), secret.as_slice());
        assert_eq!(crypto.decrypt(&ciphertext, &nonce).unwrap(), secret);
    }

    #[test]
    fn draws_a_fresh_nonce_for_every_call() {
        let crypto = crypto();
        let (first_ct, first_nonce) = crypto.encrypt(b"same input").unwrap();
        let (second_ct, second_nonce) = crypto.encrypt(b"same input").unwrap();

        assert_ne!(first_nonce, second_nonce);
        assert_ne!(first_ct, second_ct);
    }

    #[test]
    fn rejects_tampered_ciphertext() {
        let crypto = crypto();
        let (mut ciphertext, nonce) = crypto.encrypt(b"JBSWY3DPEHPK3PXP").unwrap();
        ciphertext[0] ^= 0xff;

        assert!(crypto.decrypt(&ciphertext, &nonce).is_err());
    }

    #[test]
    fn rejects_a_different_key() {
        let (ciphertext, nonce) = crypto().encrypt(b"JBSWY3DPEHPK3PXP").unwrap();
        let other = Crypto::new(&[43u8; 32]);

        assert!(other.decrypt(&ciphertext, &nonce).is_err());
    }

    #[test]
    fn rejects_a_nonce_of_the_wrong_length() {
        let crypto = crypto();
        let (ciphertext, _) = crypto.encrypt(b"secret").unwrap();

        assert!(crypto.decrypt(&ciphertext, &[0u8; 4]).is_err());
    }
}
