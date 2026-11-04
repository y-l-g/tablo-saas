//! The example domain: tenant scoping and a plan limit.

mod support;

use support::{Harness, client_for, harness, person};
use tablo::testing::body_string;
use tablo_saas::{
    billing::{BillingAccount, BillingStatus, PlanId},
    projects::{Project, ProjectResource, ProjectStatus},
    teams::Team,
};
use topcoat::router::StatusCode;

async fn project(harness: &Harness, team: &Team, name: &str) -> Project {
    toasty::create!(Project {
        team_id: team.tenant(),
        name: name.to_string(),
        status: ProjectStatus::Active,
    })
    .exec(&mut harness.db.clone())
    .await
    .expect("write the project")
}

async fn count(harness: &Harness, team: &Team) -> usize {
    Project::filter(Project::fields().team_id().eq(team.tenant()))
        .exec(&mut harness.db.clone())
        .await
        .expect("query")
        .len()
}

#[tokio::test]
async fn a_team_reads_and_edits_its_own_projects_alone() {
    let harness = harness().await;
    let (ada, team) = person(&harness.db, "Ada", "ada@example.com").await;
    let (_, other) = person(&harness.db, "Grace", "grace@example.com").await;
    project(&harness, &team, "Analytical engine").await;
    let foreign = project(&harness, &other, "Compiler").await;

    let client = client_for(&harness, &ada, &team).await;
    let projects = client.resource::<ProjectResource>();
    let list = projects.list().await;
    assert_eq!(list.rows().len(), 1);
    assert!(!list.html().contains("Compiler"));
    assert_eq!(
        projects.view(&foreign).await.status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn a_new_project_lands_on_the_team_acting() {
    let harness = harness().await;
    let (ada, team) = person(&harness.db, "Ada", "ada@example.com").await;
    let client = client_for(&harness, &ada, &team).await;
    let reply = client
        .resource::<ProjectResource>()
        .create()
        .set_key("name", "Difference engine")
        .set_key("status", "active")
        .submit()
        .await;
    assert!(reply.status().is_redirection(), "{}", reply.html());
    assert_eq!(count(&harness, &team).await, 1);
}

#[tokio::test]
async fn the_free_plan_stops_at_three_projects_and_a_paid_one_does_not() {
    let harness = harness().await;
    let (ada, team) = person(&harness.db, "Ada", "ada@example.com").await;
    for name in ["One", "Two", "Three"] {
        project(&harness, &team, name).await;
    }
    let client = client_for(&harness, &ada, &team).await;
    let create = || {
        client
            .resource::<ProjectResource>()
            .create()
            .set_key("name", "Four")
            .set_key("status", "active")
    };
    assert!(create().submit().await.declined().is_some());
    assert_eq!(count(&harness, &team).await, 3);

    toasty::create!(BillingAccount {
        team_id: team.tenant(),
        customer_id: "cus_1".to_string(),
        plan: PlanId::Pro,
        status: BillingStatus::Active,
        cancel_at_period_end: false,
    })
    .exec(&mut harness.db.clone())
    .await
    .expect("write the account");
    let paying = client_for(&harness, &ada, &team).await;
    let reply = paying
        .resource::<ProjectResource>()
        .create()
        .set_key("name", "Four")
        .set_key("status", "active")
        .submit()
        .await;
    assert!(reply.status().is_redirection(), "{}", reply.html());
    assert_eq!(count(&harness, &team).await, 4);
}

#[tokio::test]
async fn the_home_page_shows_the_plan_and_what_it_uses() {
    let harness = harness().await;
    let (ada, team) = person(&harness.db, "Ada", "ada@example.com").await;
    project(&harness, &team, "One").await;
    let client = client_for(&harness, &ada, &team).await;
    let html = body_string(client.get("/app").await).await;
    assert!(html.contains("Free"), "{html}");
    assert!(html.contains("1 / 3"), "{html}");
}
