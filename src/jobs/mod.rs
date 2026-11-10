//! Durable background work, in the same database as the data it acts on.
//!
//! A job enqueued inside a transaction commits with that transaction or not at all: this is the
//! transactional outbox, so "the invitation exists" and "its mail will go out" cannot disagree.
//! Delivery is at-least-once, so a job is idempotent: running it twice leaves what running it once
//! left. Postgres alone carries the queue, so the template needs no broker to run.
//!
//! A [`Worker`] claims due jobs with an optimistic version check, runs them with a timeout, and
//! retries a failure with exponential backoff until it has spent its attempts, when the row turns
//! [`JobState::Dead`] and waits for a person. A worker that dies leaves its lock to expire, and the
//! next worker takes the job over.

pub mod housekeeping;
mod mail;
mod models;
mod queue;
mod registry;
mod schedule;
mod worker;

pub use mail::{MailSeal, Outbox, SendMail};
pub use models::{Job as JobRow, JobState, ScheduleRun};
pub use queue::{Enqueue, enqueue};
pub use registry::{Job, Registry};
pub use schedule::Recurring;
pub use worker::Worker;
