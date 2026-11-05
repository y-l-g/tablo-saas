//! The probes and the headers every response carries.

mod support;

use support::{client_for, harness, person};
use tablo::testing::TestClient;
use topcoat::router::{StatusCode, header};

#[tokio::test]
async fn liveness_answers_without_the_database() {
    let harness = harness().await;
    let response = TestClient::new(&harness.app).get("/healthz").await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn readiness_asks_the_database() {
    let harness = harness().await;
    let response = TestClient::new(&harness.app).get("/readyz").await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn a_public_page_carries_a_request_id_and_hardening_headers() {
    let harness = harness().await;
    let response = TestClient::new(&harness.app).get("/").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().contains_key("x-request-id"));
    assert_eq!(
        response.headers()[header::X_CONTENT_TYPE_OPTIONS],
        "nosniff"
    );
}

#[tokio::test]
async fn a_panel_failure_renders_the_panel_error_page() {
    let harness = harness().await;
    let (ada, team) = person(&harness.db, "Ada", "ada@example.com").await;
    let client = client_for(&harness, &ada, &team).await;
    let response = client.get("/app/no-such-page").await;
    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "the request layer lets the panel's rewrite through"
    );
}
