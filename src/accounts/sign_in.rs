//! Signing in and signing up.

use jiff::Timestamp;
use tablo::{
    Authenticator, FieldErrors, Membership, PanelUser, Registrar, TenantId,
    auth::{hash_password, new_password_errors, verify_password},
    db::db,
};
use topcoat::{context::Cx, l10n::Locale};

use super::{User, normalize_email};
use crate::billing::{BillingAccount, Entitlements};
use crate::teams::{Member, Team, TeamRole, create_team};

/// The signed-in user and the teams they sit on, reloaded on every request, so removing a member
/// takes effect at once.
#[derive(Debug)]
pub struct SignedIn {
    pub user: User,
    pub teams: Vec<Membership>,
    pub seats: Vec<Seat>,
}

/// What a user is on one team.
#[derive(Debug, Clone, Copy)]
pub struct Seat {
    pub team: TenantId,
    pub role: TeamRole,
    /// What the team's plan allows, read here because a policy cannot query.
    pub entitlements: Entitlements,
}

impl PanelUser for SignedIn {
    fn user_id(&self) -> String {
        self.user.id.to_string()
    }

    fn display_name(&self) -> &str {
        &self.user.name
    }

    fn subtitle(&self) -> Option<&str> {
        Some(&self.user.email)
    }

    fn email(&self) -> Option<&str> {
        Some(&self.user.email)
    }

    /// Each panel is named and the rest refused, so a panel added later admits nobody until it
    /// has a rule here.
    fn can_access_panel(&self, panel: &str) -> bool {
        match panel {
            "/app" => true,
            "/admin" => self.user.staff_role.is_some(),
            _ => false,
        }
    }

    fn tenants(&self) -> &[Membership] {
        &self.teams
    }

    /// The language the user picked, which their pages and the mail a job sends them read.
    fn locale(&self) -> Option<Locale> {
        self.user.language()
    }
}

/// Reads users and creates them from the sign-up page.
pub struct Accounts;

impl Accounts {
    /// `user` as a session sees them.
    ///
    /// Runs on `ex` rather than a fresh handle: sign-up calls this while its transaction holds a
    /// connection, and a single-connection pool would wait forever for a second one.
    pub async fn signed_in(user: User, ex: &mut dyn toasty::Executor) -> topcoat::Result<SignedIn> {
        let members = Member::filter(Member::fields().user_id().eq(user.id))
            .exec(&mut *ex)
            .await?;
        let ids: Vec<uuid::Uuid> = members.iter().map(|member| member.team_id.get()).collect();
        let tenants: Vec<TenantId> = members.iter().map(|member| member.team_id).collect();
        let names = Team::filter(Team::fields().id().in_list(ids))
            .exec(&mut *ex)
            .await?;
        let accounts = BillingAccount::filter(BillingAccount::fields().team_id().in_list(tenants))
            .exec(&mut *ex)
            .await?;
        let now = Timestamp::now();
        let mut teams = Vec::with_capacity(members.len());
        let mut seats = Vec::with_capacity(members.len());
        for member in members {
            let Some(team) = names.iter().find(|team| team.id == member.team_id.get()) else {
                continue;
            };
            let account = accounts
                .iter()
                .find(|account| account.team_id == member.team_id);
            teams.push(Membership::new(member.team_id, team.name.clone()));
            seats.push(Seat {
                team: member.team_id,
                role: member.role,
                entitlements: Entitlements::of(account, now),
            });
        }
        Ok(SignedIn { user, teams, seats })
    }

    /// The account `email` signs in as.
    pub(super) async fn by_email(
        ex: &mut dyn toasty::Executor,
        email: &str,
    ) -> topcoat::Result<Option<User>> {
        Ok(
            User::filter(User::fields().email().eq(normalize_email(email)))
                .first()
                .exec(ex)
                .await?,
        )
    }
}

impl Authenticator for Accounts {
    type User = SignedIn;

    async fn verify(
        &self,
        cx: &Cx,
        login: &str,
        password: &str,
    ) -> topcoat::Result<Option<SignedIn>> {
        let mut handle = db(cx)?;
        let user = Self::by_email(&mut handle, login).await?;
        // An unknown address costs the same work as a wrong password.
        if !verify_password(password, user.as_ref().map(|user| user.password.as_str())).await {
            return Ok(None);
        }
        match user {
            Some(user) => Ok(Some(Self::signed_in(user, &mut handle).await?)),
            None => Ok(None),
        }
    }

    async fn find_by_id(&self, cx: &Cx, id: &str) -> topcoat::Result<Option<SignedIn>> {
        let Ok(id) = id.parse::<uuid::Uuid>() else {
            return Ok(None);
        };
        let mut handle = db(cx)?;
        match User::filter_by_id(id).first().exec(&mut handle).await? {
            Some(user) => Ok(Some(Self::signed_in(user, &mut handle).await?)),
            None => Ok(None),
        }
    }

    /// Keeps the language picked from the menu on the account, for every browser and for mail.
    async fn store_locale(&self, cx: &Cx, user: &SignedIn, locale: &Locale) -> topcoat::Result<()> {
        User::filter_by_id(user.user.id)
            .update()
            .locale(Some(locale.to_string()))
            .exec(&mut db(cx)?)
            .await?;
        Ok(())
    }
}

/// What the sign-up page asks for.
#[derive(tablo::FormInput)]
pub struct SignUp {
    pub name: String,
    #[form(email)]
    pub email: String,
    #[form(password)]
    pub password: String,
    #[form(password)]
    pub password_confirmation: String,
}

impl Registrar for Accounts {
    type Input = SignUp;
    type User = SignedIn;

    async fn validate(
        &self,
        _cx: &Cx,
        sign_up: &SignUp,
        ex: &mut dyn toasty::Executor,
    ) -> topcoat::Result<FieldErrors> {
        let mut errors = new_password_errors(&sign_up.password, &sign_up.password_confirmation);
        if sign_up.name.trim().is_empty() {
            errors.add("name", tablo::text!("error-name-required"));
        }
        if Self::by_email(ex, &sign_up.email).await?.is_some() {
            errors.add("email", tablo::text!("error-email-taken"));
        }
        Ok(errors)
    }

    async fn register(
        &self,
        cx: &Cx,
        sign_up: SignUp,
        ex: &mut dyn toasty::Executor,
    ) -> topcoat::Result<SignedIn> {
        let user = toasty::create!(User {
            name: sign_up.name.trim().to_string(),
            email: normalize_email(&sign_up.email),
            password: hash_password(&sign_up.password).await?,
        })
        .exec(&mut *ex)
        .await?;
        create_team(
            &mut *ex,
            &tablo::t!(cx, "team-personal", name = &user.name),
            &user,
        )
        .await?;
        Self::signed_in(user, ex).await
    }
}
