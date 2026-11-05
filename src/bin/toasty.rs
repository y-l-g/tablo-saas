//! Generates migrations from the models: `cargo toasty migration generate --name add_projects`.
//!
//! Connects to the development database, as migrations are written in its dialect.

use toasty_cli::{Config, ToastyCli};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let url = std::env::var("DATABASE_URL").map_err(|_| anyhow::anyhow!("set DATABASE_URL"))?;
    let db = toasty::Db::builder()
        .models(tablo_saas::models())
        .connect(&url)
        .await?;
    ToastyCli::with_config(db, Config::load()?)
        .parse_and_run()
        .await
}
