//! Wiping the demo back to empty, which the `demo reset` command does on a schedule.
//!
//! Every customer and what they made goes: accounts, teams, sessions, projects, the audit trail,
//! queued jobs and the billing mirror. Their Stripe customers go first, so no test subscription
//! outlives its team and bills on. What a deployment needs to run stays: the migration history,
//! and the job schedules, whose next runs a wipe would otherwise push back past the next wipe.

use jiff::Timestamp;
use tablo::auth::hash_password;
use toasty::Db;

use crate::accounts::{StaffRole, User, normalize_email};
use crate::billing::{Billing, BillingAccount};
use crate::teams::create_team;

use super::SeedAdmin;

/// The tables a wipe keeps, in the schema the connection uses.
const KEPT: &[&str] = &["__toasty_migrations", "tablo_job_schedules"];

/// How long the wipe waits for the running app to let go of a table before giving up. The next
/// scheduled run tries again.
const LOCK_TIMEOUT: &str = "30s";

/// What a reset did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Wiped {
    /// The Stripe customers deleted.
    pub customers_deleted: usize,
    /// The Stripe customers a failed call left behind, harmless in test mode.
    pub customers_left: usize,
    /// Whether an administrator was seeded.
    pub admin_seeded: bool,
}

/// Deletes every team's Stripe customer, empties every table but [`KEPT`] in one transaction, and
/// seeds `admin` in that transaction, so the app never shows a half-wiped database.
/// PostgreSQL's alone: it names its tables from `pg_tables`.
pub async fn reset(
    db: &Db,
    billing: &Billing,
    admin: Option<&SeedAdmin>,
) -> topcoat::Result<Wiped> {
    let mut wiped = Wiped::default();
    let mut handle = db.clone();
    if let Ok(stripe) = billing.gateway() {
        for account in BillingAccount::all().exec(&mut handle).await? {
            match stripe.delete_customer(account.customer_id).await {
                Ok(()) => wiped.customers_deleted += 1,
                Err(error) => {
                    tracing::warn!(%error, "a Stripe customer outlives the reset");
                    wiped.customers_left += 1;
                }
            }
        }
    }

    // Seeding hashes the password before the transaction, which then holds its locks briefly.
    let admin = match admin {
        Some(admin) => Some((admin, hash_password(&admin.password).await?)),
        None => None,
    };
    let mut tx = handle.transaction().await?;
    toasty::sql::statement(format!("SET LOCAL lock_timeout = '{LOCK_TIMEOUT}'"))
        .exec(&mut tx)
        .await?;
    let kept = KEPT
        .iter()
        .map(|table| format!("'{table}'"))
        .collect::<Vec<_>>()
        .join(", ");
    toasty::sql::statement(format!(
        "DO $wipe$
         DECLARE wiped text;
         BEGIN
           SELECT string_agg(format('%I.%I', schemaname, tablename), ', ') INTO wiped
           FROM pg_tables
           WHERE schemaname = current_schema() AND tablename NOT IN ({kept});
           IF wiped IS NOT NULL THEN
             EXECUTE 'TRUNCATE ' || wiped || ' RESTART IDENTITY';
           END IF;
         END
         $wipe$"
    ))
    .exec(&mut tx)
    .await?;

    if let Some((admin, password)) = admin {
        let user = toasty::create!(User {
            name: "Administrator".to_string(),
            email: normalize_email(&admin.email),
            email_verified_at: Some(Timestamp::now()),
            password,
            staff_role: Some(StaffRole::Administrator),
            staff_since: Some(Timestamp::now()),
        })
        .exec(&mut tx)
        .await?;
        create_team(&mut tx, "Back office", &user).await?;
        wiped.admin_seeded = true;
    }
    tx.commit().await?;
    Ok(wiped)
}
