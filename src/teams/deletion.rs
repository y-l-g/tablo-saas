//! Deleting a team, and an account with what only it owns: the right to erasure.
//!
//! Each deletion is one transaction, and a subscription's cancellation is a job queued in it: no
//! crash can delete the customer's data and keep charging them, or cancel a team that still exists.

use serde::{Deserialize, Serialize};
use tablo::{
    HeaderAction, Outcome, Page, PageDef, TenantId, WriteError, auth, auth::verify_password,
};
use topcoat::context::{Cx, context};
use topcoat::icon::iconify::iconify_icon;

use super::{Invitation, Member, Team, TeamRole, access};
use crate::accounts::{SignedIn, User};
use crate::audit::AuditEntry;
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
    AuditEntry::filter(AuditEntry::fields().team_id().eq(team))
        .delete()
        .exec(&mut *ex)
        .await?;
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
    const COLOR: tablo::Color = tablo::Color::Danger;

    fn can_run(cx: &Cx) -> bool {
        let other_teams = auth::user::<SignedIn>(cx).is_some_and(|signed| signed.seats.len() > 1);
        access::current_role(cx) == Some(TeamRole::Owner) && other_teams
    }

    async fn run(
        cx: &Cx,
        input: TeamConfirmation,
        ex: &mut dyn toasty::Executor,
    ) -> Result<Outcome, WriteError<TeamConfirmationField>> {
        let membership = auth::membership(cx)
            .ok_or_else(|| WriteError::declined(tablo::text!("error-no-team")))?;
        if input.name.trim() != membership.name {
            return Err(WriteError::field(
                TeamConfirmationField::Name,
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
    const COLOR: tablo::Color = tablo::Color::Danger;

    async fn run(
        cx: &Cx,
        input: AccountConfirmation,
        ex: &mut dyn toasty::Executor,
    ) -> Result<Outcome, WriteError<AccountConfirmationField>> {
        let signed = auth::require_user::<SignedIn>(cx)?;
        if !verify_password(&input.password, Some(&signed.user.password)).await {
            return Err(WriteError::field(
                AccountConfirmationField::Password,
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
        Member::filter(Member::fields().user_id().eq(signed.user.id))
            .delete()
            .exec(&mut *ex)
            .await?;
        // Other teams keep what happened, but no longer who did it.
        AuditEntry::filter(AuditEntry::fields().actor().eq(&signed.user.email))
            .update()
            .actor(tablo::t!(cx, "audit-deleted-actor"))
            .exec(&mut *ex)
            .await?;
        Invitation::filter(Invitation::fields().email().eq(&signed.user.email))
            .delete()
            .exec(&mut *ex)
            .await?;
        User::filter_by_id(signed.user.id).delete().exec(ex).await?;
        Ok(Outcome::done().redirect("/"))
    }
}

/// Where a person deletes their account.
pub struct AccountPage;

impl Page for AccountPage {
    fn declare() -> PageDef {
        PageDef::new()
            .slug("account")
            .label(tablo::text!("page-account"))
            .title(tablo::text!("page-account"))
            .description(tablo::text!("account-description"))
            .icon(iconify_icon!("lucide:id-card"))
            .header_action::<DeleteAccount>()
    }
}
