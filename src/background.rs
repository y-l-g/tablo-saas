//! Every kind of background work the app does, in one list, and what a job reads when it runs.

use std::time::Duration;

use toasty::Db;
use topcoat::{
    context::Cx,
    mail::{MailConfig, Transport},
};

use crate::jobs::{Recurring, Registry, Worker, housekeeping::PruneJobs};

/// Every job kind: a worker that lacks one fails such a job instead of guessing.
fn registry() -> Registry {
    Registry::default().register::<PruneJobs>()
}

/// What runs on a clock.
fn schedules() -> Vec<Recurring> {
    vec![Recurring::every(
        "prune-jobs",
        Duration::from_hours(24),
        PruneJobs,
    )]
}

/// A worker reading the same database and mail transport as the app.
pub fn worker<T: Transport + 'static>(db: Db, mail: T) -> Worker {
    let cx = Cx::new()
        .with(db.clone())
        .with(MailConfig::builder().transport(mail).build());
    Worker::new(db, registry(), schedules(), cx)
}
