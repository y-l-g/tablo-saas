//! The binary's commands. One image runs every role, so a deployment picks a role by its command.

use anyhow::Context;
use clap::{Parser, Subcommand};
use tokio::{net::TcpListener, sync::watch};
use topcoat::asset::AssetBundle;

use crate::background;
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
    /// Serves the app, and runs a worker beside it unless told not to. The default command.
    Serve {
        /// Leaves the jobs to separate `worker` processes.
        #[arg(long)]
        no_worker: bool,
    },
    /// Runs background jobs until stopped. Scale this apart from the servers.
    Worker,
    /// Applies pending migrations, then exits. Run it before a release's servers start.
    Migrate,
}

impl Cli {
    pub async fn run(self) -> anyhow::Result<()> {
        dotenvy::dotenv().ok();
        telemetry::init();
        let config = Config::from_env()?;
        let db = database::connect(&config).await?;
        let mail = Postbox::new(config.mail.smtp.as_ref())?;
        let (stop, stopped) = watch::channel(false);
        tokio::spawn(async move {
            shutdown_signal().await;
            stop.send_replace(true);
        });

        match self.command.unwrap_or(Command::Serve { no_worker: false }) {
            Command::Migrate => migrate(&db).await,
            Command::Worker => {
                background::worker(db, mail).run(stopped).await;
                Ok(())
            }
            Command::Serve { no_worker } => {
                // A development database follows the code; in production the release does it,
                // so two replicas starting together never race over the schema.
                if !config.is_production() {
                    migrate(&db).await?;
                }
                let worker = (!no_worker).then(|| {
                    tokio::spawn(background::worker(db.clone(), mail.clone()).run(stopped.clone()))
                });
                let app = App {
                    db,
                    mail,
                    assets: Some(AssetBundle::load()?),
                    config: &config,
                };
                let router = router(app).map_err(|error| anyhow::anyhow!("{error:#}"))?;
                let listener = TcpListener::bind(address()?).await?;
                let mut stopping = stopped;
                topcoat::serve_until(listener, router, async move {
                    let _ = stopping.changed().await;
                })
                .await?;
                if let Some(worker) = worker {
                    worker.await?;
                }
                Ok(())
            }
        }
    }
}

/// `HOST` and `PORT`, which a platform sets, as `topcoat::start` reads them.
fn address() -> anyhow::Result<(String, u16)> {
    let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    Ok((host, port.parse().context("PORT is a port number")?))
}

async fn migrate(db: &toasty::Db) -> anyhow::Result<()> {
    let applied = database::migrate(db).await?;
    tracing::info!(applied, "migrations applied");
    Ok(())
}

/// Ctrl+C, or the `SIGTERM` an orchestrator sends before it kills a replica.
async fn shutdown_signal() {
    let interrupt = tokio::signal::ctrl_c();
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut terminate = signal(SignalKind::terminate()).expect("install a SIGTERM handler");
        tokio::select! {
            _ = interrupt => {}
            _ = terminate.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = interrupt.await;
    }
}
