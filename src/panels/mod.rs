//! The panels: `app`, where customers work, and `admin`, where staff do.

mod admin;
mod app;
mod brand;

pub use admin::admin_panel;
pub use app::app_panel;
pub use brand::PRODUCT_NAME;

use std::time::Duration;

use tablo::{Environment, LoginThrottle, Panel};
use topcoat::l10n::locale;

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

/// What every panel shares: the brand, the deployment's ribbon, and the languages, the browser
/// choosing between them.
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

/// A throttle every replica counts against together, kept in the database: one counted in each
/// process would let N replicas allow N times the guesses, and forget them on a restart.
fn shared_throttle(attempts: u32, window: Duration) -> LoginThrottle {
    LoginThrottle::new(attempts, window).in_database()
}
