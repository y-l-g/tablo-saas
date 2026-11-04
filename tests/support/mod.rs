//! What the integration tests share: the app on a fresh database, with mail caught in memory.
#![allow(dead_code)]

pub mod stripe;

use std::sync::Arc;

use stripe::FakeStripe;
use tablo::{
    PanelHandle,
    testing::{TestClient, test_db},
};
use tablo_saas::{
    accounts::{Accounts, User},
    background,
    billing::Billing,
    jobs::Worker,
    platform::config::Config,
    router::{App, router},
    teams::{Member, Team, TeamRole, create_team},
};
use toasty::Db;
use topcoat::{mail::MemoryTransport, router::Router};

/// A running app and what a test inspects.
pub struct Harness {
    pub app: Router,
    pub panel: PanelHandle,
    pub db: Db,
    pub outbox: MemoryTransport,
    worker: Option<Worker>,
}

fn config() -> Config {
    Config::load(&|name| match name {
        "DATABASE_URL" => Some("sqlite::memory:".to_string()),
        _ => None,
    })
    .expect("a development configuration needs only a database URL")
}

/// The webhook secret a test signs with.
pub const WEBHOOK_SECRET: &str = "whsec_test";

/// The app over empty tables, with billing off.
pub async fn harness() -> Harness {
    build(Billing::new(None).expect("billing off")).await
}

/// The app over empty tables, billing through `stripe`.
pub async fn billed_harness(stripe: &FakeStripe) -> Harness {
    build(Billing::with_gateway(
        Arc::new(stripe.clone()),
        WEBHOOK_SECRET,
    ))
    .await
}

async fn build(billing: Billing) -> Harness {
    let db = test_db(tablo_saas::models()).await;
    let outbox = MemoryTransport::new();
    let config = config();
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
    );
    Harness {
        app: built.router,
        panel,
        db,
        outbox,
        worker: Some(worker),
    }
}

impl Harness {
    /// Runs every job due now, as a worker would.
    pub async fn run_jobs(&mut self) {
        let worker = self.worker.take().expect("a harness drains once");
        let worker = Arc::new(worker);
        while Arc::clone(&worker)
            .drain_once()
            .await
            .expect("the queue runs")
            > 0
        {}
        self.worker = Arc::into_inner(worker);
    }

    /// What the outbox holds once `count` mails arrived. Tablo sends its auth mail after the
    /// response, so a test waits for it rather than racing it.
    pub async fn mail(&self, count: usize) -> Vec<topcoat::mail::Mail> {
        for _ in 0..200 {
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
