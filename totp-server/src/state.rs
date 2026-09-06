use std::time::Duration;

use anyhow::Result;
use sqlx::PgPool;

use crate::{
    auth::introspection::IntrospectionClient, config::Config, crypto::Crypto,
    totp::secret_store::SecretStore,
};

/// Bound on how long a single introspection call may take. Keycloak sits on the
/// critical path of every request, so a hung connection must surface as a 503
/// rather than hanging the caller.
const INTROSPECTION_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub introspection: IntrospectionClient,
    pub secrets: SecretStore,
}

impl AppState {
    pub fn new(pool: PgPool, config: Config) -> Result<Self> {
        // One client, shared: it owns the connection pool towards Keycloak.
        let http = reqwest::Client::builder()
            .timeout(INTROSPECTION_TIMEOUT)
            .build()?;

        let crypto = Crypto::new(&config.encryption_key);

        Ok(AppState {
            introspection: IntrospectionClient::new(http, &config),
            secrets: SecretStore::new(pool, crypto),
            config,
        })
    }
}
