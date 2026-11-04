//! A production SaaS template on Tablo.
//!
//! [`platform`] holds what any app needs and no business rule: configuration, the database,
//! telemetry and HTTP middleware. [`site`] is the public website. [`router`] mounts it all, and
//! [`cli`] is the binary's entry point.
//!
//! Imports go one way: `cli → router → panels → domains → platform`.

pub mod cli;
pub mod platform;
pub mod router;
pub mod site;

/// Every table the app stores, for the database and the migration generator.
pub fn models() -> toasty::schema::ModelSet {
    tablo::models!()
}
