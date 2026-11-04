//! The `app` panel: where a customer signs up and works.

use std::time::Duration;

use tablo::{
    Auth, AuthMail, EmailVerification, LoginThrottle, Page, PageDef, Panel, PasswordReset,
    TenantRegistration, TwoFactor, Widget, WidgetDef, db::db, ui,
};
use topcoat::{
    Result,
    context::Cx,
    icon::iconify::iconify_icon,
    view::{View, view},
};

use crate::accounts::{AccountResetter, AccountVerifier, Accounts, SignedIn, UserResource};
use crate::audit::{AuditResource, TeamAudit};
use crate::billing::{BILLING_URL, BillingPage, plan_name, standing_notice};
use crate::platform::config::Config;
use crate::projects::{Project, ProjectResource};
use crate::teams::{
    AccountPage, IncomingInvitationResource, InvitationResource, Member, MemberResource, Teams,
    current_entitlements,
};

/// The panel customers use.
pub fn app_panel(config: &Config) -> Panel {
    super::shared(Panel::new("app"))
        .auth(auth(config))
        .notifications()
        .before_commit::<TeamAudit>()
        .home::<Home>()
        .resource::<ProjectResource>()
        .resource::<UserResource>()
        .resource::<MemberResource>()
        .resource::<InvitationResource>()
        .resource::<IncomingInvitationResource>()
        .resource::<AuditResource>()
        .page::<BillingPage>()
        .page::<AccountPage>()
}

/// Every auth page Tablo offers: sign-up, a verified address before anything else, reset, the
/// profile and a second factor from an app or by mail.
fn auth(config: &Config) -> Auth<SignedIn> {
    Auth::custom(Accounts)
        .registration(Accounts)
        // Past the per-address limit, a guesser rotating addresses still meets this one. It is
        // high enough that only a distributed guesser locks an owner out.
        .account_throttle(LoginThrottle::new(50, Duration::from_hours(1)))
        .mail(AuthMail::new(
            config.mail.from.clone(),
            config.base_url.clone(),
        ))
        .password_reset(PasswordReset::new(AccountResetter))
        .email_verification(EmailVerification::new(AccountVerifier))
        .profile(Accounts)
        .two_factor(TwoFactor::new().email_codes())
        .tenant_registration(TenantRegistration::new(Teams).required())
        .tenant_profile(Teams)
}

/// The page a signed-in customer lands on.
pub struct Home;

impl Page for Home {
    fn declare() -> PageDef {
        PageDef::new()
            .label(tablo::text!("page-home"))
            .icon(iconify_icon!("lucide:layout-dashboard"))
            .widget::<PlanWidget>()
    }
}

/// How the team stands against its plan, with a word when something needs doing.
pub struct PlanWidget;

impl Widget for PlanWidget {
    fn declare() -> WidgetDef {
        WidgetDef::new().full_width()
    }

    async fn render(cx: &Cx) -> Result<impl View> {
        let team = tablo::require_tenant(cx)?;
        let entitlements = current_entitlements(cx);
        let mut handle = db(cx)?;
        let projects = Project::filter(Project::fields().team_id().eq(team))
            .exec(&mut handle)
            .await?
            .len();
        let seats = Member::filter(Member::fields().team_id().eq(team))
            .exec(&mut handle)
            .await?
            .len();
        let notice = standing_notice(cx, entitlements.standing);
        let usage = |used: usize, limit: Option<u32>| match limit {
            Some(limit) => format!("{used} / {limit}"),
            None => used.to_string(),
        };
        let tiles = [
            (
                tablo::t!(cx, "billing-current"),
                plan_name(cx, entitlements.plan.id),
            ),
            (
                tablo::t!(cx, "dashboard-seats"),
                usage(seats, entitlements.plan.seat_limit),
            ),
            (
                tablo::t!(cx, "dashboard-projects"),
                usage(projects, entitlements.plan.project_limit),
            ),
        ];
        Ok(view! {
            cx =>
            <div class="flex flex-col gap-4">
                if let Some(notice) = notice {
                    <p data-billing-notice="" class="rounded-lg border border-border bg-muted p-3 text-sm">
                        <a href=(BILLING_URL) class="underline">(notice)</a>
                    </p>
                }
                <div class="grid gap-4 md:grid-cols-3">
                    for (label, value) in tiles {
                        ui::card(
                            ui::card_header(
                                ui::card_description((label))
                                ui::card_title((value))
                            )
                        )
                    }
                </div>
            </div>
        })
    }
}
