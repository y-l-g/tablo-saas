//! The queue: transactional enqueue, retries, coalescing, lanes and crash recovery.

mod support;

use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use jiff::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};
use support::harness;
use tablo_saas::jobs::{Job, JobRow, JobState, Recurring, Registry, Worker, enqueue};
use toasty::Db;
use topcoat::context::{Cx, context};

/// Counts its runs, and fails until the count passes `fail_until`.
#[derive(Clone, Serialize, Deserialize)]
struct Probe {
    fail_until: usize,
    lane: Option<String>,
}

struct Runs(Arc<AtomicUsize>);

impl Job for Probe {
    const KIND: &'static str = "probe";
    const MAX_ATTEMPTS: u32 = 3;

    fn lane(&self) -> Option<String> {
        self.lane.clone()
    }

    async fn run(self, cx: &Cx) -> anyhow::Result<()> {
        let run = context::<Runs>(cx).0.fetch_add(1, Ordering::SeqCst) + 1;
        anyhow::ensure!(run > self.fail_until, "run {run} fails");
        Ok(())
    }
}

fn worker(db: &Db, runs: &Arc<AtomicUsize>, schedules: Vec<Recurring>) -> Worker {
    Worker::new(
        db.clone(),
        Registry::default().register::<Probe>(),
        schedules,
        Cx::new().with(Runs(Arc::clone(runs))),
    )
}

async fn rows(db: &Db) -> Vec<JobRow> {
    JobRow::all().exec(&mut db.clone()).await.expect("query")
}

/// Makes every queued job due now, as if its backoff had passed.
async fn age(db: &Db) {
    JobRow::filter(JobRow::fields().state().eq(JobState::Queued))
        .update()
        .run_at(
            Timestamp::now()
                .checked_sub(SignedDuration::from_secs(1))
                .expect("in range"),
        )
        .exec(&mut db.clone())
        .await
        .expect("update");
}

const OK: Probe = Probe {
    fail_until: 0,
    lane: None,
};

#[tokio::test]
async fn a_job_runs_and_is_marked_done() {
    let harness = harness().await;
    let runs = Arc::new(AtomicUsize::new(0));
    enqueue(&OK)
        .exec(&mut harness.db.clone())
        .await
        .expect("enqueue");

    worker(&harness.db, &runs, vec![])
        .drain()
        .await
        .expect("drain");

    assert_eq!(runs.load(Ordering::SeqCst), 1);
    assert_eq!(rows(&harness.db).await[0].state, JobState::Succeeded);
}

#[tokio::test]
async fn a_rolled_back_transaction_enqueues_nothing() {
    let harness = harness().await;
    let mut db = harness.db.clone();
    let mut tx = db.transaction().await.expect("begin");
    enqueue(&OK).exec(&mut tx).await.expect("enqueue");
    tx.rollback().await.expect("rollback");
    assert!(rows(&harness.db).await.is_empty());
}

#[tokio::test]
async fn a_failure_waits_its_backoff_then_succeeds() {
    let harness = harness().await;
    let runs = Arc::new(AtomicUsize::new(0));
    let job = Probe {
        fail_until: 1,
        lane: None,
    };
    enqueue(&job)
        .exec(&mut harness.db.clone())
        .await
        .expect("enqueue");

    worker(&harness.db, &runs, vec![])
        .drain()
        .await
        .expect("drain");
    let row = rows(&harness.db).await.remove(0);
    assert_eq!(row.state, JobState::Queued);
    assert!(row.run_at > Timestamp::now(), "it waits its backoff");
    assert!(row.last_error.expect("kept").contains("run 1 fails"));

    age(&harness.db).await;
    worker(&harness.db, &runs, vec![])
        .drain()
        .await
        .expect("drain");
    assert_eq!(rows(&harness.db).await[0].state, JobState::Succeeded);
}

#[tokio::test]
async fn a_job_out_of_attempts_turns_dead() {
    let harness = harness().await;
    let runs = Arc::new(AtomicUsize::new(0));
    let job = Probe {
        fail_until: 100,
        lane: None,
    };
    enqueue(&job)
        .exec(&mut harness.db.clone())
        .await
        .expect("enqueue");
    for _ in 0..3 {
        worker(&harness.db, &runs, vec![])
            .drain()
            .await
            .expect("drain");
        age(&harness.db).await;
    }
    assert_eq!(runs.load(Ordering::SeqCst), 3);
    assert_eq!(rows(&harness.db).await[0].state, JobState::Dead);
}

