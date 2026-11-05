//! Putting work on the queue.

use jiff::{SignedDuration, Timestamp};

use super::{JobRow, JobState, registry::Job};

/// What to enqueue, and how: `enqueue(ex, &job).delay(..)`, or `.dedupe(key)`.
pub struct Enqueue<'a, J> {
    job: &'a J,
    delay: SignedDuration,
    dedupe_key: Option<String>,
}

/// Queues `job` on `ex`: pass the caller's transaction, so the job commits with its write.
pub fn enqueue<J: Job>(job: &J) -> Enqueue<'_, J> {
    Enqueue {
        job,
        delay: SignedDuration::ZERO,
        dedupe_key: None,
    }
}

impl<J: Job> Enqueue<'_, J> {
    /// Runs no sooner than `delay` from now.
    #[must_use]
    pub fn delay(mut self, delay: SignedDuration) -> Self {
        self.delay = delay;
        self
    }

    /// While a job with `key` waits, this one coalesces into it: ten seat changes in a burst make
    /// one Stripe call.
    #[must_use]
    pub fn dedupe(mut self, key: impl Into<String>) -> Self {
        self.dedupe_key = Some(key.into());
        self
    }

    /// Writes the row on `ex`.
    pub async fn exec(self, ex: &mut dyn toasty::Executor) -> anyhow::Result<()> {
        let id = uuid::Uuid::now_v7();
        let run_at = Timestamp::now()
            .checked_add(self.delay)
            .unwrap_or_else(|_| Timestamp::now());
        let payload = serde_json::to_string(self.job)?;
        let dedupe_key = self.dedupe_key.unwrap_or_else(|| id.to_string());
        JobRow::upsert_by_dedupe_key(dedupe_key)
            .on_create(|create| {
                create
                    .id(id)
                    .kind(J::KIND.to_string())
                    .payload(payload)
                    .state(JobState::Queued)
                    .lane(self.job.lane())
                    .max_attempts(u64::from(J::MAX_ATTEMPTS))
                    .run_at(run_at)
            })
            .or_ignore()
            .exec(ex)
            .await?;
        Ok(())
    }
}
