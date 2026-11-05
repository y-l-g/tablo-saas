//! The team's billing page.

use jiff::Timestamp;
use tablo::{Page, PageDef, csrf, db::db, tenant_id, ui};
use topcoat::{
    Result,
    context::{Cx, context},
    icon::iconify::iconify_icon,
    router::{error::forbidden, request},
    view::{BoxView, View, ViewExt, view},
};

use super::{
    Billing, BillingInterval, CURRENCY, Entitlements, Plan, PlanId, Standing, account_of,
    refresh_customer, stripe::Invoice,
};
use crate::teams::{Member, TeamRole, current_role, may_manage_members};

/// Where the page lives.
pub const URL: &str = "/app/billing";

/// A minor-unit amount, as a person reads it.
pub fn money(amount: i64, currency: &str) -> String {
    format!(
        "{}.{:02} {}",
        amount / 100,
        amount.abs() % 100,
        currency.to_uppercase()
    )
}

/// A plan's name.
pub fn plan_name(cx: &Cx, plan: PlanId) -> String {
    match plan {
        PlanId::Free => tablo::t!(cx, "plan-free"),
        PlanId::Pro => tablo::t!(cx, "plan-pro"),
        PlanId::Business => tablo::t!(cx, "plan-business"),
    }
}

/// How a team stands, in a sentence, when there is something to say.
pub fn standing_notice(cx: &Cx, standing: Standing) -> Option<String> {
    let date = |at: Timestamp| at.strftime("%Y-%m-%d").to_string();
    match standing {
        Standing::Good => None,
        Standing::Trialing(until) => Some(tablo::t!(cx, "billing-trialing", date = date(until))),
        Standing::Ending(at) => Some(tablo::t!(cx, "billing-ending", date = date(at))),
        Standing::Grace(until) => Some(tablo::t!(cx, "billing-grace", date = date(until))),
        Standing::Lapsed => Some(tablo::t!(cx, "billing-lapsed")),
    }
}

/// The current plan, what it uses, what else is on sale, and past invoices.
pub struct BillingPage;

impl Page for BillingPage {
    fn declare() -> PageDef {
        PageDef::new()
            .slug("billing")
            .label(tablo::text!("page-billing"))
            .icon(iconify_icon!("lucide:credit-card"))
            .navigation_group(tablo::text!("navigation-team"))
            .access(may_manage_members)
    }

    async fn render(cx: &Cx) -> Result<impl View> {
        let team = tenant_id(cx).ok_or_else(forbidden)?;
        let billing = context::<Billing>(cx);
        let mut handle = db(cx)?;
        let mut account = account_of(&mut handle, team)
            .await
            .map_err(|error| topcoat::Error::msg(format!("{error:#}")))?;
        // Back from Checkout, the webhook may not have landed yet: ask Stripe now, so the buyer
        // sees what they bought. The webhook repeats this harmlessly.
        let returning = request::uri(cx)
            .query()
            .is_some_and(|query| query.contains("checkout=success"));
        if let (true, Some(found)) = (returning, account.as_ref()) {
            match refresh_customer(billing, &mut handle, &found.customer_id).await {
                Ok(refreshed) => account = refreshed,
                Err(error) => tracing::warn!(%error, "the return from Checkout could not refresh"),
            }
        }
        let entitlements = Entitlements::of(account.as_ref(), Timestamp::now());
        let seats = Member::filter(Member::fields().team_id().eq(team))
            .exec(&mut handle)
            .await?
            .len();
        // The page stands through a Stripe outage: invoices are the part that needs Stripe.
        let invoices: Vec<Invoice> = match &account {
            Some(account) if billing.is_enabled() => billing
                .gateway()
                .invoices(account.customer_id.clone())
                .await
                .unwrap_or_else(|error| {
                    tracing::warn!(%error, "the invoices could not load");
                    Vec::new()
                }),
            _ => Vec::new(),
        };
        let is_owner = current_role(cx) == Some(TeamRole::Owner);
        let subscribed = account
            .as_ref()
            .is_some_and(|account| !account.status.is_ended());
        let can_buy = is_owner && billing.is_enabled() && !subscribed;
        let can_manage = is_owner && billing.is_enabled() && account.is_some();
        let token = csrf::ensure_token(cx);
        let notice = standing_notice(cx, entitlements.standing);
        let current = plan_name(cx, entitlements.plan.id);
        let seat_line = match entitlements.plan.seat_limit {
            Some(limit) => tablo::t!(cx, "billing-seats-limited", used = seats, limit = limit),
            None => tablo::t!(cx, "billing-seats", used = seats),
        };
        let outline = ui::button_variants(ui::ButtonVariant::Outline, ui::ButtonSize::Md);
        Ok(view! {
            cx =>
            ui::page(
                ui::page_header(
                    ui::page_title((tablo::t!(cx, "page-billing")))
                    ui::page_description((tablo::t!(cx, "billing-description")))
                )
                ui::page_content(
                    <div class="flex flex-col gap-6">
                        if let Some(notice) = notice {
                            <p data-billing-notice="" class="rounded-lg border border-border bg-muted p-3 text-sm">
                                (notice)
                            </p>
                        }
                        ui::card(
                            ui::card_header(
                                ui::card_title((tablo::t!(cx, "billing-current")))
                                ui::card_description((seat_line))
                            )
                            ui::card_content(
                                <div class="flex items-center justify-between gap-4">
                                    <p class="text-lg font-semibold" data-billing-plan="">(current)</p>
                                    if can_manage {
                                        <form method="post" action="/app/billing/portal">
                                            (csrf::field(cx, &token))
                                            <button type="submit" class=(outline.clone())>
                                                (tablo::t!(cx, "billing-manage"))
                                            </button>
                                        </form>
                                    }
                                </div>
                            )
                        )
                        if !billing.is_enabled() {
                            <p class="text-sm text-muted-foreground">(tablo::t!(cx, "billing-off"))</p>
                        }
                        if can_buy {
                            (plan_cards(cx, &token))
                        }
                        if !invoices.is_empty() {
                            (invoice_list(cx, invoices))
                        }
                    </div>
                )
            )
        })
    }
}

