//! Every kind of background work the app does, in one list, and what a job reads when it runs.

use std::time::Duration;

use tablo::{
    PanelHandle,
    jobs::{AuthMailQueue, Worker},
};
use topcoat::mail::{Mailer, Transport};

use crate::billing::{
    Billing,
    jobs::{
        ProcessStripeEvent, PruneStripeEvents, ReconcileBilling, RefreshCustomer,
        SendBillingNotice, SyncSeats,
    },
};
use crate::platform::config::Settings;
use crate::teams::{CancelSubscription, PruneInvitations, SendInvitation};

/// A worker running every job kind against the mounted panel's context, with what the jobs read
/// beyond it. A worker claims only the kinds it registers, so a kind missing here waits in the
/// queue forever. `mail` delivers, and `queue` opens what the panels' auth mail queued.
pub fn worker<T: Transport + Clone + Send + Sync + 'static>(
    panel: &PanelHandle,
    settings: Settings,
    billing: Billing,
    mail: T,
    queue: &AuthMailQueue,
) -> topcoat::Result<Worker> {
    Ok(Worker::new(panel)?
        .auth_mail(queue)
        .register::<SendInvitation>()
        .register::<CancelSubscription>()
        .register::<ProcessStripeEvent>()
        .register::<SendBillingNotice>()
        .register::<SyncSeats>()
        .register::<RefreshCustomer>()
        .every(Duration::from_hours(24), PruneInvitations)
        .every(Duration::from_hours(6), ReconcileBilling)
        .every(Duration::from_hours(24), PruneStripeEvents)
        .context(move |cx| {
            cx.with(settings.clone())
                .with(billing.clone())
                .with(Mailer::new(mail.clone()))
        }))
}
