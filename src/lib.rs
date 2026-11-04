//! A production SaaS template on Tablo.
//!
//! [`platform`] holds what any app needs and no business rule: configuration, the database,
//! telemetry, mail and HTTP middleware. [`accounts`] holds who signs in, [`panels`] mounts the
//! domains, [`site`] is the public website, [`router`] puts it all on one router, and [`cli`] is
//! the binary's entry point.
//!
//! Imports go one way: `cli → router → panels → domains → platform`.

pub mod accounts;
pub mod cli;
pub mod panels;
pub mod platform;
pub mod router;
pub mod site;

pub use panels::PRODUCT_NAME;

/// Every table the app stores, for the database and the migration generator.
pub fn models() -> toasty::schema::ModelSet {
    tablo::models!(accounts::User)
}
