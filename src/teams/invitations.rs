//! Inviting people: the team's outgoing invitations, and the ones addressed to the signed-in user.

use jiff::{SignedDuration, Timestamp};
use tablo::{Notification, Widget, WidgetDef, auth, jobs::enqueue, prelude::*};
use topcoat::{
    context::Cx,
    icon::iconify::iconify_icon,
    view::{View, attributes, view},
};

use super::{
    Invitation, Member, SendInvitation, TeamRole,
    access::{current_entitlements, may_manage_members},
    members::AssignableRole,
};
use crate::accounts::{SignedIn, User, normalize_email};
use crate::billing::{Entitlements, account_of, jobs::sync_seats_later};

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

    fn declare() -> HeaderActionDef {
        HeaderActionDef::new()
            // Staff acting as a customer invite nobody, themselves least of all: an accepted
            // invitation would outlast the session. Tablo's impersonation is read-only, so it is.
            .can_run(may_manage_members)
    }

    async fn run(
        cx: &Cx,
        input: InviteInput,
        ex: &mut dyn toasty::Executor,
    ) -> Result<Outcome, WriteError<InviteInput>> {
        let refuse = |message| Err(WriteError::<InviteInput>::field(|f| f.email, message));
        let membership = auth::membership(cx)
            .ok_or_else(|| WriteError::declined(tablo::text!("error-no-team")))?;
        let signed = auth::require_user::<SignedIn>(cx)?;
        let email = normalize_email(&input.email);
        if !email.contains('@') {
            return refuse(tablo::text!("error-email-invalid"));
        }
        if input.role == TeamRole::Owner {
            return Err(WriteError::<InviteInput>::field(
                |f| f.role,
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
        let members = tablo::db::count_in(
            &mut *ex,
            Member::filter(Member::fields().team_id().eq(membership.tenant)),
        )
        .await?;
        let waiting = tablo::db::count_in(
            &mut *ex,
            Invitation::filter(
                Invitation::fields()
                    .team_id()
                    .eq(membership.tenant)
                    .and(Invitation::fields().expires_at().gt(now))
                    .and(Invitation::fields().email().ne(&email)),
            ),
        )
        .await?;
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
        .await?;
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
        let col = Invitation::columns();
        ResourceDef::new()
            // `invitations` is the signed-in user's own, in the same settings area.
            .slug("pending-invitations")
            .icon(iconify_icon!("lucide:mail"))
            .tenancy(Tenancy::column(lens!(Invitation.team_id)))
            .without_page(ResourcePage::View)
            .without_global_search()
            .policy(
                Abilities::new()
                    .read(may_manage_members)
                    .run::<RevokeInvitation, _>(|cx, _| may_manage_members(cx)),
            )
            .action::<RevokeInvitation>()
            .table(
                Table::new((
                    col.email.text().searchable(),
                    col.role.text().badge(),
                    col.invited_by.text(),
                    col.expires_at.date().sortable(),
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

    fn declare() -> ActionDef<InvitationResource> {
        ActionDef::new().confirm().color(Color::Danger)
    }

    async fn run(
        _cx: &Cx,
        invitations: &[Invitation],
        (): (),
        ex: &mut dyn toasty::Executor,
    ) -> Result<Outcome, WriteError<()>> {
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
        let col = Invitation::columns();
        ResourceDef::new()
            .slug("invitations")
            .label(tablo::text!("resource-incoming-invitation"))
            .plural_label(tablo::text!("resource-incoming-invitation.plural"))
            .icon(iconify_icon!("lucide:user-plus"))
            .tenancy(Tenancy::none())
            .query_fn(|cx| {
                Some(
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
                    col.team_name.text(),
                    col.invited_by.text(),
                    col.role.text().badge(),
                    col.expires_at.date(),
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

    fn declare() -> ActionDef<IncomingInvitationResource> {
        ActionDef::new().places(Places::ROW).color(Color::Success)
    }

    async fn run(
        cx: &Cx,
        invitations: &[Invitation],
        (): (),
        ex: &mut dyn toasty::Executor,
    ) -> Result<Outcome, WriteError<()>> {
        let signed = auth::require_user::<SignedIn>(cx)?;
        for invitation in invitations {
            // The plan may have shrunk since the invitation went out.
            let account = account_of(&mut *ex, invitation.team_id).await?;
            let members = tablo::db::count_in(
                &mut *ex,
                Member::filter(Member::fields().team_id().eq(invitation.team_id)),
            )
            .await?;
            if !Entitlements::of(account.as_ref(), Timestamp::now()).may_add_seat(members) {
                return Err(WriteError::declined(tablo::text!("error-team-full")));
            }
            // An upsert, so a double click joins once.
            Member::upsert_by_team_id_and_user_id(invitation.team_id, signed.user.id)
                .on_create(|create| create.role(invitation.role))
                .or_ignore()
                .exec(&mut *ex)
                .await?;
            sync_seats_later(&mut *ex, invitation.team_id).await?;
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

    fn declare() -> ActionDef<IncomingInvitationResource> {
        ActionDef::new().places(Places::ROW).confirm()
    }

    async fn run(
        _cx: &Cx,
        invitations: &[Invitation],
        (): (),
        ex: &mut dyn toasty::Executor,
    ) -> Result<Outcome, WriteError<()>> {
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
        let waiting = tablo::db::count(
            cx,
            Invitation::filter(
                Invitation::fields()
                    .email()
                    .eq(own_email(cx))
                    .and(Invitation::fields().expires_at().gt(Timestamp::now())),
            ),
        )
        .await?;
        let notice = (waiting > 0)
            .then(|| tablo::t!(cx, "invitations-waiting", count = waiting))
            .zip(tablo::url::resource::<IncomingInvitationResource>(cx));
        Ok(view! {
            if let Some((notice, incoming)) = notice {
                tablo::ui::alert(
                    attrs: attributes! { data-invitations-notice="" },
                    tablo::ui::alert_title(<a href=(incoming) class="underline">(notice)</a>)
                )
            }
        })
    }
}
