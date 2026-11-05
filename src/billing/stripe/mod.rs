//! Stripe, behind the handful of calls the app makes.
//!
//! A thin client of our own rather than a crate: the app needs a dozen endpoints, a pinned API
//! version and its own retry rule, and no Rust SDK has reached a stable release. The [`Gateway`]
//! trait is what the rest of the app knows, so tests run against a fake.

mod client;
mod signature;
mod types;

use std::{future::Future, pin::Pin, sync::Arc};

pub use client::StripeClient;
pub use signature::{SignatureError, sign, verify_signature};
pub use types::*;

use crate::platform::config::StripeSettings;

/// What a gateway call returns.
pub type Call<'a, T> = Pin<Box<dyn Future<Output = Result<T, StripeError>> + Send + 'a>>;

/// The calls the app makes to Stripe. Methods take owned values so a call is one boxed future.
pub trait Gateway: Send + Sync {
    fn product_exists(&self, id: String) -> Call<'_, bool>;
    fn create_product(&self, id: String, name: String) -> Call<'_, ()>;
    fn price_by_lookup_key(&self, key: String) -> Call<'_, Option<Price>>;
    fn create_price(&self, price: NewPrice) -> Call<'_, Price>;
    fn configure_portal(&self, products: Vec<PortalProduct>) -> Call<'_, ()>;
    /// Creates a team's customer, once however often it is called with one `team`.
    fn create_customer(&self, customer: NewCustomer) -> Call<'_, String>;
    /// A hosted Checkout page's URL.
    fn create_checkout(&self, checkout: NewCheckout) -> Call<'_, String>;
    /// A hosted customer portal page's URL.
    fn create_portal_session(&self, customer: String, return_url: String) -> Call<'_, String>;
    fn subscription(&self, id: String) -> Call<'_, Subscription>;
    /// The customer's subscriptions, ended ones included, newest first.
    fn customer_subscriptions(&self, customer: String) -> Call<'_, Vec<Subscription>>;
    fn set_seats(&self, change: SeatChange) -> Call<'_, ()>;
    fn cancel_subscription(&self, id: String) -> Call<'_, ()>;
    fn invoices(&self, customer: String) -> Call<'_, Vec<Invoice>>;
}

/// The gateway the app reads from context, which refuses every call when Stripe is not configured,
/// so a development checkout runs with no keys.
#[derive(Clone)]
pub struct Billing {
    gateway: Arc<dyn Gateway>,
    webhook_secret: Option<String>,
    enabled: bool,
}

impl Billing {
    pub fn new(settings: Option<&StripeSettings>) -> anyhow::Result<Self> {
        Ok(match settings {
            Some(settings) => Self {
                gateway: Arc::new(StripeClient::new(&settings.secret_key, &settings.api_base)?),
                webhook_secret: Some(settings.webhook_secret.clone()),
                enabled: true,
            },
            None => Self {
                gateway: Arc::new(Unconfigured),
                webhook_secret: None,
                enabled: false,
            },
        })
    }

    /// Billing through `gateway`, as a test states it.
    pub fn with_gateway(gateway: Arc<dyn Gateway>, webhook_secret: &str) -> Self {
        Self {
            gateway,
            webhook_secret: Some(webhook_secret.to_string()),
            enabled: true,
        }
    }

    pub fn gateway(&self) -> &dyn Gateway {
        self.gateway.as_ref()
    }

    pub fn webhook_secret(&self) -> Option<&str> {
        self.webhook_secret.as_deref()
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

/// Stands in when no Stripe key is set.
struct Unconfigured;

fn refuse<'a, T: Send + 'a>() -> Call<'a, T> {
    Box::pin(async { Err(StripeError::NotConfigured) })
}

impl Gateway for Unconfigured {
    fn product_exists(&self, _id: String) -> Call<'_, bool> {
        refuse()
    }
    fn create_product(&self, _id: String, _name: String) -> Call<'_, ()> {
        refuse()
    }
    fn price_by_lookup_key(&self, _key: String) -> Call<'_, Option<Price>> {
        refuse()
    }
    fn create_price(&self, _price: NewPrice) -> Call<'_, Price> {
        refuse()
    }
    fn configure_portal(&self, _products: Vec<PortalProduct>) -> Call<'_, ()> {
        refuse()
    }
    fn create_customer(&self, _customer: NewCustomer) -> Call<'_, String> {
        refuse()
    }
    fn create_checkout(&self, _checkout: NewCheckout) -> Call<'_, String> {
        refuse()
    }
    fn create_portal_session(&self, _customer: String, _return_url: String) -> Call<'_, String> {
        refuse()
    }
    fn subscription(&self, _id: String) -> Call<'_, Subscription> {
        refuse()
    }
    fn customer_subscriptions(&self, _customer: String) -> Call<'_, Vec<Subscription>> {
        refuse()
    }
    fn set_seats(&self, _change: SeatChange) -> Call<'_, ()> {
        refuse()
    }
    fn cancel_subscription(&self, _id: String) -> Call<'_, ()> {
        refuse()
    }
    fn invoices(&self, _customer: String) -> Call<'_, Vec<Invoice>> {
        refuse()
    }
}
