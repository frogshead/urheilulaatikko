//! RFC 7662 introspection against a mocked Keycloak.
//!
//! These run without Docker: the point is to pin down the contract the client
//! must honour, including the two rules that carry the security weight — never
//! fail open, and always check `aud`.

use serde_json::json;
use totp_server::{auth::introspection::IntrospectionClient, config::Config, error::AppError};
use wiremock::{
    matchers::{body_string_contains, header_exists, method, path},
    Mock, MockServer, ResponseTemplate,
};

const INTROSPECT_PATH: &str = "/protocol/openid-connect/token/introspect";

fn config(issuer: &str) -> Config {
    Config {
        database_url: "postgres://unused".to_string(),
        host: "127.0.0.1".to_string(),
        port: 3000,
        keycloak_issuer: issuer.to_string(),
        keycloak_client_id: "totp-server".to_string(),
        keycloak_client_secret: "dev-secret".to_string(),
        expected_audience: "totp-server".to_string(),
        admin_role: "totp-admin".to_string(),
        encryption_key: [0u8; 32],
        totp_skew_steps: 1,
        rate_limit_max_requests: 6,
        rate_limit_window_seconds: 300,
    }
}

fn client(server: &MockServer) -> IntrospectionClient {
    IntrospectionClient::new(reqwest::Client::new(), &config(&server.uri()))
}

async fn mock_response(template: ResponseTemplate) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(INTROSPECT_PATH))
        .respond_with(template)
        .mount(&server)
        .await;
    server
}

fn active_token(aud: serde_json::Value) -> serde_json::Value {
    json!({
        "active": true,
        "sub": "8f14e45f-ea3a-4b1e-9c7e-000000000001",
        "aud": aud,
        "exp": chrono::Utc::now().timestamp() + 300,
        "realm_access": { "roles": ["default-roles-lock-demo"] },
    })
}

#[tokio::test]
async fn accepts_an_active_token_with_the_expected_audience() {
    let server =
        mock_response(ResponseTemplate::new(200).set_body_json(active_token(json!("totp-server"))))
            .await;

    let user = client(&server).authenticate("a.valid.token").await.unwrap();

    assert_eq!(user.subject, "8f14e45f-ea3a-4b1e-9c7e-000000000001");
    assert!(user
        .realm_roles
        .contains(&"default-roles-lock-demo".to_string()));
}

#[tokio::test]
async fn accepts_an_audience_delivered_as_a_list() {
    let aud = json!(["account", "totp-server"]);
    let server = mock_response(ResponseTemplate::new(200).set_body_json(active_token(aud))).await;

    assert!(client(&server).authenticate("a.valid.token").await.is_ok());
}

#[tokio::test]
async fn sends_client_credentials_and_the_token_as_a_form_field() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(INTROSPECT_PATH))
        // Basic auth with the confidential client's credentials.
        .and(header_exists("authorization"))
        .and(body_string_contains("token=a.valid.token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(active_token(json!("totp-server"))))
        .expect(1)
        .mount(&server)
        .await;

    assert!(client(&server).authenticate("a.valid.token").await.is_ok());
    // Dropping the server asserts the expectation.
}

#[tokio::test]
async fn rejects_an_inactive_token_with_401() {
    let server =
        mock_response(ResponseTemplate::new(200).set_body_json(json!({"active": false}))).await;

    let err = client(&server).authenticate("revoked").await.unwrap_err();
    assert!(matches!(err, AppError::Unauthorized), "got {err:?}");
}

#[tokio::test]
async fn rejects_a_token_for_another_audience_with_403() {
    let body = active_token(json!(["account", "some-other-service"]));
    let server = mock_response(ResponseTemplate::new(200).set_body_json(body)).await;

    let err = client(&server)
        .authenticate("wrong.audience")
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::Forbidden), "got {err:?}");
}

