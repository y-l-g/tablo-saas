//! The panels: `app`, where customers work, and `admin`, where staff do.

mod admin;
mod app;
mod brand;

pub use admin::admin_panel;
pub use app::app_panel;
pub use brand::PRODUCT_NAME;

use tablo::Panel;
use topcoat::l10n::locale;

use crate::platform::words;

/// Where the billing buttons land. A browser checks `form-action` against a form's redirects too,
/// so Tablo's default policy stops a form that answers with a redirect to Stripe. A custom checkout
/// domain goes here as well.
const STRIPE_ORIGINS: &str = "https://checkout.stripe.com https://billing.stripe.com";

/// The panels' policy: Tablo's default, letting a form land on Stripe. A policy of our own replaces
/// the one Tablo adapts to `topcoat dev`, so this names the dev server's origin as Tablo does.
fn content_security_policy(dev_server: Option<&str>) -> String {
    let policy = Panel::DEFAULT_CONTENT_SECURITY_POLICY.replace(
        "form-action 'self'",
        &format!("form-action 'self' {STRIPE_ORIGINS}"),
    );
    let Some(dev) = dev_server else {
        return policy;
    };
    let socket = dev.replacen("http", "ws", 1);
    policy.replacen(
        "'unsafe-eval';",
        &format!("'unsafe-eval' {dev}; connect-src 'self' {dev} {socket};"),
        1,
    )
}

/// What every panel shares: the brand and the languages, the browser choosing between them.
fn shared(panel: Panel) -> Panel {
    panel
        .brand(brand::brand())
        .locales([locale!("en"), locale!("fr")])
        .translations(locale!("en"), words::ENGLISH)
        .translations(locale!("fr"), words::FRENCH)
}
