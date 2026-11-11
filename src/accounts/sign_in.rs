//! Signing in and signing up.

use jiff::Timestamp;
use tablo::{
    FieldErrors, Lens, Membership, PanelUser, PasswordAccount, PasswordAuth, Registrar, TenantId,
    auth::{AuthFactor, FactorKind, hash_password, new_password_errors},
    db::db,
    lens,
};
use topcoat::{context::Cx, l10n::Locale};

use super::{User, normalize_email};
use crate::teams::{Member, Team, TeamRole, create_team};

/// The signed-in user and the teams they sit on, reloaded on every request, so removing a member
/// takes effect at once.
#[derive(Debug)]
pub struct SignedIn {
    pub user: User,
    pub teams: Vec<Membership>,
    pub seats: Vec<Seat>,
    /// Whether a member of staff holds a second factor on the app panel that was confirmed before
    /// the `staff` command granted their role, which acting as a customer asks for.
    ///
    /// A factor signs in to the one panel that enrolled it, so the back office's does not count.
    /// And setting one up asks for the password alone, so one confirmed after the grant may be a
    /// thief's: only the deploy-level grant vouches for it. Always `false` for a customer, whose
    /// rows are not read.
    pub app_factor: bool,
}

/// What a user is on one team. What the team's plan allows is
/// [`TeamPlan`](crate::teams::TeamPlan), which the panel loads for the team the request acts for.
#[derive(Debug, Clone, Copy)]
pub struct Seat {
    pub team: TenantId,
    pub role: TeamRole,
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
            APP_PANEL => true,
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

/// Whether `user` holds an authenticator or mailed codes on the app panel, confirmed no later
/// than `vouched`. A recovery code alone is what remains of a factor, not one.
async fn has_app_factor(
    ex: &mut dyn toasty::Executor,
    user: uuid::Uuid,
    vouched: Timestamp,
) -> topcoat::Result<bool> {
    let factors = AuthFactor::filter(
        AuthFactor::fields()
            .user_id()
            .eq(user.to_string())
            .and(AuthFactor::fields().panel().eq(APP_PANEL.to_string())),
    )
    .exec(ex)
    .await?;
    Ok(factors.iter().any(|factor| {
        factor.kind != FactorKind::RecoveryCode
            && factor
                .confirmed_at
                .is_some_and(|confirmed| confirmed <= vouched)
    }))
}

/// Whether the request's signed-in user is a customer whom staff act as.
///
/// Tablo keeps the actor off the target's credentials and tenant settings. What else stays the
/// customer's own call refuses here too: who their teams include, and their money.
pub fn is_impersonated(cx: &Cx) -> bool {
    tablo::auth::impersonator::<SignedIn>(cx).is_some()
}

/// The prefix of the panel customers use.
const APP_PANEL: &str = "/app";

/// Creates users from the sign-up page and from a first sign-in with Google.
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
        let names = Team::filter(Team::fields().id().in_list(ids))
            .exec(&mut *ex)
            .await?;
        let mut teams = Vec::with_capacity(members.len());
        let mut seats = Vec::with_capacity(members.len());
        for member in members {
            let Some(team) = names.iter().find(|team| team.id == member.team_id.get()) else {
                continue;
            };
            teams.push(Membership::new(member.team_id, team.name.clone()));
            seats.push(Seat {
                team: member.team_id,
                role: member.role,
            });
        }
        let app_factor = match (user.staff_role, user.staff_since) {
            (Some(_), Some(vouched)) => has_app_factor(&mut *ex, user.id, vouched).await?,
            _ => false,
        };
        Ok(SignedIn {
            user,
            teams,
            seats,
            app_factor,
        })
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

/// Signs users in, and edits, resets and verifies their accounts.
pub fn password_auth() -> PasswordAuth<User> {
    PasswordAuth::default()
}

/// The columns [`password_auth`] reads, and what it signs in: the user with their teams.
impl PasswordAccount for User {
    type User = SignedIn;

    fn name() -> Lens<Self, String> {
        lens!(User.name)
    }

    fn email() -> Lens<Self, String> {
        lens!(User.email)
    }

    fn password_hash() -> Lens<Self, String> {
        lens!(User.password)
    }

    fn email_verified_at() -> Lens<Self, Option<Timestamp>> {
        lens!(User.email_verified_at)
    }

    fn account(user: &SignedIn) -> &Self {
        &user.user
    }

    async fn sign_in(
        self,
        _cx: &Cx,
        ex: &mut dyn toasty::Executor,
    ) -> topcoat::Result<Option<SignedIn>> {
        Ok(Some(Accounts::signed_in(self, ex).await?))
    }

    /// Keeps the language picked from the menu on the account, for every browser and for mail.
    async fn store_locale(cx: &Cx, user: &SignedIn, locale: &Locale) -> topcoat::Result<()> {
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
