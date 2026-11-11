//! Staff acting as a customer from the app panel's support list, and the back office's log of it.

mod support;

use jiff::{SignedDuration, Timestamp};
use support::{
    Harness, billed_harness, client_for, harness, person, seat, staff_client, stripe::FakeStripe,
};
use tablo::{
    AuditEntry,
    auth::{AuthFactor, FactorKind, IMPERSONATE_FIELD},
    testing::{
        FieldOf, Reply, TestClient, action_label, body_string, form_body, impersonation_banner,
    },
};
use tablo_saas::{
    accounts::{Accounts, StaffRole, User},
    audit,
    backoffice::ImpersonationResource,
    billing::{BillingAccount, BillingStatus, PlanId},
    support::{ActAs, SupportResource},
    teams::{
        IncomingInvitationResource, Invitation, Member, MemberResource, TeamRole,
        actions::{AcceptInvitation, RemoveMember, TransferOwnership},
    },
};
use topcoat::router::{StatusCode, header::LOCATION};

const CSRF: &str = "3f0f6d3a-8f43-4f4d-9c59-2d3b3b1f4a11";

/// When a factor was confirmed, relative to the `staff` command's grant.
#[derive(Clone, Copy, PartialEq)]
enum Factor {
    None,
    BeforeGrant,
    AfterGrant,
}

/// A member of staff signed in to the app panel, with a second factor there as `factor` says.
async fn staff_in_app<'a>(
    harness: &'a Harness,
    email: &str,
    role: StaffRole,
    factor: Factor,
) -> (User, TestClient<'a>) {
    let (user, _) = person(&harness.db, "Sam Support", email).await;
    let mut db = harness.db.clone();
    let granted = Timestamp::now();
    User::filter_by_id(user.id)
        .update()
        .staff_role(Some(role))
        .staff_since(Some(granted))
        .exec(&mut db)
        .await
        .expect("make staff");
    let minute = SignedDuration::from_mins(1);
    let confirmed = match factor {
        Factor::None => None,
        Factor::BeforeGrant => Some(granted - minute),
        Factor::AfterGrant => Some(granted + minute),
    };
    if let Some(confirmed) = confirmed {
        toasty::create!(AuthFactor {
            user_id: user.id.to_string(),
            panel: "/app".to_string(),
            kind: FactorKind::EmailCode,
            secret: String::new(),
            confirmed_at: Some(confirmed),
            last_step: None,
            failures: 0,
            retry_at: None,
            created_at: confirmed,
        })
        .exec(&mut db)
        .await
        .expect("write the factor");
    }
    let user = User::get_by_id(&mut db, user.id).await.expect("query");
    let signed = Accounts::signed_in(user.clone(), &mut db)
        .await
        .expect("load the account");
    let client = TestClient::new(&harness.app)
        .panel(&harness.panel)
        .sign_in(&harness.db, "app", &signed)
        .await;
    (user, client)
}

fn act_as(user: &User) -> String {
    form_body(&[(IMPERSONATE_FIELD, &user.id.to_string())])
}

async fn search<'c, 'a>(
    client: &'c TestClient<'a>,
    term: &str,
) -> Reply<'c, 'a, FieldOf<SupportResource>> {
    client
        .resource::<SupportResource>()
        .list_with(&format!("q={term}"))
        .await
}

/// Whether `list` holds a row reading `email`.
fn lists(list: &Reply<'_, '_, FieldOf<SupportResource>>, email: &str) -> bool {
    list.rows()
        .iter()
        .any(|row| row.cells.iter().any(|cell| cell == email))
}

/// Whether `list` offers to act as `user`.
fn offers_to_act_as(
    client: &TestClient<'_>,
    list: &Reply<'_, '_, FieldOf<SupportResource>>,
    user: &User,
) -> bool {
    let url = client
        .resource::<SupportResource>()
        .action::<ActAs>(user)
        .url()
        .to_string();
    action_label(list.html(), &url).is_some()
}

async fn banner(client: &TestClient<'_>) -> Option<tablo::testing::ImpersonationBanner> {
    let page = client.get("/app").await;
    assert_eq!(page.status(), StatusCode::OK);
    impersonation_banner(&body_string(page).await)
}

