//! The back office and the audit trails.

mod support;

use support::{client_for, harness, person, seat, staff_client};
use tablo::{AuditEntry, AuditLog, testing::TestClient};
use tablo_saas::{
    accounts::{Accounts, StaffRole},
    backoffice::{JobResource, RetryJob},
    billing::{BillingAccount, BillingStatus, PlanId},
    jobs::{JobRow, JobState},
    teams::{MemberResource, TeamRole, actions::RemoveMember},
};
use topcoat::router::StatusCode;

async fn dead_job(db: &toasty::Db) -> JobRow {
    toasty::create!(JobRow {
        id: uuid::Uuid::now_v7(),
        kind: "prune-jobs".to_string(),
        payload: "null".to_string(),
        state: JobState::Dead,
        dedupe_key: uuid::Uuid::now_v7().to_string(),
        attempts: 8_u64,
        max_attempts: 8_u64,
        run_at: jiff::Timestamp::now(),
    })
    .exec(&mut db.clone())
    .await
    .expect("write the job")
}

#[tokio::test]
async fn a_customer_does_not_enter_the_back_office() {
    let harness = harness().await;
    let (ada, _) = person(&harness.db, "Ada", "ada@example.com").await;
    let signed = Accounts::signed_in(ada, &mut harness.db.clone())
        .await
        .expect("load");
    let client = TestClient::new(&harness.app)
        .sign_in(&harness.db, "admin", &signed)
        .await;
    let response = client.get("/admin/jobs").await;
    assert!(!response.status().is_success(), "{}", response.status());
}

#[tokio::test]
async fn staff_are_sent_to_set_up_a_second_factor_first() {
    let harness = harness().await;
    let (user, _) = person(&harness.db, "Staff", "staff@example.com").await;
    tablo_saas::accounts::User::filter_by_id(user.id)
        .update()
        .staff_role(Some(StaffRole::Administrator))
        .exec(&mut harness.db.clone())
        .await
        .expect("make staff");
    let user = tablo_saas::accounts::User::get_by_id(&mut harness.db.clone(), user.id)
        .await
        .expect("query");
    let signed = Accounts::signed_in(user, &mut harness.db.clone())
        .await
        .expect("load");
    let client = TestClient::new(&harness.app)
        .sign_in(&harness.db, "admin", &signed)
        .await;
    let response = client.get("/admin/jobs").await;
    assert!(response.status().is_redirection(), "{}", response.status());
    assert!(
        response.headers()["location"]
            .to_str()
            .expect("ascii")
            .contains("two-factor")
    );
}

#[tokio::test]
async fn an_administrator_retries_a_dead_job_and_the_trail_says_so() {
    let harness = harness().await;
    let job = dead_job(&harness.db).await;
    let client = staff_client(&harness, "root@example.com", StaffRole::Administrator).await;
    let jobs = client.resource::<JobResource>();
    assert_eq!(jobs.list().await.rows().len(), 1);

    let reply = jobs.action::<RetryJob>(&job).submit().await;
    assert!(reply.status().is_redirection(), "{}", reply.status());
    let row = JobRow::filter_by_id(job.id)
        .first()
        .exec(&mut harness.db.clone())
        .await
        .expect("query")
        .expect("the job");
    assert_eq!(row.state, JobState::Queued);
    assert_eq!(row.attempts, 0);

    let trail = AuditEntry::all()
        .exec(&mut harness.db.clone())
        .await
        .expect("query");
    assert_eq!(trail.len(), 1);
    assert_eq!(trail[0].panel, "/admin");
    assert_eq!(trail[0].event, "action:retry");
    assert_eq!(trail[0].record, Some(job.id.to_string()));
    assert_eq!(client.resource::<AuditLog>().list().await.rows().len(), 1);
}

#[tokio::test]
async fn support_reads_the_queue_and_changes_nothing() {
    let harness = harness().await;
    let job = dead_job(&harness.db).await;
    let client = staff_client(&harness, "help@example.com", StaffRole::Support).await;
    let jobs = client.resource::<JobResource>();
    assert_eq!(jobs.list().await.status(), StatusCode::OK);
    let reply = jobs.action::<RetryJob>(&job).submit().await;
    assert!(reply.status().is_client_error(), "{}", reply.status());
    assert_eq!(
        client.resource::<AuditLog>().list().await.status(),
        StatusCode::FORBIDDEN,
        "the staff trail is for administrators"
    );
}

#[tokio::test]
async fn a_write_lands_on_the_team_trail_and_the_plan_gates_reading_it() {
    let harness = harness().await;
    let (ada, team) = person(&harness.db, "Ada", "ada@example.com").await;
    let (grace, _) = person(&harness.db, "Grace", "grace@example.com").await;
    let grace_seat = seat(&harness.db, &grace, &team, TeamRole::Member).await;
    let client = client_for(&harness, &ada, &team).await;
    client
        .resource::<MemberResource>()
        .action::<RemoveMember>(&grace_seat)
        .submit()
        .await;

    let trail = AuditEntry::all()
        .exec(&mut harness.db.clone())
        .await
        .expect("query");
    assert_eq!(trail.len(), 1);
    assert_eq!(trail[0].panel, "/app");
    assert_eq!(trail[0].actor_id, Some(ada.id.to_string()));
    assert_eq!(trail[0].tenant, Some(team.tenant()));
    assert_eq!(trail[0].event, "action:remove");
    assert_eq!(
        client.resource::<AuditLog>().list().await.status(),
        StatusCode::FORBIDDEN,
        "the free plan keeps the trail and does not show it"
    );

    toasty::create!(BillingAccount {
        team_id: team.tenant(),
        customer_id: "cus_1".to_string(),
        plan: PlanId::Business,
        status: BillingStatus::Active,
        cancel_at_period_end: false,
    })
    .exec(&mut harness.db.clone())
    .await
    .expect("write the account");
    let paying = client_for(&harness, &ada, &team).await;
    let list = paying.resource::<AuditLog>().list().await;
    assert_eq!(list.status(), StatusCode::OK);
    assert_eq!(list.rows().len(), 1);
}
