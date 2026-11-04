//! The billing work that happens off the request.

use jiff::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};
use tablo::{TenantId, db::db};
use topcoat::context::{Cx, context};

use super::{
    Billing, BillingAccount, StripeEvent, refresh_customer,
    stripe::{SeatChange, StripeError},
};
use crate::accounts::User;
use crate::jobs::{Job, enqueue};
use crate::platform::{
    config::Settings,
    mail::{Message, send_message},
};
use crate::teams::{Member, TeamRole};

/// How long a seat change waits, so a burst of them makes one Stripe call.
const SEAT_DEBOUNCE: SignedDuration = SignedDuration::from_secs(30);

/// How long the event inbox remembers. Stripe redelivers for three days.
const EVENT_MEMORY: SignedDuration = SignedDuration::from_hours(30 * 24);

fn handle(cx: &Cx) -> anyhow::Result<toasty::Db> {
    db(cx).map_err(|error| anyhow::anyhow!("{error:#}"))
}

/// Acts on a stored Stripe event: rewrites the mirror from the API, then tells the owner what they
/// need to hear. The lane keeps one customer's events in order and the sync one at a time.
#[derive(Serialize, Deserialize)]
pub struct ProcessStripeEvent {
    pub event: String,
    pub customer: String,
}

impl Job for ProcessStripeEvent {
    const KIND: &'static str = "process-stripe-event";

    fn lane(&self) -> Option<String> {
        Some(format!("customer:{}", self.customer))
    }

    async fn run(self, cx: &Cx) -> anyhow::Result<()> {
        let mut handle = handle(cx)?;
        let Some(event) = StripeEvent::filter_by_id(&self.event)
            .first()
            .exec(&mut handle)
            .await?
        else {
            return Ok(());
        };
        if event.processed_at.is_some() {
            return Ok(());
        }
        let account = refresh_customer(context::<Billing>(cx), &mut handle, &self.customer).await?;
        let mut tx = handle.transaction().await?;
        if let (Some(account), Some(notice)) = (account, Notice::of(&event.kind)) {
            enqueue(&SendBillingNotice {
                team: account.team_id.get(),
                notice,
            })
            .dedupe(format!("notice:{}", self.event))
            .exec(&mut tx)
            .await?;
        }
        StripeEvent::filter_by_id(&self.event)
            .update()
            .processed_at(Some(Timestamp::now()))
            .exec(&mut tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
}

/// What the owner is told about their subscription.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Notice {
    PaymentFailed,
    TrialEnding,
}

impl Notice {
    fn of(kind: &str) -> Option<Self> {
        match kind {
            "invoice.payment_failed" => Some(Self::PaymentFailed),
            "customer.subscription.trial_will_end" => Some(Self::TrialEnding),
            _ => None,
        }
    }
}

/// Mails the team's owner.
#[derive(Serialize, Deserialize)]
pub struct SendBillingNotice {
    pub team: uuid::Uuid,
    pub notice: Notice,
}

impl Job for SendBillingNotice {
    const KIND: &'static str = "send-billing-notice";

    async fn run(self, cx: &Cx) -> anyhow::Result<()> {
        let mut handle = handle(cx)?;
        let owner = Member::filter(
            Member::fields()
                .team_id()
                .eq(TenantId::from(self.team))
                .and(Member::fields().role().eq(TeamRole::Owner)),
        )
        .first()
        .exec(&mut handle)
        .await?;
        let Some(owner) = owner else {
            return Ok(());
        };
        let user = User::get_by_id(&mut handle, owner.user_id).await?;
        let url = format!("{}/app/billing", context::<Settings>(cx).base_url);
        let (subject, body) = match self.notice {
            Notice::PaymentFailed => (
                tablo::t!(cx, "mail-payment-failed-subject"),
                tablo::t!(cx, "mail-payment-failed-body"),
            ),
            Notice::TrialEnding => (
                tablo::t!(cx, "mail-trial-ending-subject"),
                tablo::t!(cx, "mail-trial-ending-body"),
            ),
        };
        send_message(
            cx,
            Message {
                to: &user.email,
                subject: &subject,
                title: &subject,
                body: &body,
                action: &tablo::t!(cx, "mail-billing-action"),
                url: &url,
            },
        )
        .await
    }
}

/// Queues a seat sync for `team`, on the caller's transaction.
pub async fn sync_seats_later(ex: &mut dyn toasty::Executor, team: TenantId) -> anyhow::Result<()> {
    enqueue(&SyncSeats { team: team.get() })
        .delay(SEAT_DEBOUNCE)
        .dedupe(format!("seats:{}", team.get()))
        .exec(ex)
        .await
}

/// Makes the subscription's quantity match the member list.
#[derive(Serialize, Deserialize)]
pub struct SyncSeats {
    pub team: uuid::Uuid,
}

impl Job for SyncSeats {
    const KIND: &'static str = "sync-seats";

