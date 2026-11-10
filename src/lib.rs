//! A production SaaS template on Tablo.
//!
//! [`platform`] holds what any app needs and no business rule: configuration, the database,
//! telemetry, mail and HTTP middleware. [`jobs`] is the durable queue. [`accounts`] holds who signs in, [`teams`] the tenants they act for, [`support`]
//! lets staff act as a customer, [`panels`] mounts the
//! domains, [`site`] is the public website, [`router`] puts it all on one router, and [`cli`] is
//! the binary's entry point.
//!
//! Imports go one way: `cli → router → panels → domains → platform`.

pub mod accounts;
pub mod audit;
pub mod background;
pub mod backoffice;
pub mod billing;
pub mod cli;
pub mod jobs;
pub mod panels;
pub mod platform;
pub mod projects;
pub mod router;
pub mod site;
pub mod support;
pub mod teams;

pub use panels::PRODUCT_NAME;

/// Every table the app stores, for the database and the migration generator.
pub fn models() -> toasty::schema::ModelSet {
    tablo::models!(
        accounts::User,
        jobs::JobRow,
        jobs::ScheduleRun,
        teams::Team,
        teams::Member,
        teams::Invitation,
        billing::BillingAccount,
        billing::StripeEvent,
        projects::Project
    )
}
