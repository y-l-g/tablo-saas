//! The probes an orchestrator polls. Liveness asks only that the process answers: a database
//! outage must not restart every replica. Readiness takes a replica out of rotation until the
//! database answers.

use tablo::db::db;
use topcoat::{
    Result,
    context::Cx,
    router::{error::service_unavailable, route},
};

use super::database;

/// How soon a load balancer should ask again.
const RETRY_AFTER_SECONDS: u64 = 5;

/// `GET /healthz`.
#[route(GET "/healthz")]
async fn liveness() -> Result<&'static str> {
    Ok("ok")
}

/// `GET /readyz`.
#[route(GET "/readyz")]
async fn readiness(cx: &Cx) -> Result<&'static str> {
    if database::ping(&db(cx)?).await {
        Ok("ready")
    } else {
        Err(service_unavailable(RETRY_AFTER_SECONDS).into())
    }
}
