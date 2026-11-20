//! The button that opens Stripe's billing portal, where an owner changes the plan, the card or
//! the seats Stripe bills.

use tablo::{HeaderAction, HeaderActionDef, Outcome, WriteError, tenant_id};
use topcoat::context::{Cx, context};

use super::{Billing, BillingPage, account_of};
use crate::platform::http::public_url;
use crate::teams::{TeamRole, current_role};

/// Opens a portal session for the team the session acts for, and lands on it.
pub struct ManageBilling;

impl HeaderAction for ManageBilling {
    type Input = ();
    const NAME: &'static str = "manage-billing";

    fn declare() -> HeaderActionDef {
        HeaderActionDef::new()
            // The owner alone: the portal cancels and changes cards. Tablo refuses it to staff
            // acting as them, as every write.
            .can_run(|cx| {
                current_role(cx) == Some(TeamRole::Owner) && context::<Billing>(cx).is_enabled()
            })
    }

    async fn run(
        cx: &Cx,
        (): (),
        ex: &mut dyn toasty::Executor,
    ) -> Result<Outcome, WriteError<()>> {
        let team =
            tenant_id(cx).ok_or_else(|| WriteError::declined(tablo::text!("error-no-team")))?;
        let account = account_of(ex, team).await.map_err(unavailable)?;
        let Some(account) = account else {
            return Err(WriteError::declined(tablo::text!(
                "error-no-billing-account"
            )));
        };
        let page = public_url(cx, tablo::url::page::<BillingPage>(cx)).map_err(unavailable)?;
        let url = context::<Billing>(cx)
            .gateway()
            .create_portal_session(account.customer_id, page)
            .await
            .map_err(unavailable)?;
        Ok(Outcome::done().redirect_off_site(url))
    }
}

fn unavailable(error: impl std::fmt::Display) -> WriteError<()> {
    tracing::error!(%error, "a billing step failed");
    WriteError::declined(tablo::text!("error-billing-unavailable"))
}
