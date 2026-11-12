//! The plan cards' buttons, which leave for Stripe Checkout: one header action of the billing
//! page, placed on each card with that card's plan and interval.

use tablo::{FormInput, HeaderAction, HeaderActionDef, Outcome, WriteError, auth, tenant_id};
use topcoat::context::{Cx, context};

use super::{
    Billing, BillingInterval, Plan, PlanId, ensure_account, lookup_key, page::URL,
    stripe::NewCheckout,
};
use crate::accounts::{SignedIn, is_impersonated};
use crate::platform::config::Settings;
use crate::teams::{Member, TeamRole, current_role};

/// What a plan card's button carries: which plan, billed how often.
#[derive(FormInput)]
pub struct Choice {
    /// The plan's slug.
    pub plan: String,
    /// The billing interval, as Stripe spells it.
    pub interval: String,
}

/// Opens a Checkout session for the team the session acts for, and lands on it.
pub struct Checkout;

impl HeaderAction for Checkout {
    type Input = Choice;
    const NAME: &'static str = "checkout";

    fn declare() -> HeaderActionDef {
        HeaderActionDef::new()
            // The owner alone, and not staff acting as them: Checkout charges the customer's card.
            .can_run(|cx| {
                current_role(cx) == Some(TeamRole::Owner)
                    && !is_impersonated(cx)
                    && context::<Billing>(cx).is_enabled()
            })
    }

    async fn run(
        cx: &Cx,
        choice: Choice,
        ex: &mut dyn toasty::Executor,
    ) -> Result<Outcome, WriteError<Choice>> {
        let invalid = || WriteError::declined(tablo::text!("error-plan-invalid"));
        let plan = Plan::by_slug(&choice.plan)
            .filter(|plan| plan.id != PlanId::Free)
            .ok_or_else(invalid)?;
        let interval = BillingInterval::from_stripe(&choice.interval).ok_or_else(invalid)?;
        let signed = auth::require_user::<SignedIn>(cx)?;
        let team =
            tenant_id(cx).ok_or_else(|| WriteError::declined(tablo::text!("error-no-team")))?;
        let membership = auth::membership(cx)
            .ok_or_else(|| WriteError::declined(tablo::text!("error-no-team")))?;
        let account = ensure_account(cx, &mut *ex, team, &membership.name, &signed.user.email)
            .await
            .map_err(unavailable)?;
        if !account.status.is_ended() {
            // A second subscription would bill twice: the portal changes the running one.
            return Ok(Outcome::done());
        }

        let billing = context::<Billing>(cx);
        let key = lookup_key(&plan, interval);
        let price = billing
            .gateway()
            .price_by_lookup_key(key.clone())
            .await
            .map_err(unavailable)?
            .ok_or_else(|| unavailable(format!("no price `{key}`: run `billing sync-catalog`")))?;
        let seats = Member::filter(Member::fields().team_id().eq(team))
            .exec(&mut *ex)
            .await?
            .len();
        let settings = context::<Settings>(cx);
        let url = billing
            .gateway()
            .create_checkout(NewCheckout {
                customer: account.customer_id,
                team: team.get().to_string(),
                price: price.id,
                seats: i64::try_from(seats.max(1)).map_err(|_| invalid())?,
                // A team that subscribed before has had its trial.
                trial_days: account
                    .subscription_id
                    .is_none()
                    .then_some(settings.trial_days)
                    .flatten(),
                automatic_tax: settings.automatic_tax,
                success_url: format!("{}{URL}?checkout=success", settings.base_url),
                cancel_url: format!("{}{URL}", settings.base_url),
            })
            .await
            .map_err(unavailable)?;
        Ok(Outcome::done().redirect_off_site(url))
    }
}

fn unavailable(error: impl std::fmt::Display) -> WriteError<Choice> {
    tracing::error!(%error, "a billing step failed");
    WriteError::declined(tablo::text!("error-billing-unavailable"))
}
