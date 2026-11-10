//! The mirror of a team's Stripe subscription.

use jiff::Timestamp;
use tablo::TenantId;

use super::{BillingInterval, PlanId};

/// A team's billing state: one row per team that ever started paying, written only by the sync
/// from Stripe. A team without a row is on the free plan.
#[derive(Debug, Clone, toasty::Model, tablo::ModelColumns)]
#[table = "billing_accounts"]
pub struct BillingAccount {
    #[key]
    #[auto]
    pub id: uuid::Uuid,
    #[unique]
    pub team_id: TenantId,
    #[unique]
    pub customer_id: String,
    pub plan: PlanId,
    pub status: BillingStatus,
    pub subscription_id: Option<String>,
    /// The subscription's seat line, which a seat change updates.
    pub subscription_item_id: Option<String>,
    pub billing_interval: Option<BillingInterval>,
    pub seats: Option<i64>,
    pub current_period_end: Option<Timestamp>,
    pub cancel_at_period_end: bool,
    pub trial_ends_at: Option<Timestamp>,
    /// When the subscription turned past due, which starts the grace period.
    pub past_due_since: Option<Timestamp>,
    pub synced_at: Option<Timestamp>,
    #[default(Timestamp::now())]
    pub created_at: Timestamp,
    #[update(Timestamp::now())]
    pub updated_at: Timestamp,
    #[version]
    pub version: u64,
}

/// A subscription's status, as Stripe spells it, with `None` before there is one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, toasty::Embed, tablo::Options)]
#[serde(rename_all = "snake_case")]
pub enum BillingStatus {
    #[option(color = "gray")]
    None,
    #[option(color = "info")]
    Trialing,
    #[option(color = "success")]
    Active,
    #[option(color = "warning")]
    PastDue,
    #[option(color = "danger")]
    Unpaid,
    #[option(color = "gray")]
    Canceled,
    #[option(color = "warning")]
    Incomplete,
    #[option(color = "gray")]
    IncompleteExpired,
    #[option(color = "gray")]
    Paused,
}

impl BillingStatus {
    /// Reads Stripe's word, `None` for one this template has not met.
    pub fn from_stripe(status: &str) -> Option<Self> {
        Some(match status {
            "trialing" => Self::Trialing,
            "active" => Self::Active,
            "past_due" => Self::PastDue,
            "unpaid" => Self::Unpaid,
            "canceled" => Self::Canceled,
            "incomplete" => Self::Incomplete,
            "incomplete_expired" => Self::IncompleteExpired,
            "paused" => Self::Paused,
            _ => return None,
        })
    }

    /// Whether the subscription is over, so a team may buy a new one.
    pub fn is_ended(self) -> bool {
        matches!(self, Self::None | Self::Canceled | Self::IncompleteExpired)
    }
}

/// A Stripe event that arrived: the inbox that makes delivery idempotent. It keeps the id, the
/// kind and the customer, and not the payload, which holds personal data the API can give again.
#[derive(Debug, Clone, toasty::Model, tablo::ModelColumns)]
#[table = "stripe_events"]
pub struct StripeEvent {
    #[key]
    pub id: String,
    pub kind: String,
    #[index]
    pub customer_id: String,
    #[default(Timestamp::now())]
    pub received_at: Timestamp,
    pub processed_at: Option<Timestamp>,
}
