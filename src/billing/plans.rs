//! The plan catalog: what is sold, for how much, and what it allows.
//!
//! To change a price, add a new lookup key rather than editing an amount: Stripe prices cannot
//! change, and subscribers keep the one they bought. [`plan_of_lookup_key`] reads the plan from the
//! key's first word, so `pro_month_2027` still means Pro.

use std::fmt;

/// The currency of every price.
pub const CURRENCY: &str = "eur";

/// A plan a team can be on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, toasty::Embed, tablo::Options)]
pub enum PlanId {
    #[option(color = "gray")]
    Free,
    #[option(color = "info")]
    Pro,
    #[option(color = "success")]
    Business,
}

/// How often a subscription renews.
#[derive(Debug, Clone, Copy, PartialEq, Eq, toasty::Embed, tablo::Options)]
pub enum BillingInterval {
    Month,
    Year,
}

impl BillingInterval {
    pub const BOTH: [Self; 2] = [Self::Month, Self::Year];

    /// The word Stripe and lookup keys use.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Month => "month",
            Self::Year => "year",
        }
    }

    pub fn from_stripe(interval: &str) -> Option<Self> {
        match interval {
            "month" => Some(Self::Month),
            "year" => Some(Self::Year),
            _ => None,
        }
    }
}

impl fmt::Display for BillingInterval {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A capability a plan turns on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feature {
    /// The team's audit trail.
    AuditLog,
    /// Priority support, shown on the pricing page.
    PrioritySupport,
}

/// What a plan sells. Amounts are minor units of [`CURRENCY`], per seat.
#[derive(Debug, Clone, Copy)]
pub struct Plan {
    pub id: PlanId,
    pub monthly_amount: i64,
    pub yearly_amount: i64,
    /// How many seats a team on this plan may fill, `None` for no limit.
    pub seat_limit: Option<u32>,
    /// How many projects a team on this plan may keep, `None` for no limit.
    pub project_limit: Option<u32>,
    pub features: &'static [Feature],
}

/// The free plan a team without a subscription falls to.
pub const FREE: Plan = Plan {
    id: PlanId::Free,
    monthly_amount: 0,
    yearly_amount: 0,
    seat_limit: Some(3),
    project_limit: Some(3),
    features: &[],
};

const PRO: Plan = Plan {
    id: PlanId::Pro,
    monthly_amount: 1_900,
    yearly_amount: 19_000,
    seat_limit: Some(25),
    project_limit: Some(50),
    features: &[],
};

const BUSINESS: Plan = Plan {
    id: PlanId::Business,
    monthly_amount: 4_900,
    yearly_amount: 49_000,
    seat_limit: None,
    project_limit: None,
    features: &[Feature::AuditLog, Feature::PrioritySupport],
};

impl Plan {
    /// Every plan, cheapest first, as the pricing page lists them.
    pub const ALL: [Self; 3] = [FREE, PRO, BUSINESS];

    /// The plans sold through Stripe.
    pub fn paid() -> impl Iterator<Item = Self> {
        Self::ALL.into_iter().filter(|plan| plan.id != PlanId::Free)
    }

    pub fn of(id: PlanId) -> Self {
        match id {
            PlanId::Free => FREE,
            PlanId::Pro => PRO,
            PlanId::Business => BUSINESS,
        }
    }

    pub fn amount(&self, interval: BillingInterval) -> i64 {
        match interval {
            BillingInterval::Month => self.monthly_amount,
            BillingInterval::Year => self.yearly_amount,
        }
    }

    pub fn has(&self, feature: Feature) -> bool {
        self.features.contains(&feature)
    }

    /// The Stripe product this plan is sold as.
    pub fn product_id(&self) -> String {
        format!("tablo_saas_{}", self.slug())
    }

    /// The lowercase word naming this plan in lookup keys and URLs.
    pub fn slug(&self) -> &'static str {
        match self.id {
            PlanId::Free => "free",
            PlanId::Pro => "pro",
            PlanId::Business => "business",
        }
    }

    pub fn by_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|plan| plan.slug() == slug)
    }
}

/// The lookup key a plan's price goes by.
pub fn lookup_key(plan: &Plan, interval: BillingInterval) -> String {
    format!("{}_{interval}", plan.slug())
}

/// The plan a Stripe price sells, from its lookup key's first word.
pub fn plan_of_lookup_key(key: &str) -> Option<Plan> {
    let slug = key.split('_').next()?;
    Plan::paid().find(|plan| plan.slug() == slug)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lookup_key_names_its_plan_through_price_changes() {
        let pro = Plan::of(PlanId::Pro);
        let key = lookup_key(&pro, BillingInterval::Year);
        assert_eq!(key, "pro_year");
        assert_eq!(
            plan_of_lookup_key(&key).map(|plan| plan.id),
            Some(PlanId::Pro)
        );
        assert_eq!(
            plan_of_lookup_key("business_month_2027").map(|plan| plan.id),
            Some(PlanId::Business)
        );
    }

    #[test]
    fn a_lookup_key_never_sells_the_free_plan() {
        assert!(plan_of_lookup_key("free_month").is_none());
        assert!(plan_of_lookup_key("unknown").is_none());
    }

    #[test]
    fn every_year_costs_less_than_twelve_months() {
        for plan in Plan::paid() {
            assert!(
                plan.yearly_amount < plan.monthly_amount * 12,
                "{:?}",
                plan.id
            );
        }
    }
}
