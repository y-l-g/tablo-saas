//! Who is on a team, and how that changes.

mod support;

use support::{client_for, harness, person, seat};
use tablo::testing::{TestClient, body_string, form_body};
use tablo_saas::{
    accounts::{Accounts, User},
    teams::{
        IncomingInvitationResource, Invitation, InvitationResource, Member, MemberResource,
        TeamRole,
        actions::{
            AcceptInvitation, ChangeRole, LeaveTeam, RemoveMember, RevokeInvitation,
            TransferOwnership,
        },
    },
};
use topcoat::router::{Body, StatusCode};

const CSRF: &str = "3f0f6d3a-8f43-4f4d-9c59-2d3b3b1f4a11";

/// Posts the invite form the members page opens.
async fn invite(client: &TestClient<'_>, email: &str, role: &str) -> http::Response<Body> {
    client
        .csrf(CSRF)
        .post_form(
            "/app/members/-/actions/invite",
            form_body(&[("email", email), ("role", role), ("csrf_token", CSRF)]),
        )
        .await
}

async fn members_of(harness: &support::Harness, team: &tablo_saas::teams::Team) -> Vec<Member> {
    Member::filter(Member::fields().team_id().eq(team.tenant()))
        .exec(&mut harness.db.clone())
        .await
        .expect("query")
}

async fn invitations(harness: &support::Harness) -> Vec<Invitation> {
    Invitation::all()
        .exec(&mut harness.db.clone())
        .await
        .expect("query")
}

fn role_of(members: &[Member], user: &User) -> Option<TeamRole> {
    members
        .iter()
        .find(|member| member.user_id == user.id)
        .map(|member| member.role)
}

#[tokio::test]
async fn a_sign_up_owns_a_personal_team() {
    let harness = harness().await;
    let client = TestClient::new(&harness.app);
    let response = client
        .submit(
            "/app/register",
            &form_body(&[
                ("name", "Ada"),
                ("email", "ada@example.com"),
                ("password", "correct horse battery staple"),
                ("password_confirmation", "correct horse battery staple"),
            ]),
        )
        .await;
    assert!(response.status().is_redirection(), "{}", response.status());

    let members = Member::all()
        .exec(&mut harness.db.clone())
        .await
        .expect("query");
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].role, TeamRole::Owner);
}

#[tokio::test]
async fn an_invitation_is_mailed_by_a_job_and_accepted_by_the_invited_account() {
    let mut harness = harness().await;
    let (ada, team) = person(&harness.db, "Ada", "ada@example.com").await;
    let (grace, personal) = person(&harness.db, "Grace", "grace@example.com").await;

    let client = client_for(&harness, &ada, &team).await;
    let response = invite(&client, "Grace@Example.com", "admin").await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert!(harness.outbox.sent().is_empty(), "the job sends it");

    harness.run_jobs().await;
    let sent = harness.mail(1).await;
    assert_eq!(sent[0].to()[0].address(), "grace@example.com");
    assert_eq!(sent[0].subject(), format!("Join {}", team.name));

    let invited = client_for(&harness, &grace, &personal).await;
    let incoming = invited.resource::<IncomingInvitationResource>();
    let list = incoming.list().await;
    assert_eq!(list.status(), StatusCode::OK);
    assert_eq!(
        list.rows().len(),
        1,
        "the invitee reads it from their own team"
    );

    let invitation = invitations(&harness).await.remove(0);
    let reply = incoming
        .action::<AcceptInvitation>(&invitation)
        .submit()
        .await;
    assert!(reply.status().is_redirection(), "{}", reply.status());
    assert_eq!(
        role_of(&members_of(&harness, &team).await, &grace),
        Some(TeamRole::Admin)
    );
    assert!(invitations(&harness).await.is_empty());
}

#[tokio::test]
async fn nobody_reads_or_accepts_an_invitation_for_another_address() {
    let harness = harness().await;
    let (ada, team) = person(&harness.db, "Ada", "ada@example.com").await;
    let (mallory, personal) = person(&harness.db, "Mallory", "mallory@example.com").await;
    invite(
        &client_for(&harness, &ada, &team).await,
        "grace@example.com",
        "admin",
    )
    .await;
    let invitation = invitations(&harness).await.remove(0);

    let client = client_for(&harness, &mallory, &personal).await;
    let incoming = client.resource::<IncomingInvitationResource>();
    assert_eq!(incoming.list().await.rows().len(), 0);
    let reply = incoming
        .action::<AcceptInvitation>(&invitation)
        .submit()
        .await;
    assert!(reply.status().is_client_error(), "{}", reply.status());
    assert_eq!(members_of(&harness, &team).await.len(), 1);
}

