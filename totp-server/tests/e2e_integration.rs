//! End-to-end tests against a real Keycloak and a real Postgres.
//!
//! These need a working Docker daemon, so they are `#[ignore]`d and run with:
//!
//! ```text
//! cargo test --test e2e_integration -- --ignored
//! ```
//!
//! Unlike the wiremock tests, nothing here is faked: tokens are minted by
//! Keycloak from the same realm export the compose environment imports, and the
//! router is the one `main` serves.

use std::{net::SocketAddr, time::Duration};

use serde_json::json;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::{
    core::{Mount, WaitFor},
    runners::AsyncRunner,
    GenericImage, ImageExt,
};
use totp_server::{config::Config, db, routes, state::AppState};

const REALM: &str = "lock-demo";
const CLIENT_SECRET: &str = "dev-secret-change-me";
const USER_PASSWORD: &str = "integration-user-pw";
const ADMIN_PASSWORD: &str = "integration-admin-pw";

/// Everything a test needs: a running server plus the Keycloak behind it.
struct Harness {
    server: String,
    keycloak: String,
    http: reqwest::Client,
    // Held so the containers outlive the test.
    _postgres: testcontainers_modules::testcontainers::ContainerAsync<Postgres>,
    _keycloak: testcontainers_modules::testcontainers::ContainerAsync<GenericImage>,
}

