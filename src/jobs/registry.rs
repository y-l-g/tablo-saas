//! The kinds of job a worker knows how to run.

use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc};

use serde::{Serialize, de::DeserializeOwned};
use topcoat::context::Cx;

/// A kind of background work. Its fields are its payload.
///
/// `run` may run twice for one enqueue, after a crash or a lost lock, so it is idempotent. It reads
/// the world as it is when it runs, not as it was at enqueue: the row, not a copy of it.
pub trait Job: Serialize + DeserializeOwned + Send + Sync + 'static {
    /// What the queue calls this kind. A rename strands queued jobs, so it outlives the type.
    const KIND: &'static str;
    /// How many runs it gets before it turns dead.
    const MAX_ATTEMPTS: u32 = 8;

    /// The lane it runs in, when jobs of a key must not overlap.
    fn lane(&self) -> Option<String> {
        None
    }

    fn run(self, cx: &Cx) -> impl Future<Output = anyhow::Result<()>> + Send;
}

type Run = Arc<
    dyn for<'a> Fn(&'a str, &'a Cx) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'a>>
        + Send
        + Sync,
>;

/// Every kind a worker runs, each domain adding its own.
#[derive(Default, Clone)]
pub struct Registry {
    runs: HashMap<&'static str, Run>,
}

impl Registry {
    #[must_use]
    pub fn register<J: Job>(mut self) -> Self {
        self.runs.insert(
            J::KIND,
            Arc::new(|payload, cx| {
                Box::pin(async move { serde_json::from_str::<J>(payload)?.run(cx).await })
            }),
        );
        self
    }

    pub(super) fn get(&self, kind: &str) -> Option<Run> {
        self.runs.get(kind).cloned()
    }
}
