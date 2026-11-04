//! The binary's commands. One image runs every role, so a deployment picks a role by its command.

use clap::{Parser, Subcommand};
use topcoat::asset::AssetBundle;

use crate::platform::{config::Config, database, mail::Postbox, telemetry};
use crate::router::{App, router};

/// A production SaaS on Tablo.
#[derive(Parser)]
#[command(version, about)]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Serves the app. The default command.
    Serve,
    /// Applies pending migrations, then exits. Run it before a release's servers start.
    Migrate,
}

impl Cli {
    pub async fn run(self) -> anyhow::Result<()> {
        dotenvy::dotenv().ok();
        telemetry::init();
        let config = Config::from_env()?;
        let db = database::connect(&config).await?;
        match self.command.unwrap_or(Command::Serve) {
            Command::Migrate => migrate(&db).await,
            Command::Serve => {
                // A development database follows the code; in production the release does it,
                // so two replicas starting together never race over the schema.
                if !config.is_production() {
                    migrate(&db).await?;
                }
                let app = App {
                    db,
                    mail: Postbox::new(config.mail.smtp.as_ref())?,
                    assets: Some(AssetBundle::load()?),
                    config: &config,
                };
                let router = router(app).map_err(|error| anyhow::anyhow!("{error:#}"))?;
                topcoat::start(router).await?;
                Ok(())
            }
        }
    }
}

async fn migrate(db: &toasty::Db) -> anyhow::Result<()> {
    let applied = database::migrate(db).await?;
    tracing::info!(applied, "migrations applied");
    Ok(())
}
