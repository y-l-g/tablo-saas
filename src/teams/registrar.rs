//! Creating a team, and renaming the one the session acts for.

use tablo::{FieldErrors, Membership, TenantId, auth::TenantEditor};
use topcoat::context::Cx;

use super::{Member, Team, TeamRole, access};
use crate::accounts::{SignedIn, User};

/// The name a team takes.
#[derive(tablo::FormInput)]
pub struct NewTeam {
    pub name: String,
}

impl NewTeam {
    fn errors(&self) -> FieldErrors {
        let mut errors = FieldErrors::new();
        if self.name.trim().is_empty() {
            errors.add("name", tablo::text!("error-team-name-required"));
        }
        errors
    }
}

/// Writes `name`'s team with `user` as its owner, on the caller's transaction.
pub async fn create_team(
    ex: &mut dyn toasty::Executor,
    name: &str,
    user: &User,
) -> topcoat::Result<Team> {
    let team = toasty::create!(Team {
        name: name.trim().to_string(),
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

impl tablo::auth::TenantRegistrar for Teams {
    type User = SignedIn;
    type Input = NewTeam;

    async fn validate(
        &self,
        _cx: &Cx,
        _user: &SignedIn,
        input: &NewTeam,
        _ex: &mut dyn toasty::Executor,
    ) -> topcoat::Result<FieldErrors> {
        Ok(input.errors())
    }

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

    async fn validate(
        &self,
        _cx: &Cx,
        _user: &SignedIn,
        _team: &Membership,
        input: &NewTeam,
        _ex: &mut dyn toasty::Executor,
    ) -> topcoat::Result<FieldErrors> {
        Ok(input.errors())
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
            .name(input.name.trim().to_string())
            .exec(ex)
            .await?;
        Ok(())
    }
}
