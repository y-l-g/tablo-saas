//! Claiming and running jobs.

use std::{collections::HashSet, sync::Arc, time::Duration};

use jiff::{SignedDuration, Timestamp};
use toasty::Db;
use tokio::{sync::watch, task::JoinSet};
use topcoat::context::Cx;

use super::{JobRow, JobState, Recurring, Registry};

/// How long a run may take before its lock lapses for another worker.
const RUN_TIMEOUT: Duration = Duration::from_mins(2);
/// A lock outlives the timeout, so a live run is never taken over.
const LOCK: SignedDuration = SignedDuration::from_mins(3);
/// How many jobs one worker runs at once.
const BATCH: usize = 16;
/// How long an idle worker waits before it asks again.
const IDLE: Duration = Duration::from_secs(1);
/// How often a worker looks for lapsed locks and due schedules.
const SWEEP_EVERY: u32 = 15;
const BACKOFF_BASE_SECONDS: i64 = 30;
const BACKOFF_CAP_SECONDS: i64 = 60 * 60;

/// Runs jobs until told to stop.
pub struct Worker {
    db: Db,
    registry: Registry,
    schedules: Vec<Recurring>,
    /// Builds what a job reads: the database, the mail transport, the Stripe client. Each run
    /// gets its own, because a context is one unit of work and keeps what it loads.
    context: Box<dyn Fn() -> Cx + Send + Sync>,
}

impl Worker {
    pub fn new(
        db: Db,
        registry: Registry,
        schedules: Vec<Recurring>,
        context: impl Fn() -> Cx + Send + Sync + 'static,
    ) -> Self {
        Self {
            db,
            registry,
            schedules,
            context: Box::new(context),
        }
    }

    /// Polls until `shutdown` turns true, then lets the runs in hand finish.
    pub async fn run(self, mut shutdown: watch::Receiver<bool>) {
        let worker = Arc::new(self);
        let mut idle_ticks = SWEEP_EVERY;
        loop {
            if *shutdown.borrow() {
                return;
            }
            if idle_ticks >= SWEEP_EVERY {
                idle_ticks = 0;
                worker.sweep().await;
            }
            match Arc::clone(&worker).drain_once().await {
                Ok(0) => {
                    idle_ticks += 1;
                    tokio::select! {
                        () = tokio::time::sleep(IDLE) => {}
                        _ = shutdown.changed() => {}
                    }
                }
                Ok(_) => {}
                Err(error) => {
                    tracing::error!(error = %format!("{error:#}"), "claiming jobs failed");
                    tokio::time::sleep(IDLE).await;
                }
            }
        }
    }

    /// Takes lapsed locks back and enqueues due schedules. A failure here waits for the next
    /// sweep: nothing is lost.
    async fn sweep(&self) {
        if let Err(error) = self.recover().await {
            tracing::error!(error = %format!("{error:#}"), "recovering jobs failed");
        }
        for schedule in &self.schedules {
            if let Err(error) = schedule.tick(&self.db).await {
                tracing::error!(error = %format!("{error:#}"), "a schedule failed");
            }
        }
    }

    /// Runs one batch to its end and answers how many jobs it claimed. The tests' way of saying
    /// "the worker ran".
    pub async fn drain_once(self: Arc<Self>) -> anyhow::Result<usize> {
        let claimed = self.claim().await?;
        let count = claimed.len();
        let mut runs = JoinSet::new();
        for job in claimed {
            let worker = Arc::clone(&self);
            runs.spawn(async move { worker.execute(job).await });
        }
        while let Some(run) = runs.join_next().await {
            if let Err(error) = run {
                tracing::error!(%error, "a job's task ended abnormally");
            }
        }
        Ok(count)
    }

    /// Runs every job due now, schedules included, until none is left.
    pub async fn drain(self) -> anyhow::Result<()> {
        let worker = Arc::new(self);
        worker.sweep().await;
        while Arc::clone(&worker).drain_once().await? > 0 {}
        Ok(())
    }

    /// Up to a batch of due jobs, each now held by this worker, skipping lanes already busy.
    async fn claim(&self) -> anyhow::Result<Vec<JobRow>> {
        let mut db = self.db.clone();
        let now = Timestamp::now();
        let due = JobRow::filter(
            JobRow::fields()
                .state()
                .eq(JobState::Queued)
                .and(JobRow::fields().run_at().le(now)),
        )
        .order_by(JobRow::fields().run_at().asc())
        .limit(BATCH)
        .exec(&mut db)
        .await?;
        if due.is_empty() {
            return Ok(Vec::new());
        }
        let mut busy: HashSet<String> =
            JobRow::filter(JobRow::fields().state().eq(JobState::Running))
                .exec(&mut db)
                .await?
                .into_iter()
                .filter_map(|job| job.lane)
                .collect();
        let locked_until = now.checked_add(LOCK)?;
        let mut claimed = Vec::with_capacity(due.len());
        for mut job in due {
            if job.lane.as_ref().is_some_and(|lane| busy.contains(lane)) {
                continue;
            }
            let free_key = job.id.to_string();
            let attempts = job.attempts + 1;
            let won = job
                .update()
                .state(JobState::Running)
                .attempts(attempts)
                .locked_until(Some(locked_until))
                .dedupe_key(free_key)
                .exec(&mut db)
                .await
                .is_ok();
            if won {
                if let Some(lane) = &job.lane {
                    busy.insert(lane.clone());
                }
                claimed.push(job);
            }
        }
        Ok(claimed)
    }