#[tokio::test]
async fn an_invitation_past_its_date_cannot_be_accepted() {
    let harness = harness().await;
    let (ada, team) = person(&harness.db, "Ada", "ada@example.com").await;
    let (grace, personal) = person(&harness.db, "Grace", "grace@example.com").await;
    invite(
        &client_for(&harness, &ada, &team).await,
        "grace@example.com",
        "member",
    )
    .await;
    let invitation = invitations(&harness).await.remove(0);
    Invitation::filter_by_id(invitation.id)
        .update()
        .expires_at(jiff::Timestamp::UNIX_EPOCH)
        .exec(&mut harness.db.clone())
        .await
        .expect("age it");

    let client = client_for(&harness, &grace, &personal).await;
    let reply = client
        .resource::<IncomingInvitationResource>()
        .action::<AcceptInvitation>(&invitation)
        .submit()
        .await;
    assert!(reply.status().is_client_error(), "{}", reply.status());
    assert_eq!(members_of(&harness, &team).await.len(), 1);
}

#[tokio::test]
async fn inviting_an_address_twice_renews_the_invitation() {
    let harness = harness().await;
    let (ada, team) = person(&harness.db, "Ada", "ada@example.com").await;
    let client = client_for(&harness, &ada, &team).await;
    invite(&client, "grace@example.com", "member").await;
    invite(&client, "grace@example.com", "admin").await;

    let rows = invitations(&harness).await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].role, TeamRole::Admin);
}

#[tokio::test]
async fn an_invitation_refuses_a_current_member_and_an_owner_role() {
    let harness = harness().await;
    let (ada, team) = person(&harness.db, "Ada", "ada@example.com").await;
    let (grace, _) = person(&harness.db, "Grace", "grace@example.com").await;
    seat(&harness.db, &grace, &team, TeamRole::Member).await;
    let client = client_for(&harness, &ada, &team).await;
    invite(&client, "grace@example.com", "member").await;
    invite(&client, "hopper@example.com", "owner").await;
    assert!(invitations(&harness).await.is_empty());
}