/// The paid plans, each with a button per billing interval.
fn plan_cards<'a>(cx: &'a Cx, token: &str) -> BoxView<'a> {
    let token = token.to_string();
    let primary = ui::button_variants(ui::ButtonVariant::Primary, ui::ButtonSize::Md);
    let card_class = "flex flex-col gap-3";
    let plans: Vec<_> = Plan::paid().collect();
    view! {
        cx =>
                            <div class="grid gap-4 md:grid-cols-2">
                                for plan in plans {
                                    ui::card(
                                        ui::card_header(
                                            ui::card_title((plan_name(cx, plan.id)))
                                            ui::card_description((tablo::t!(
                                                cx,
                                                "billing-per-seat",
                                                price = money(plan.monthly_amount, CURRENCY)
                                            )))
                                        )
                                        ui::card_content(
                                            <div class=(card_class)>
                                                for interval in BillingInterval::BOTH {
                                                    <form method="post" action="/app/billing/checkout">
                                                        (csrf::field(cx, &token))
                                                        <input type="hidden" name="plan" value=(plan.slug())>
                                                        <input type="hidden" name="interval" value=(interval.as_str())>
                                                        <button type="submit" class=(primary.clone())>
                                                            (if interval == BillingInterval::Month {
                                                                tablo::t!(cx, "billing-choose-monthly")
                                                            } else {
                                                                tablo::t!(
                                                                    cx,
                                                                    "billing-choose-yearly",
                                                                    price = money(plan.yearly_amount, CURRENCY)
                                                                )
                                                            })
                                                        </button>
                                                    </form>
                                                }
                                            </div>
                                        )
                                    )
                                }
                            </div>
    }
    .boxed()
}

/// Past invoices, each linking to Stripe's hosted copy.
fn invoice_list(cx: &Cx, invoices: Vec<Invoice>) -> BoxView<'_> {
    view! {
        cx =>
                            ui::card(
                                ui::card_header(ui::card_title((tablo::t!(cx, "billing-invoices"))))
                                ui::card_content(
                                    <ul class="flex flex-col gap-2 text-sm">
                                        for invoice in invoices {
                                            <li class="flex justify-between gap-4">
                                                <span>(invoice.number.clone().unwrap_or_default())</span>
                                                <span>(money(invoice.total, &invoice.currency))</span>
                                                if let Some(url) = invoice.hosted_invoice_url.clone() {
                                                    <a href=(url) class="underline" rel="noopener">
                                                        (tablo::t!(cx, "billing-invoice-open"))
                                                    </a>
                                                }
                                            </li>
                                        }
                                    </ul>
                                )
                            )
    }
    .boxed()
}
