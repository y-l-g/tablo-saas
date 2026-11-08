//! What the environment sets, read once at start. A value the app cannot use stops the start
//! rather than the first request that reads it, and this file lists every variable the app reads.

use std::net::IpAddr;

use anyhow::{Context, bail, ensure};
use topcoat::{cookie::Key, mail::Mailbox};

/// Where the app runs, which decides how strict its configuration is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Development,
    Production,
}

/// The app's settings, from the environment.
pub struct Config {
    pub environment: Environment,
    /// The deployment's name, such as `Staging`, which every panel page shows on a ribbon; `None`
    /// shows none. Production shows none unless `APP_ENVIRONMENT_NAME` names one.
    pub environment_name: Option<String>,
    /// The URL the app answers on, without a trailing slash, which every mailed link and Stripe
    /// return URL starts with: never derived from a request's `Host` header.
    pub base_url: String,
    pub database_url: String,
    pub database_pool_size: usize,
    /// Signs upload links and seals two-factor secrets, so it outlives restarts and is shared by
    /// replicas.
    pub cookie_key: Key,
    /// The CIDR ranges of the reverse proxies whose forwarding headers the app believes.
    pub trusted_proxies: Vec<String>,
    pub mail: MailSettings,
    /// `None` runs the app with billing off, which production refuses.
    pub stripe: Option<StripeSettings>,
    /// The `topcoat dev` server, whose live reload the panels' policy must allow.
    pub dev_server: Option<String>,
}

/// How the app reaches Stripe.
#[derive(Clone)]
pub struct StripeSettings {
    pub secret_key: String,
    pub webhook_secret: String,
    /// Stripe's API, or a local `stripe-mock`.
    pub api_base: String,
    /// How many days a first subscription runs free, `None` for no trial.
    pub trial_days: Option<u32>,
    /// Whether Checkout collects tax through Stripe Tax, which the account must have turned on.
    pub automatic_tax: bool,
}

/// What a job or a page needs of the configuration, without the secrets.
#[derive(Clone)]
pub struct Settings {
    pub base_url: String,
    pub mail_from: Mailbox,
    pub trial_days: Option<u32>,
    pub automatic_tax: bool,
}

/// Where mail comes from and how it leaves.
pub struct MailSettings {
    /// The sender, an address the mail provider has verified.
    pub from: Mailbox,
    /// The server mail goes through; `None`, in development alone, logs it.
    pub smtp: Option<Smtp>,
}

/// An SMTP server and its sign-in.
pub struct Smtp {
    pub host: String,
    /// 465 connects with implicit TLS, any other port with STARTTLS.
    pub port: u16,
    pub credentials: Option<(String, String)>,
}

impl Config {
    /// Reads the environment, with `.env` beneath it in development.
    pub fn from_env() -> anyhow::Result<Self> {
        Self::load(&|name| std::env::var(name).ok().filter(|value| !value.is_empty()))
    }

