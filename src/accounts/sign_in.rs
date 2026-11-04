//! Signing in and signing up.

use tablo::{
    Authenticator, FieldErrors, PanelUser, Registrar,
    auth::{hash_password, new_password_errors, verify_password},
    db::db,
};
use topcoat::context::Cx;

use super::{User, normalize_email};

/// The signed-in user.
#[derive(Debug)]
pub struct SignedIn {
    pub user: User,
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
        matches!(panel, "/app")
    }
}

/// Reads users and creates them from the sign-up page.
pub struct Accounts;

impl Accounts {
    /// `user` as a session sees them.
    ///
    /// Runs on `ex` rather than a fresh handle: sign-up calls this while its transaction holds a
    /// connection, and a single-connection pool would wait forever for a second one.
    pub(super) async fn signed_in(
        user: User,
        _ex: &mut dyn toasty::Executor,
    ) -> topcoat::Result<SignedIn> {
        Ok(SignedIn { user })
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
        _cx: &Cx,
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
        Self::signed_in(user, ex).await
    }
}
