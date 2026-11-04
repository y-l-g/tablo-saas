//! What the integration tests share: the app on a fresh database, with mail caught in memory.
#![allow(dead_code)]

use tablo::testing::test_db;
use tablo_saas::{
    platform::config::Config,
    router::{App, router},
};
use toasty::Db;
use topcoat::{mail::MemoryTransport, router::Router};

/// A running app and what a test inspects.
pub struct Harness {
    pub app: Router,
    pub db: Db,
    pub outbox: MemoryTransport,
}

fn config() -> Config {
    Config::load(&|name| match name {
        "DATABASE_URL" => Some("sqlite::memory:".to_string()),
        _ => None,
    })
    .expect("a development configuration needs only a database URL")
}

/// The app over empty tables.
pub async fn harness() -> Harness {
    let db = test_db(tablo_saas::models()).await;
    let outbox = MemoryTransport::new();
    let app = router(App {
        db: db.clone(),
        config: &config(),
        mail: outbox.clone(),
        assets: None,
    })
    .expect("the panels mount");
    Harness { app, db, outbox }
}

impl Harness {
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
