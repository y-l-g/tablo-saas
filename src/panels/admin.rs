//! The `admin` panel: staff only, with a second factor required.

use tablo::{Auth, AuthMail, Panel, TwoFactor};

use crate::accounts::{Accounts, SignedIn};
use crate::audit::{StaffAudit, StaffAuditResource};
use crate::backoffice::{
    AccountResource, CustomerResource, JobResource, SubscriptionResource, WebhookResource,
};
use crate::platform::config::Config;

/// The panel staff use. It has no sign-up: `staff grant` makes staff of an existing account.
pub fn admin_panel(config: &Config) -> Panel {
    super::shared(Panel::new("admin"))
        .auth(auth(config))
        .before_commit::<StaffAudit>()
        .resource::<CustomerResource>()
        .resource::<AccountResource>()
        .resource::<SubscriptionResource>()
        .resource::<WebhookResource>()
        .resource::<JobResource>()
        .resource::<StaffAuditResource>()
}

fn auth(config: &Config) -> Auth<SignedIn> {
    Auth::custom(Accounts)
        .mail(AuthMail::new(
            config.mail.from.clone(),
            config.base_url.clone(),
        ))
        .two_factor(TwoFactor::new().email_codes().required())
}
