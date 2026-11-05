//! What an account changes about itself: its details, its password and its address.

use jiff::Timestamp;
use tablo::{
    FieldErrors, Profile, ProfileEditor, ProfileUpdate,
    auth::{EmailChange, hash_password, request_email_change, verify_password},
    db::db,
};
use topcoat::context::Cx;

use super::{Accounts, SignedIn, User, normalize_email};

/// The id `PanelUser::user_id` spelled.
fn account_id(user_id: &str) -> topcoat::Result<uuid::Uuid> {
    user_id
        .parse()
        .map_err(|_| topcoat::Error::msg("the account's id is not a uuid"))
}

/// Resets a password, for the "Forgot your password?" page.
pub struct AccountResetter;

impl tablo::auth::PasswordResetter for AccountResetter {
    type User = SignedIn;

    async fn find_by_email(&self, cx: &Cx, email: &str) -> topcoat::Result<Option<SignedIn>> {
        let mut handle = db(cx)?;
        match Accounts::by_email(&mut handle, email).await? {
            Some(user) => Ok(Some(Accounts::signed_in(user, &mut handle).await?)),
            None => Ok(None),
        }
    }

    async fn reset_password(
        &self,
        _cx: &Cx,
        user_id: &str,
        password: &str,
        ex: &mut dyn toasty::Executor,
    ) -> topcoat::Result<()> {
        User::filter_by_id(account_id(user_id)?)
            .update()
            .password(hash_password(password).await?)
            .exec(ex)
            .await?;
        Ok(())
    }
}

/// Records that an account proved it reads its address.
pub struct AccountVerifier;

impl tablo::auth::EmailVerifier for AccountVerifier {
    type User = SignedIn;

    fn is_verified(&self, user: &SignedIn) -> bool {
        user.user.email_verified_at.is_some()
    }

    async fn store_verified_email(
        &self,
        _cx: &Cx,
        user_id: &str,
        email: &str,
        ex: &mut dyn toasty::Executor,
    ) -> topcoat::Result<()> {
        User::filter_by_id(account_id(user_id)?)
            .update()
            .email(normalize_email(email))
            .email_verified_at(Some(Timestamp::now()))
            .exec(ex)
            .await?;
        Ok(())
    }
}

/// Edits the signed-in account, asking for the current password before the address or the
/// password changes.
impl ProfileEditor for Accounts {
    type Input = Profile;
    type User = SignedIn;

    fn fill(&self, _cx: &Cx, signed: &SignedIn) -> Profile {
        Profile {
            name: signed.user.name.clone(),
            email: signed.user.email.clone(),
            password: None,
            password_confirmation: String::new(),
            current_password: None,
        }
    }

    async fn validate(
        &self,
        _cx: &Cx,
        signed: &SignedIn,
        profile: &Profile,
        ex: &mut dyn toasty::Executor,
    ) -> topcoat::Result<FieldErrors> {
        let user = &signed.user;
        let mut errors = profile.password_errors();
        let email = normalize_email(&profile.email);
        let new_email = email != user.email;
        if new_email || profile.password.is_some() {
            match &profile.current_password {
                None => errors.add(
                    "current_password",
                    tablo::text!("tablo-profile-current-password-needed"),
                ),
                Some(current) if !verify_password(current, Some(&user.password)).await => {
                    errors.add(
                        "current_password",
                        tablo::text!("tablo-profile-current-password-wrong"),
                    );
                }
                Some(_) => {}
            }
        }
        // Only the account's owner learns whether another account holds an address: a stolen
        // session without the password must not probe them.
        if new_email
            && !errors.contains_key("current_password")
            && Accounts::by_email(ex, &email)
                .await?
                .is_some_and(|other| other.id != user.id)
        {
            errors.add("email", tablo::text!("error-email-taken"));
        }
        Ok(errors)
    }

    async fn save(
        &self,
        cx: &Cx,
        signed: &SignedIn,
        profile: Profile,
        ex: &mut dyn toasty::Executor,
    ) -> topcoat::Result<ProfileUpdate> {
        let email = normalize_email(&profile.email);
        // A new address waits for the link mailed to it, and the old one gets a link keeping it.
        let stored = email != signed.user.email
            && request_email_change(cx, &mut *ex, signed, &email).await? == EmailChange::Immediate;
        let mut update = User::filter_by_id(signed.user.id)
            .update()
            .name(profile.name.trim().to_string());
        if stored {
            update.set_email(email);
            update.set_email_verified_at(None);
        }
        if let Some(password) = &profile.password {
            update.set_password(hash_password(password).await?);
        }
        update.exec(ex).await?;
        Ok(if stored || profile.password.is_some() {
            ProfileUpdate::Credentials
        } else {
            ProfileUpdate::Details
        })
    }
}
