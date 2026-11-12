//! The queue's tables.

use jiff::Timestamp;

/// One unit of background work.
#[derive(Debug, Clone, toasty::Model, tablo::PanelModel)]
#[table = "jobs"]
pub struct Job {
    #[key]
    #[auto]
    pub id: uuid::Uuid,
    /// The `Job::KIND` that runs it.
    #[index]
    pub kind: String,
    /// Its fields, as JSON. Never a secret in the clear: mail jobs name rows and render when they
    /// run, and `send-mail` carries its mail sealed.
    pub payload: String,
    pub state: JobState,
    /// Runs in one lane run one at a time, so two syncs of one customer never interleave.
    #[index]
    pub lane: Option<String>,
    /// While queued, a second enqueue of the same key changes nothing. A claim frees it, so work
    /// arriving during a run queues behind that run instead of being lost.
    #[unique]
    pub dedupe_key: String,
    #[default(0)]
    pub attempts: u64,
    pub max_attempts: u64,
    /// Not before this instant.
    #[index]
    pub run_at: Timestamp,
    /// Who holds a running job gives it up at this instant, dead or alive.
    pub locked_until: Option<Timestamp>,
    /// The last failure, for the person reading a dead job.
    pub last_error: Option<String>,
    #[default(Timestamp::now())]
    pub created_at: Timestamp,
    pub finished_at: Option<Timestamp>,
    /// Makes a claim a compare-and-swap: of two workers, the second fails its update.
    #[version]
    pub version: u64,
}

/// Where a job stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, toasty::Embed, tablo::Options)]
pub enum JobState {
    #[option(color = "gray")]
    Queued,
    #[option(color = "info")]
    Running,
    #[option(color = "success")]
    Succeeded,
    /// Out of attempts: a person decides.
    #[option(color = "danger")]
    Dead,
}

/// When a recurring job next falls due, one row per schedule, shared by every worker.
#[derive(Debug, Clone, toasty::Model)]
#[table = "job_schedules"]
pub struct ScheduleRun {
    #[key]
    pub name: String,
    pub next_run_at: Timestamp,
    #[version]
    pub version: u64,
}