#[tokio::test]
async fn staff_find_a_customer_and_act_as_them_until_stop() {
    let harness = harness().await;
    let (ada, _) = person(&harness.db, "Ada Lovelace", "ada@example.com").await;
    person(&harness.db, "Grace Hopper", "grace@example.com").await;
    let (_, client) = staff_in_app(
        &harness,
        "sam@example.com",
        StaffRole::Support,
        Factor::BeforeGrant,
    )
    .await;

    let found = search(&client, "ada").await;
    assert_eq!(found.status(), StatusCode::OK);
    assert!(lists(&found, "ada@example.com"));
    assert!(!lists(&found, "grace@example.com"));
    assert!(offers_to_act_as(&client, &found, &ada));
    assert!(
        !found.html().contains("data-support-notice"),
        "{}",
        found.html()
    );
    let found = search(&client, "LOVELACE").await;
    assert!(
        lists(&found, "ada@example.com"),
        "a surname in no address, in another case, finds her on PostgreSQL too"
    );

    let started = client
        .resource::<SupportResource>()
        .action::<ActAs>(&ada)
        .submit()
        .await;
    assert_eq!(started.status(), StatusCode::SEE_OTHER);
    assert_eq!(started.location(), Some("/app"));
    let shown = banner(&client).await.expect("the banner shows");
    assert_eq!(shown.user_id, ada.id.to_string());
    assert!(shown.text.contains("Ada Lovelace"), "{}", shown.text);
    assert_eq!(
        client.resource::<SupportResource>().list().await.status(),
        StatusCode::FORBIDDEN,
        "acting as a customer, the list is the customer's to see, and they may not"
    );

    let stopped = client.submit("/app/impersonate/stop", "").await;
    assert_eq!(stopped.status(), StatusCode::SEE_OTHER);
    assert!(banner(&client).await.is_none());

    let mut events: Vec<String> = AuditEntry::all()
        .exec(&mut harness.db.clone())
        .await
        .expect("query")
        .into_iter()
        .filter(|entry| entry.panel == "/app")
        .map(|entry| entry.event)
        .collect();
    events.sort_unstable();
    assert_eq!(
        events,
        ["action:act-as", "impersonate", "stop_impersonating"],
        "the button's run, then what Tablo records of acting as her"
    );
}

/// Signing out while acting as a customer ends it, and the log says so: a start with no stop
/// is left only by a session that expired.
#[tokio::test]
async fn signing_out_while_acting_as_a_customer_logs_a_stop() {
    let harness = harness().await;
    let (ada, _) = person(&harness.db, "Ada Lovelace", "ada@example.com").await;
    let (_, client) = staff_in_app(
        &harness,
        "sam@example.com",
        StaffRole::Support,
        Factor::BeforeGrant,
    )
    .await;
    client.submit("/app/impersonate", &act_as(&ada)).await;
    let out = client.submit("/app/logout", "").await;
    assert_eq!(out.status(), StatusCode::SEE_OTHER);

    let mut events: Vec<String> = AuditEntry::all()
        .exec(&mut harness.db.clone())
        .await
        .expect("query")
        .into_iter()
        .filter(|entry| entry.panel == "/app")
        .map(|entry| entry.event)
        .collect();
    events.sort_unstable();
    assert_eq!(events, ["impersonate", "stop_impersonating"]);
}

#[tokio::test]
async fn an_administrator_reads_who_acted_as_whom() {
    let harness = harness().await;
    let (ada, _) = person(&harness.db, "Ada Lovelace", "ada@example.com").await;
    let (_, client) = staff_in_app(
        &harness,
        "sam@example.com",
        StaffRole::Support,
        Factor::BeforeGrant,
    )
    .await;
    client.submit("/app/impersonate", &act_as(&ada)).await;
    client.submit("/app/impersonate/stop", "").await;

    let admin = staff_client(&harness, "root@example.com", StaffRole::Administrator).await;
    let rows = admin
        .resource::<ImpersonationResource>()
        .list()
        .await
        .rows();
    assert_eq!(rows.len(), 2);
    for row in &rows {
        let text = row.cells.join(" | ");
        assert!(text.contains("Sam Support"), "{text}");
        assert!(text.contains("Ada Lovelace"), "{text}");
        assert!(text.contains(&ada.id.to_string()), "{text}");
    }
    let events: Vec<String> = rows.iter().map(|row| row.cells.join(" | ")).collect();
    assert!(
        events
            .iter()
            .any(|text| text.contains("Started") && text.contains(" UTC")),
        "a start says when it ends at the latest: {events:?}"
    );
    assert!(
        events
            .iter()
            .any(|text| text.contains("Stopped") && !text.contains(" UTC")),
        "{events:?}"
    );

    let support = staff_client(&harness, "help@example.com", StaffRole::Support).await;
    assert_eq!(
        support
            .resource::<ImpersonationResource>()
            .list()
            .await
            .status(),
        StatusCode::FORBIDDEN,
        "the log is for administrators, as the staff trail is"
    );
}

