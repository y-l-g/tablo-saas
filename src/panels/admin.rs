//! The `admin` panel: staff only, with a second factor required.

use std::time::Duration;

use tablo::{Auth, AuthMail, Panel, TwoFactor};

use crate::accounts::{SignedIn, password_auth};
use crate::audit;
use crate::backoffice::{
    AccountResource, CustomerResource, ImpersonationResource, JobResource, SubscriptionResource,
    WebhookResource,
};
use crate::platform::config::Config;

/// The panel staff use. It has no sign-up: `staff grant` makes staff of an existing account.
pub fn admin_panel(config: &Config) -> Panel {
    super::shared(Panel::new("admin"), config)
        .auth(auth(config))
        .audit_log(audit::staff_trail)
        .resource::<CustomerResource>()
        .resource::<AccountResource>()
        .resource::<SubscriptionResource>()
        .resource::<WebhookResource>()
        .resource::<JobResource>()
        .resource::<ImpersonationResource>()
}

fn auth(config: &Config) -> Auth<SignedIn> {
    Auth::custom(password_auth())
        .throttle(super::shared_throttle(5, Duration::from_mins(1)))
        .mail(AuthMail::new(
            config.mail.from.clone(),
            config.base_url.clone(),
        ))
        .two_factor(
            TwoFactor::new()
                .email_codes()
                .throttle(super::shared_throttle(5, Duration::from_mins(1)))
                .required(),
        )
}
