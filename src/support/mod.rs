//! Staff acting as a customer, to see what they see: Tablo's impersonation on the app panel, and
//! the staff-only page that starts it.
//!
//! Staff work in the back office, but a customer's pages live in the app panel, and Tablo acts as
//! another user of the same panel only. So a member of staff signs in to the app panel too, which
//! requires a second factor of staff there, finds the customer on [`SupportPage`] and acts as them
//! for at most [`LIFETIME`].
//!
//! What it may reach is narrow: staff act as customers, never as staff, and Tablo keeps them off
//! the target's profile, factors and team settings. Membership and billing refuse too, see
//! [`is_impersonated`](crate::accounts::is_impersonated).
//! The start and the stop land in the app panel's audit log, which the back office reads.

use std::time::Duration;

use serde::Deserialize;
use tablo::{Impersonation, Page, PageDef, PanelUser, auth, db::db, ui};
use topcoat::{
    Result,
    context::Cx,
    icon::iconify::iconify_icon,
    router::request,
    view::{BoxView, View, ViewExt, attributes, view},
};

use crate::accounts::{Accounts, SignedIn, User};

/// Where the page lives.
pub const URL: &str = "/app/support";

/// Where a member of staff adds the app panel's second factor.
const TWO_FACTOR_URL: &str = "/app/settings/two-factor";

/// How long acting as a customer lasts before the actor is themselves again: long enough to
/// reproduce a report, short enough that a forgotten tab ends on its own.
pub const LIFETIME: Duration = Duration::from_mins(30);

/// How many customers a search lists. Each row loads its account's teams, so a short page keeps
/// the search quick; a narrower term finds the rest.
const RESULTS: usize = 20;

/// Who acts as whom on the app panel.
///
/// Tablo asks the rule again on every request, so revoking a staff role, removing the factor or
/// granting the customer a staff role ends a session at once. Staff belong to none of a
/// customer's teams, hence `across_tenants`.
pub fn impersonation() -> Impersonation<SignedIn> {
    Impersonation::new(|_cx, actor: &SignedIn, target: &SignedIn| may_act_as(actor, target))
        .lifetime(LIFETIME)
        .across_tenants()
}

/// Whether `actor` may act as `target`: a member of staff with a second factor on the app panel,
/// acting as a customer. Acting as staff would hand one role another's back-office reach.
pub fn may_act_as(actor: &SignedIn, target: &SignedIn) -> bool {
    actor.user.staff_role.is_some() && actor.app_factor && target.user.staff_role.is_none()
}

fn is_staff(cx: &Cx) -> bool {
    auth::user::<SignedIn>(cx).is_some_and(|signed| signed.user.staff_role.is_some())
}

/// The query string a search submits.
#[derive(Deserialize, Default)]
struct Search {
    #[serde(default)]
    q: String,
}

/// One customer a search found.
struct Found<'a> {
    name: String,
    email: String,
    teams: String,
    act_as: BoxView<'a>,
}

/// Finds a customer by name or address and acts as them. Only staff see it, and only as
/// themselves: while acting as a customer, the signed-in user is that customer.
pub struct SupportPage;

impl Page for SupportPage {
    fn declare() -> PageDef {
        PageDef::new()
            .slug("support")
            .label(tablo::text!("page-support"))
            .icon(iconify_icon!("lucide:shield-check"))
            .access(is_staff)
    }

