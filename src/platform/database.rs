//! The connection pool and the migrations embedded in the binary.

use std::time::Duration;

use toasty::{Db, migration::MigrationSet};

use super::config::Config;

/// The SQL files `cargo toasty migration generate` wrote, compiled in, so a deployed image needs
/// nothing beside its binary to bring a database up to date.
static MIGRATIONS: MigrationSet = toasty::embed_migrations!();

/// Connects the pool the whole process shares. A checkout that waits more than a few seconds fails
/// the request rather than queueing it behind an exhausted pool.
pub async fn connect(config: &Config) -> toasty::Result<Db> {
    Db::builder()
        .models(crate::models())
        .max_pool_size(config.database_pool_size)
        .pool_wait_timeout(Some(Duration::from_secs(5)))
        .pool_create_timeout(Some(Duration::from_secs(10)))
        .connect(&config.database_url)
        .await
}

/// Applies the migrations the database has not seen, in order, and answers how many ran.
pub async fn migrate(db: &Db) -> toasty::Result<usize> {
    Ok(MIGRATIONS.apply(db).await?.applied())
}

/// Whether the database answers, for the readiness probe.
pub async fn ping(db: &Db) -> bool {
    let mut db = db.clone();
    toasty::sql::query("SELECT 1").exec(&mut db).await.is_ok()
}
