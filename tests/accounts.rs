//! Signing up, verifying an address and signing in, through the panel's own forms.

mod support;

use support::harness;
use tablo::testing::{TestClient, form_body};
use tablo_saas::accounts::User;
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
