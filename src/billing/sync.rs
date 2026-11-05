//! Keeping the mirror true: every path here asks Stripe what is so, and writes that.

use jiff::Timestamp;
use tablo::TenantId;
use topcoat::context::{Cx, context};

use super::{
    Billing, BillingAccount, BillingInterval, BillingStatus, PlanId, plan_of_lookup_key,
    stripe::{NewCustomer, Subscription},
};

fn timestamp(seconds: i64) -> Option<Timestamp> {
    Timestamp::from_second(seconds).ok()
}

/// The team's account, creating its Stripe customer on the first paid step. The Stripe call
/// carries the team as its idempotency key, so a race makes one customer; the row's unique team
/// makes one account.
pub async fn ensure_account(
    cx: &Cx,
    ex: &mut dyn toasty::Executor,
    team: TenantId,
    name: &str,
    email: &str,
) -> anyhow::Result<BillingAccount> {
    if let Some(account) = account_of(ex, team).await? {
        return Ok(account);
    }
    let customer = context::<Billing>(cx)
        .gateway()
        .create_customer(NewCustomer {
            team: team.get().to_string(),
            name: name.to_string(),
            email: email.to_string(),
        })
        .await?;
    BillingAccount::upsert_by_team_id(team)
        .on_create(|create| {
            create
                .customer_id(customer)
                .plan(PlanId::Free)
                .status(BillingStatus::None)
                .cancel_at_period_end(false)
        })
        .or_ignore()
        .exec(&mut *ex)
        .await?;
    account_of(ex, team)
        .await?
        .ok_or_else(|| anyhow::anyhow!("the account was just written"))
}

/// The team's account, if it ever had one.
pub async fn account_of(
    ex: &mut dyn toasty::Executor,
    team: TenantId,
) -> anyhow::Result<Option<BillingAccount>> {
    Ok(
        BillingAccount::filter(BillingAccount::fields().team_id().eq(team))
            .first()
            .exec(ex)
            .await?,
    )
}

/// The subscription that decides a customer's plan: a running one if any, else the latest.
fn deciding(mut subscriptions: Vec<Subscription>) -> Option<Subscription> {
    subscriptions.sort_by_key(|subscription| std::cmp::Reverse(subscription.created));
    let running = subscriptions.iter().position(|subscription| {
        BillingStatus::from_stripe(&subscription.status).is_some_and(|status| !status.is_ended())
    });
    match running {
        Some(index) => Some(subscriptions.swap_remove(index)),
        None => subscriptions.into_iter().next(),
    }
}

/// What a mirror row should say about a subscription.
struct Mirror {
    plan: PlanId,
    status: BillingStatus,
    subscription: String,
    item: Option<String>,
    interval: Option<BillingInterval>,
    seats: Option<i64>,
    period_end: Option<Timestamp>,
    cancel_at_period_end: bool,
    trial_end: Option<Timestamp>,
}

fn read(subscription: Subscription) -> anyhow::Result<Mirror> {
    let status = BillingStatus::from_stripe(&subscription.status)
        .ok_or_else(|| anyhow::anyhow!("unknown status `{}`", subscription.status))?;
    let line = subscription.items.data.first();
    let plan = match line.and_then(|line| line.price.lookup_key.as_deref()) {
        Some(key) => {
            plan_of_lookup_key(key)
                .ok_or_else(|| anyhow::anyhow!("price `{key}` sells no plan in the catalog"))?
                .id
        }
        // A price without a lookup key was not made by `billing sync-catalog`: failing puts the
        // job on the dead list, where a person sees it, rather than guessing a plan.
        None => anyhow::bail!(
            "subscription `{}` has no priced line with a lookup key",
            subscription.id
        ),
    };
    Ok(Mirror {
        plan,
        status,
        item: line.map(|line| line.id.clone()),
        interval: line
            .and_then(|line| line.price.recurring.as_ref())
            .and_then(|recurring| BillingInterval::from_stripe(&recurring.interval)),
        seats: line.and_then(|line| line.quantity),
        period_end: line.and_then(|line| timestamp(line.current_period_end)),
        cancel_at_period_end: subscription.cancel_at_period_end,
        trial_end: subscription.trial_end.and_then(timestamp),
        subscription: subscription.id,
    })
}

/// Rewrites the customer's mirror from Stripe. It reads Stripe before opening any write, and
/// holds no transaction across the call. Safe to run at any time, in any order, as often as asked.
pub async fn refresh_customer(
    billing: &Billing,
    ex: &mut dyn toasty::Executor,
    customer: &str,
) -> anyhow::Result<Option<BillingAccount>> {
    let Some(mut account) =
        BillingAccount::filter(BillingAccount::fields().customer_id().eq(customer))
            .first()
            .exec(&mut *ex)
            .await?
    else {
        // Another app sharing the Stripe account, or a customer deleted here.
        tracing::warn!(customer, "a Stripe customer with no account here");
        return Ok(None);
    };
    let subscriptions = billing
        .gateway()
        .customer_subscriptions(customer.to_string())
        .await?;
    let now = Timestamp::now();
    let Some(subscription) = deciding(subscriptions) else {
        account.update().synced_at(Some(now)).exec(&mut *ex).await?;
        return Ok(Some(account));
    };
    let mirror = read(subscription)?;
    let past_due_since =
        (mirror.status == BillingStatus::PastDue).then(|| account.past_due_since.unwrap_or(now));
    account
        .update()
        .synced_at(Some(now))
        .plan(mirror.plan)
        .status(mirror.status)
        .subscription_id(Some(mirror.subscription))
        .subscription_item_id(mirror.item)
        .billing_interval(mirror.interval)
        .seats(mirror.seats)
        .current_period_end(mirror.period_end)
        .cancel_at_period_end(mirror.cancel_at_period_end)
        .trial_ends_at(mirror.trial_end)
        .past_due_since(past_due_since)
        .exec(&mut *ex)
        .await?;
    Ok(Some(account))
}
