//! What a member's role lets them do. Everything the team pages decide is decided here.

use jiff::Timestamp;
use tablo::{Load, TenantId, auth, db::db, loaded, tenant_id};
use topcoat::context::Cx;

use super::{Member, TeamRole};
use crate::accounts::SignedIn;
use crate::billing::{BillingAccount, Entitlements};

/// What the plan of the team the request acts for allows, which the app panel loads once per
/// request for its policies, since a policy cannot query.
pub struct TeamPlan(pub Entitlements);

impl Load for TeamPlan {
    async fn load(cx: &Cx) -> topcoat::Result<Self> {
        let now = Timestamp::now();
        let Some(team) = tenant_id(cx) else {
            return Ok(Self(Entitlements::of(None, now)));
        };
        let account = BillingAccount::filter(BillingAccount::fields().team_id().eq(team))
            .first()
            .exec(&mut db(cx)?)
            .await?;
        Ok(Self(Entitlements::of(account.as_ref(), now)))
    }
}

/// The signed-in user's role on `team`, `None` when they are not on it.
pub fn role_in(signed: &SignedIn, team: TenantId) -> Option<TeamRole> {
    signed
        .seats
        .iter()
        .find(|seat| seat.team == team)
        .map(|seat| seat.role)
}

/// The signed-in user's role on the team the request acts for.
pub fn current_role(cx: &Cx) -> Option<TeamRole> {
    let signed = auth::user::<SignedIn>(cx)?;
    role_in(signed, tenant_id(cx)?)
}

/// What the current team's plan allows, the free plan when the session acts for none or the
/// request loaded no [`TeamPlan`].
pub fn current_entitlements(cx: &Cx) -> Entitlements {
    loaded::<TeamPlan>(cx).map_or_else(|| Entitlements::of(None, Timestamp::now()), |plan| plan.0)
}

/// Whether the request may invite and remove people and rename the team.
pub fn may_manage_members(cx: &Cx) -> bool {
    matches!(current_role(cx), Some(TeamRole::Owner | TeamRole::Admin))
}

/// Whether an acting role may change `target`: an owner changes anyone but themself, an admin
/// changes plain members, and the owner's seat moves only by a transfer.
pub(super) fn may_change(acting: Option<TeamRole>, target: &Member, own_id: uuid::Uuid) -> bool {
    if target.user_id == own_id || target.role == TeamRole::Owner {
        return false;
    }
    match acting {
        Some(TeamRole::Owner) => true,
        Some(TeamRole::Admin) => target.role == TeamRole::Member,
        Some(TeamRole::Member) | None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(role: TeamRole) -> Member {
        Member {
            id: uuid::Uuid::now_v7(),
            team_id: uuid::Uuid::now_v7().into(),
            user_id: uuid::Uuid::now_v7(),
            user: toasty::Deferred::default(),
            role,
            created_at: jiff::Timestamp::now(),
        }
    }

    #[test]
    fn an_owner_changes_anyone_but_the_owner() {
        let me = uuid::Uuid::now_v7();
        assert!(may_change(
            Some(TeamRole::Owner),
            &member(TeamRole::Admin),
            me
        ));
        assert!(may_change(
            Some(TeamRole::Owner),
            &member(TeamRole::Member),
            me
        ));
        assert!(!may_change(
            Some(TeamRole::Owner),
            &member(TeamRole::Owner),
            me
        ));
    }

    #[test]
    fn an_admin_changes_plain_members_alone() {
        let me = uuid::Uuid::now_v7();
        assert!(may_change(
            Some(TeamRole::Admin),
            &member(TeamRole::Member),
            me
        ));
        assert!(!may_change(
            Some(TeamRole::Admin),
            &member(TeamRole::Admin),
            me
        ));
    }

    #[test]
    fn a_member_changes_nobody_and_nobody_changes_themself() {
        let target = member(TeamRole::Member);
        assert!(!may_change(
            Some(TeamRole::Member),
            &target,
            uuid::Uuid::now_v7()
        ));
        assert!(!may_change(Some(TeamRole::Owner), &target, target.user_id));
    }
}
