//! The binary's commands. One image runs every role, so a deployment picks a role by its command.

use anyhow::Context;
use clap::{Parser, Subcommand};
use tokio::{net::TcpListener, sync::watch};
use topcoat::asset::AssetBundle;

use crate::accounts::{StaffRole, User, normalize_email};
use crate::background;
use crate::billing::{Billing, sync_catalog};
use crate::jobs::MailSeal;
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
    /// Who works in the back office.
    Staff {
        #[command(subcommand)]
        command: StaffCommand,
    },
    /// Stripe housekeeping.
    Billing {
        #[command(subcommand)]
        command: BillingCommand,
    },
}

#[derive(Subcommand)]
enum StaffCommand {
    /// Gives an account a role in the back office.
    Grant {
        email: String,
        /// `administrator` or `support`.
        #[arg(long, default_value = "support")]
        role: String,
    },
    /// Takes an account's role away.
    Revoke { email: String },
}

#[derive(Subcommand)]
enum BillingCommand {
    /// Creates the plan catalog's products and prices on Stripe. Safe to repeat.
    SyncCatalog,
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

        let command = self.command.unwrap_or(Command::Serve { no_worker: false });
        if let Command::Migrate = command {
            return migrate(&db).await;
        }
        if let Command::Staff { command } = command {
            return staff(&db, command).await;
        }
        let billing = Billing::new(config.stripe.as_ref())?;
        if let Command::Billing {
            command: BillingCommand::SyncCatalog,
        } = command
        {
            let synced = sync_catalog(billing.gateway()).await?;
            tracing::info!(?synced, "the plan catalog is on Stripe");
            return Ok(());
        }
        // A development database follows the code; in production the release does it, so two
        // replicas starting together never race over the schema.
        if !config.is_production() {
            migrate(&db).await?;
        }
        let assets = matches!(command, Command::Serve { .. })
            .then(AssetBundle::load)
            .transpose()?;
        let built = router(App {
            db: db.clone(),
            mail: mail.clone(),
            billing: billing.clone(),
            assets,
            config: &config,
        })
        .map_err(|error| anyhow::anyhow!("{error:#}"))?;
        let worker = background::worker(
            db,
            &built.app_panel,
            config.settings(),
            billing,
            mail,
            MailSeal::new(&config.cookie_key),
        );

        match command {
            Command::Migrate | Command::Billing { .. } | Command::Staff { .. } => {
                unreachable!("handled above")
            }
            Command::Worker => {
                worker.run(stopped).await;
                Ok(())
            }
            Command::Serve { no_worker } => {
                let worker = (!no_worker).then(|| tokio::spawn(worker.run(stopped.clone())));
                let mut stopping = stopped;
                topcoat::serve_until(
                    TcpListener::bind(address()?).await?,
                    built.router,
                    async move {
                        let _ = stopping.changed().await;
                    },
                )
                .await?;
                if let Some(worker) = worker {
                    worker.await?;
                }
                Ok(())
            }
        }
    }
}

/// Sets or clears an account's staff role. The back office has no page for this: granting access
/// to everything is a deploy-level act, and a stolen staff session cannot do it.
async fn staff(db: &toasty::Db, command: StaffCommand) -> anyhow::Result<()> {
    let (email, role) = match command {
        StaffCommand::Grant { email, role } => {
            let role = match role.as_str() {
                "administrator" => StaffRole::Administrator,
                "support" => StaffRole::Support,
                other => anyhow::bail!("a role is `administrator` or `support`, not `{other}`"),
            };
            (email, Some(role))
        }
        StaffCommand::Revoke { email } => (email, None),
    };
    let mut handle = db.clone();
    let email = normalize_email(&email);
    let found = User::filter(User::fields().email().eq(&email))
        .first()
        .exec(&mut handle)
        .await?;
    let Some(user) = found else {
        anyhow::bail!("no account uses {email}: sign up first");
    };
    User::filter_by_id(user.id)
        .update()
        .staff_role(role)
        .exec(&mut handle)
        .await?;
    tracing::info!(%email, ?role, "the staff role changed");
    Ok(())
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
