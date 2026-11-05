//! What the app sends to Stripe and reads back: the fields it uses, and no others.

use std::collections::HashMap;

use serde::Deserialize;
use thiserror::Error;

use crate::billing::BillingInterval;

/// Why a Stripe call failed.
#[derive(Debug, Error)]
pub enum StripeError {
    #[error("Stripe is not configured")]
    NotConfigured,
    #[error("Stripe answered {status}: {message}")]
    Api {
        status: u16,
        code: Option<String>,
        message: String,
    },
    #[error("reaching Stripe failed: {0}")]
    Transport(String),
    #[error("reading Stripe's answer failed: {0}")]
    Decode(String),
}

impl StripeError {
    /// Whether the same request might work: a rate limit, an outage or a lost connection.
    pub fn is_transient(&self) -> bool {
        match self {
            Self::Api { status, .. } => *status == 429 || *status >= 500,
            Self::Transport(_) => true,
            Self::NotConfigured | Self::Decode(_) => false,
        }
    }
}

/// A price to create.
#[derive(Debug, Clone)]
pub struct NewPrice {
    pub product: String,
    pub lookup_key: String,
    pub amount: i64,
    pub currency: &'static str,
    pub interval: BillingInterval,
}

/// A product and the prices a subscriber may move between in the customer portal.
#[derive(Debug, Clone)]
pub struct PortalProduct {
    pub product: String,
    pub prices: Vec<String>,
}

/// A customer to create for a team.
#[derive(Debug, Clone)]
pub struct NewCustomer {
    pub team: String,
    pub name: String,
    pub email: String,
}

/// A Checkout session to open.
#[derive(Debug, Clone)]
pub struct NewCheckout {
    pub customer: String,
    pub team: String,
    pub price: String,
    pub seats: i64,
    pub trial_days: Option<u32>,
    pub automatic_tax: bool,
    pub success_url: String,
    pub cancel_url: String,
}

/// A subscription's seat line changing size.
#[derive(Debug, Clone)]
pub struct SeatChange {
    pub subscription: String,
    pub item: String,
    pub seats: i64,
}

/// A Stripe list page.
#[derive(Debug, Clone, Deserialize)]
pub struct Page<T> {
    pub data: Vec<T>,
}

/// A price, with the fields the sync reads.
#[derive(Debug, Clone, Deserialize)]
pub struct Price {
    pub id: String,
    pub lookup_key: Option<String>,
    pub recurring: Option<Recurring>,
}

/// How a price repeats.
#[derive(Debug, Clone, Deserialize)]
pub struct Recurring {
    pub interval: String,
}

/// A line of a subscription. The billing period sits here since API version 2025-03-31.
#[derive(Debug, Clone, Deserialize)]
pub struct SubscriptionItem {
    pub id: String,
    pub quantity: Option<i64>,
    pub current_period_end: i64,
    pub price: Price,
}

/// A subscription, unexpanded.
#[derive(Debug, Clone, Deserialize)]
pub struct Subscription {
    pub id: String,
    pub customer: String,
    pub status: String,
    pub items: Page<SubscriptionItem>,
    pub cancel_at_period_end: bool,
    pub trial_end: Option<i64>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
    pub created: i64,
}

/// A finalized invoice, for the billing page. Amounts are minor units.
#[derive(Debug, Clone, Deserialize)]
pub struct Invoice {
    pub id: String,
    pub number: Option<String>,
    pub status: Option<String>,
    pub total: i64,
    pub currency: String,
    pub created: i64,
    pub hosted_invoice_url: Option<String>,
}

/// What a webhook delivers. Its `object` is a hint, read only for which customer to refresh.
#[derive(Debug, Clone, Deserialize)]
pub struct Event {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub created: i64,
    pub data: EventData,
}

/// A webhook's payload.
#[derive(Debug, Clone, Deserialize)]
pub struct EventData {
    pub object: serde_json::Value,
}
