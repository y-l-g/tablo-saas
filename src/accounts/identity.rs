//! Signing in with Google: which account each Google identity is, linked on first sign-in.

use jiff::Timestamp;
use tablo::{
    IdentityLinker, ProviderIdentity,
    auth::{AuthFactor, AuthSession, AuthToken, hash_password},
};
use topcoat::context::Cx;

use super::{Accounts, SignedIn, User, normalize_email};
use crate::teams::create_team;

/// An account at a provider that signs in as a user. The issuer and subject name it there for
/// good, where its address can change hands.
#[derive(Debug, Clone, toasty::Model)]
#[unique(issuer, subject)]
pub struct Identity {
    #[key]
    #[auto]
    pub id: uuid::Uuid,
    #[index]
    pub user_id: uuid::Uuid,
    /// The provider, such as `https://accounts.google.com`.
    pub issuer: String,
    /// The provider's id for the account, unique with `issuer`.
    pub subject: String,
    #[default(Timestamp::now())]
    pub created_at: Timestamp,
}

impl IdentityLinker for Accounts {
    type User = SignedIn;

    /// A linked identity signs in as its user. A new one links to the user holding the address
    /// the provider verified, or signs up as a new user with a personal team. An address the
    /// provider has not verified signs nobody in: whoever registered it there could be anyone.
    async fn sign_in(
        &self,
        cx: &Cx,
        identity: &ProviderIdentity,
        ex: &mut dyn toasty::Executor,
    ) -> topcoat::Result<Option<SignedIn>> {
        let linked = Identity::filter(
            Identity::fields()
                .issuer()
                .eq(identity.issuer.clone())
                .and(Identity::fields().subject().eq(identity.subject.clone())),
        )
        .first()
        .exec(&mut *ex)
        .await?;
        if let Some(linked) = linked {
            return match User::filter_by_id(linked.user_id)
                .first()
                .exec(&mut *ex)
                .await?
            {
                Some(user) => Ok(Some(Self::signed_in(user, ex).await?)),
                None => Ok(None),
            };
        }
        let (Some(email), true) = (&identity.email, identity.email_verified) else {
            return Ok(None);
        };
        let user = match Self::by_email(&mut *ex, email).await? {
            Some(user) if user.email_verified_at.is_some() => user,
            Some(user) => claim_unverified(ex, user).await?,
            None => sign_up(cx, ex, identity, email).await?,
        };
        toasty::create!(Identity {
            user_id: user.id,
            issuer: identity.issuer.clone(),
            subject: identity.subject.clone(),
        })
        .exec(&mut *ex)
        .await?;
        Ok(Some(Self::signed_in(user, ex).await?))
    }
}

/// A password nobody knows: an account made through a provider signs in through it, or sets a
/// password with a reset link.
async fn unknown_password() -> topcoat::Result<String> {
    hash_password(&uuid::Uuid::new_v4().to_string()).await
}

/// Hands `user`, whose address nobody proved, to whoever the provider says holds it.
///
/// Anyone can sign up with an address they do not read and wait for its owner, so what the
/// unproved account holds is the squatter's: its password, its sessions, its pending links and
/// its second factors all go, and the address counts as proved.
async fn claim_unverified(ex: &mut dyn toasty::Executor, user: User) -> topcoat::Result<User> {
    let id = user.id.to_string();
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
    let now = Timestamp::now();
    User::filter_by_id(user.id)
        .update()
        .password(unknown_password().await?)
        .email_verified_at(Some(now))
        .exec(&mut *ex)
        .await?;
    Ok(User {
        email_verified_at: Some(now),
        ..user
    })
}

/// A new user for `identity`, whose provider verified `email`, with their personal team.
async fn sign_up(
    cx: &Cx,
    ex: &mut dyn toasty::Executor,
    identity: &ProviderIdentity,
    email: &str,
) -> topcoat::Result<User> {
    let email = normalize_email(email);
    let name = identity
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map_or_else(
            || email.split('@').next().unwrap_or(&email).to_string(),
            str::to_string,
        );
    let user = toasty::create!(User {
        name,
        email,
        email_verified_at: Some(Timestamp::now()),
        password: unknown_password().await?,
    })
    .exec(&mut *ex)
    .await?;
    create_team(
        &mut *ex,
        &tablo::t!(cx, "team-personal", name = &user.name),
        &user,
    )
    .await?;
    Ok(user)
}