    /// Runs `job` and records how it ended.
    async fn execute(&self, job: JobRow) {
        let span =
            tracing::info_span!("job", kind = %job.kind, id = %job.id, attempt = job.attempts);
        let _entered = span.enter();
        let cx = (self.context)();
        let outcome = match self.registry.get(&job.kind) {
            None => Err(anyhow::anyhow!(
                "no job of kind `{}` is registered",
                job.kind
            )),
            Some(run) => tokio::time::timeout(RUN_TIMEOUT, run(&job.payload, &cx))
                .await
                .unwrap_or_else(|_| Err(anyhow::anyhow!("timed out after {RUN_TIMEOUT:?}"))),
        };
        if let Err(error) = self.record(job, outcome).await {
            // The lock lapses and the job runs again, which an idempotent job survives.
            tracing::error!(error = %format!("{error:#}"), "recording a job's outcome failed");
        }
    }

    async fn record(&self, mut job: JobRow, outcome: anyhow::Result<()>) -> anyhow::Result<()> {
        let mut db = self.db.clone();
        let now = Timestamp::now();
        match outcome {
            Ok(()) => {
                job.update()
                    .state(JobState::Succeeded)
                    .locked_until(None)
                    .finished_at(Some(now))
                    .exec(&mut db)
                    .await?;
            }
            Err(error) => {
                let message = format!("{error:#}");
                if job.attempts >= job.max_attempts {
                    tracing::error!(error = %message, "a job is dead");
                    job.update()
                        .state(JobState::Dead)
                        .locked_until(None)
                        .finished_at(Some(now))
                        .last_error(Some(message))
                        .exec(&mut db)
                        .await?;
                } else {
                    tracing::warn!(error = %message, "a job will retry");
                    let run_at = now.checked_add(backoff(job.attempts, job.id))?;
                    job.update()
                        .state(JobState::Queued)
                        .locked_until(None)
                        .run_at(run_at)
                        .last_error(Some(message))
                        .exec(&mut db)
                        .await?;
                }
            }
        }
        Ok(())
    }

    /// Requeues running jobs whose lock lapsed: their worker died.
    async fn recover(&self) -> anyhow::Result<()> {
        let mut db = self.db.clone();
        let now = Timestamp::now();
        let lapsed = JobRow::filter(
            JobRow::fields()
                .state()
                .eq(JobState::Running)
                .and(JobRow::fields().locked_until().le(now)),
        )
        .exec(&mut db)
        .await?;
        for mut job in lapsed {
            tracing::warn!(kind = %job.kind, id = %job.id, "a job's lock lapsed");
            let exhausted = job.attempts >= job.max_attempts;
            let state = if exhausted {
                JobState::Dead
            } else {
                JobState::Queued
            };
            // A lost race means another worker recovered it first.
            let _ = job
                .update()
                .state(state)
                .locked_until(None)
                .last_error(Some("the worker running it stopped".to_string()))
                .exec(&mut db)
                .await;
        }
        Ok(())
    }
}

/// How long the run after `attempt` waits: doubling from 30 seconds to an hour, with up to a
/// quarter more drawn from the job's id, so a burst of failures does not retry as one.
fn backoff(attempt: u64, id: uuid::Uuid) -> SignedDuration {
    let exponent = u32::try_from(attempt.saturating_sub(1))
        .unwrap_or(u32::MAX)
        .min(16);
    let seconds = BACKOFF_BASE_SECONDS
        .saturating_mul(1 << exponent)
        .min(BACKOFF_CAP_SECONDS);
    let spread = i64::from(id.as_bytes()[15]) * seconds / (4 * 255);
    SignedDuration::from_secs(seconds + spread)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_up_to_its_cap_and_never_shrinks() {
        let id = uuid::Uuid::nil();
        assert_eq!(backoff(1, id).as_secs(), 30);
        assert_eq!(backoff(2, id).as_secs(), 60);
        assert_eq!(backoff(3, id).as_secs(), 120);
        assert_eq!(backoff(40, id).as_secs(), BACKOFF_CAP_SECONDS);
    }

    #[test]
    fn jitter_adds_at_most_a_quarter() {
        let id = uuid::Uuid::from_u128(u128::MAX);
        assert_eq!(backoff(1, id).as_secs(), 30 + 7);
    }
}