#[tokio::test]
async fn a_plain_member_cannot_invite_or_read_the_pending_list() {
    let harness = harness().await;
    let (ada, team) = person(&harness.db, "Ada", "ada@example.com").await;
    let (grace, _) = person(&harness.db, "Grace", "grace@example.com").await;
    seat(&harness.db, &grace, &team, TeamRole::Member).await;
    invite(
        &client_for(&harness, &ada, &team).await,
        "hopper@example.com",
        "member",
    )
    .await;

    let client = client_for(&harness, &grace, &team).await;
    let response = invite(&client, "turing@example.com", "member").await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(invitations(&harness).await.len(), 1);
    let list = client.resource::<InvitationResource>().list().await;
    assert_eq!(list.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn an_admin_revokes_an_invitation() {
    let harness = harness().await;
    let (ada, team) = person(&harness.db, "Ada", "ada@example.com").await;
    let client = client_for(&harness, &ada, &team).await;
    invite(&client, "grace@example.com", "member").await;
    let invitation = invitations(&harness).await.remove(0);
    let reply = client
        .resource::<InvitationResource>()
        .action::<RevokeInvitation>(&invitation)
        .submit()
        .await;
    assert!(reply.status().is_redirection(), "{}", reply.status());
    assert!(invitations(&harness).await.is_empty());
}

#[tokio::test]
async fn the_members_page_shows_the_current_team_alone() {
    let harness = harness().await;
    let (ada, team) = person(&harness.db, "Ada", "ada@example.com").await;
    let (grace, other) = person(&harness.db, "Grace", "grace@example.com").await;

    let client = client_for(&harness, &ada, &team).await;
    let list = client.resource::<MemberResource>().list().await;
    assert_eq!(list.rows().len(), 1);
    assert!(list.html().contains("Ada"));
    assert!(!list.html().contains("Grace"));

    let foreign = members_of(&harness, &other).await.remove(0);
    let reply = client
        .resource::<MemberResource>()
        .action::<RemoveMember>(&foreign)
        .submit()
        .await;
    assert!(reply.status().is_client_error(), "{}", reply.status());
    assert_eq!(
        role_of(&members_of(&harness, &other).await, &grace),
        Some(TeamRole::Owner)
    );
}

#[tokio::test]
async fn the_members_page_names_each_member() {
    let harness = harness().await;
    let (ada, team) = person(&harness.db, "Ada", "ada@example.com").await;
    let (grace, _) = person(&harness.db, "Grace", "grace@example.com").await;
    seat(&harness.db, &grace, &team, TeamRole::Member).await;
    let client = client_for(&harness, &ada, &team).await;
    let list = client.resource::<MemberResource>().list().await;
    assert_eq!(list.rows().len(), 2);
    assert!(list.html().contains("grace@example.com"), "{}", list.html());
}

#[tokio::test]
async fn a_session_cannot_switch_to_a_team_it_is_not_on() {
    let harness = harness().await;
    let (ada, team) = person(&harness.db, "Ada", "ada@example.com").await;
    let (_, other) = person(&harness.db, "Grace", "grace@example.com").await;
    let signed = Accounts::signed_in(ada, &mut harness.db.clone())
        .await
        .expect("load the account");
    let client = TestClient::new(&harness.app)
        .sign_in(&harness.db, "app", &signed)
        .await;

    let response = client
        .submit(
            "/app/tenant",
            &form_body(&[("tenant", &other.id.to_string())]),
        )
        .await;
    assert!(!response.status().is_success(), "{}", response.status());

    let page = body_string(client.get("/app/members").await).await;
    assert!(page.contains("Ada"), "still on {}", team.name);
    assert!(!page.contains("Grace"));
}

#[tokio::test]
async fn an_admin_changes_plain_members_but_not_other_admins() {
    let harness = harness().await;
    let (ada, team) = person(&harness.db, "Ada", "ada@example.com").await;
    let (admin, _) = person(&harness.db, "Admin", "admin@example.com").await;
    let (other, _) = person(&harness.db, "Other", "other@example.com").await;
    let (plain, _) = person(&harness.db, "Plain", "plain@example.com").await;
    seat(&harness.db, &admin, &team, TeamRole::Admin).await;
    let other_seat = seat(&harness.db, &other, &team, TeamRole::Admin).await;
    let plain_seat = seat(&harness.db, &plain, &team, TeamRole::Member).await;

    let client = client_for(&harness, &admin, &team).await;
    let members = client.resource::<MemberResource>();
    let reply = members.action::<RemoveMember>(&other_seat).submit().await;
    assert!(reply.status().is_client_error(), "{}", reply.status());
    let reply = members
        .action::<ChangeRole>(&plain_seat)
        .set_key("role", "admin")
        .submit()
        .await;
    assert!(reply.status().is_redirection(), "{}", reply.status());

    let rows = members_of(&harness, &team).await;
    assert_eq!(role_of(&rows, &other), Some(TeamRole::Admin));
    assert_eq!(role_of(&rows, &plain), Some(TeamRole::Admin));
    assert_eq!(role_of(&rows, &ada), Some(TeamRole::Owner));
}

#[tokio::test]
async fn the_owner_seat_moves_only_by_a_transfer() {
    let harness = harness().await;
    let (ada, team) = person(&harness.db, "Ada", "ada@example.com").await;
    let (grace, _) = person(&harness.db, "Grace", "grace@example.com").await;
    let grace_seat = seat(&harness.db, &grace, &team, TeamRole::Member).await;

    let client = client_for(&harness, &ada, &team).await;
    let members = client.resource::<MemberResource>();
    members
        .action::<ChangeRole>(&grace_seat)
        .set_key("role", "owner")
        .submit()
        .await;
    assert_eq!(
        role_of(&members_of(&harness, &team).await, &grace),
        Some(TeamRole::Member),
        "the role form never names an owner"
    );

    let reply = members
        .action::<TransferOwnership>(&grace_seat)
        .submit()
        .await;
    assert!(reply.status().is_redirection(), "{}", reply.status());
    let rows = members_of(&harness, &team).await;
    assert_eq!(role_of(&rows, &grace), Some(TeamRole::Owner));
    assert_eq!(role_of(&rows, &ada), Some(TeamRole::Admin));
}

#[tokio::test]
async fn nobody_leaves_their_last_team_and_an_owner_transfers_first() {
    let harness = harness().await;
    let (ada, team) = person(&harness.db, "Ada", "ada@example.com").await;
    let (grace, personal) = person(&harness.db, "Grace", "grace@example.com").await;
    let grace_seat = seat(&harness.db, &grace, &team, TeamRole::Member).await;
    let own_seat = members_of(&harness, &personal).await.remove(0);

    let client = client_for(&harness, &grace, &personal).await;
    let reply = client
        .resource::<MemberResource>()
        .action::<LeaveTeam>(&own_seat)
        .submit()
        .await;
    assert!(reply.status().is_client_error(), "{}", reply.status());

    let client = client_for(&harness, &grace, &team).await;
    let reply = client
        .resource::<MemberResource>()
        .action::<LeaveTeam>(&grace_seat)
        .submit()
        .await;
    assert!(reply.status().is_redirection(), "{}", reply.status());
    assert_eq!(role_of(&members_of(&harness, &team).await, &grace), None);
    assert_eq!(
        role_of(&members_of(&harness, &team).await, &ada),
        Some(TeamRole::Owner)
    );
}
