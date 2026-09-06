use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::sync::broadcast;

use crate::{auth::AuthenticatedUser, config::Config, error::AppError};

/// RFC 7662 token introspection against Keycloak.
///
/// Introspection is used instead of local JWT signature verification so that a
/// token revoked in Keycloak stops working immediately. That makes every request
/// depend on Keycloak being reachable, which is a deliberate trade: see
/// [`IntrospectionOutcome::Unavailable`] — this client never fails open.
#[derive(Clone)]
pub struct IntrospectionClient {
    http: reqwest::Client,
    url: String,
    client_id: String,
    client_secret: String,
    expected_audience: String,
    /// In-flight requests keyed by token hash, so N concurrent requests bearing
    /// the same token cost Keycloak one introspection instead of N. This is
    /// deduplication, not caching: nothing outlives the request it piggybacks
    /// on, so revocation still takes effect on the very next request.
    inflight: Arc<Mutex<HashMap<[u8; 32], broadcast::Sender<IntrospectionOutcome>>>>,
}

/// Cloneable result so it can be broadcast to everyone waiting on one call.
#[derive(Clone, Debug)]
enum IntrospectionOutcome {
    Active(AuthenticatedUser),
    /// Token missing, expired, or `active: false`.
    Inactive,
    /// Token is valid but its `aud` does not name this server.
    WrongAudience,
    /// Keycloak unreachable, erroring, or answering nonsense.
    Unavailable(String),
}

impl From<IntrospectionOutcome> for Result<AuthenticatedUser, AppError> {
    fn from(outcome: IntrospectionOutcome) -> Self {
        match outcome {
            IntrospectionOutcome::Active(user) => Ok(user),
            IntrospectionOutcome::Inactive => Err(AppError::Unauthorized),
            IntrospectionOutcome::WrongAudience => Err(AppError::Forbidden),
            IntrospectionOutcome::Unavailable(reason) => Err(AppError::IntrospectionUnavailable(
                anyhow::anyhow!("{reason}"),
            )),
        }
    }
}

#[derive(Debug, Deserialize)]
struct IntrospectionResponse {
    active: bool,
    sub: Option<String>,
    aud: Option<Audience>,
    exp: Option<i64>,
    realm_access: Option<RealmAccess>,
}

/// Keycloak emits `aud` as a bare string when there is one audience and as an
/// array when there are several. RFC 7662 permits both.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Audience {
    One(String),
    Many(Vec<String>),
}

