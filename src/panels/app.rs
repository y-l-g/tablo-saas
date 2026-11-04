//! The `app` panel: where a customer signs up and works.

use std::time::Duration;

use tablo::{
    Auth, AuthMail, EmailVerification, LoginThrottle, Page, PageDef, Panel, PasswordReset,
    TenantRegistration, TwoFactor,
};
use topcoat::icon::iconify::iconify_icon;

use crate::accounts::{AccountResetter, AccountVerifier, Accounts, SignedIn, UserResource};
use crate::platform::config::Config;
use crate::teams::{IncomingInvitationResource, InvitationResource, MemberResource, Teams};

/// The panel customers use.
pub fn app_panel(config: &Config) -> Panel {
    super::shared(Panel::new("app"))
        .auth(auth(config))
        .notifications()
        .home::<Home>()
        .resource::<UserResource>()
        .resource::<MemberResource>()
        .resource::<InvitationResource>()
        .resource::<IncomingInvitationResource>()
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
    }
}
