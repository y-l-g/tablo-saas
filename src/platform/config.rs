//! What the environment sets, read once at start. A value the app cannot use stops the start
//! rather than the first request that reads it, and this file lists every variable the app reads.

use std::net::IpAddr;

use anyhow::{Context, bail, ensure};
use topcoat::cookie::Key;

/// Where the app runs, which decides how strict its configuration is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Development,
    Production,
}

/// The app's settings, from the environment.
pub struct Config {
    pub environment: Environment,
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

        Ok(Self {
            environment,
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
        })
    }

    pub fn is_production(&self) -> bool {
        self.environment == Environment::Production
    }
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
        ]
    }

    #[test]
    fn a_complete_production_environment_loads() {
        let config = Config::load(&environment(&production())).expect("loads");
        assert!(config.is_production());
        assert_eq!(config.base_url, "https://app.example.com");
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
