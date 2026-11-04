//! Every kind of background work the app does, in one list, and what a job reads when it runs.

use std::time::Duration;

use tablo::PanelHandle;
use toasty::Db;
use topcoat::mail::{MailConfig, Transport};

use crate::billing::{
    Billing,
    jobs::{
        ProcessStripeEvent, PruneStripeEvents, ReconcileBilling, RefreshCustomer,
        SendBillingNotice, SyncSeats,
    },
};
use crate::jobs::{Recurring, Registry, Worker, housekeeping::PruneJobs};
use crate::platform::config::Settings;
use crate::teams::{CancelSubscription, PruneInvitations, SendInvitation};

/// Every job kind: a worker that lacks one fails such a job instead of guessing.
fn registry() -> Registry {
    Registry::default()
        .register::<PruneJobs>()
        .register::<SendInvitation>()
        .register::<CancelSubscription>()
        .register::<PruneInvitations>()
        .register::<ProcessStripeEvent>()
        .register::<SendBillingNotice>()
        .register::<SyncSeats>()
        .register::<ReconcileBilling>()
        .register::<RefreshCustomer>()
        .register::<PruneStripeEvents>()
}

/// What runs on a clock.
fn schedules() -> Vec<Recurring> {
    vec![
        Recurring::every("prune-jobs", Duration::from_hours(24), PruneJobs),
        Recurring::every(
            "prune-invitations",
            Duration::from_hours(24),
            PruneInvitations,
        ),
        Recurring::every(
            "reconcile-billing",
            Duration::from_hours(6),
            ReconcileBilling,
        ),
        Recurring::every(
            "prune-stripe-events",
            Duration::from_hours(24),
            PruneStripeEvents,
        ),
    ]
}

/// A worker reading what the app reads, through the mounted panel: its database, its catalogs for
/// the words a mail says, and its resources.
pub fn worker<T: Transport + 'static>(
    db: Db,
    panel: &PanelHandle,
    settings: Settings,
    billing: Billing,
    mail: T,
) -> Worker {
    let cx = panel
        .context()
        .with(settings)
        .with(billing)
        .with(MailConfig::builder().transport(mail).build());
    Worker::new(db, registry(), schedules(), cx)
}
