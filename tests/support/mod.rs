//! What the integration tests share: the app on a fresh database, with mail caught in memory.
#![allow(dead_code)]

pub mod stripe;

use std::sync::Arc;

use stripe::FakeStripe;
use tablo::{
    PanelHandle,
    testing::{
        FakeProvider, TEST_DATABASE_URL, TestClient, authenticator_code, body_string, form_body,
        migrated_db, response_cookies, test_db, two_factor_secret,
    },
};
use tablo_saas::{
    accounts::{Accounts, StaffRole, User},
    background,
    billing::Billing,
    jobs::{MailSeal, Worker},
    platform::{config::Config, database},
    router::{App, router},
    teams::{Member, Team, TeamRole, create_team},
};
use toasty::Db;
use topcoat::{mail::MemoryTransport, router::Router};

/// A running app and what a test inspects.
pub struct Harness {
    pub app: Router,
    pub panel: PanelHandle,
    pub admin_panel: PanelHandle,
    pub db: Db,
    pub outbox: MemoryTransport,
    worker: Arc<Worker>,
}

/// The development configuration, signing in with Google through `google` when given.
fn config(google: Option<&FakeProvider>) -> Config {
    Config::load(&|name| match name {
        "DATABASE_URL" => Some("sqlite::memory:".to_string()),
        "GOOGLE_CLIENT_ID" => google.map(|_| GOOGLE_CLIENT.0.to_string()),
        "GOOGLE_CLIENT_SECRET" => google.map(|_| GOOGLE_CLIENT.1.to_string()),
        "GOOGLE_ISSUER" => google.map(|fake| fake.issuer().to_string()),
        _ => None,
    })
    .expect("a development configuration needs only a database URL")
}

/// The OAuth client a [`google_harness`]'s stand-in for Google knows: its id and secret.
pub const GOOGLE_CLIENT: (&str, &str) = ("google-client", "google-secret");

/// The webhook secret a test signs with.
pub const WEBHOOK_SECRET: &str = "whsec_test";

/// The app over empty tables, with billing off.
pub async fn harness() -> Harness {
    build(Billing::new(None).expect("billing off"), None).await
}

/// The app over empty tables, with billing off, signing in with Google through `google`, a
/// stand-in started with [`GOOGLE_CLIENT`].
pub async fn google_harness(google: &FakeProvider) -> Harness {
    build(Billing::new(None).expect("billing off"), Some(google)).await
}

/// The app over empty tables, billing through `stripe`.
pub async fn billed_harness(stripe: &FakeStripe) -> Harness {
    build(
        Billing::with_gateway(Arc::new(stripe.clone()), WEBHOOK_SECRET),
        None,
    )
    .await
}

/// The tables the migrations made, foreign keys and checks included, on the PostgreSQL database
/// `TABLO_TEST_DATABASE_URL` names. The migrations are PostgreSQL's, so in-memory SQLite gets
/// the tables pushed from the models instead, without what only a migration states.
async fn database() -> Db {
    if std::env::var(TEST_DATABASE_URL).is_ok() {
        migrated_db(tablo_saas::models(), &database::MIGRATIONS).await
    } else {
        test_db(tablo_saas::models()).await
    }
}

async fn build(billing: Billing, google: Option<&FakeProvider>) -> Harness {
    let db = database().await;
    let outbox = MemoryTransport::new();
    let config = config(google);
    let built = router(App {
        db: db.clone(),
        config: &config,
        mail: outbox.clone(),
        billing: billing.clone(),
        assets: None,
    })
    .expect("the panels mount");
    let panel = built.app_panel.clone();
    let worker = background::worker(
        db.clone(),
        &built.app_panel,
        config.settings(),
        billing,
        outbox.clone(),
        MailSeal::new(&config.cookie_key),
    );
    Harness {
        app: built.router,
        panel,
        admin_panel: built.admin_panel,
        db,
        outbox,
        worker: Arc::new(worker),
    }
}