    /// Reads settings through `var`, so a test states its environment.
    pub fn load(var: &dyn Fn(&str) -> Option<String>) -> anyhow::Result<Self> {
        let environment = match var("APP_ENV").as_deref() {
            None | Some("development") => Environment::Development,
            Some("production") => Environment::Production,
            Some(other) => bail!("APP_ENV is `development` or `production`, not `{other}`"),
        };
        let production = environment == Environment::Production;

        let base_url = var("APP_BASE_URL")
            .unwrap_or_else(|| "http://127.0.0.1:3000".to_string())
            .trim_end_matches('/')
            .to_string();
        ensure!(
            !production || base_url.starts_with("https://"),
            "APP_BASE_URL must be https in production: the panels' cookies are `Secure`"
        );

        let cookie_key = match var("APP_COOKIE_KEY") {
            Some(secret) => parse_key(&secret).context("APP_COOKIE_KEY")?,
            None if production => bail!("APP_COOKIE_KEY is required in production"),
            None => Key::generate(),
        };

        let mail = mail_settings(var)?;
        ensure!(
            !production || mail.smtp.is_some(),
            "SMTP_HOST is required in production: sign-up mails a verification link"
        );

        let stripe = stripe_settings(var)?;
        ensure!(
            !production || stripe.is_some(),
            "STRIPE_SECRET_KEY and STRIPE_WEBHOOK_SECRET are required in production"
        );

        let environment_name = var("APP_ENVIRONMENT_NAME")
            .or_else(|| (!production).then(|| "Development".to_string()));

        Ok(Self {
            environment,
            environment_name,
            base_url,
            database_url: var("DATABASE_URL").context("DATABASE_URL is required")?,
            database_pool_size: match var("DATABASE_POOL_SIZE") {
                Some(size) => size.parse().context("DATABASE_POOL_SIZE is a number")?,
                None => 16,
            },
            cookie_key,
            trusted_proxies: var("TRUSTED_PROXIES")
                .map(|list| parse_proxies(&list))
                .transpose()?
                .unwrap_or_default(),
            mail,
            stripe,
            dev_server: var("TOPCOAT_DEV_URL").filter(|_| !production),
        })
    }

    pub fn settings(&self) -> Settings {
        Settings {
            base_url: self.base_url.clone(),
            mail_from: self.mail.from.clone(),
            trial_days: self.stripe.as_ref().and_then(|stripe| stripe.trial_days),
            automatic_tax: self
                .stripe
                .as_ref()
                .is_some_and(|stripe| stripe.automatic_tax),
        }
    }

    pub fn is_production(&self) -> bool {
        self.environment == Environment::Production
    }
}

/// `MAIL_FROM`, and the server `SMTP_HOST` names on `SMTP_PORT` (587 by default), signing in with
/// `SMTP_USERNAME` and `SMTP_PASSWORD` when both are set.
fn mail_settings(var: &dyn Fn(&str) -> Option<String>) -> anyhow::Result<MailSettings> {
    let address = var("MAIL_FROM").unwrap_or_else(|| "no-reply@example.com".to_string());
    let from = Mailbox::named(crate::PRODUCT_NAME, &address).context("MAIL_FROM")?;
    let smtp = var("SMTP_HOST")
        .map(|host| {
            let port = match var("SMTP_PORT") {
                Some(port) => port.parse().context("SMTP_PORT is a port number")?,
                None => 587,
            };
            anyhow::Ok(Smtp {
                host,
                port,
                credentials: var("SMTP_USERNAME").zip(var("SMTP_PASSWORD")),
            })
        })
        .transpose()?;
    Ok(MailSettings { from, smtp })
}

/// A key from its 64 bytes spelled as 128 hex characters.
fn parse_key(secret: &str) -> anyhow::Result<Key> {
    ensure!(
        secret.len() == 128 && secret.is_ascii(),
        "holds 128 hex characters: `openssl rand -hex 64`"
    );
    let bytes = (0..secret.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&secret[at..at + 2], 16))
        .collect::<Result<Vec<u8>, _>>()
        .context("holds hex characters")?;
    Key::try_from(bytes.as_slice()).map_err(|error| anyhow::anyhow!("{error}"))
}

/// The comma-separated ranges of `TRUSTED_PROXIES`, each a CIDR or a bare address.
fn parse_proxies(list: &str) -> anyhow::Result<Vec<String>> {
    list.split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            let address = entry.split('/').next().unwrap_or(entry);
            address.parse::<IpAddr>().with_context(|| {
                format!("TRUSTED_PROXIES: `{entry}` is not an address or a CIDR")
            })?;
            Ok(entry.to_string())
        })
        .collect()
}

