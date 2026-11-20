//! The panels: `app`, where customers work, and `admin`, where staff do.

mod admin;
mod app;
mod brand;

pub use admin::admin_panel;
pub use app::app_panel;
pub use brand::{LOGO_URL, PRODUCT_NAME};

use tablo::{AuthMail, Environment, Panel, jobs::AuthMailQueue};
use topcoat::l10n::locale;

use crate::accounts::SignedIn;
use crate::platform::{config::Config, words};

/// Where the billing buttons land. A browser checks `form-action` against a form's redirects too,
/// so Tablo's default policy stops a form that answers with a redirect to Stripe. A custom checkout
/// domain goes here as well.
const STRIPE_ORIGINS: [&str; 2] = ["https://checkout.stripe.com", "https://billing.stripe.com"];

/// Tablo's default policy, letting a form land on Stripe.
fn lands_on_stripe(panel: Panel) -> Panel {
    STRIPE_ORIGINS.into_iter().fold(panel, |panel, origin| {
        panel.content_security_policy_source("form-action", origin)
    })
}

/// What every panel shares: the brand, the deployment's ribbon, and the languages, the browser choosing between them.
fn shared(panel: Panel, config: &Config) -> Panel {
    let panel = match &config.environment_name {
        Some(name) => panel.environment(Environment::new(name.clone())),
        None => panel,
    };
    panel
        .brand(brand::brand())
        .locales([locale!("en"), locale!("fr")])
        .translations(locale!("en"), words::ENGLISH)
        .translations(locale!("fr"), words::FRENCH)
}

/// The sender and host every panel's auth mail carries. Each letter is queued in the transaction
/// that records its token, and the worker sends it: a crash after the response loses none.
fn auth_mail(config: &Config) -> AuthMail {
    AuthMail::new(config.mail.from.clone(), config.base_url.as_str())
        .delivery(AuthMailQueue::new(&config.cookie_key))
}

/// Who must hold a second factor: whom `rule` names, but a demo's public administrator, whose
/// mailed codes nobody would read.
fn second_factor_rule(
    config: &Config,
    rule: impl Fn(&SignedIn) -> bool + Send + Sync + 'static,
) -> impl Fn(&topcoat::context::Cx, &SignedIn) -> bool + Send + Sync + 'static {
    let public = config
        .demo
        .as_ref()
        .and_then(|demo| demo.admin.as_ref())
        .map(|admin| admin.email.clone());
    move |_cx, signed| rule(signed) && public.as_deref() != Some(signed.user.email.as_str())
}