#[tokio::test]
async fn rejects_a_token_with_no_audience_at_all_with_403() {
    let body = json!({
        "active": true,
        "sub": "user-1",
        "exp": chrono::Utc::now().timestamp() + 300,
    });
    let server = mock_response(ResponseTemplate::new(200).set_body_json(body)).await;

    let err = client(&server)
        .authenticate("no.audience")
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::Forbidden), "got {err:?}");
}

#[tokio::test]
async fn rejects_an_expired_token_even_if_keycloak_reports_it_active() {
    let body = json!({
        "active": true,
        "sub": "user-1",
        "aud": "totp-server",
        "exp": chrono::Utc::now().timestamp() - 1,
    });
    let server = mock_response(ResponseTemplate::new(200).set_body_json(body)).await;

    let err = client(&server).authenticate("expired").await.unwrap_err();
    assert!(matches!(err, AppError::Unauthorized), "got {err:?}");
}

#[tokio::test]
async fn returns_503_when_keycloak_errors_rather_than_failing_open() {
    let server = mock_response(ResponseTemplate::new(500)).await;

    let err = client(&server)
        .authenticate("a.valid.token")
        .await
        .unwrap_err();
    assert!(
        matches!(err, AppError::IntrospectionUnavailable(_)),
        "an upstream failure must never be treated as a valid token, got {err:?}"
    );
}

#[tokio::test]
async fn returns_503_when_keycloak_is_unreachable() {
    // Port 1 is privileged, so nothing can be listening there and the connection
    // is refused immediately rather than timing out.
    let client = IntrospectionClient::new(reqwest::Client::new(), &config("http://127.0.0.1:1"));

    let err = client.authenticate("a.valid.token").await.unwrap_err();
    assert!(
        matches!(err, AppError::IntrospectionUnavailable(_)),
        "got {err:?}"
    );
}

#[tokio::test]
async fn returns_503_when_the_response_is_not_valid_json() {
    let server =
        mock_response(ResponseTemplate::new(200).set_body_string("<html>oops</html>")).await;

    let err = client(&server)
        .authenticate("a.valid.token")
        .await
        .unwrap_err();
    assert!(
        matches!(err, AppError::IntrospectionUnavailable(_)),
        "got {err:?}"
    );
}

#[tokio::test]
async fn returns_503_when_the_response_omits_the_subject() {
    let body = json!({
        "active": true,
        "aud": "totp-server",
        "exp": chrono::Utc::now().timestamp() + 300,
    });
    let server = mock_response(ResponseTemplate::new(200).set_body_json(body)).await;

    let err = client(&server).authenticate("no.sub").await.unwrap_err();
    assert!(
        matches!(err, AppError::IntrospectionUnavailable(_)),
        "got {err:?}"
    );
}

/// Concurrent requests bearing the same token must cost Keycloak one call.
/// This is deduplication of in-flight work, not caching — see the note on
/// `IntrospectionClient`.
#[tokio::test]
async fn deduplicates_concurrent_introspections_of_the_same_token() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(INTROSPECT_PATH))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(active_token(json!("totp-server")))
                .set_delay(std::time::Duration::from_millis(150)),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server);
    let results = futures_lite::future::zip(
        futures_lite::future::zip(
            client.authenticate("same.token"),
            client.authenticate("same.token"),
        ),
        futures_lite::future::zip(
            client.authenticate("same.token"),
            client.authenticate("same.token"),
        ),
    )
    .await;

    assert!(results.0 .0.is_ok() && results.0 .1.is_ok());
    assert!(results.1 .0.is_ok() && results.1 .1.is_ok());
}

/// A later request must reach Keycloak again, or a revoked token would keep
/// working for as long as the process lives.
#[tokio::test]
async fn does_not_cache_across_sequential_requests() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(INTROSPECT_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(active_token(json!("totp-server"))))
        .expect(3)
        .mount(&server)
        .await;

    let client = client(&server);
    for _ in 0..3 {
        assert!(client.authenticate("same.token").await.is_ok());
    }
}
