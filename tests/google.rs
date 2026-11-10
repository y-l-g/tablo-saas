//! Signing in with Google: a new address signs up, a known one links, a linked identity returns,
//! and an address Google has not verified signs nobody in.

mod support;

use support::{GOOGLE_CLIENT, Harness, google_harness, person};
use tablo::{
    auth::{AuthSession, hash_password},
    testing::{
        FakeAccount, FakeProvider, TestClient, provider_buttons, response_cookies,
        session_cookie_value,
    },
};
use tablo_saas::{
    accounts::{Accounts, Identity, User},
    teams::{Member, Team, create_team},
};
use topcoat::router::header::LOCATION;

/// Google, stood in for on a loopback port.
async fn google() -> FakeProvider {
    FakeProvider::start(GOOGLE_CLIENT.0, GOOGLE_CLIENT.1).await
}

/// Signs `account` in from the login page's Google button: where the browser lands, and the
/// session it holds after, if any.
async fn continue_with(
    harness: &Harness,
    google: &FakeProvider,
    account: &FakeAccount,
) -> (String, Option<String>) {
    let client = TestClient::new(&harness.app);
    let page = tablo::testing::body_string(client.get("/app/login").await).await;
    let buttons = provider_buttons(&page);
    assert_eq!(buttons.len(), 1, "one Google button");
    let left = client.get(&buttons[0].1).await;
    assert_eq!(left.status(), 303);
    let location = left.headers()[LOCATION].to_str().unwrap().to_string();
    let callback = google.authorize(&location, account);
    let landed = TestClient::new(&harness.app)
        .cookies(&response_cookies(&left))
        .get(&callback)
        .await;
    assert_eq!(landed.status(), 303);
    (
        landed.headers()[LOCATION].to_str().unwrap().to_string(),
        session_cookie_value(&landed),
    )
}

async fn user_by_email(harness: &Harness, email: &str) -> Option<User> {
    User::filter(User::fields().email().eq(email))
        .first()
        .exec(&mut harness.db.clone())
        .await
        .expect("query")
}

async fn identities(harness: &Harness) -> Vec<Identity> {
    Identity::all()
        .exec(&mut harness.db.clone())
        .await
        .expect("query")
}

#[tokio::test]
async fn a_new_verified_address_signs_up_with_a_personal_team() {
    let google = google().await;
    let harness = google_harness(&google).await;
    let account = FakeAccount {
        name: Some("Ada Lovelace".to_string()),
        ..FakeAccount::verified("ada-1", "Ada@Example.com")
    };
    let (landed, session) = continue_with(&harness, &google, &account).await;
    assert_eq!(landed, "/app");
    assert!(session.is_some());

    let ada = user_by_email(&harness, "ada@example.com")
        .await
        .expect("the account exists");
    assert_eq!(ada.name, "Ada Lovelace");
    assert!(ada.email_verified_at.is_some(), "Google proved the address");
    assert!(ada.staff_role.is_none());
    let seats = Member::filter(Member::fields().user_id().eq(ada.id))
        .exec(&mut harness.db.clone())
        .await
        .expect("query");
    assert_eq!(seats.len(), 1, "a personal team");
    let links = identities(&harness).await;
    assert_eq!(links.len(), 1);
    assert_eq!(
        (
            links[0].user_id,
            links[0].issuer.as_str(),
            links[0].subject.as_str()
        ),
        (ada.id, google.issuer(), "ada-1")
    );
}

/// The identity, not the address, names the account once linked: a changed address at Google
/// still signs in as the same user.
#[tokio::test]
async fn a_linked_identity_returns_to_its_account() {
    let google = google().await;
    let harness = google_harness(&google).await;
    continue_with(
        &harness,
        &google,
        &FakeAccount::verified("ada-1", "ada@example.com"),
    )
    .await;
    let (_, session) = continue_with(
        &harness,
        &google,
        &FakeAccount::verified("ada-1", "ada@new.example"),
    )
    .await;
    assert!(session.is_some());
    assert!(user_by_email(&harness, "ada@new.example").await.is_none());
    assert_eq!(identities(&harness).await.len(), 1);
    let users = User::all()
        .exec(&mut harness.db.clone())
        .await
        .expect("query");
    assert_eq!(users.len(), 1);
}

