//! Erasure, and the public site.

mod support;

use support::{Harness, billed_harness, client_for, harness, person, seat, stripe::FakeStripe};
use tablo::testing::{body_string, form_body};
use tablo_saas::{
    accounts::User,
    audit::AuditEntry,
    billing::{BillingAccount, BillingStatus, PlanId},
    jobs::JobRow,
    teams::{Member, Team, TeamRole, create_team},
};
use topcoat::router::{StatusCode, header::CONTENT_TYPE};

const CSRF: &str = "3f0f6d3a-8f43-4f4d-9c59-2d3b3b1f4a11";
const PASSWORD: &str = "correct horse battery staple";

async fn subscribed(harness: &Harness, team: &Team) {
    toasty::create!(BillingAccount {
        team_id: team.tenant(),
        customer_id: "cus_1".to_string(),
        plan: PlanId::Pro,
        status: BillingStatus::Active,
        subscription_id: Some("sub_1".to_string()),
        cancel_at_period_end: false,
    })
    .exec(&mut harness.db.clone())
    .await
    .expect("write the account");
}

async fn with_password(harness: &Harness, user: &User) -> User {
    let hash = tablo::auth::hash_password(PASSWORD).await.expect("hash");
    User::filter_by_id(user.id)
        .update()
        .password(hash)
        .exec(&mut harness.db.clone())
        .await
        .expect("set the password");
    User::get_by_id(&mut harness.db.clone(), user.id)
        .await
        .expect("query")
}

async fn teams(harness: &Harness) -> usize {
    Team::all()
        .exec(&mut harness.db.clone())
        .await
        .expect("query")
        .len()
}

#[tokio::test]
async fn deleting_a_team_cancels_its_subscription_through_a_job() {
    let stripe = FakeStripe::default();
    let mut harness = billed_harness(&stripe).await;
    let (ada, personal) = person(&harness.db, "Ada", "ada@example.com").await;
    let team = create_team(&mut harness.db.clone(), "Analytical", &ada)
        .await
        .expect("write the team");
    subscribed(&harness, &team).await;

    let client = client_for(&harness, &ada, &team).await;
    let wrong = client
        .csrf(CSRF)
        .post_form(
            "/app/members/-/actions/delete-team",
            form_body(&[("name", "nope"), ("confirm", "1"), ("csrf_token", CSRF)]),
        )
        .await;
    assert_eq!(wrong.status(), StatusCode::OK, "the form says why");
    assert_eq!(teams(&harness).await, 2);

    let response = client
        .csrf(CSRF)
        .post_form(
            "/app/members/-/actions/delete-team",
            form_body(&[
                ("name", "Analytical"),
                ("confirm", "1"),
                ("csrf_token", CSRF),
            ]),
        )
        .await;
    assert!(response.status().is_redirection(), "{}", response.status());
    assert_eq!(teams(&harness).await, 1);
    assert_eq!(
        stripe.with(|state| state.canceled.len()),
        0,
        "not in the request"
    );

    harness.run_jobs().await;
    assert_eq!(stripe.with(|state| state.canceled.clone()), ["sub_1"]);
    let seats = Member::filter(Member::fields().team_id().eq(team.tenant()))
        .exec(&mut harness.db.clone())
        .await
        .expect("query");
    assert_eq!(seats.len(), 0);
    assert_eq!(personal.name, "Ada's team");
    let trail = AuditEntry::all()
        .exec(&mut harness.db.clone())
        .await
        .expect("query");
    assert_eq!(
        trail.len(),
        0,
        "a row here would break the foreign key on PostgreSQL"
    );
}

