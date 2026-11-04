//! What a team may do, decided in one place and with no I/O.

use jiff::{SignedDuration, Timestamp};

use super::{BillingAccount, BillingStatus, Feature, Plan, PlanId, plans::FREE};

/// How long a failed payment leaves a team its plan, while Stripe retries the card.
pub const GRACE: SignedDuration = SignedDuration::from_hours(7 * 24);

/// What a team's plan allows now.
#[derive(Debug, Clone, Copy)]
pub struct Entitlements {
    pub plan: Plan,
    pub standing: Standing,
}

/// How a team stands with its plan, for the banners.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    /// Nothing to say: the free plan or a healthy subscription.
    Good,
    /// On trial until this moment.
    Trialing(Timestamp),
    /// Paid up, and ending at this moment rather than renewing.
    Ending(Timestamp),
    /// A payment failed, and the plan stands until this moment.
    Grace(Timestamp),
    /// The subscription lapsed, so the team fell to the free plan. Nothing is deleted.
    Lapsed,
}

impl Entitlements {
    /// What `account` allows at `now`. A team with no account is on the free plan.
    pub fn of(account: Option<&BillingAccount>, now: Timestamp) -> Self {
        let Some(account) = account else {
            return Self::free(Standing::Good);
        };
        let plan = Plan::of(account.plan);
        match account.status {
            BillingStatus::Trialing => Self {
                plan,
                standing: account
                    .trial_ends_at
                    .map_or(Standing::Good, Standing::Trialing),
            },
            BillingStatus::Active => Self {
                plan,
                standing: match account.current_period_end {
                    Some(end) if account.cancel_at_period_end => Standing::Ending(end),
                    _ => Standing::Good,
                },
            },
            BillingStatus::PastDue => {
                let since = account.past_due_since.unwrap_or(now);
                match since.checked_add(GRACE) {
                    Ok(until) if until > now => Self {
                        plan,
                        standing: Standing::Grace(until),
                    },
                    _ => Self::free(Standing::Lapsed),
                }
            }
            BillingStatus::Unpaid | BillingStatus::Paused => Self::free(Standing::Lapsed),
            BillingStatus::None
            | BillingStatus::Canceled
            | BillingStatus::Incomplete
            | BillingStatus::IncompleteExpired => Self::free(Standing::Good),
        }
    }

    fn free(standing: Standing) -> Self {
        Self {
            plan: FREE,
            standing,
        }
    }

    pub fn has(&self, feature: Feature) -> bool {
        self.plan.has(feature)
    }

    /// Whether a team of `seats` may take one more person.
    pub fn may_add_seat(&self, seats: usize) -> bool {
        within(self.plan.seat_limit, seats)
    }

    /// Whether a team with `projects` may start one more.
    pub fn may_add_project(&self, projects: usize) -> bool {
        within(self.plan.project_limit, projects)
    }

    pub fn is_paid(&self) -> bool {
        self.plan.id != PlanId::Free
    }
}

fn within(limit: Option<u32>, used: usize) -> bool {
    limit.is_none_or(|limit| used < limit as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: Timestamp = Timestamp::constant(1_800_000_000, 0);
    const DAY: i64 = 86_400;

    fn account(status: BillingStatus) -> BillingAccount {
        BillingAccount {
            id: uuid::Uuid::now_v7(),
            team_id: uuid::Uuid::now_v7().into(),
            customer_id: "cus_test".to_string(),
            plan: PlanId::Pro,
            status,
            subscription_id: Some("sub_test".to_string()),
            subscription_item_id: Some("si_test".to_string()),
            billing_interval: Some(crate::billing::BillingInterval::Month),
            seats: Some(1),
            current_period_end: Some(at(30)),
            cancel_at_period_end: false,
            trial_ends_at: None,
            past_due_since: None,
            synced_at: Some(NOW),
            created_at: NOW,
            updated_at: NOW,
            version: 1,
        }
    }

    fn at(days: i64) -> Timestamp {
        Timestamp::from_second(NOW.as_second() + days * DAY).expect("in range")
    }

    #[test]
    fn no_account_is_the_free_plan() {
        let entitlements = Entitlements::of(None, NOW);
        assert_eq!(entitlements.plan.id, PlanId::Free);
        assert_eq!(entitlements.standing, Standing::Good);
    }

    #[test]
    fn an_active_subscription_gives_its_plan() {
        let entitlements = Entitlements::of(Some(&account(BillingStatus::Active)), NOW);
        assert_eq!(entitlements.plan.id, PlanId::Pro);
        assert_eq!(entitlements.standing, Standing::Good);
    }

    #[test]
    fn a_trial_gives_the_plan_and_says_when_it_ends() {
        let mut trial = account(BillingStatus::Trialing);
        trial.trial_ends_at = Some(at(7));
        let entitlements = Entitlements::of(Some(&trial), NOW);
        assert_eq!(entitlements.plan.id, PlanId::Pro);
        assert_eq!(entitlements.standing, Standing::Trialing(at(7)));
    }

    #[test]
    fn a_cancelled_renewal_keeps_the_plan_until_the_period_ends() {
        let mut ending = account(BillingStatus::Active);
        ending.cancel_at_period_end = true;
        let entitlements = Entitlements::of(Some(&ending), NOW);
        assert_eq!(entitlements.plan.id, PlanId::Pro);
        assert_eq!(entitlements.standing, Standing::Ending(at(30)));
    }

    #[test]
    fn a_failed_payment_keeps_the_plan_through_the_grace_period_alone() {
        let mut late = account(BillingStatus::PastDue);
        late.past_due_since = Some(at(-6));
        let within = Entitlements::of(Some(&late), NOW);
        assert_eq!(within.plan.id, PlanId::Pro);
        assert_eq!(within.standing, Standing::Grace(at(1)));

        late.past_due_since = Some(at(-8));
        let after = Entitlements::of(Some(&late), NOW);
        assert_eq!(after.plan.id, PlanId::Free);
        assert_eq!(after.standing, Standing::Lapsed);
    }

    #[test]
    fn nothing_past_a_subscription_gives_the_plan() {
        for status in [
            BillingStatus::Canceled,
            BillingStatus::Unpaid,
            BillingStatus::Incomplete,
            BillingStatus::IncompleteExpired,
            BillingStatus::Paused,
            BillingStatus::None,
        ] {
            let entitlements = Entitlements::of(Some(&account(status)), NOW);
            assert_eq!(entitlements.plan.id, PlanId::Free, "{status:?}");
        }
    }

    #[test]
    fn limits_stop_at_the_plan_and_unlimited_never_does() {
        let free = Entitlements::of(None, NOW);
        assert!(free.may_add_seat(2));
        assert!(!free.may_add_seat(3));
        let mut account = account(BillingStatus::Active);
        account.plan = PlanId::Business;
        let business = Entitlements::of(Some(&account), NOW);
        assert!(business.may_add_seat(10_000));
        assert!(business.may_add_project(10_000));
        assert!(business.has(Feature::AuditLog));
        assert!(!free.has(Feature::AuditLog));
    }
}
