//! Plans, subscriptions and what they allow.
//!
//! Stripe owns the money and the subscription's truth; the [`BillingAccount`] row is a mirror of
//! it, refreshed from Stripe's API rather than from the bodies of webhooks, so events can arrive
//! twice or out of order without harm. The plan catalog lives here in code, and Stripe's copy is
//! found by lookup key, so every environment agrees on what a plan means and no price id leaks
//! into the source. What a team may do is one pure function of the mirror: [`Entitlements::of`].

mod banner;
mod catalog;
mod checkout;
mod entitlements;
pub mod jobs;
mod models;
mod page;
mod plans;
mod portal;
pub mod stripe;
mod sync;
mod webhook;

pub use banner::{StandingBanner, is_urgent};
pub use catalog::sync_catalog;
pub use entitlements::{Entitlements, Standing};
pub use models::{BillingAccount, BillingStatus, StripeEvent};
pub use page::{BillingPage, URL as BILLING_URL, money, plan_name, standing_notice};
pub use plans::{BillingInterval, CURRENCY, Feature, Plan, PlanId, lookup_key, plan_of_lookup_key};
pub use stripe::Billing;
pub use sync::{account_of, ensure_account, refresh_customer};
