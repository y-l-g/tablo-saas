//! The team's members, and what changes them.

use std::convert::Infallible;

use tablo::{auth, prelude::*};
use topcoat::{context::Cx, icon::iconify::iconify_icon};

use super::{
    Member, TeamRole,
    access::{self, current_role, may_manage_members},
    invitations::InviteMember,
};
use crate::accounts::{SignedIn, User};
use crate::billing::jobs::sync_seats_later;

/// Who is on the team the request acts for.
pub struct MemberResource;

fn own_id(cx: &Cx) -> Option<uuid::Uuid> {
    auth::user::<SignedIn>(cx).map(|signed| signed.user.id)
}

impl Resource for MemberResource {
    type Model = Member;
    type Form = NoForm<Member>;

    fn declare() -> ResourceDef<Self> {
        ResourceDef::new()
            .slug("members")
            .icon(iconify_icon!("lucide:users"))
            .tenancy(Tenancy::column(lens!(Member.team_id)))
            .without_page(ResourcePage::View)
            .without_global_search()
            .policy(
                Abilities::new()
                    .view_any(|_cx| true)
                    .view(|_cx, _member| true)
                    .run::<ChangeRole, _>(may_change)
                    .run::<RemoveMember, _>(may_change)
                    .run::<TransferOwnership, _>(|cx, _| current_role(cx) == Some(TeamRole::Owner))
                    .run::<LeaveTeam, _>(|cx, member| own_id(cx) == Some(member.user_id))
                    .run_header::<InviteMember>(may_manage_members),
            )
            .header_action::<InviteMember>()
            .action::<ChangeRole>()
            .action::<TransferOwnership>()
            .action::<RemoveMember>()
            .action::<LeaveTeam>()
            .table(
                Table::new((
                    // No resource administers accounts here: tenancy already limits the rows to
                    // this team, and the column reads nothing else of the account.
                    RelationColumn::unscoped(relation!(Member.user), |user: &User| {
                        format!("{} <{}>", user.name, user.email)
                    }),
                    TextColumn::new(lens!(Member.role)).badge(),
                    DateColumn::new(lens!(Member.created_at)).sortable(),
                ))
                .default_sort(lens!(Member.created_at)),
            )
    }
}

fn may_change(cx: &Cx, member: &Member) -> bool {
    own_id(cx).is_some_and(|own| access::may_change(current_role(cx), member, own))
}

/// What changing a role asks for.
#[derive(FormInput)]
pub struct RoleChange {
    #[form(options = AssignableRole)]
    pub role: TeamRole,
}

/// The roles a person can be given: the owner's seat moves only by a transfer.
pub struct AssignableRole;

impl tablo::Options for AssignableRole {
    fn options() -> Vec<(String, Text)> {
        <TeamRole as tablo::Options>::options()
            .into_iter()
            .filter(|(value, _)| value != "owner")
            .collect()
    }
}

/// Queues the seat sync for the teams `members` sit on.
async fn seats_changed(
    members: &[Member],
    ex: &mut dyn toasty::Executor,
) -> Result<(), WriteError<Infallible>> {
    for member in members {
        sync_seats_later(&mut *ex, member.team_id)
            .await
            .map_err(|error| topcoat::Error::msg(format!("{error:#}")))?;
    }
    Ok(())
}

/// Makes a member an admin, or an admin a plain member.
pub struct ChangeRole;

impl Action<MemberResource> for ChangeRole {
    type Input = RoleChange;
    const NAME: &'static str = "change-role";
    const PLACES: Places = Places::ROW;

    fn can_run(cx: &Cx, member: &Member) -> bool {
        may_change(cx, member)
    }

    async fn run(
        _cx: &Cx,
        members: &[Member],
        change: RoleChange,
        ex: &mut dyn toasty::Executor,
    ) -> Result<Outcome, WriteError<RoleChangeField>> {
        // The form offers no owner, and a hand-built post is refused here.
        if change.role == TeamRole::Owner {
            return Err(WriteError::field(
                RoleChangeField::Role,
                tablo::text!("error-owner-by-transfer"),
            ));
        }
        Member::filter(filter_keys(members))
            .update()
            .role(change.role)
            .exec(ex)
            .await?;
        Ok(Outcome::done())
    }
}

/// Takes a member off the team, ending their access at their next request.
pub struct RemoveMember;

impl Action<MemberResource> for RemoveMember {
    type Input = ();
    const NAME: &'static str = "remove";
    const PLACES: Places = Places::ROW;
    const CONFIRM: bool = true;
    const COLOR: Color = Color::Danger;

    fn can_run(cx: &Cx, member: &Member) -> bool {
        may_change(cx, member)
    }

    async fn run(
        _cx: &Cx,
        members: &[Member],
        (): (),
        ex: &mut dyn toasty::Executor,
    ) -> Result<Outcome, WriteError<Infallible>> {
        Member::filter(filter_keys(members))
            .delete()
            .exec(&mut *ex)
            .await?;
        seats_changed(members, ex).await?;
        Ok(Outcome::done())
    }
}

/// Hands the team to another member, the owner becoming an admin.
pub struct TransferOwnership;

impl Action<MemberResource> for TransferOwnership {
    type Input = ();
    const NAME: &'static str = "transfer-ownership";
    const PLACES: Places = Places::ROW;
    const CONFIRM: bool = true;
    const COLOR: Color = Color::Warning;

    fn can_run(cx: &Cx, member: &Member) -> bool {
        current_role(cx) == Some(TeamRole::Owner) && own_id(cx) != Some(member.user_id)
    }

    async fn run(
        cx: &Cx,
        members: &[Member],
        (): (),
        ex: &mut dyn toasty::Executor,
    ) -> Result<Outcome, WriteError<Infallible>> {
        let [successor] = members else {
            return Err(WriteError::declined(tablo::text!("error-transfer-one")));
        };
        let own = own_id(cx).ok_or_else(|| WriteError::declined(tablo::text!("error-no-user")))?;
        Member::filter(
            Member::fields()
                .team_id()
                .eq(successor.team_id)
                .and(Member::fields().user_id().eq(own)),
        )
        .update()
        .role(TeamRole::Admin)
        .exec(&mut *ex)
        .await?;
        Member::filter_by_id(successor.id)
            .update()
            .role(TeamRole::Owner)
            .exec(ex)
            .await?;
        Ok(Outcome::done())
    }
}

/// Takes the signed-in user off the team. An owner transfers first, and nobody leaves their last
/// team, so a session always has a team to act for.
pub struct LeaveTeam;

impl Action<MemberResource> for LeaveTeam {
    type Input = ();
    const NAME: &'static str = "leave";
    const PLACES: Places = Places::ROW;
    const CONFIRM: bool = true;
    const COLOR: Color = Color::Danger;

    fn can_run(cx: &Cx, member: &Member) -> bool {
        let last_team = auth::user::<SignedIn>(cx).is_none_or(|signed| signed.seats.len() < 2);
        own_id(cx) == Some(member.user_id) && member.role != TeamRole::Owner && !last_team
    }

    async fn run(
        _cx: &Cx,
        members: &[Member],
        (): (),
        ex: &mut dyn toasty::Executor,
    ) -> Result<Outcome, WriteError<Infallible>> {
        Member::filter(filter_keys(members))
            .delete()
            .exec(&mut *ex)
            .await?;
        seats_changed(members, ex).await?;
        Ok(Outcome::done())
    }
}