#[tokio::test]
async fn a_member_cannot_delete_a_team_nor_an_owner_their_last() {
    let harness = harness().await;
    let (ada, team) = person(&harness.db, "Ada", "ada@example.com").await;
    let (grace, _) = person(&harness.db, "Grace", "grace@example.com").await;
    seat(&harness.db, &grace, &team, TeamRole::Member).await;
    let body = form_body(&[
        ("name", "Ada's team"),
        ("confirm", "1"),
        ("csrf_token", CSRF),
    ]);

    for client in [
        client_for(&harness, &grace, &team).await,
        client_for(&harness, &ada, &team).await,
    ] {
        let response = client
            .csrf(CSRF)
            .post_form("/app/members/-/actions/delete-team", body.clone())
            .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
    assert_eq!(teams(&harness).await, 2);
}

#[tokio::test]
async fn deleting_an_account_erases_it_with_the_teams_only_it_used() {
    let stripe = FakeStripe::default();
    let mut harness = billed_harness(&stripe).await;
    let (ada, team) = person(&harness.db, "Ada", "ada@example.com").await;
    subscribed(&harness, &team).await;
    let ada = with_password(&harness, &ada).await;

    let client = client_for(&harness, &ada, &team).await;
    let response = client
        .csrf(CSRF)
        .post_form(
            "/app/account/-/actions/delete-account",
            form_body(&[
                ("password", PASSWORD),
                ("confirm", "1"),
                ("csrf_token", CSRF),
            ]),
        )
        .await;
    assert!(response.status().is_redirection(), "{}", response.status());
    assert_eq!(teams(&harness).await, 0);
    assert_eq!(
        User::all()
            .exec(&mut harness.db.clone())
            .await
            .expect("query")
            .len(),
        0
    );
    let response = client.get("/app").await;
    assert!(
        response.status().is_redirection(),
        "the session died with the account"
    );
    harness.run_jobs().await;
    assert_eq!(stripe.with(|state| state.canceled.clone()), ["sub_1"]);
}

#[tokio::test]
async fn other_teams_keep_the_trail_without_the_name_of_who_left() {
    let harness = harness().await;
    let (ada, personal) = person(&harness.db, "Ada", "ada@example.com").await;
    let (_, shared) = person(&harness.db, "Grace", "grace@example.com").await;
    seat(&harness.db, &ada, &shared, TeamRole::Member).await;
    toasty::create!(AuditEntry {
        team_id: shared.tenant(),
        actor: "ada@example.com".to_string(),
        resource: "Project".to_string(),
        change: "create".to_string(),
        record: String::new(),
        at: jiff::Timestamp::now(),
    })
    .exec(&mut harness.db.clone())
    .await
    .expect("write the entry");
    let ada = with_password(&harness, &ada).await;

    let client = client_for(&harness, &ada, &personal).await;
    let response = client
        .csrf(CSRF)
        .post_form(
            "/app/account/-/actions/delete-account",
            form_body(&[
                ("password", PASSWORD),
                ("confirm", "1"),
                ("csrf_token", CSRF),
            ]),
        )
        .await;
    assert!(response.status().is_redirection(), "{}", response.status());
    let trail = AuditEntry::all()
        .exec(&mut harness.db.clone())
        .await
        .expect("query");
    assert_eq!(trail.len(), 1);
    assert!(!trail[0].actor.contains("ada"), "{}", trail[0].actor);
}

#[tokio::test]
async fn an_owner_of_a_shared_team_transfers_before_leaving() {
    let harness = harness().await;
    let (ada, team) = person(&harness.db, "Ada", "ada@example.com").await;
    let (grace, _) = person(&harness.db, "Grace", "grace@example.com").await;
    seat(&harness.db, &grace, &team, TeamRole::Member).await;
    let ada = with_password(&harness, &ada).await;

    let client = client_for(&harness, &ada, &team).await;
    client
        .csrf(CSRF)
        .post_form(
            "/app/account/-/actions/delete-account",
            form_body(&[
                ("password", PASSWORD),
                ("confirm", "1"),
                ("csrf_token", CSRF),
            ]),
        )
        .await;
    assert_eq!(teams(&harness).await, 2);
    assert!(
        User::filter_by_id(ada.id)
            .first()
            .exec(&mut harness.db.clone())
            .await
            .expect("query")
            .is_some()
    );
}

#[tokio::test]
async fn a_wrong_password_deletes_nothing() {
    let harness = harness().await;
    let (ada, team) = person(&harness.db, "Ada", "ada@example.com").await;
    let ada = with_password(&harness, &ada).await;
    let client = client_for(&harness, &ada, &team).await;
    client
        .csrf(CSRF)
        .post_form(
            "/app/account/-/actions/delete-account",
            form_body(&[
                ("password", "wrong"),
                ("confirm", "1"),
                ("csrf_token", CSRF),
            ]),
        )
        .await;
    assert_eq!(teams(&harness).await, 1);
    let queued = JobRow::all()
        .exec(&mut harness.db.clone())
        .await
        .expect("query");
    assert_eq!(queued.len(), 0);
}

#[tokio::test]
async fn the_public_pages_answer_and_the_price_comes_from_the_catalog() {
    let harness = harness().await;
    let client = tablo::testing::TestClient::new(&harness.app);
    for path in ["/", "/terms", "/privacy"] {
        assert_eq!(client.get(path).await.status(), StatusCode::OK, "{path}");
    }
    let page = body_string(client.get("/pricing").await).await;
    assert!(page.contains("19.00 EUR"), "{page}");
}

#[tokio::test]
async fn robots_and_the_sitemap_describe_the_public_site() {
    let harness = harness().await;
    let client = tablo::testing::TestClient::new(&harness.app);
    let robots = client.get("/robots.txt").await;
    assert_eq!(robots.headers()[CONTENT_TYPE], "text/plain; charset=utf-8");
    assert!(body_string(robots).await.contains("Disallow: /app"));
    let sitemap = body_string(client.get("/sitemap.xml").await).await;
    assert!(sitemap.contains("/pricing</loc>"), "{sitemap}");
}