impl Harness {
    /// Runs every job due now, as a worker would.
    pub async fn run_jobs(&mut self) {
        self.drain().await;
    }

    async fn drain(&self) {
        while Arc::clone(&self.worker)
            .drain_once()
            .await
            .expect("the queue runs")
            > 0
        {}
    }

    /// What the outbox holds once `count` mails arrived, running the queue meanwhile. Tablo queues
    /// its auth mail after the response, so a test waits for it rather than racing it.
    pub async fn mail(&self, count: usize) -> Vec<topcoat::mail::Mail> {
        for _ in 0..200 {
            self.drain().await;
            let sent = self.outbox.sent();
            if sent.len() >= count {
                return sent;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        panic!("expected {count} mails, got {}", self.outbox.sent().len());
    }
}

/// A verified account with its personal team, ready for `TestClient::sign_in`.
pub async fn person(db: &Db, name: &str, email: &str) -> (User, Team) {
    let mut handle = db.clone();
    let user = toasty::create!(User {
        name: name.to_string(),
        email: email.to_string(),
        password: "unused".to_string(),
        email_verified_at: Some(jiff::Timestamp::now()),
    })
    .exec(&mut handle)
    .await
    .expect("write the account");
    let team = create_team(&mut handle, &format!("{name}'s team"), &user)
        .await
        .expect("write the team");
    (user, team)
}

/// Puts `user` on `team`.
pub async fn seat(db: &Db, user: &User, team: &Team, role: TeamRole) -> Member {
    toasty::create!(Member {
        team_id: team.tenant(),
        user_id: user.id,
        role: role,
    })
    .exec(&mut db.clone())
    .await
    .expect("write the seat")
}

/// A client signed in as `user`, on `team`.
pub async fn client_for<'a>(harness: &'a Harness, user: &User, team: &Team) -> TestClient<'a> {
    let signed = Accounts::signed_in(user.clone(), &mut harness.db.clone())
        .await
        .expect("load the account");
    TestClient::new(&harness.app)
        .panel(&harness.panel)
        .sign_in(&harness.db, "app", &signed)
        .await
        .tenant(team.tenant())
}

/// A staff client past the back office's second factor, enrolled through the real form.
pub async fn staff_client<'a>(
    harness: &'a Harness,
    email: &str,
    role: StaffRole,
) -> TestClient<'a> {
    const PASSWORD: &str = "correct horse battery staple";
    const CSRF: &str = "3f0f6d3a-8f43-4f4d-9c59-2d3b3b1f4a11";
    let (user, _) = person(&harness.db, "Staff", email).await;
    User::filter_by_id(user.id)
        .update()
        .staff_role(Some(role))
        .password(tablo::auth::hash_password(PASSWORD).await.expect("hash"))
        .exec(&mut harness.db.clone())
        .await
        .expect("make staff");
    let user = User::get_by_id(&mut harness.db.clone(), user.id)
        .await
        .expect("query");
    let signed = Accounts::signed_in(user, &mut harness.db.clone())
        .await
        .expect("load the account");
    let client = TestClient::new(&harness.app)
        .panel(&harness.admin_panel)
        .sign_in(&harness.db, "admin", &signed)
        .await;

    let started = client
        .csrf(CSRF)
        .post_form(
            "/admin/two-factor",
            form_body(&[
                ("intent", "start"),
                ("current_password", PASSWORD),
                ("csrf_token", CSRF),
            ]),
        )
        .await;
    let html = body_string(started).await;
    let secret = two_factor_secret(&html).expect("the set-up shows its key");
    let confirmed = client
        .csrf(CSRF)
        .post_form(
            "/admin/two-factor",
            form_body(&[
                ("intent", "confirm"),
                ("code", &authenticator_code(&secret)),
                ("csrf_token", CSRF),
            ]),
        )
        .await;
    client.cookies(&response_cookies(&confirmed))
}
