//! Every kind of background work the app does, in one list, and what a job reads when it runs.

use std::time::Duration;

use tablo::PanelHandle;
use toasty::Db;
use topcoat::mail::{MailConfig, Transport};

use crate::jobs::{Recurring, Registry, Worker, housekeeping::PruneJobs};
use crate::platform::config::Settings;
use crate::teams::{PruneInvitations, SendInvitation};

/// Every job kind: a worker that lacks one fails such a job instead of guessing.
fn registry() -> Registry {
    Registry::default()
        .register::<PruneJobs>()
        .register::<SendInvitation>()
        .register::<PruneInvitations>()
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
    ]
}

/// A worker reading what the app reads, through the mounted panel: its database, its catalogs for
/// the words a mail says, and its resources.
pub fn worker<T: Transport + 'static>(
    db: Db,
    panel: &PanelHandle,
    settings: Settings,
    mail: T,
) -> Worker {
    let cx = panel
        .context()
        .with(settings)
        .with(MailConfig::builder().transport(mail).build());
    Worker::new(db, registry(), schedules(), cx)
}