impl Audience {
    fn contains(&self, expected: &str) -> bool {
        match self {
            Audience::One(value) => value == expected,
            Audience::Many(values) => values.iter().any(|v| v == expected),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RealmAccess {
    #[serde(default)]
    roles: Vec<String>,
}

/// Removes the in-flight entry when the leading request finishes *or* is
/// cancelled, so a dropped leader cannot wedge later requests for that token.
struct InflightGuard {
    key: [u8; 32],
    map: Arc<Mutex<HashMap<[u8; 32], broadcast::Sender<IntrospectionOutcome>>>>,
}

impl Drop for InflightGuard {
    fn drop(&mut self) {
        if let Ok(mut map) = self.map.lock() {
            map.remove(&self.key);
        }
    }
}

impl IntrospectionClient {
    pub fn new(http: reqwest::Client, config: &Config) -> Self {
        IntrospectionClient {
            http,
            url: config.introspection_url(),
            client_id: config.keycloak_client_id.clone(),
            client_secret: config.keycloak_client_secret.clone(),
            expected_audience: config.expected_audience.clone(),
            inflight: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn authenticate(&self, token: &str) -> Result<AuthenticatedUser, AppError> {
        self.introspect(token).await.into()
    }

    async fn introspect(&self, token: &str) -> IntrospectionOutcome {
        let key: [u8; 32] = Sha256::digest(token.as_bytes()).into();

        // Either become the leader for this token or subscribe to the leader's result.
        let follower = {
            let mut inflight = self.inflight.lock().expect("inflight mutex poisoned");
            match inflight.get(&key) {
                Some(tx) => Some(tx.subscribe()),
                None => {
                    let (tx, _rx) = broadcast::channel(1);
                    inflight.insert(key, tx);
                    None
                }
            }
        };

        if let Some(mut rx) = follower {
            match rx.recv().await {
                Ok(outcome) => return outcome,
                // The leader was cancelled before publishing; fall through and
                // do the call ourselves rather than failing the request.
                Err(_) => return self.call(token).await,
            }
        }

        let guard = InflightGuard {
            key,
            map: Arc::clone(&self.inflight),
        };
        let outcome = self.call(token).await;

        if let Ok(inflight) = self.inflight.lock() {
            if let Some(tx) = inflight.get(&key) {
                // Fails only when nobody is waiting, which is the common case.
                let _ = tx.send(outcome.clone());
            }
        }
        drop(guard);

        outcome
    }

    async fn call(&self, token: &str) -> IntrospectionOutcome {
        let response = self
            .http
            .post(&self.url)
            .basic_auth(&self.client_id, Some(&self.client_secret))
            .form(&[("token", token)])
            .send()
            .await;

        // Network failure. Never fail open: without a positive answer from
        // Keycloak the request is rejected with 503.
        let response = match response {
            Ok(response) => response,
            Err(err) => {
                return IntrospectionOutcome::Unavailable(format!(
                    "could not reach the introspection endpoint: {err}"
                ))
            }
        };

        let status = response.status();
        if !status.is_success() {
            return IntrospectionOutcome::Unavailable(format!(
                "introspection endpoint returned HTTP {status}"
            ));
        }

        let body: IntrospectionResponse = match response.json().await {
            Ok(body) => body,
            Err(err) => {
                return IntrospectionOutcome::Unavailable(format!(
                    "could not parse the introspection response: {err}"
                ))
            }
        };

        self.evaluate(body)
    }

    fn evaluate(&self, body: IntrospectionResponse) -> IntrospectionOutcome {
        if !body.active {
            return IntrospectionOutcome::Inactive;
        }

        // Keycloak already enforces expiry, but a stale or misconfigured
        // introspection endpoint must not be able to extend a token's life.
        if let Some(exp) = body.exp {
            if exp <= chrono::Utc::now().timestamp() {
                return IntrospectionOutcome::Inactive;
            }
        }

        // Mandatory, not optional: without it any token issued by the realm —
        // including one minted for an unrelated client — would unlock a PIN.
        let audience_matches = body
            .aud
            .as_ref()
            .is_some_and(|aud| aud.contains(&self.expected_audience));
        if !audience_matches {
            return IntrospectionOutcome::WrongAudience;
        }

        let Some(subject) = body.sub else {
            return IntrospectionOutcome::Unavailable(
                "introspection response omitted the `sub` claim".to_string(),
            );
        };

        IntrospectionOutcome::Active(AuthenticatedUser {
            subject,
            realm_roles: body.realm_access.map(|r| r.roles).unwrap_or_default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client(expected_audience: &str) -> IntrospectionClient {
        IntrospectionClient {
            http: reqwest::Client::new(),
            url: "http://unused".to_string(),
            client_id: "totp-server".to_string(),
            client_secret: "secret".to_string(),
            expected_audience: expected_audience.to_string(),
            inflight: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn response(active: bool, aud: Option<Audience>, exp: Option<i64>) -> IntrospectionResponse {
        IntrospectionResponse {
            active,
            sub: Some("user-1".to_string()),
            aud,
            exp,
            realm_access: None,
        }
    }

    #[test]
    fn accepts_an_audience_given_as_a_bare_string() {
        let outcome = client("totp-server").evaluate(response(
            true,
            Some(Audience::One("totp-server".to_string())),
            None,
        ));
        assert!(matches!(outcome, IntrospectionOutcome::Active(_)));
    }

    #[test]
    fn accepts_an_audience_given_as_a_list() {
        let aud = Audience::Many(vec!["account".to_string(), "totp-server".to_string()]);
        let outcome = client("totp-server").evaluate(response(true, Some(aud), None));
        assert!(matches!(outcome, IntrospectionOutcome::Active(_)));
    }

    #[test]
    fn rejects_a_missing_audience() {
        let outcome = client("totp-server").evaluate(response(true, None, None));
        assert!(matches!(outcome, IntrospectionOutcome::WrongAudience));
    }

    #[test]
    fn rejects_an_expired_token_even_when_keycloak_says_active() {
        let past = chrono::Utc::now().timestamp() - 1;
        let aud = Audience::One("totp-server".to_string());
        let outcome = client("totp-server").evaluate(response(true, Some(aud), Some(past)));
        assert!(matches!(outcome, IntrospectionOutcome::Inactive));
    }

    #[test]
    fn rejects_an_inactive_token() {
        let aud = Audience::One("totp-server".to_string());
        let outcome = client("totp-server").evaluate(response(false, Some(aud), None));
        assert!(matches!(outcome, IntrospectionOutcome::Inactive));
    }
}