/// The app panel requires a factor of staff, whom customers' data is open to, so one without is
/// sent to set it up before anything else, acting as a customer included.
#[tokio::test]
async fn staff_without_a_factor_on_the_app_panel_are_sent_to_set_one_up() {
    let harness = harness().await;
    let (ada, _) = person(&harness.db, "Ada Lovelace", "ada@example.com").await;
    let (_, client) = staff_in_app(
        &harness,
        "sam@example.com",
        StaffRole::Administrator,
        Factor::None,
    )
    .await;

    let page = client.get("/app/support?q=ada").await;
    assert_eq!(page.status(), StatusCode::TEMPORARY_REDIRECT);
    let setup = page.headers()[LOCATION].to_str().expect("ascii");
    assert!(setup.contains("two-factor"), "{setup}");

    let refused = client.submit("/app/impersonate", &act_as(&ada)).await;
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn staff_never_act_as_staff() {
    let harness = harness().await;
    let (other, _) = staff_in_app(
        &harness,
        "root@example.com",
        StaffRole::Administrator,
        Factor::BeforeGrant,
    )
    .await;
    let (_, client) = staff_in_app(
        &harness,
        "sam@example.com",
        StaffRole::Support,
        Factor::BeforeGrant,
    )
    .await;

    let all = client.resource::<SupportResource>().list().await;
    assert_eq!(all.status(), StatusCode::OK);
    assert!(
        !lists(&all, "root@example.com") && !lists(&all, "sam@example.com"),
        "the list holds customers only"
    );
    let refused = client
        .resource::<SupportResource>()
        .action::<ActAs>(&other)
        .submit()
        .await;
    assert_eq!(
        refused.status(),
        StatusCode::NOT_FOUND,
        "staff are outside the list's query, so no action loads one"
    );
    let refused = client.submit("/app/impersonate", &act_as(&other)).await;
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);
    assert!(banner(&client).await.is_none());
}

#[tokio::test]
async fn a_customer_neither_sees_the_page_nor_acts_as_anyone() {
    let harness = harness().await;
    let (ada, team) = person(&harness.db, "Ada Lovelace", "ada@example.com").await;
    let (grace, _) = person(&harness.db, "Grace Hopper", "grace@example.com").await;
    let client = client_for(&harness, &ada, &team).await;

    assert_eq!(
        client.resource::<SupportResource>().list().await.status(),
        StatusCode::FORBIDDEN
    );
    let refused = client
        .resource::<SupportResource>()
        .action::<ActAs>(&grace)
        .submit()
        .await;
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);
    let refused = client.submit("/app/impersonate", &act_as(&grace)).await;
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn billing_stays_the_customers_while_staff_act_as_them() {
    let harness = harness().await;
    let (ada, _) = person(&harness.db, "Ada Lovelace", "ada@example.com").await;
    let (_, client) = staff_in_app(
        &harness,
        "sam@example.com",
        StaffRole::Support,
        Factor::BeforeGrant,
    )
    .await;
    client.submit("/app/impersonate", &act_as(&ada)).await;

    assert_eq!(
        client.get("/app/settings/billing").await.status(),
        StatusCode::OK,
        "the page reads Ada's team, which she owns"
    );
    let checkout = client
        .submit(
            "/app/settings/billing/-/actions/checkout",
            &form_body(&[("plan", "pro"), ("interval", "month")]),
        )
        .await;
    assert_eq!(checkout.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        client.get("/app/settings/profile").await.status(),
        StatusCode::FORBIDDEN,
        "nor are her credentials staff's to change"
    );
}

#[tokio::test]
async fn a_factor_added_after_the_grant_is_not_trusted() {
    let harness = harness().await;
    let (ada, _) = person(&harness.db, "Ada Lovelace", "ada@example.com").await;
    let (_, client) = staff_in_app(
        &harness,
        "sam@example.com",
        StaffRole::Support,
        Factor::AfterGrant,
    )
    .await;

    let found = search(&client, "ada").await;
    assert!(
        found.html().contains("data-support-notice"),
        "{}",
        found.html()
    );
    assert!(lists(&found, "ada@example.com"));
    assert!(!offers_to_act_as(&client, &found, &ada));
    let refused = client.submit("/app/impersonate", &act_as(&ada)).await;
    assert_eq!(
        refused.status(),
        StatusCode::FORBIDDEN,
        "whoever held the password then may have enrolled it"
    );
}

