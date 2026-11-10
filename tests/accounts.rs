//! Signing up, verifying an address and signing in, through the panel's own forms.

mod support;

use support::harness;
use tablo::{
    auth::AuthThrottle,
    testing::{TestClient, form_body},
};
use tablo_saas::{accounts::User, jobs::JobRow};
use topcoat::router::Body;
use topcoat::router::header::LOCATION;

/// Posts the sign-up form, with its CSRF pair.
async fn sign_up(client: &TestClient<'_>, email: &str) -> http::Response<Body> {
    client
        .submit(
            "/app/register",
            &form_body(&[
                ("name", "Ada Lovelace"),
                ("email", email),
                ("password", "correct horse battery staple"),
                ("password_confirmation", "correct horse battery staple"),
            ]),
        )
        .await
}

#[tokio::test]
async fn a_sign_up_stores_a_lowercase_address_and_mails_a_verification_link() {
    let harness = harness().await;
    let client = TestClient::new(&harness.app);
    let response = sign_up(&client, "Ada@Example.COM").await;
    assert!(response.status().is_redirection(), "{}", response.status());

    let user = User::filter(User::fields().email().eq("ada@example.com"))
        .first()
        .exec(&mut harness.db.clone())
        .await
        .expect("query")
        .expect("the account exists");
    assert!(user.email_verified_at.is_none());
    assert!(user.staff_role.is_none(), "a sign-up is never staff");

    let sent = harness.mail(1).await;
    assert_eq!(sent[0].to()[0].address(), "ada@example.com");
}

#[tokio::test]
async fn auth_mail_waits_in_the_queue_sealed_until_the_worker_sends_it() {
    let harness = harness().await;
    let client = TestClient::new(&harness.app);
    sign_up(&client, "ada@example.com").await;

    let mut queued = Vec::new();
    for _ in 0..200 {
        queued = JobRow::filter(JobRow::fields().kind().eq("send-mail"))
            .exec(&mut harness.db.clone())
            .await
            .expect("query");
        if !queued.is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert_eq!(queued.len(), 1, "the verification mail is queued");
    assert!(harness.outbox.sent().is_empty(), "the worker sends it");
    let payload = &queued[0].payload;
    assert!(!payload.contains("ada@example.com"), "{payload}");
    assert!(!payload.contains("token"), "{payload}");

    let sent = harness.mail(1).await;
    assert_eq!(sent[0].to()[0].address(), "ada@example.com");
    assert!(sent[0].text().expect("a text body").contains("token="));
}

#[tokio::test]
async fn an_address_is_taken_whatever_its_case() {
    let harness = harness().await;
    let first = TestClient::new(&harness.app);
    sign_up(&first, "ada@example.com").await;

    let second = TestClient::new(&harness.app);
    let response = sign_up(&second, "ADA@example.com").await;
    let count = User::all()
        .exec(&mut harness.db.clone())
        .await
        .expect("query")
        .len();
    assert_eq!(count, 1, "{}", response.status());
}

#[tokio::test]
async fn an_unverified_account_is_sent_to_verify_before_anything_else() {
    let harness = harness().await;
    let client = TestClient::new(&harness.app);
    let response = sign_up(&client, "ada@example.com").await;
    let cookies = tablo::testing::response_cookies(&response);
    let response = client.cookies(&cookies).get("/app").await;
    assert_eq!(
        response.headers()[LOCATION].to_str().expect("ascii"),
        "/app/verify-email"
    );
}

/// Sign-in attempts count in the database, where every replica reads them: per address and per
/// account, and never under the address in the clear.
#[tokio::test]
async fn a_failed_sign_in_is_counted_where_every_replica_sees_it() {
    let harness = harness().await;
    let client = TestClient::new(&harness.app);
    let response = client
        .submit(
            "/app/login",
            &form_body(&[("email", "ada@example.com"), ("password", "wrong")]),
        )
        .await;
    assert!(response.status().is_client_error(), "{}", response.status());

    let rows = AuthThrottle::all()
        .exec(&mut harness.db.clone())
        .await
        .expect("query");
    let mut scopes: Vec<_> = rows.iter().map(|row| row.scope.as_str()).collect();
    scopes.sort_unstable();
    assert_eq!(scopes, ["/app:account", "/app:sign-in"]);
    assert!(rows.iter().all(|row| !row.key.contains("ada")));
}
