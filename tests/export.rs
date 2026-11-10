//! The right of access: each list's CSV, and the whole account as one JSON file.

mod support;

use support::{Harness, client_for, harness, person, seat};
use tablo::{
    AuditEntry,
    testing::{TestClient, body_string, form_body},
};
use tablo_saas::{
    billing::{BillingAccount, BillingStatus, PlanId},
    projects::{Project, ProjectStatus},
    teams::{EXPORT_URL, Invitation, Team, TeamRole},
};
use topcoat::router::{
    StatusCode,
    header::{CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_TYPE, LOCATION},
};

const CSRF: &str = "3f0f6d3a-8f43-4f4d-9c59-2d3b3b1f4a11";

async fn project(harness: &Harness, team: &Team, name: &str) {
    toasty::create!(Project {
        team_id: team.tenant(),
        name: name.to_string(),
        status: ProjectStatus::Active,
    })
    .exec(&mut harness.db.clone())
    .await
    .expect("write the project");
}

async fn invitation(harness: &Harness, team: &Team, email: &str) {
    toasty::create!(Invitation {
        team_id: team.tenant(),
        team_name: team.name.clone(),
        email: email.to_string(),
        role: TeamRole::Member,
        invited_by: "Someone".to_string(),
        expires_at: jiff::Timestamp::now() + jiff::SignedDuration::from_hours(24),
    })
    .exec(&mut harness.db.clone())
    .await
    .expect("write the invitation");
}

async fn did(harness: &Harness, team: &Team, actor: uuid::Uuid, event: &str) {
    toasty::create!(AuditEntry {
        panel: "/app".to_string(),
        tenant: Some(team.tenant()),
        actor_id: Some(actor.to_string()),
        actor: "whoever".to_string(),
        event: event.to_string(),
        resource: Some("projects".to_string()),
        record: None,
        url: None,
        changes: toasty::Json(Vec::new()),
        created_at: jiff::Timestamp::now(),
    })
    .exec(&mut harness.db.clone())
    .await
    .expect("write the entry");
}

async fn csv(client: &TestClient<'_>, url: &str) -> String {
    let response = client.get(url).await;
    assert_eq!(response.status(), StatusCode::OK, "{url}");
    let kind = response.headers()[CONTENT_TYPE].to_str().expect("ascii");
    assert!(kind.starts_with("text/csv"), "{kind}");
    body_string(response).await
}

#[tokio::test]
async fn a_team_exports_its_own_projects_and_members_as_csv() {
    let harness = harness().await;
    let (ada, team) = person(&harness.db, "Ada", "ada@example.com").await;
    let (grace, other) = person(&harness.db, "Grace", "grace@example.com").await;
    let (alan, _) = person(&harness.db, "Alan", "alan@example.com").await;
    seat(&harness.db, &alan, &team, TeamRole::Member).await;
    project(&harness, &team, "Analytical engine").await;
    project(&harness, &other, "Compiler").await;

    let client = client_for(&harness, &ada, &team).await;
    let projects = csv(&client, "/app/projects/-/export").await;
    assert!(projects.contains("Analytical engine"), "{projects}");
    assert!(!projects.contains("Compiler"), "{projects}");

    let members = csv(&client, "/app/settings/members/-/export").await;
    assert!(members.contains("Ada <ada@example.com>"), "{members}");
    assert!(members.contains("Alan <alan@example.com>"), "{members}");
    assert!(!members.contains(&grace.email), "{members}");
}