    async fn render(cx: &Cx) -> Result<impl View> {
        let signed = auth::require_user::<SignedIn>(cx)?;
        let search: Search = request::uri(cx)
            .query()
            .and_then(|query| serde_urlencoded::from_str(query).ok())
            .unwrap_or_default();
        let term = search.q.trim();
        let found = if term.is_empty() {
            Vec::new()
        } else {
            customers(cx, term).await?
        };
        let searched = !term.is_empty();
        let empty = searched && found.is_empty();
        Ok(view! {
            ui::page(
                ui::page_header(
                    ui::page_title((tablo::t!(cx, "page-support")))
                    ui::page_description((tablo::t!(cx, "support-description", minutes = LIFETIME.as_secs() / 60)))
                )
                ui::page_content(
                    <div class="flex flex-col gap-6">
                        if !signed.app_factor {
                            <p data-support-notice="" class="rounded-lg border border-border bg-muted p-3 text-sm">
                                <a href=(TWO_FACTOR_URL) class="underline">
                                    (tablo::t!(cx, "support-needs-factor"))
                                </a>
                            </p>
                        }
                        <form method="get" action=(URL) class="flex gap-2" role="search">
                            ui::input(attrs: attributes! {
                                type="search"
                                name="q"
                                value=(search.q.clone())
                                placeholder=(tablo::t!(cx, "support-search"))
                                aria-label=(tablo::t!(cx, "support-search"))
                            })
                            ui::button(
                                variant: ui::ButtonVariant::Secondary,
                                attrs: attributes! { type="submit" },
                                (tablo::t!(cx, "support-find"))
                            )
                        </form>
                        if empty {
                            <p class="text-sm text-muted-foreground">(tablo::t!(cx, "support-none"))</p>
                        }
                        if !found.is_empty() {
                            (results(cx, found))
                        }
                    </div>
                )
            )
        })
    }
}

/// The customers whose name or address holds `term`, ignoring case, newest first, with a button
/// acting as each that renders only where the rule allows it.
///
/// Tablo's `contains_expr` is a plain `LIKE`, which ignores case on SQLite but not on PostgreSQL,
/// where "lovelace" would miss "Ada Lovelace". So `ILIKE` where the database has it; SQLite has
/// not, and Toasty refuses it there, but its `LIKE` already folds ASCII.
async fn customers<'a>(cx: &'a Cx, term: &str) -> Result<Vec<Found<'a>>> {
    let mut handle = db(cx)?;
    let pattern = like_pattern(term);
    let matches = if handle.capability().native_ilike {
        let fields = User::fields();
        fields
            .email()
            .ilike_with_escape(pattern.clone(), LIKE_ESCAPE)
            .or(fields.name().ilike_with_escape(pattern, LIKE_ESCAPE))
    } else {
        let fields = User::fields();
        fields
            .email()
            .like_with_escape(pattern.clone(), LIKE_ESCAPE)
            .or(fields.name().like_with_escape(pattern, LIKE_ESCAPE))
    };
    let users = User::filter(User::fields().staff_role().is_none().and(matches))
        .order_by(User::fields().created_at().desc())
        .limit(RESULTS)
        .exec(&mut handle)
        .await?;
    let mut found = Vec::with_capacity(users.len());
    for user in users {
        let customer = Accounts::signed_in(user, &mut handle).await?;
        let teams = customer
            .tenants()
            .iter()
            .map(|membership| membership.name.clone())
            .collect::<Vec<_>>()
            .join(", ");
        found.push(Found {
            act_as: auth::impersonate_button(cx, &customer),
            name: customer.user.name,
            email: customer.user.email,
            teams,
        });
    }
    Ok(found)
}

const LIKE_ESCAPE: char = '\\';

/// `term` matched anywhere, with `LIKE`'s wildcards escaped so a `%` finds a `%`.
fn like_pattern(term: &str) -> String {
    let mut pattern = String::with_capacity(term.len() + 2);
    pattern.push('%');
    for c in term.chars() {
        if matches!(c, LIKE_ESCAPE | '%' | '_') {
            pattern.push(LIKE_ESCAPE);
        }
        pattern.push(c);
    }
    pattern.push('%');
    pattern
}

fn results<'a>(cx: &'a Cx, found: Vec<Found<'a>>) -> BoxView<'a> {
    view! {
        ui::card(
            ui::table(
                ui::table_header(
                    ui::table_row(
                        ui::table_head((tablo::t!(cx, "support-name")))
                        ui::table_head((tablo::t!(cx, "support-email")))
                        ui::table_head((tablo::t!(cx, "support-teams")))
                        ui::table_head()
                    )
                )
                ui::table_body(
                    for customer in found {
                        ui::table_row(
                            attrs: attributes! { data-customer=(customer.email.clone()) },
                            ui::table_cell((customer.name))
                            ui::table_cell((customer.email))
                            ui::table_cell((customer.teams))
                            ui::table_cell((customer.act_as))
                        )
                    }
                )
            )
        )
    }
    .boxed()
}