#[tokio::test]
async fn a_verified_address_links_the_account_holding_it() {
    let google = google().await;
    let harness = google_harness(&google).await;
    let (ada, _) = person(&harness.db, "Ada", "ada@example.com").await;
    let (_, session) = continue_with(
        &harness,
        &google,
        &FakeAccount::verified("ada-1", "ada@example.com"),
    )
    .await;
    assert!(session.is_some());
    let links = identities(&harness).await;
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].user_id, ada.id);
}

/// An account whose owner never proved the address may be a squatter's, waiting: Google's proof
/// erases it, with its name, its team and its sessions, and signs the owner up afresh.
#[tokio::test]
async fn an_unproved_account_gives_way_to_whoever_google_says_holds_the_address() {
    let google = google().await;
    let harness = google_harness(&google).await;
    let squatted = toasty::create!(User {
        name: "Not Ada".to_string(),
        email: "ada@example.com".to_string(),
        password: hash_password("the squatter's password")
            .await
            .expect("hash"),
    })
    .exec(&mut harness.db.clone())
    .await
    .expect("write the account");
    create_team(&mut harness.db.clone(), "Not Ada's team", &squatted)
        .await
        .expect("the squatter's team");
    let signed = Accounts::signed_in(squatted.clone(), &mut harness.db.clone())
        .await
        .expect("load the account");
    let _squatter = TestClient::new(&harness.app)
        .sign_in(&harness.db, "app", &signed)
        .await;

    let account = FakeAccount {
        name: Some("Ada Lovelace".to_string()),
        ..FakeAccount::verified("ada-1", "ada@example.com")
    };
    let (_, session) = continue_with(&harness, &google, &account).await;
    assert!(session.is_some());
    let gone = User::filter_by_id(squatted.id)
        .first()
        .exec(&mut harness.db.clone())
        .await
        .expect("query");
    assert!(gone.is_none(), "the unproved account is erased");
    let ada = user_by_email(&harness, "ada@example.com")
        .await
        .expect("the owner signed up");
    assert_eq!(ada.name, "Ada Lovelace");
    let teams = Team::all()
        .exec(&mut harness.db.clone())
        .await
        .expect("query");
    assert_eq!(teams.len(), 1, "the squatter's team went with the account");
    assert!(teams[0].name.contains("Ada Lovelace"), "{}", teams[0].name);
    let sessions = AuthSession::all()
        .exec(&mut harness.db.clone())
        .await
        .expect("query");
    assert_eq!(
        sessions.len(),
        1,
        "only the session Google's sign-in started"
    );
    assert_eq!(sessions[0].user_id, ada.id.to_string());
}

#[tokio::test]
async fn an_address_google_has_not_verified_signs_nobody_in() {
    let google = google().await;
    let harness = google_harness(&google).await;
    let account = FakeAccount {
        email_verified: false,
        ..FakeAccount::verified("mallory", "ada@example.com")
    };
    let (landed, session) = continue_with(&harness, &google, &account).await;
    assert!(landed.starts_with("/app/login"), "{landed}");
    assert!(session.is_none());
    assert!(user_by_email(&harness, "ada@example.com").await.is_none());
    assert!(identities(&harness).await.is_empty());
}

#[tokio::test]
async fn without_google_settings_the_login_page_offers_no_button() {
    let harness = support::harness().await;
    let page =
        tablo::testing::body_string(TestClient::new(&harness.app).get("/app/login").await).await;
    assert_eq!(provider_buttons(&page), Vec::<(String, String)>::new());
}
