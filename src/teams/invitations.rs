//! Inviting people: the team's outgoing invitations, and the ones addressed to the signed-in user.

use std::convert::Infallible;

use jiff::{SignedDuration, Timestamp};
use tablo::{Notification, auth, prelude::*};
use topcoat::{context::Cx, icon::iconify::iconify_icon};

use super::{
    Invitation, Member, SendInvitation, TeamRole, access::may_manage_members,
    members::AssignableRole,
};
use crate::accounts::{SignedIn, User, normalize_email};
use crate::jobs::enqueue;

/// How long an invitation stands.
const LIFETIME: SignedDuration = SignedDuration::from_hours(14 * 24);

/// What inviting someone asks for.
#[derive(FormInput)]
pub struct InviteInput {
    #[form(email)]
    pub email: String,
    #[form(options = AssignableRole)]
    pub role: TeamRole,
}

/// Invites an address to the team: writes the row and queues its mail in one transaction.
pub struct InviteMember;

impl HeaderAction for InviteMember {
    type Input = InviteInput;
    const NAME: &'static str = "invite";

    fn can_run(cx: &Cx) -> bool {
        may_manage_members(cx)
    }

    async fn run(
        cx: &Cx,
        input: InviteInput,
        ex: &mut dyn toasty::Executor,
    ) -> Result<Outcome, WriteError<InviteInputField>> {
        let refuse = |message| Err(WriteError::field(InviteInputField::Email, message));
        let membership = auth::membership(cx)
            .ok_or_else(|| WriteError::declined(tablo::text!("error-no-team")))?;
        let signed = auth::require_user::<SignedIn>(cx)?;
        let email = normalize_email(&input.email);
        if !email.contains('@') {
            return refuse(tablo::text!("error-email-invalid"));
        }
        if input.role == TeamRole::Owner {
            return Err(WriteError::field(
                InviteInputField::Role,
                tablo::text!("error-owner-by-transfer"),
            ));
        }
        let invitee = User::filter(User::fields().email().eq(&email))
            .first()
            .exec(&mut *ex)
            .await?;
        if let Some(invitee) = invitee {
            let on_team = Member::filter(
                Member::fields()
                    .team_id()
                    .eq(membership.tenant)
                    .and(Member::fields().user_id().eq(invitee.id)),
            )
            .first()
            .exec(&mut *ex)
            .await?;
            if on_team.is_some() {
                return refuse(tablo::text!("error-already-member"));
            }
        }
        // Inviting an address again renews the invitation rather than failing on the unique index.
        let invitation = Invitation::upsert_by_team_id_and_email(membership.tenant, &email)
            .team_name(membership.name.clone())
            .role(input.role)
            .invited_by(signed.user.name.clone())
            .expires_at(
                Timestamp::now()
                    .checked_add(LIFETIME)
                    .map_err(|error| WriteError::from(topcoat::Error::msg(error.to_string())))?,
            )
            .exec(&mut *ex)
            .await?;
        enqueue(&SendInvitation {
            invitation: invitation.id,
        })
        .exec(ex)
        .await
        .map_err(|error| topcoat::Error::msg(format!("{error:#}")))?;
        Ok(Outcome::done().notify(Notification::success(tablo::t!(
            cx,
            "flash-invitation-sent",
            email = email
        ))))
    }
}

/// The team's outgoing invitations.
pub struct InvitationResource;

impl Resource for InvitationResource {
    type Model = Invitation;
    type Form = NoForm<Invitation>;

    fn declare() -> ResourceDef<Self> {
        ResourceDef::new()
            .slug("invitations")
            .icon(iconify_icon!("lucide:mail"))
            .navigation_group(tablo::text!("navigation-team"))
            .tenancy(Tenancy::column(lens!(Invitation.team_id)))
            .without_page(ResourcePage::View)
            .without_global_search()
            .policy(
                Abilities::new()
                    .view_any(may_manage_members)
                    .view(|cx, _invitation| may_manage_members(cx))
                    .run::<RevokeInvitation, _>(|cx, _| may_manage_members(cx)),
            )
            .action::<RevokeInvitation>()
            .table(
                Table::new((
                    TextColumn::new(lens!(Invitation.email)).searchable(),
                    TextColumn::new(lens!(Invitation.role)).badge(),
                    TextColumn::new(lens!(Invitation.invited_by)),
                    DateColumn::new(lens!(Invitation.expires_at)).sortable(),
                ))
                .default_sort_desc(lens!(Invitation.created_at)),
            )
    }
}