fn stripe_settings(var: &dyn Fn(&str) -> Option<String>) -> anyhow::Result<Option<StripeSettings>> {
    let (Some(secret_key), Some(webhook_secret)) =
        (var("STRIPE_SECRET_KEY"), var("STRIPE_WEBHOOK_SECRET"))
    else {
        return Ok(None);
    };
    Ok(Some(StripeSettings {
        secret_key,
        webhook_secret,
        api_base: var("STRIPE_API_BASE").unwrap_or_else(|| "https://api.stripe.com".to_string()),
        trial_days: var("BILLING_TRIAL_DAYS")
            .map(|days| days.parse().context("BILLING_TRIAL_DAYS is a number"))
            .transpose()?
            .filter(|days| *days > 0),
        automatic_tax: var("BILLING_AUTOMATIC_TAX").as_deref() == Some("true"),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn environment(pairs: &[(&'static str, String)]) -> impl Fn(&str) -> Option<String> {
        let pairs = pairs.to_vec();
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| value.clone())
        }
    }

    fn production() -> Vec<(&'static str, String)> {
        vec![
            ("APP_ENV", "production".to_string()),
            ("APP_BASE_URL", "https://app.example.com/".to_string()),
            ("APP_COOKIE_KEY", "ab".repeat(64)),
            ("DATABASE_URL", "postgresql://db/app".to_string()),
            ("SMTP_HOST", "smtp.example.com".to_string()),
            ("STRIPE_SECRET_KEY", "sk_test".to_string()),
            ("STRIPE_WEBHOOK_SECRET", "whsec_test".to_string()),
        ]
    }

    #[test]
    fn production_refuses_to_run_without_stripe() {
        let pairs: Vec<_> = production()
            .into_iter()
            .filter(|(key, _)| *key != "STRIPE_WEBHOOK_SECRET")
            .collect();
        assert!(Config::load(&environment(&pairs)).is_err());
    }

    #[test]
    fn development_runs_with_billing_off() {
        let config = Config::load(&environment(&[(
            "DATABASE_URL",
            "sqlite::memory:".to_string(),
        )]))
        .expect("loads");
        assert!(config.stripe.is_none());
        assert_eq!(config.environment_name.as_deref(), Some("Development"));
    }

    #[test]
    fn a_complete_production_environment_loads() {
        let config = Config::load(&environment(&production())).expect("loads");
        assert!(config.is_production());
        assert_eq!(config.base_url, "https://app.example.com");
        assert_eq!(
            config.environment_name, None,
            "production wears no ribbon unless named"
        );
    }

    #[test]
    fn production_refuses_a_plain_http_base_url() {
        let mut pairs = production();
        pairs[1].1 = "http://app.example.com".to_string();
        assert!(Config::load(&environment(&pairs)).is_err());
    }

    #[test]
    fn production_refuses_a_generated_cookie_key() {
        let pairs: Vec<_> = production()
            .into_iter()
            .filter(|(key, _)| *key != "APP_COOKIE_KEY")
            .collect();
        assert!(Config::load(&environment(&pairs)).is_err());
    }

    #[test]
    fn production_refuses_to_run_without_mail() {
        let pairs: Vec<_> = production()
            .into_iter()
            .filter(|(key, _)| *key != "SMTP_HOST")
            .collect();
        assert!(Config::load(&environment(&pairs)).is_err());
    }

    #[test]
    fn development_needs_only_a_database() {
        let pairs = [("DATABASE_URL", "sqlite::memory:".to_string())];
        let config = Config::load(&environment(&pairs)).expect("loads");
        assert!(!config.is_production());
    }

    #[test]
    fn a_short_cookie_key_is_refused() {
        assert!(parse_key("abcd").is_err());
    }

    #[test]
    fn a_malformed_proxy_is_refused() {
        assert!(parse_proxies("10.0.0.0/8, not-an-ip").is_err());
        assert_eq!(
            parse_proxies("10.0.0.0/8, 192.168.1.1").expect("parses"),
            ["10.0.0.0/8", "192.168.1.1"]
        );
    }
}
