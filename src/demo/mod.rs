//! A public demo: everything a deployment needs to let anyone in, back office included, and to
//! wipe what they leave on a schedule. Nothing else in the template depends on it.
//!
//! `APP_DEMO=true` turns it on. Without it, every hook below changes nothing. To drop the demo
//! from your app, delete this folder and `tests/demo.rs`, then what `grep -rni demo src tests
//! .env.example README.md` still finds: a config field, the `demo-reset` command, one line in
//! each panel's auth and the test harness's demo flag, past the links to Tablo's own demo.

mod reset;

use anyhow::{bail, ensure};
use tablo::Panel;
use topcoat::context::Cx;

pub use reset::{Wiped, reset};

use crate::accounts::{SignedIn, normalize_email};
use crate::platform::config::{Config, StripeSettings};

/// A public demo, which `reset` wipes. Its administrator's address and password are public: the
/// back office shows them on its sign-in page and asks that account no second factor, since
/// nobody reads its mail.
pub struct Demo {
    /// The administrator `reset` seeds after each wipe, `None` for nobody.
    pub admin: Option<SeedAdmin>,
}

/// An administrator's address, as [`normalize_email`] spells it, and password.
pub struct SeedAdmin {
    pub email: String,
    pub password: String,
}

impl Demo {
    /// `APP_DEMO=true`, with `DEMO_ADMIN_EMAIL` and `DEMO_ADMIN_PASSWORD` naming the
    /// administrator. A wipe deletes Stripe customers, so a live key refuses a demo.
    pub fn load(
        var: &dyn Fn(&str) -> Option<String>,
        stripe: Option<&StripeSettings>,
    ) -> anyhow::Result<Option<Self>> {
        let admin = match (var("DEMO_ADMIN_EMAIL"), var("DEMO_ADMIN_PASSWORD")) {
            (Some(email), Some(password)) => Some(SeedAdmin {
                email: normalize_email(&email),
                password,
            }),
            (None, None) => None,
            _ => bail!("DEMO_ADMIN_EMAIL and DEMO_ADMIN_PASSWORD are set together or not at all"),
        };
        if var("APP_DEMO").as_deref() != Some("true") {
            ensure!(
                admin.is_none(),
                "DEMO_ADMIN_EMAIL makes a public administrator: set APP_DEMO=true to mean it"
            );
            return Ok(None);
        }
        ensure!(
            stripe.is_none_or(|stripe| !stripe.secret_key.contains("_live_")),
            "APP_DEMO wipes Stripe customers, which a live key must never do"
        );
        Ok(Some(Self { admin }))
    }
}

fn admin(config: &Config) -> Option<&SeedAdmin> {
    config.demo.as_ref()?.admin.as_ref()
}

/// The panel, naming the demo administrator's account on its sign-in page.
pub fn login_hint(panel: Panel, config: &Config) -> Panel {
    match admin(config) {
        Some(admin) => panel.login_hint(format!("{} / {}", admin.email, admin.password)),
        None => panel,
    }
}

/// Who must hold a second factor: whom `rule` names, but the demo administrator, whose mailed
/// codes nobody would read.
pub fn second_factor(
    config: &Config,
    rule: impl Fn(&SignedIn) -> bool + Send + Sync + 'static,
) -> impl Fn(&Cx, &SignedIn) -> bool + Send + Sync + 'static {
    let public = admin(config).map(|admin| admin.email.clone());
    move |_cx, signed| rule(signed) && public.as_deref() != Some(signed.user.email.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(pairs: &[(&str, &str)], key: &str) -> anyhow::Result<Option<Demo>> {
        let stripe = StripeSettings {
            secret_key: key.to_string(),
            webhook_secret: "whsec_test".to_string(),
            api_base: String::new(),
            trial_days: None,
            automatic_tax: false,
        };
        let var = |name: &str| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_string())
        };
        Demo::load(&var, Some(&stripe))
    }

    const ADMIN: [(&str, &str); 2] = [
        ("DEMO_ADMIN_EMAIL", "Admin@Example.com"),
        ("DEMO_ADMIN_PASSWORD", "password"),
    ];

    #[test]
    fn a_deployment_is_no_demo_unless_it_says_so() {
        assert!(load(&[], "sk_test_1").expect("loads").is_none());
    }

    #[test]
    fn a_demo_names_its_administrator() {
        let pairs = [ADMIN[0], ADMIN[1], ("APP_DEMO", "true")];
        let demo = load(&pairs, "sk_test_1").expect("loads").expect("a demo");
        assert_eq!(demo.admin.expect("seeds").email, "admin@example.com");
    }

    #[test]
    fn a_public_administrator_outside_a_demo_is_refused() {
        assert!(load(&ADMIN, "sk_test_1").is_err());
    }

    #[test]
    fn a_demo_refuses_a_live_stripe_key() {
        assert!(load(&[("APP_DEMO", "true")], "sk_live_1").is_err());
    }
}
