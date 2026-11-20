//! Creating a team, and renaming the one the session acts for.

use tablo::{Membership, TenantEditor, TenantId};
use topcoat::context::Cx;

use super::{Member, Team, TeamRole, access};
use crate::accounts::{SignedIn, User};

/// The name a team takes. Tablo trims it and refuses it blank, which is all either page asks.
#[derive(tablo::FormInput)]
pub struct NewTeam {
    pub name: String,
}

/// Writes `name`'s team with `user` as its owner, on the caller's transaction.
pub async fn create_team(
    ex: &mut dyn toasty::Executor,
    name: &str,
    user: &User,
) -> topcoat::Result<Team> {
    let team = toasty::create!(Team {
        name: name.to_string(),
    })
    .exec(&mut *ex)
    .await?;
    let team_id: TenantId = team.tenant();
    toasty::create!(Member {
        team_id: team_id,
        user_id: user.id,
        role: TeamRole::Owner,
    })
    .exec(ex)
    .await?;
    Ok(team)
}

/// Creates a team for a signed-in user, and renames the current one.
pub struct Teams;

impl tablo::TenantRegistrar for Teams {
    type User = SignedIn;
    type Input = NewTeam;

    async fn register(
        &self,
        _cx: &Cx,
        user: &SignedIn,
        input: NewTeam,
        ex: &mut dyn toasty::Executor,
    ) -> topcoat::Result<Membership> {
        let team = create_team(ex, &input.name, &user.user).await?;
        Ok(Membership::new(team.tenant(), team.name))
    }
}

impl TenantEditor for Teams {
    type Input = NewTeam;
    type User = SignedIn;

    fn may_edit(&self, _cx: &Cx, user: &SignedIn, team: &Membership) -> bool {
        matches!(
            access::role_in(user, team.tenant),
            Some(TeamRole::Owner | TeamRole::Admin)
        )
    }

    async fn fill(
        &self,
        _cx: &Cx,
        _user: &SignedIn,
        team: &Membership,
    ) -> topcoat::Result<NewTeam> {
        Ok(NewTeam {
            name: team.name.clone(),
        })
    }

    async fn save(
        &self,
        _cx: &Cx,
        _user: &SignedIn,
        team: &Membership,
        input: NewTeam,
        ex: &mut dyn toasty::Executor,
    ) -> topcoat::Result<()> {
        Team::filter_by_id(team.tenant.get())
            .update()
            .name(input.name)
            .exec(ex)
            .await?;
        Ok(())
    }
}
