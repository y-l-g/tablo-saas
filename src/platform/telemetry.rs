//! Structured logs on stdout, where a container platform collects them.

use tracing_subscriber::{EnvFilter, fmt};

/// Installs the subscriber. `RUST_LOG` sets the levels, `LOG_FORMAT=json` the shape.
pub fn init() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let json = std::env::var("LOG_FORMAT").is_ok_and(|format| format == "json");
    if json {
        fmt()
            .with_env_filter(filter)
            .json()
            .with_current_span(true)
            .init();
    } else {
        fmt().with_env_filter(filter).init();
    }
}
