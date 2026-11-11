//! The `app` panel: where a customer signs up and works.

use std::time::Duration;

use tablo::{
    Area, Auth, AuthMail, EmailVerification, Oidc, OidcProvider, Page, PageDef, Panel,
    PasswordReset, TenantRegistration, TwoFactor, Widget, WidgetDef, db::db, ui,
};
use topcoat::{
    Result,
    context::Cx,
    icon::iconify::iconify_icon,
    view::{View, view},
};

use crate::accounts::{Accounts, SignedIn, password_auth};
use crate::audit;
use crate::billing::{
    BILLING_URL, BillingPage, StandingBanner, is_urgent, plan_name, standing_notice,
};
use crate::platform::config::{Config, GOOGLE_ISSUER, GoogleSettings};
use crate::projects::{Project, ProjectResource};
use crate::support::{self, SupportResource};
use crate::teams::{
    IncomingInvitationResource, IncomingInvitationsWidget, InvitationResource, Member,
    MemberResource, TeamPlan, Teams,
    actions::{DeleteAccount, DeleteTeam, ExportAccount},
    current_entitlements, may_manage_members,
};

/// The panel customers use.
pub fn app_panel(config: &Config) -> Panel {
    super::lands_on_stripe(super::shared(Panel::new("app"), config))
        .auth(auth(config))
        // What the team's plan allows, which the policies read.
        .load::<TeamPlan>()
        .notifications()
        .banner::<StandingBanner>()
        .audit_log(audit::team_trail)
        .home::<Home>()
        .resource::<ProjectResource>()
        .resource::<SupportResource>()
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
                .profile_with(|def| {
                    def.header_action::<ExportAccount>()
                        .header_action::<DeleteAccount>()
                })
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
/// profile and a second factor from an app or by mail, required of staff; "Continue with Google"
/// when configured; and staff acting as a customer.
fn auth(config: &Config) -> Auth<SignedIn> {
    let auth = Auth::custom(password_auth())
        .registration(Accounts)
        .throttle(super::shared_throttle(5, Duration::from_mins(1)))
        // Past the per-address limit, a guesser rotating addresses still meets this one. It is
        // high enough that only a distributed guesser locks an owner out.
        .account_throttle(super::shared_throttle(50, Duration::from_hours(1)))
        .profile_throttle(super::shared_throttle(5, Duration::from_mins(1)))
        .mail(AuthMail::new(
            config.mail.from.clone(),
            config.base_url.clone(),
        ))
        .password_reset(PasswordReset::new(password_auth()))
        .email_verification(EmailVerification::new(password_auth()))
        .profile(password_auth())
        // Staff sign in here to act as customers, so their account reaches every customer's
        // data: a password alone is not enough for it. Customers choose for themselves.
        .two_factor(
            TwoFactor::new()
                .email_codes()
                .throttle(super::shared_throttle(5, Duration::from_mins(1)))
                .required_for(|_cx, signed: &SignedIn| signed.user.staff_role.is_some()),
        )
        .tenant_registration(TenantRegistration::new(Teams).required())
        .tenant_profile(Teams)
        .impersonation(support::impersonation());
    match &config.google {
        Some(google) => {
            auth.oidc(Oidc::new(config.base_url.clone(), Accounts).provider(provider(google)))
        }
        None => auth,
    }
}

/// Google, or the stand-in a test names as its issuer under the same key.
fn provider(google: &GoogleSettings) -> OidcProvider {
    let GoogleSettings {
        client_id,
        client_secret,
        issuer,
    } = google.clone();
    if issuer == GOOGLE_ISSUER {
        OidcProvider::google(client_id, client_secret)
    } else {
        OidcProvider::new("google", "Google", issuer, client_id, client_secret)
    }
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
        // What the banner above the page already says is not said twice.
        let bannered = is_urgent(entitlements.standing) && may_manage_members(cx);
        let notice = (!bannered)
            .then(|| standing_notice(cx, entitlements.standing))
            .flatten();
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