/// Renders the committed realm template with test passwords, exactly as the
/// compose init container does, so both paths import the same realm.
fn render_realm() -> std::path::PathBuf {
    let template = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docker/keycloak/realm-export.template.json"
    ))
    .expect("realm template");

    let rendered = template
        .replace("__TEST_USER_PASSWORD__", USER_PASSWORD)
        .replace("__TEST_ADMIN_PASSWORD__", ADMIN_PASSWORD);

    // Written under the crate directory rather than /tmp: the Docker daemon may
    // not share this process's /tmp, and it has to be able to bind-mount the file.
    let dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/target/it"));
    std::fs::create_dir_all(dir).expect("create the fixture directory");

    let path = dir.join(format!(
        "realm-{}-{:?}.json",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::write(&path, rendered).expect("write rendered realm");
    path
}

impl Harness {
    async fn start() -> Harness {
        Harness::start_with_rate_limit(100).await
    }

    /// `rate_limit` is the per-subject budget; tests that are not about rate
    /// limiting pass a large one so they cannot trip over each other.
    async fn start_with_rate_limit(rate_limit: u32) -> Harness {
        // Same major version as docker-compose runs, so the tests exercise the
        // schema against the database the deployment actually uses.
        let postgres = Postgres::default()
            .with_tag("16-alpine")
            .start()
            .await
            .expect("start postgres");
        let database_url = format!(
            "postgres://postgres:postgres@127.0.0.1:{}/postgres",
            postgres.get_host_port_ipv4(5432).await.unwrap()
        );

        let realm_file = render_realm();
        let keycloak = GenericImage::new("quay.io/keycloak/keycloak", "26.0")
            .with_wait_for(WaitFor::message_on_stdout(
                "Running the server in development mode",
            ))
            .with_cmd(["start-dev", "--import-realm"])
            .with_env_var("KC_BOOTSTRAP_ADMIN_USERNAME", "admin")
            .with_env_var("KC_BOOTSTRAP_ADMIN_PASSWORD", "admin")
            .with_mount(Mount::bind_mount(
                realm_file.to_string_lossy().to_string(),
                "/opt/keycloak/data/import/realm-export.json",
            ))
            .with_startup_timeout(Duration::from_secs(180))
            .start()
            .await
            .expect("start keycloak");

        let keycloak_url = format!(
            "http://127.0.0.1:{}/realms/{REALM}",
            keycloak.get_host_port_ipv4(8080).await.unwrap()
        );

        let config = Config {
            database_url: database_url.clone(),
            host: "127.0.0.1".to_string(),
            port: 0,
            keycloak_issuer: keycloak_url.clone(),
            keycloak_client_id: "totp-server".to_string(),
            keycloak_client_secret: CLIENT_SECRET.to_string(),
            expected_audience: "totp-server".to_string(),
            admin_role: "totp-admin".to_string(),
            encryption_key: [9u8; 32],
            totp_skew_steps: 1,
            rate_limit_max_requests: rate_limit,
            rate_limit_window_seconds: 300,
        };

        let pool = db::create_pool(&database_url).await.expect("connect");
        db::run_migrations(&pool).await.expect("migrate");

        let app = routes::create_router(AppState::new(pool, config).unwrap()).unwrap();
        let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        Harness {
            server: format!("http://{addr}"),
            keycloak: keycloak_url,
            http: reqwest::Client::new(),
            _postgres: postgres,
            _keycloak: keycloak,
        }
    }

    async fn login(&self, client_id: &str, username: &str, password: &str) -> String {
        let body = self
            .http
            .post(format!("{}/protocol/openid-connect/token", self.keycloak))
            .form(&[
                ("grant_type", "password"),
                ("client_id", client_id),
                ("username", username),
                ("password", password),
            ])
            .send()
            .await
            .expect("token request")
            .json::<serde_json::Value>()
            .await
            .expect("token response");

        body["access_token"]
            .as_str()
            .unwrap_or_else(|| panic!("no access token in {body}"))
            .to_string()
    }

    async fn subject_of(&self, token: &str) -> String {
        let body = self
            .http
            .post(format!(
                "{}/protocol/openid-connect/token/introspect",
                self.keycloak
            ))
            .basic_auth("totp-server", Some(CLIENT_SECRET))
            .form(&[("token", token)])
            .send()
            .await
            .expect("introspect")
            .json::<serde_json::Value>()
            .await
            .expect("introspection body");

        body["sub"].as_str().expect("sub claim").to_string()
    }

    async fn post(
        &self,
        path: &str,
        token: Option<&str>,
        body: serde_json::Value,
    ) -> reqwest::Response {
        let mut request = self.http.post(format!("{}{path}", self.server)).json(&body);
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        request.send().await.expect("request to the totp server")
    }
}

/// Independent RFC 6238 implementation, so the assertion does not lean on the
/// same code path it is checking.
fn expected_pin(secret_base32: &str, unix_time: u64) -> String {
    use hmac::{digest::KeyInit, Hmac, Mac};
    use sha1::Sha1;

    let key = base32::decode(base32::Alphabet::Rfc4648 { padding: false }, secret_base32)
        .expect("base32 secret");

    let mut mac = Hmac::<Sha1>::new_from_slice(&key).unwrap();
    mac.update(&(unix_time / 30).to_be_bytes());
    let digest = mac.finalize().into_bytes();

    let offset = (digest[digest.len() - 1] & 0x0f) as usize;
    let value = u32::from_be_bytes(digest[offset..offset + 4].try_into().unwrap()) & 0x7fff_ffff;

    format!("{:06}", value % 1_000_000)
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// The whole flow: admin provisions a secret, the user gets a PIN, and that PIN
/// is the one an independent implementation computes from the same secret.
#[tokio::test]
#[ignore = "requires Docker"]
async fn provisions_a_secret_and_issues_a_matching_pin() {
    let harness = Harness::start().await;

    let admin_token = harness.login("lock-cli", "testadmin", ADMIN_PASSWORD).await;
    let user_token = harness.login("lock-cli", "testuser", USER_PASSWORD).await;
    let subject = harness.subject_of(&user_token).await;

    // Before provisioning there is no secret to hand out.
    let response = harness
        .post("/v1/totp/request", Some(&user_token), json!({}))
        .await;
    assert_eq!(response.status(), 404);

    let response = harness
        .post(
            "/v1/totp/provision",
            Some(&admin_token),
            json!({ "subject": subject }),
        )
        .await;
    assert_eq!(response.status(), 200);
    let provisioned: serde_json::Value = response.json().await.unwrap();
    let secret_base32 = provisioned["secret_base32"].as_str().unwrap().to_string();
    assert!(provisioned["otpauth_url"]
        .as_str()
        .unwrap()
        .starts_with("otpauth://totp/"));

    let before = unix_now();
    let response = harness
        .post("/v1/totp/request", Some(&user_token), json!({}))
        .await;
    assert_eq!(response.status(), 200);
    let issued: serde_json::Value = response.json().await.unwrap();
    let after = unix_now();

    let pin = issued["pin"].as_str().unwrap();
    assert_eq!(pin.len(), 6);
    // Accept either step if the request straddled a boundary.
    assert!(
        pin == expected_pin(&secret_base32, before) || pin == expected_pin(&secret_base32, after),
        "server PIN {pin} matches neither step around the request"
    );

    let valid_for = issued["valid_for_seconds"].as_u64().unwrap();
    assert!((1..=30).contains(&valid_for), "implausible ttl {valid_for}");

    // Missing and malformed credentials are refused outright.
    assert_eq!(
        harness
            .post("/v1/totp/request", None, json!({}))
            .await
            .status(),
        401
    );
    assert_eq!(
        harness
            .post("/v1/totp/request", Some("not-a-token"), json!({}))
            .await
            .status(),
        401
    );

    // A token that is genuinely valid in this realm but was issued to a client
    // without the `totp-access` scope must still be refused. Without the
    // mandatory audience check this would be a 200.
    let wrong_audience = harness
        .login("no-audience-cli", "testuser", USER_PASSWORD)
        .await;
    assert_eq!(
        harness
            .post("/v1/totp/request", Some(&wrong_audience), json!({}))
            .await
            .status(),
        403
    );

    // Provisioning is gated on the admin realm role, not merely on being logged in.
    assert_eq!(
        harness
            .post(
                "/v1/totp/provision",
                Some(&user_token),
                json!({ "subject": subject }),
            )
            .await
            .status(),
        403
    );
}

/// Requests past the configured budget must be refused rather than served.
#[tokio::test]
#[ignore = "requires Docker"]
async fn enforces_the_per_subject_rate_limit() {
    const LIMIT: u32 = 6;
    let harness = Harness::start_with_rate_limit(LIMIT).await;

    let admin_token = harness.login("lock-cli", "testadmin", ADMIN_PASSWORD).await;
    let user_token = harness.login("lock-cli", "testuser", USER_PASSWORD).await;
    let subject = harness.subject_of(&user_token).await;

    harness
        .post(
            "/v1/totp/provision",
            Some(&admin_token),
            json!({ "subject": subject }),
        )
        .await;

    let mut statuses = Vec::new();
    for _ in 0..(LIMIT + 2) {
        statuses.push(
            harness
                .post("/v1/totp/request", Some(&user_token), json!({}))
                .await
                .status()
                .as_u16(),
        );
    }

    assert_eq!(
        statuses.iter().filter(|s| **s == 200).count(),
        LIMIT as usize,
        "expected exactly {LIMIT} requests to be served: {statuses:?}"
    );
    assert!(
        statuses.iter().rev().take(2).all(|s| *s == 429),
        "the requests past the limit were not refused: {statuses:?}"
    );
}