#[tokio::test]
async fn who_a_team_includes_stays_the_customers_call() {
    let harness = harness().await;
    let (ada, team) = person(&harness.db, "Ada Lovelace", "ada@example.com").await;
    let (grace, other) = person(&harness.db, "Grace Hopper", "grace@example.com").await;
    let grace_seat = seat(&harness.db, &grace, &team, TeamRole::Member).await;
    toasty::create!(Invitation {
        team_id: other.tenant(),
        team_name: other.name.clone(),
        email: "ada@example.com".to_string(),
        role: TeamRole::Member,
        invited_by: "Grace Hopper".to_string(),
        expires_at: Timestamp::now() + SignedDuration::from_hours(24),
    })
    .exec(&mut harness.db.clone())
    .await
    .expect("invite Ada");
    let (_, client) = staff_in_app(
        &harness,
        "sam@example.com",
        StaffRole::Support,
        Factor::BeforeGrant,
    )
    .await;
    client.submit("/app/impersonate", &act_as(&ada)).await;
    assert!(banner(&client).await.is_some());

    client
        .csrf(CSRF)
        .post_form(
            "/app/settings/members/-/actions/invite",
            form_body(&[
                ("email", "sam@example.com"),
                ("role", "admin"),
                ("csrf_token", CSRF),
            ]),
        )
        .await;
    let members = client.resource::<MemberResource>();
    members.action::<RemoveMember>(&grace_seat).submit().await;
    members
        .action::<TransferOwnership>(&grace_seat)
        .submit()
        .await;
    let waiting = Invitation::all()
        .exec(&mut harness.db.clone())
        .await
        .expect("query");
    let incoming = waiting
        .iter()
        .find(|invitation| invitation.email == "ada@example.com")
        .expect("Ada's invitation");
    client
        .resource::<IncomingInvitationResource>()
        .action::<AcceptInvitation>(incoming)
        .submit()
        .await;

    let invited: Vec<&str> = waiting.iter().map(|i| i.email.as_str()).collect();
    assert_eq!(invited, vec!["ada@example.com"], "staff invited nobody");
    let grace_now = Member::get_by_id(&mut harness.db.clone(), grace_seat.id)
        .await
        .expect("Grace is still on the team");
    assert_eq!(grace_now.role, TeamRole::Member, "and owns nothing");
    let ada_seats = Member::filter(Member::fields().user_id().eq(ada.id))
        .exec(&mut harness.db.clone())
        .await
        .expect("query");
    assert_eq!(ada_seats.len(), 1, "Ada joined no team");
    assert_eq!(
        Invitation::all()
            .exec(&mut harness.db.clone())
            .await
            .expect("query")
            .len(),
        1,
        "her invitation still waits for her"
    );
}

#[tokio::test]
async fn the_billing_portal_stays_the_customers() {
    let stripe = FakeStripe::default();
    let harness = billed_harness(&stripe).await;
    let (ada, team) = person(&harness.db, "Ada Lovelace", "ada@example.com").await;
    toasty::create!(BillingAccount {
        team_id: team.tenant(),
        customer_id: "cus_1".to_string(),
        plan: PlanId::Free,
        status: BillingStatus::None,
        cancel_at_period_end: false,
    })
    .exec(&mut harness.db.clone())
    .await
    .expect("write the account");
    let (_, client) = staff_in_app(
        &harness,
        "sam@example.com",
        StaffRole::Support,
        Factor::BeforeGrant,
    )
    .await;
    client.submit("/app/impersonate", &act_as(&ada)).await;

    let response = client
        .csrf(CSRF)
        .post_form(
            "/app/settings/billing/-/actions/manage-billing",
            form_body(&[("csrf_token", CSRF)]),
        )
        .await;
    assert!(
        response
            .headers()
            .get(LOCATION)
            .is_none_or(|location| !location.to_str().unwrap_or_default().contains("stripe")),
        "{:?}",
        response.headers()
    );
}

#[tokio::test]
async fn deleting_the_staff_members_team_keeps_the_log() {
    let harness = harness().await;
    let (ada, _) = person(&harness.db, "Ada Lovelace", "ada@example.com").await;
    let (sam, client) = staff_in_app(
        &harness,
        "sam@example.com",
        StaffRole::Support,
        Factor::BeforeGrant,
    )
    .await;
    client.submit("/app/impersonate", &act_as(&ada)).await;
    client.submit("/app/impersonate/stop", "").await;

    let mut db = harness.db.clone();
    let own = Member::filter(Member::fields().user_id().eq(sam.id))
        .exec(&mut db)
        .await
        .expect("query");
    for seat in own {
        audit::forget_team(&mut db, seat.team_id)
            .await
            .expect("forget the team");
    }
    let admin = staff_client(&harness, "root@example.com", StaffRole::Administrator).await;
    assert_eq!(
        admin
            .resource::<ImpersonationResource>()
            .list()
            .await
            .rows()
            .len(),
        2,
        "the start, filed under Sam's own team, outlives it"
    );
}
