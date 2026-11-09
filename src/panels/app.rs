//! The `app` panel: where a customer signs up and works.

use std::time::Duration;

use tablo::{
    Area, Auth, AuthMail, EmailVerification, LoginThrottle, Page, PageDef, Panel, PasswordReset,
    TenantRegistration, TwoFactor, Widget, WidgetDef, db::db, ui,
};
use topcoat::{
    Result,
    context::Cx,
    icon::iconify::iconify_icon,
    view::{View, view},
};

use crate::accounts::{AccountResetter, AccountVerifier, Accounts, SignedIn};
use crate::audit;
use crate::billing::{BILLING_URL, BillingPage, plan_name, standing_notice};
use crate::platform::config::Config;
use crate::projects::{Project, ProjectResource};
use crate::teams::{
    IncomingInvitationResource, IncomingInvitationsWidget, InvitationResource, Member,
    MemberResource, Teams,
    actions::{DeleteAccount, DeleteTeam},
    current_entitlements,
};

/// The panel customers use.
pub fn app_panel(config: &Config) -> Panel {
    super::lands_on_stripe(super::shared(Panel::new("app"), config))
        .auth(auth(config))
        .notifications()
        .audit_log(audit::team_trail)
        .home::<Home>()
        .resource::<ProjectResource>()
        .area(settings())
}

/// Everything a customer configures rather than works in, out of the sidebar: what follows the
/// person across teams, then what belongs to the team the session acts for. The user menu leads
/// to the first group through Profile, the team menu to the second through its settings.
fn settings() -> Area {
    Area::new("settings")
        .without_navigation()
        .group(tablo::text!("settings-account"), |group| {
            group
                .profile_with(PageDef::header_action::<DeleteAccount>)
                .two_factor_with(|def| def.label(tablo::text!("settings-security")))
                .resource::<IncomingInvitationResource>()
        })
        .tenant_group(|group| {
            group
                .tenant_profile_with(|def| {
                    def.slug("team")
                        .label(tablo::text!("settings-general"))
                        .header_action::<DeleteTeam>()
                })
                .resource::<MemberResource>()
                .resource::<InvitationResource>()
                .page::<BillingPage>()
                .audit_log()
        })
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
            .widget::<IncomingInvitationsWidget>()
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
