//! Deleting a team, and an account with what only it owns: the right to erasure.
//!
//! Each deletion is one transaction, and a subscription's cancellation is a job queued in it: no
//! crash can delete the customer's data and keep charging them, or cancel a team that still exists.

use serde::{Deserialize, Serialize};
use tablo::{
    HeaderAction, HeaderActionDef, Outcome, TenantId, WriteError, auth,
    auth::{AuthFactor, AuthSession, AuthToken, ColumnPreference, verify_password},
    notification::StoredNotification,
};
use topcoat::context::{Cx, context};

use super::{Invitation, Member, Team, TeamRole, access};
use crate::accounts::{Identity, SignedIn, User};
use crate::audit;
use crate::billing::{Billing, BillingAccount};
use crate::jobs::{Job, enqueue};
use crate::projects::Project;

/// Cancels a subscription whose team is gone. It carries the id because the row went with the team.
#[derive(Serialize, Deserialize)]
pub struct CancelSubscription {
    pub subscription: String,
}

impl Job for CancelSubscription {
    const KIND: &'static str = "cancel-subscription";

    async fn run(self, cx: &Cx) -> anyhow::Result<()> {
        context::<Billing>(cx)
            .gateway()
            .cancel_subscription(self.subscription)
            .await?;
        Ok(())
    }
}

fn failed(error: impl std::fmt::Display) -> topcoat::Error {
    topcoat::Error::msg(error.to_string())
}

/// Deletes `team` and everything it owns, on the caller's transaction. Foreign keys would cascade
/// in production; naming each table keeps it true on any database and shows what erasure covers.
pub async fn delete_team(ex: &mut dyn toasty::Executor, team: TenantId) -> topcoat::Result<()> {
    let account = BillingAccount::filter(BillingAccount::fields().team_id().eq(team))
        .first()
        .exec(&mut *ex)
        .await?;
    if let Some(account) = account
        && !account.status.is_ended()
        && let Some(subscription) = account.subscription_id
    {
        enqueue(&CancelSubscription { subscription })
            .exec(&mut *ex)
            .await
            .map_err(failed)?;
    }
    BillingAccount::filter(BillingAccount::fields().team_id().eq(team))
        .delete()
        .exec(&mut *ex)
        .await?;
    Project::filter(Project::fields().team_id().eq(team))
        .delete()
        .exec(&mut *ex)
        .await?;
    audit::forget_team(&mut *ex, team).await?;
    Invitation::filter(Invitation::fields().team_id().eq(team))
        .delete()
        .exec(&mut *ex)
        .await?;
    Member::filter(Member::fields().team_id().eq(team))
        .delete()
        .exec(&mut *ex)
        .await?;
    Team::filter_by_id(team.get()).delete().exec(ex).await?;
    Ok(())
}

/// Deletes the rows that sign `user` in beside the account, on the caller's transaction: Tablo's
/// sessions, pending links and codes, second factors, list column choices and notifications,
/// which it leaves behind since only the app knows a user went, and the Google identities linked
/// to it.
pub async fn forget_sign_in(
    ex: &mut dyn toasty::Executor,
    user: uuid::Uuid,
) -> topcoat::Result<()> {
    Identity::filter(Identity::fields().user_id().eq(user))
        .delete()
        .exec(&mut *ex)
        .await?;
    let id = user.to_string();
    AuthSession::filter(AuthSession::fields().user_id().eq(&id))
        .delete()
        .exec(&mut *ex)
        .await?;
    AuthToken::filter(AuthToken::fields().user_id().eq(&id))
        .delete()
        .exec(&mut *ex)
        .await?;
    AuthFactor::filter(AuthFactor::fields().user_id().eq(&id))
        .delete()
        .exec(&mut *ex)
        .await?;
    ColumnPreference::filter(ColumnPreference::fields().user_id().eq(&id))
        .delete()
        .exec(&mut *ex)
        .await?;
    StoredNotification::filter(StoredNotification::fields().user_id().eq(&id))
        .delete()
        .exec(ex)
        .await?;
    Ok(())
}