#[tokio::test]
async fn a_dedupe_key_coalesces_a_burst_into_one_job() {
    let harness = harness().await;
    let mut db = harness.db.clone();
    for _ in 0..5 {
        enqueue(&OK)
            .dedupe("seats:team")
            .exec(&mut db)
            .await
            .expect("enqueue");
    }
    assert_eq!(rows(&harness.db).await.len(), 1);
}

#[tokio::test]
async fn work_arriving_during_a_run_queues_behind_it() {
    let harness = harness().await;
    let runs = Arc::new(AtomicUsize::new(0));
    let mut db = harness.db.clone();
    enqueue(&OK)
        .dedupe("key")
        .exec(&mut db)
        .await
        .expect("enqueue");
    worker(&harness.db, &runs, vec![])
        .drain()
        .await
        .expect("drain");
    enqueue(&OK)
        .dedupe("key")
        .exec(&mut db)
        .await
        .expect("enqueue");
    assert_eq!(rows(&harness.db).await.len(), 2, "the claim freed the key");
}

#[tokio::test]
async fn a_busy_lane_holds_its_next_job_back() {
    let harness = harness().await;
    let runs = Arc::new(AtomicUsize::new(0));
    let job = Probe {
        fail_until: 0,
        lane: Some("customer".to_string()),
    };
    let mut db = harness.db.clone();
    enqueue(&job).exec(&mut db).await.expect("enqueue");
    enqueue(&job).exec(&mut db).await.expect("enqueue");

    let worker = Arc::new(worker(&harness.db, &runs, vec![]));
    assert_eq!(Arc::clone(&worker).drain_once().await.expect("drain"), 1);
    assert_eq!(Arc::clone(&worker).drain_once().await.expect("drain"), 1);
    assert_eq!(runs.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn a_lapsed_lock_gives_the_job_to_another_worker() {
    let harness = harness().await;
    let runs = Arc::new(AtomicUsize::new(0));
    let mut db = harness.db.clone();
    enqueue(&OK).exec(&mut db).await.expect("enqueue");
    let past = Timestamp::now()
        .checked_sub(SignedDuration::from_mins(1))
        .expect("in range");
    JobRow::all()
        .update()
        .state(JobState::Running)
        .locked_until(Some(past))
        .run_at(past)
        .exec(&mut db)
        .await
        .expect("a worker took it and died");

    worker(&harness.db, &runs, vec![])
        .drain()
        .await
        .expect("drain");

    assert_eq!(runs.load(Ordering::SeqCst), 1);
    assert_eq!(rows(&harness.db).await[0].state, JobState::Succeeded);
}

#[tokio::test]
async fn an_unknown_kind_fails_rather_than_vanishes() {
    let harness = harness().await;
    let mut db = harness.db.clone();
    enqueue(&OK).exec(&mut db).await.expect("enqueue");
    Worker::new(harness.db.clone(), Registry::default(), vec![], Cx::new())
        .drain()
        .await
        .expect("drain");
    let row = rows(&harness.db).await.remove(0);
    assert!(row.last_error.expect("kept").contains("no job of kind"));
}

#[tokio::test]
async fn a_schedule_enqueues_once_per_period_across_workers() {
    let harness = harness().await;
    let runs = Arc::new(AtomicUsize::new(0));
    let schedule = Recurring::every("probe", Duration::from_hours(1), OK);
    worker(&harness.db, &runs, vec![schedule.clone()])
        .drain()
        .await
        .expect("drain");
    assert_eq!(
        runs.load(Ordering::SeqCst),
        0,
        "the first run is one interval away"
    );

    tablo_saas::jobs::ScheduleRun::filter_by_name("probe")
        .update()
        .next_run_at(
            Timestamp::now()
                .checked_sub(SignedDuration::from_secs(1))
                .expect("in range"),
        )
        .exec(&mut harness.db.clone())
        .await
        .expect("fall due");

    worker(&harness.db, &runs, vec![schedule.clone()])
        .drain()
        .await
        .expect("drain");
    worker(&harness.db, &runs, vec![schedule])
        .drain()
        .await
        .expect("drain");
    assert_eq!(runs.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn racing_workers_run_a_job_once() {
    let harness = harness().await;
    let runs = Arc::new(AtomicUsize::new(0));
    let mut db = harness.db.clone();
    for _ in 0..8 {
        enqueue(&OK).exec(&mut db).await.expect("enqueue");
    }
    let racers: Vec<_> = (0..4)
        .map(|_| Arc::new(worker(&harness.db, &runs, vec![])))
        .map(|worker| tokio::spawn(async move { worker.drain_once().await }))
        .collect();
    for racer in racers {
        racer.await.expect("join").expect("drain");
    }
    assert_eq!(runs.load(Ordering::SeqCst), 8);
}
