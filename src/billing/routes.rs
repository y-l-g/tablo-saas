//! The plan cards' buttons, which leave for Stripe Checkout. They are a plain route, not a Tablo
//! action: an action renders its own button, in a page's header or a row, and asks for its input
//! in a dialog, where each card's button carries its plan and interval.

use serde::Deserialize;
use tablo::{auth, csrf, db::db, tenant_id};
use topcoat::{
    Result,
    context::{Cx, context},
    router::{
        Body,
        error::{bad_request, forbidden, see_other, service_unavailable},
        response::{IntoResponse, Response},
        route, to_bytes,
    },
};

use super::{
    Billing, BillingInterval, Plan, PlanId, ensure_account, lookup_key, page::URL,
    stripe::NewCheckout,
};
use crate::accounts::SignedIn;
use crate::platform::config::Settings;
use crate::teams::{Member, TeamRole, current_role};

/// How long to tell a browser to wait while Stripe is down.
const RETRY_SECONDS: u64 = 60;

const BODY_LIMIT: usize = 16 * 1024;

#[derive(Deserialize)]
struct CheckoutForm {
    csrf_token: Option<String>,
    plan: String,
    interval: String,
}

/// Lets the request through when it comes from the team's owner and carries the form's token.
fn owner<'a>(cx: &'a Cx, token: Option<&str>) -> Result<&'a SignedIn> {
    csrf::verify(cx, token)?;
    let signed = auth::require_user::<SignedIn>(cx)?;
    if current_role(cx) != Some(TeamRole::Owner) {
        return Err(forbidden().into());
    }
    Ok(signed)
}

fn invalid() -> topcoat::Error {
    bad_request("invalid billing form").into()
}

fn unavailable(error: impl std::fmt::Display) -> topcoat::Error {
    tracing::error!(%error, "a billing step failed");
    service_unavailable(RETRY_SECONDS).into()
}

async fn parse<T: for<'de> Deserialize<'de>>(body: Body) -> Result<T> {
    let bytes = to_bytes(body, BODY_LIMIT).await?;
    serde_urlencoded::from_bytes(&bytes).map_err(|_| invalid())
}

#[route(POST "/app/settings/billing/checkout")]
async fn checkout(cx: &Cx, body: Body) -> Result<Response> {
    let form: CheckoutForm = parse(body).await?;
    let signed = owner(cx, form.csrf_token.as_deref())?;
    let plan = Plan::by_slug(&form.plan)
        .filter(|plan| plan.id != PlanId::Free)
        .ok_or_else(invalid)?;
    let interval = BillingInterval::from_stripe(&form.interval).ok_or_else(invalid)?;
    let team = tenant_id(cx).ok_or_else(forbidden)?;
    let membership = auth::membership(cx).ok_or_else(forbidden)?;
    let mut handle = db(cx)?;
    let account = ensure_account(cx, &mut handle, team, &membership.name, &signed.user.email)
        .await
        .map_err(unavailable)?;
    if !account.status.is_ended() {
        // A second subscription would bill twice: the portal changes the running one.
        return see_other(URL).into_response(cx);
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
        .exec(&mut handle)
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
    see_other(url).into_response(cx)
}
