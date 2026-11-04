//! The app's router: the routes it discovers and the state they read.

use toasty::Db;
use topcoat::router::{Router, RouterBuilderDiscoverExt, TrustedProxies};

use crate::platform::{config::Config, http::Https};

/// Builds the router the server runs and the tests drive.
pub fn router(db: Db, config: &Config) -> Router {
    let mut builder = Router::builder()
        .discover()
        .with(db)
        .with(config.cookie_key.clone())
        .with(Https(config.base_url.starts_with("https://")));
    if !config.trusted_proxies.is_empty() {
        builder =
            builder.trusted_proxies(TrustedProxies::new().networks(config.trusted_proxies.clone()));
    }
    builder.build()
}
