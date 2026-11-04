//! Recurring jobs. A row per schedule holds when it next falls due, and a version check on that
//! row lets exactly one of however many workers enqueue each run.

use std::{future::Future, pin::Pin, sync::Arc, time::Duration};

use jiff::{SignedDuration, Timestamp};
use toasty::Db;

use super::{ScheduleRun, enqueue, registry::Job};

type Enqueuer = Arc<
    dyn for<'a> Fn(
            &'a mut dyn toasty::Executor,
        ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'a>>
        + Send
        + Sync,
>;

/// A job that runs every so often.
#[derive(Clone)]
pub struct Recurring {
    name: &'static str,
    every: Duration,
    enqueue: Enqueuer,
}

impl Recurring {
    /// `job` every `every`, the first run one interval after the schedule first appears.
    pub fn every<J: Job + Clone>(name: &'static str, every: Duration, job: J) -> Self {
        Self {
            name,
            every,
            enqueue: Arc::new(move |ex| {
                let job = job.clone();
                Box::pin(async move { enqueue(&job).dedupe(J::KIND).exec(ex).await })
            }),
        }
    }

    fn next_after(&self, now: Timestamp) -> anyhow::Result<Timestamp> {
        Ok(now.checked_add(SignedDuration::try_from(self.every)?)?)
    }

    /// Enqueues the run if one is due and no other worker got there first.
    pub(super) async fn tick(&self, db: &Db) -> anyhow::Result<()> {
        let mut db = db.clone();
        let now = Timestamp::now();
        let Some(mut row) = ScheduleRun::filter_by_name(self.name)
            .first()
            .exec(&mut db)
            .await?
        else {
            ScheduleRun::upsert_by_name(self.name)
                .on_create(|create| create.next_run_at(self.next_after(now).unwrap_or(now)))
                .or_ignore()
                .exec(&mut db)
                .await?;
            return Ok(());
        };
        if row.next_run_at > now {
            return Ok(());
        }
        let mut tx = db.transaction().await?;
        if row
            .update()
            .next_run_at(self.next_after(now)?)
            .exec(&mut tx)
            .await
            .is_err()
        {
            // Another worker won this run.
            return Ok(());
        }
        (self.enqueue)(&mut tx).await?;
        tx.commit().await?;
        Ok(())
    }
}
