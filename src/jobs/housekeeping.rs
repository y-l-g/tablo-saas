//! The queue cleaning up after itself.

use jiff::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};
use tablo::db::db;
use topcoat::context::Cx;

use super::{Job, JobRow, JobState};

/// How long a finished job stays, for whoever asks what ran.
const KEEP: SignedDuration = SignedDuration::from_hours(14 * 24);

/// Deletes jobs that succeeded long ago. Dead jobs stay until a person deals with them.
#[derive(Clone, Serialize, Deserialize)]
pub struct PruneJobs;

impl Job for PruneJobs {
    const KIND: &'static str = "prune-jobs";

    async fn run(self, cx: &Cx) -> anyhow::Result<()> {
        let mut handle = db(cx).map_err(|error| anyhow::anyhow!("{error:#}"))?;
        let cutoff = Timestamp::now().checked_sub(KEEP)?;
        JobRow::filter(
            JobRow::fields()
                .state()
                .eq(JobState::Succeeded)
                .and(JobRow::fields().finished_at().le(cutoff)),
        )
        .delete()
        .exec(&mut handle)
        .await?;
        Ok(())
    }
}