/// Erases the account `user`, whose owner never proved its address, for a provider sign-in to
/// sign up in its place: the teams it alone is on, its seats on others, its name in the audit
/// log and every way it signs in. Invitations to the address stay, since they are its true
/// owner's.
pub async fn erase_unproved(
    cx: &Cx,
    ex: &mut dyn toasty::Executor,
    user: uuid::Uuid,
) -> topcoat::Result<()> {
    let seats = Member::filter(Member::fields().user_id().eq(user))
        .exec(&mut *ex)
        .await?;
    for seat in seats {
        let members = Member::filter(Member::fields().team_id().eq(seat.team_id))
            .exec(&mut *ex)
            .await?
            .len();
        if members == 1 {
            delete_team(&mut *ex, seat.team_id).await?;
        }
    }
    erase(cx, ex, user).await
}

/// Deletes `user`'s remaining seats, their name in the audit log, every way they sign in and the
/// account, on the caller's transaction.
async fn erase(cx: &Cx, ex: &mut dyn toasty::Executor, user: uuid::Uuid) -> topcoat::Result<()> {
    Member::filter(Member::fields().user_id().eq(user))
        .delete()
        .exec(&mut *ex)
        .await?;
    // Other teams keep what happened, but no longer who did it.
    audit::forget_actor(cx, &mut *ex, user).await?;
    forget_sign_in(&mut *ex, user).await?;
    User::filter_by_id(user).delete().exec(ex).await?;
    Ok(())
}

/// What deleting a team asks for: its name, typed.
#[derive(tablo::FormInput)]
pub struct TeamConfirmation {
    pub name: String,
}

/// Deletes the team the session acts for. Its owner must have another team to land on.
pub struct DeleteTeam;

impl HeaderAction for DeleteTeam {
    type Input = TeamConfirmation;
    const NAME: &'static str = "delete-team";

    fn declare() -> HeaderActionDef {
        HeaderActionDef::new()
            .color(tablo::Color::Danger)
            .can_run(|cx| {
                let other_teams =
                    auth::user::<SignedIn>(cx).is_some_and(|signed| signed.seats.len() > 1);
                access::current_role(cx) == Some(TeamRole::Owner) && other_teams
            })
    }

    async fn run(
        cx: &Cx,
        input: TeamConfirmation,
        ex: &mut dyn toasty::Executor,
    ) -> Result<Outcome, WriteError<TeamConfirmation>> {
        let membership = auth::membership(cx)
            .ok_or_else(|| WriteError::declined(tablo::text!("error-no-team")))?;
        if input.name.trim() != membership.name {
            return Err(WriteError::<TeamConfirmation>::field(
                |f| f.name,
                tablo::text!("error-team-name-mismatch"),
            ));
        }
        delete_team(ex, membership.tenant).await?;
        Ok(Outcome::done().redirect("/app"))
    }
}

/// What deleting an account asks for: its password.
#[derive(tablo::FormInput)]
pub struct AccountConfirmation {
    #[form(password)]
    pub password: String,
}

/// Deletes the signed-in account and the teams it alone is on. It refuses while they own a team
/// others use, so nobody loses work to a colleague's departure.
pub struct DeleteAccount;

impl HeaderAction for DeleteAccount {
    type Input = AccountConfirmation;
    const NAME: &'static str = "delete-account";

    fn declare() -> HeaderActionDef {
        HeaderActionDef::new().color(tablo::Color::Danger)
    }

    async fn run(
        cx: &Cx,
        input: AccountConfirmation,
        ex: &mut dyn toasty::Executor,
    ) -> Result<Outcome, WriteError<AccountConfirmation>> {
        let signed = auth::require_user::<SignedIn>(cx)?;
        if !verify_password(&input.password, Some(&signed.user.password)).await {
            return Err(WriteError::<AccountConfirmation>::field(
                |f| f.password,
                tablo::text!("error-password-wrong"),
            ));
        }
        let mut alone_on = Vec::new();
        for seat in &signed.seats {
            let members = Member::filter(Member::fields().team_id().eq(seat.team))
                .exec(&mut *ex)
                .await?
                .len();
            if members == 1 {
                alone_on.push(seat.team);
            } else if seat.role == TeamRole::Owner {
                return Err(WriteError::declined(tablo::text!("error-transfer-first")));
            }
        }
        for team in alone_on {
            delete_team(&mut *ex, team).await?;
        }
        Invitation::filter(Invitation::fields().email().eq(&signed.user.email))
            .delete()
            .exec(&mut *ex)
            .await?;
        erase(cx, ex, signed.user.id).await?;
        Ok(Outcome::done().redirect("/"))
    }
}
