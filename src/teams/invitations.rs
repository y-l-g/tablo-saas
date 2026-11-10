//! Inviting people: the team's outgoing invitations, and the ones addressed to the signed-in user.

use std::convert::Infallible;

use jiff::{SignedDuration, Timestamp};
use tablo::{Notification, Widget, WidgetDef, auth, db::db, prelude::*};
use topcoat::{
    context::Cx,
    icon::iconify::iconify_icon,
    view::{View, view},
};

use super::{
    Invitation, Member, SendInvitation, TeamRole,
    access::{current_entitlements, may_manage_members},
    members::AssignableRole,
};
use crate::accounts::{SignedIn, User, is_impersonated, normalize_email};
use crate::billing::{Entitlements, account_of, jobs::sync_seats_later};
use crate::jobs::enqueue;

/// Where the signed-in user answers the invitations addressed to them: the account group of the
/// settings area. A mail names it outside any request, so it is spelled out rather than resolved.
pub const INCOMING_URL: &str = "/app/settings/invitations";

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

    /// Staff acting as a customer invite nobody, themselves least of all: an accepted invitation
    /// would outlast the session.
    fn can_run(cx: &Cx) -> bool {
        may_manage_members(cx) && !is_impersonated(cx)
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
        let now = Timestamp::now();
        let members = Member::filter(Member::fields().team_id().eq(membership.tenant))
            .exec(&mut *ex)
            .await?
            .len();
        let waiting = Invitation::filter(
            Invitation::fields()
                .team_id()
                .eq(membership.tenant)
                .and(Invitation::fields().expires_at().gt(now))
                .and(Invitation::fields().email().ne(&email)),
        )
        .exec(&mut *ex)
        .await?
        .len();
        if !current_entitlements(cx).may_add_seat(members + waiting) {
            return refuse(tablo::text!("error-seat-limit"));
        }
        // Inviting an address again renews the invitation rather than failing on the unique index.
        let invitation = Invitation::upsert_by_team_id_and_email(membership.tenant, &email)
            .team_name(membership.name.clone())
            .role(input.role)
            .invited_by(signed.user.name.clone())
            .expires_at(
                now.checked_add(LIFETIME)
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
            // `invitations` is the signed-in user's own, in the same settings area.
            .slug("pending-invitations")
            .icon(iconify_icon!("lucide:mail"))
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

    fn can_run(cx: &Cx, _invitation: &Invitation) -> bool {
        !is_impersonated(cx)
    }

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
            .slug("invitations")
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

    fn can_run(cx: &Cx, _invitation: &Invitation) -> bool {
        !is_impersonated(cx)
    }

    async fn run(
        cx: &Cx,
        invitations: &[Invitation],
        (): (),
        ex: &mut dyn toasty::Executor,
    ) -> Result<Outcome, WriteError<Infallible>> {
        let signed = auth::require_user::<SignedIn>(cx)?;
        let as_error = |error: anyhow::Error| topcoat::Error::msg(format!("{error:#}"));
        for invitation in invitations {
            // The plan may have shrunk since the invitation went out.
            let account = account_of(&mut *ex, invitation.team_id)
                .await
                .map_err(as_error)?;
            let members = Member::filter(Member::fields().team_id().eq(invitation.team_id))
                .exec(&mut *ex)
                .await?
                .len();
            if !Entitlements::of(account.as_ref(), Timestamp::now()).may_add_seat(members) {
                return Err(WriteError::declined(tablo::text!("error-team-full")));
            }
            // An upsert, so a double click joins once.
            Member::upsert_by_team_id_and_user_id(invitation.team_id, signed.user.id)
                .on_create(|create| create.role(invitation.role))
                .or_ignore()
                .exec(&mut *ex)
                .await?;
            sync_seats_later(&mut *ex, invitation.team_id)
                .await
                .map_err(as_error)?;
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

    fn can_run(cx: &Cx, _invitation: &Invitation) -> bool {
        !is_impersonated(cx)
    }

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

/// A word on the home page when invitations wait for the signed-in user, since the settings area
/// that holds them is out of the sidebar. It renders nothing when none do.
pub struct IncomingInvitationsWidget;

impl Widget for IncomingInvitationsWidget {
    fn declare() -> WidgetDef {
        WidgetDef::new().full_width()
    }

    async fn render(cx: &Cx) -> topcoat::Result<impl View> {
        let waiting = Invitation::filter(
            Invitation::fields()
                .email()
                .eq(own_email(cx))
                .and(Invitation::fields().expires_at().gt(Timestamp::now())),
        )
        .exec(&mut db(cx)?)
        .await?
        .len();
        let notice = (waiting > 0).then(|| tablo::t!(cx, "invitations-waiting", count = waiting));
        Ok(view! {
            if let Some(notice) = notice {
                <p data-invitations-notice="" class="rounded-lg border border-border bg-muted p-3 text-sm">
                    <a href=(INCOMING_URL) class="underline">(notice)</a>
                </p>
            }
        })
    }
}