#[tokio::test]
async fn an_account_downloads_what_it_owns_and_nothing_else() {
    let harness = harness().await;
    let (ada, personal) = person(&harness.db, "Ada", "ada@example.com").await;
    let (grace, shared) = person(&harness.db, "Grace", "grace@example.com").await;
    let (_, inviting) = person(&harness.db, "Alan", "alan@example.com").await;
    seat(&harness.db, &ada, &shared, TeamRole::Member).await;
    project(&harness, &personal, "Analytical engine").await;
    project(&harness, &shared, "Compiler").await;
    invitation(&harness, &personal, "charles@example.com").await;
    invitation(&harness, &shared, "edsger@example.com").await;
    invitation(&harness, &inviting, "ada@example.com").await;
    toasty::create!(BillingAccount {
        team_id: personal.tenant(),
        customer_id: "cus_ada".to_string(),
        plan: PlanId::Pro,
        status: BillingStatus::Active,
        subscription_id: Some("sub_ada".to_string()),
        cancel_at_period_end: false,
    })
    .exec(&mut harness.db.clone())
    .await
    .expect("write the account");
    did(&harness, &shared, ada.id, "ada-did-this").await;
    did(&harness, &shared, grace.id, "grace-did-this").await;

    let client = client_for(&harness, &ada, &personal).await;
    let response = client.get(EXPORT_URL).await;
    assert_eq!(response.status(), StatusCode::OK);
    let headers = response.headers().clone();
    assert_eq!(headers[CONTENT_TYPE], "application/json");
    assert_eq!(headers[CACHE_CONTROL], "no-store");
    let disposition = headers[CONTENT_DISPOSITION].to_str().expect("ascii");
    assert!(
        disposition.starts_with("attachment; filename=\"account-"),
        "{disposition}"
    );
    let body = body_string(response).await;
    assert!(!body.contains("unused"), "the password hash stays home");
    assert!(!body.contains("Compiler"), "a team Ada does not own");
    assert!(
        !body.contains("edsger"),
        "who a team Ada does not own invited"
    );
    assert!(!body.contains("grace-did-this"), "what someone else did");
    let file: serde_json::Value = serde_json::from_str(&body).expect("json");

    assert_eq!(file["user"]["email"], "ada@example.com");
    assert_eq!(file["user"]["name"], "Ada");
    let mut roles: Vec<_> = file["memberships"]
        .as_array()
        .expect("list")
        .iter()
        .map(|seat| format!("{} {}", seat["team_name"], seat["role"]))
        .collect();
    roles.sort();
    assert_eq!(
        roles,
        [r#""Ada's team" "owner""#, r#""Grace's team" "member""#]
    );

    let owned = file["owned_teams"].as_array().expect("list");
    assert_eq!(owned.len(), 1);
    assert_eq!(owned[0]["name"], "Ada's team");
    assert_eq!(owned[0]["projects"][0]["name"], "Analytical engine");
    assert_eq!(owned[0]["projects"][0]["status"], "active");
    assert_eq!(owned[0]["invitations"][0]["email"], "charles@example.com");
    assert_eq!(owned[0]["billing_account"]["customer_id"], "cus_ada");
    assert_eq!(owned[0]["billing_account"]["plan"], "pro");
    assert_eq!(owned[0]["billing_account"]["status"], "active");

    let received = file["invitations"].as_array().expect("list");
    assert_eq!(received.len(), 1);
    assert_eq!(received[0]["team_name"], "Alan's team");

    let trail = file["audit_entries"].as_array().expect("list");
    assert_eq!(trail.len(), 1);
    assert_eq!(trail[0]["event"], "ada-did-this");
    assert_eq!(trail[0]["team_id"], shared.id.to_string());
    assert_eq!(file["notifications"], serde_json::json!([]));
}

#[tokio::test]
async fn the_profile_button_lands_on_the_download_and_a_stranger_gets_nothing() {
    let harness = harness().await;
    let (ada, team) = person(&harness.db, "Ada", "ada@example.com").await;

    let client = client_for(&harness, &ada, &team).await;
    let profile = body_string(client.get("/app/settings/profile").await).await;
    assert!(profile.contains("Download my data"), "{profile}");
    let response = client
        .csrf(CSRF)
        .post_form(
            "/app/settings/profile/-/actions/export-account",
            form_body(&[("csrf_token", CSRF)]),
        )
        .await;
    assert!(response.status().is_redirection(), "{}", response.status());
    assert_eq!(response.headers()[LOCATION], EXPORT_URL);

    let stranger = TestClient::new(&harness.app).get(EXPORT_URL).await;
    assert_ne!(stranger.status(), StatusCode::OK);
    let body = body_string(stranger).await;
    assert!(!body.contains("ada@example.com"), "{body}");
}
