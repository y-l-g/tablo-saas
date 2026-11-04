//! The probes and the headers every response carries.

use tablo::testing::{TestClient, test_db};
use tablo_saas::{platform::config::Config, router::router};
use topcoat::router::{StatusCode, header};

async fn app() -> topcoat::router::Router {
    let config =
        Config::load(&|name| (name == "DATABASE_URL").then(|| "sqlite::memory:".to_string()))
            .expect("config");
    router(test_db(tablo_saas::models()).await, &config)
}

#[tokio::test]
async fn liveness_answers_without_the_database() {
    let app = app().await;
    let response = TestClient::new(&app).get("/healthz").await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn readiness_asks_the_database() {
    let app = app().await;
    let response = TestClient::new(&app).get("/readyz").await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn a_public_page_carries_a_request_id_and_hardening_headers() {
    let app = app().await;
    let response = TestClient::new(&app).get("/").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().contains_key("x-request-id"));
    assert_eq!(
        response.headers()[header::X_CONTENT_TYPE_OPTIONS],
        "nosniff"
    );
}
