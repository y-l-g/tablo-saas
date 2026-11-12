//! Staff acting as a customer, to see what they see: Tablo's impersonation on the app panel, and
//! the staff-only list of customers that starts it.
//!
//! Staff work in the back office, but a customer's pages live in the app panel, and Tablo acts as
//! another user of the same panel only. So a member of staff signs in to the app panel too, which
//! requires a second factor of staff there, finds the customer on [`SupportResource`] and acts as
//! them for at most [`LIFETIME`].
//!
//! What it may reach is narrow: staff act as customers, never as staff, and Tablo keeps them off
//! the target's profile, factors and team settings. Membership and billing refuse too, see
//! [`is_impersonated`](crate::accounts::is_impersonated).
//! The start and the stop land in the app panel's audit log, which the back office reads.

use std::time::Duration;

use tablo::{Impersonation, Widget, WidgetDef, auth, prelude::*};
use topcoat::{
    context::Cx,
    icon::iconify::iconify_icon,
    view::{View, view},
};

use crate::accounts::{Accounts, SignedIn, User};

/// Where a member of staff adds the app panel's second factor.
const TWO_FACTOR_URL: &str = "/app/settings/two-factor";

/// Where acting as a customer lands: their panel's home, as they see it.
const APP_HOME: &str = "/app";

/// How long acting as a customer lasts before the actor is themselves again: long enough to
/// reproduce a report, short enough that a forgotten tab ends on its own.
pub const LIFETIME: Duration = Duration::from_mins(30);

/// Who acts as whom on the app panel.
///
/// Tablo asks the rule again on every request, so revoking a staff role, removing the factor or
/// granting the customer a staff role ends a session at once. Staff belong to none of a
/// customer's teams, hence `across_tenants`.
pub fn impersonation() -> Impersonation<SignedIn> {
    Impersonation::new(|_cx, actor: &SignedIn, target: &SignedIn| may_act_as(actor, &target.user))
        .lifetime(LIFETIME)
        .across_tenants()
}

/// Whether `actor` may act as `target`: a member of staff with a second factor on the app panel,
/// acting as a customer. Acting as staff would hand one role another's back-office reach.
pub fn may_act_as(actor: &SignedIn, target: &User) -> bool {
    actor.user.staff_role.is_some() && actor.app_factor && target.staff_role.is_none()
}

/// Whether the signed-in user is staff. While they act as a customer, the signed-in user is that
/// customer, so the list closes until they stop.
fn is_staff(cx: &Cx) -> bool {
    auth::user::<SignedIn>(cx).is_some_and(|signed| signed.user.staff_role.is_some())
}

/// Every customer, found by name or address as staff type, each with a button acting as them.
///
/// Customers are users with no staff role. The app panel serves the resource to every team, so the
/// policy alone keeps it to staff: it lists every customer of every team.
pub struct SupportResource;

impl Resource for SupportResource {
    type Model = User;
    type Form = NoForm<User>;

    fn declare() -> ResourceDef<Self> {
        let col = User::columns();
        ResourceDef::new()
            .slug("support")
            .label(tablo::text!("page-support"))
            .plural_label(tablo::text!("page-support"))
            .icon(iconify_icon!("lucide:shield-check"))
            .tenancy(Tenancy::none())
            .query(User::fields().staff_role().is_none())
            .record_title(lens!(User.email))
            .policy(
                Abilities::<User>::new()
                    .view_any(is_staff)
                    .view(|cx, _| is_staff(cx))
                    .run::<ActAs, _>(|cx, _| is_staff(cx)),
            )
            .without_page(ResourcePage::View)
            .without_global_search()
            .action::<ActAs>()
            .list_widget::<SupportNotice>()
            .table(
                Table::new((
                    col.name.text().searchable().sortable(),
                    col.email.text().searchable().sortable(),
                    ComputedColumn::new(tablo::text!("support-teams"), |_, user: &User| {
                        user.memberships.get().len().to_string()
                    })
                    .include_unscoped(User::fields().memberships()),
                    col.created_at.date().sortable(),
                ))
                .default_sort_desc(lens!(User.created_at)),
            )
    }
}

/// Acts as the row's customer, landing on their home. Tablo's own
/// [`Impersonate`](tablo::Impersonate) acts as a record of the panel's user type, which a `User`
/// is not: [`SignedIn`] loads its teams on top of it.
pub struct ActAs;

impl Action<SupportResource> for ActAs {
    type Input = ();

    const NAME: &'static str = "act-as";

    fn declare() -> ActionDef<SupportResource> {
        ActionDef::new()
            .places(Places::ROW)
            .icon(tablo::ui::icons::EYE)
            .label(tablo::text!("support-act-as"))
            // The button shows where the rule allows; running it asks the rule again.
            .can_run(|cx, user: &User| {
                auth::user::<SignedIn>(cx).is_some_and(|actor| may_act_as(actor, user))
            })
            .success(|_| tablo::text!("support-acting"))
    }

    async fn run(
        cx: &Cx,
        users: &[User],
        (): (),
        ex: &mut dyn toasty::Executor,
    ) -> Result<Outcome, WriteError<()>> {
        let [user] = users else {
            return Err(WriteError::declined(tablo::text!("support-act-as-one")));
        };
        let target = Accounts::signed_in(user.clone(), &mut *ex).await?;
        auth::impersonate(cx, ex, &target).await?;
        Ok(Outcome::done().redirect(APP_HOME))
    }
}

/// What acting as a customer does, above the list, and the factor it needs when the member of
/// staff holds none the rule trusts.
pub struct SupportNotice;

impl Widget for SupportNotice {
    fn declare() -> WidgetDef {
        WidgetDef::new().full_width()
    }

    async fn render(cx: &Cx) -> topcoat::Result<impl View> {
        let signed = auth::require_user::<SignedIn>(cx)?;
        let minutes = LIFETIME.as_secs() / 60;
        Ok(view! {
            <div class="flex flex-col gap-3">
                <p class="text-sm text-muted-foreground">
                    (tablo::t!(cx, "support-description", minutes = minutes))
                </p>
                if !signed.app_factor {
                    <p data-support-notice="" class="rounded-lg border border-border bg-muted p-3 text-sm">
                        <a href=(TWO_FACTOR_URL) class="underline">
                            (tablo::t!(cx, "support-needs-factor"))
                        </a>
                    </p>
                }
            </div>
        })
    }
}