    fn lane(&self) -> Option<String> {
        Some(format!("seats:{}", self.team))
    }

    async fn run(self, cx: &Cx) -> anyhow::Result<()> {
        let mut handle = handle(cx)?;
        let team = TenantId::from(self.team);
        let Some(account) = super::account_of(&mut handle, team).await? else {
            return Ok(());
        };
        let (Some(subscription), Some(item), false) = (
            account.subscription_id.clone(),
            account.subscription_item_id.clone(),
            account.status.is_ended(),
        ) else {
            return Ok(());
        };
        let members = Member::filter(Member::fields().team_id().eq(team))
            .exec(&mut handle)
            .await?;
        let seats = i64::try_from(members.len().max(1))?;
        if account.seats == Some(seats) {
            return Ok(());
        }
        let billing = context::<Billing>(cx);
        billing
            .gateway()
            .set_seats(SeatChange {
                subscription,
                item,
                seats,
            })
            .await?;
        refresh_customer(billing, &mut handle, &account.customer_id).await?;
        Ok(())
    }
}

/// Refreshes every account from Stripe, which catches an event that never arrived: an endpoint
/// that was down past Stripe's retries, or a change made while it was disabled.
#[derive(Clone, Serialize, Deserialize)]
pub struct ReconcileBilling;

impl Job for ReconcileBilling {
    const KIND: &'static str = "reconcile-billing";

    async fn run(self, cx: &Cx) -> anyhow::Result<()> {
        if !context::<Billing>(cx).is_enabled() {
            return Ok(());
        }
        let mut handle = handle(cx)?;
        let accounts = BillingAccount::all().exec(&mut handle).await?;
        for account in accounts
            .into_iter()
            .filter(|account| !account.status.is_ended())
        {
            enqueue(&RefreshCustomer {
                customer: account.customer_id.clone(),
            })
            .dedupe(format!("refresh:{}", account.customer_id))
            .exec(&mut handle)
            .await?;
        }
        Ok(())
    }
}

/// Rewrites one customer's mirror.
#[derive(Serialize, Deserialize)]
pub struct RefreshCustomer {
    pub customer: String,
}

impl Job for RefreshCustomer {
    const KIND: &'static str = "refresh-customer";

    fn lane(&self) -> Option<String> {
        Some(format!("customer:{}", self.customer))
    }

    async fn run(self, cx: &Cx) -> anyhow::Result<()> {
        let mut handle = handle(cx)?;
        match refresh_customer(context::<Billing>(cx), &mut handle, &self.customer).await {
            Err(error) if is_not_configured(&error) => Ok(()),
            result => result.map(|_| ()),
        }
    }
}

fn is_not_configured(error: &anyhow::Error) -> bool {
    matches!(
        error.downcast_ref::<StripeError>(),
        Some(StripeError::NotConfigured)
    )
}

/// Forgets events past Stripe's redelivery window.
#[derive(Clone, Serialize, Deserialize)]
pub struct PruneStripeEvents;

impl Job for PruneStripeEvents {
    const KIND: &'static str = "prune-stripe-events";

    async fn run(self, cx: &Cx) -> anyhow::Result<()> {
        let cutoff = Timestamp::now().checked_sub(EVENT_MEMORY)?;
        StripeEvent::filter(StripeEvent::fields().received_at().le(cutoff))
            .delete()
            .exec(&mut handle(cx)?)
            .await?;
        Ok(())
    }
}