/// Withdraws an invitation.
pub struct RevokeInvitation;

impl Action<InvitationResource> for RevokeInvitation {
    type Input = ();
    const NAME: &'static str = "revoke";
    const CONFIRM: bool = true;
    const COLOR: Color = Color::Danger;

    async fn run(
        _cx: &Cx,
        invitations: &[Invitation],
        (): (),
        ex: &mut dyn toasty::Executor,
    ) -> Result<Outcome, WriteError<Infallible>> {
        Invitation::filter(filter_keys(invitations))
            .delete()
            .exec(ex)
            .await?;
        Ok(Outcome::done())
    }
}

/// The invitations addressed to the signed-in user, from any team. The panel verified the
/// address before serving this page, so reading it here proves the invitee is who was asked.
pub struct IncomingInvitationResource;

fn own_email(cx: &Cx) -> String {
    auth::user::<SignedIn>(cx).map_or_else(String::new, |signed| signed.user.email.clone())
}

impl Resource for IncomingInvitationResource {
    type Model = Invitation;
    type Form = NoForm<Invitation>;

    fn declare() -> ResourceDef<Self> {
        ResourceDef::new()
            .slug("incoming-invitations")
            .label(tablo::text!("resource-incoming-invitation"))
            .plural_label(tablo::text!("resource-incoming-invitation.plural"))
            .icon(iconify_icon!("lucide:user-plus"))
            .tenancy(Tenancy::none())
            .query(|cx, query| {
                query.filter(
                    Invitation::fields()
                        .email()
                        .eq(own_email(cx))
                        .and(Invitation::fields().expires_at().gt(Timestamp::now())),
                )
            })
            .without_page(ResourcePage::View)
            .without_global_search()
            .policy(
                Abilities::<Invitation>::new()
                    .view_any(|_cx| true)
                    .view(|cx, invitation| invitation.email == own_email(cx))
                    .run::<AcceptInvitation, _>(|cx, invitation| invitation.email == own_email(cx))
                    .run::<DeclineInvitation, _>(|cx, invitation| {
                        invitation.email == own_email(cx)
                    }),
            )
            .action::<AcceptInvitation>()
            .action::<DeclineInvitation>()
            .table(
                Table::new((
                    TextColumn::new(lens!(Invitation.team_name)),
                    TextColumn::new(lens!(Invitation.invited_by)),
                    TextColumn::new(lens!(Invitation.role)).badge(),
                    DateColumn::new(lens!(Invitation.expires_at)),
                ))
                .default_sort_desc(lens!(Invitation.created_at)),
            )
    }
}

/// Joins the team.
pub struct AcceptInvitation;

impl Action<IncomingInvitationResource> for AcceptInvitation {
    type Input = ();
    const NAME: &'static str = "accept";
    const PLACES: Places = Places::ROW;
    const COLOR: Color = Color::Success;

    async fn run(
        cx: &Cx,
        invitations: &[Invitation],
        (): (),
        ex: &mut dyn toasty::Executor,
    ) -> Result<Outcome, WriteError<Infallible>> {
        let signed = auth::require_user::<SignedIn>(cx)?;
        for invitation in invitations {
            // An upsert, so a double click joins once.
            Member::upsert_by_team_id_and_user_id(invitation.team_id, signed.user.id)
                .on_create(|create| create.role(invitation.role))
                .or_ignore()
                .exec(&mut *ex)
                .await?;
        }
        Invitation::filter(filter_keys(invitations))
            .delete()
            .exec(ex)
            .await?;
        Ok(Outcome::done())
    }
}

/// Turns the invitation down.
pub struct DeclineInvitation;

impl Action<IncomingInvitationResource> for DeclineInvitation {
    type Input = ();
    const NAME: &'static str = "decline";
    const PLACES: Places = Places::ROW;
    const CONFIRM: bool = true;

    async fn run(
        _cx: &Cx,
        invitations: &[Invitation],
        (): (),
        ex: &mut dyn toasty::Executor,
    ) -> Result<Outcome, WriteError<Infallible>> {
        Invitation::filter(filter_keys(invitations))
            .delete()
            .exec(ex)
            .await?;
        Ok(Outcome::done())
    }
}
