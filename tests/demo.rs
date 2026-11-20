//! The public demo. `reset` truncates through PostgreSQL's catalog, so these run on the PostgreSQL
//! test database alone, as in CI.

mod support;

use support::{
    DEMO_ADMIN, billed_harness, demo_harness, person,
    stripe::{FakeStripe, subscription},
};
use tablo::{
    jobs::{JobSchedule, QueuedJob},
    testing::{TEST_DATABASE_URL, TestClient, body_string, form_body, response_cookies},
};
use tablo_saas::{
    accounts::{StaffRole, User},
    billing::{Billing, BillingAccount, BillingStatus, PlanId},
    demo::{Wiped, reset},
    platform::database,
    projects::{Project, ProjectStatus},
    teams::Team,
};
use topcoat::router::header::LOCATION;

fn on_postgres() -> bool {
    std::env::var(TEST_DATABASE_URL).is_ok()
}

#[tokio::test]
async fn a_reset_empties_the_database_and_deletes_the_stripe_customers() {
    if !on_postgres() {
        return;
    }
    let stripe = FakeStripe::default();
    let harness = billed_harness(&stripe).await;
    let mut db = harness.db.clone();
    let (_, team) = person(&harness.db, "Ada", "ada@example.com").await;
    toasty::create!(BillingAccount {
        team_id: team.tenant(),
        customer_id: "cus_ada".to_string(),
        plan: PlanId::Pro,
        status: BillingStatus::Trialing,
        cancel_at_period_end: false,
    })
    .exec(&mut db)
    .await
    .expect("write the account");
    stripe.with(|state| {
        state
            .subscriptions
            .push(subscription("cus_ada", "trialing", "pro_month", 1));
    });
    toasty::create!(Project {
        team_id: team.tenant(),
        name: "Launch".to_string(),
        status: ProjectStatus::Active,
    })
    .exec(&mut db)
    .await
    .expect("write a project");
    JobSchedule::create()
        .kind("prune")
        .next_run_at(jiff::Timestamp::now())
        .exec(&mut db)
        .await
        .expect("write a schedule");

    let billing = Billing::with_gateway(std::sync::Arc::new(stripe.clone()), "whsec_test");
    let wiped = reset(&harness.db, &billing, None).await.expect("reset");

    assert_eq!(
        wiped,
        Wiped {
            customers_deleted: 1,
            customers_left: 0,
            admin_seeded: false,
        }
    );
    assert_eq!(
        stripe.with(|state| state.deleted_customers.clone()),
        ["cus_ada"]
    );
    assert!(User::all().exec(&mut db).await.expect("query").is_empty());
    assert!(Team::all().exec(&mut db).await.expect("query").is_empty());
    assert!(
        Project::all()
            .exec(&mut db)
            .await
            .expect("query")
            .is_empty()
    );
    assert!(
        BillingAccount::all()
            .exec(&mut db)
            .await
            .expect("query")
            .is_empty()
    );
    assert!(
        QueuedJob::all()
            .exec(&mut db)
            .await
            .expect("query")
            .is_empty()
    );
    assert_eq!(
        JobSchedule::all().exec(&mut db).await.expect("query").len(),
        1,
        "a wipe keeps the schedules, or daily jobs would never fall due"
    );
    assert_eq!(
        database::migrate(&harness.db).await.expect("migrate"),
        0,
        "a wipe keeps the migration history"
    );
}

#[tokio::test]
async fn a_reset_seeds_the_public_administrator_who_needs_no_second_factor() {
    if !on_postgres() {
        return;
    }
    let stripe = FakeStripe::default();
    let (harness, config) = demo_harness(&stripe).await;
    person(&harness.db, "Ada", "ada@example.com").await;
    let billing = Billing::with_gateway(std::sync::Arc::new(stripe), "whsec_test");
    let admin = config.demo.as_ref().and_then(|demo| demo.admin.as_ref());

    let wiped = reset(&harness.db, &billing, admin).await.expect("reset");
    assert!(wiped.admin_seeded);
    // The next run wipes the administrator it seeded and seeds them again.
    reset(&harness.db, &billing, admin)
        .await
        .expect("reset again");

    let users = User::all()
        .exec(&mut harness.db.clone())
        .await
        .expect("query");
    let [seeded] = users.as_slice() else {
        panic!("one account: {users:?}");
    };
    assert_eq!(seeded.email, DEMO_ADMIN.0);
    assert_eq!(seeded.staff_role, Some(StaffRole::Administrator));
    assert!(seeded.email_verified_at.is_some());

    let visitor = TestClient::new(&harness.app);
    let page = body_string(visitor.get("/admin/login").await).await;
    assert!(
        page.contains(&format!("{} / {}", DEMO_ADMIN.0, DEMO_ADMIN.1)),
        "the sign-in page names the public account: {page}"
    );
    let signed_in = visitor
        .submit(
            "/admin/login",
            &form_body(&[("email", DEMO_ADMIN.0), ("password", DEMO_ADMIN.1)]),
        )
        .await;
    assert_eq!(signed_in.status(), 303);
    let admin = visitor.cookies(&response_cookies(&signed_in));
    let customers = admin.get("/admin/customers").await;
    assert_eq!(
        customers.status(),
        200,
        "no second factor stands in the way: {:?}",
        customers.headers().get(LOCATION)
    );
}

#[tokio::test]
async fn a_stripe_outage_leaves_customers_behind_but_still_wipes() {
    if !on_postgres() {
        return;
    }
    let stripe = FakeStripe::default();
    let harness = billed_harness(&stripe).await;
    let (_, team) = person(&harness.db, "Ada", "ada@example.com").await;
    toasty::create!(BillingAccount {
        team_id: team.tenant(),
        customer_id: "cus_ada".to_string(),
        plan: PlanId::Free,
        status: BillingStatus::None,
        cancel_at_period_end: false,
    })
    .exec(&mut harness.db.clone())
    .await
    .expect("write the account");
    stripe.with(|state| state.down = true);
    let billing = Billing::with_gateway(std::sync::Arc::new(stripe), "whsec_test");

    let wiped = reset(&harness.db, &billing, None).await.expect("reset");
    assert_eq!(wiped.customers_left, 1);
    assert!(
        User::all()
            .exec(&mut harness.db.clone())
            .await
            .expect("query")
            .is_empty()
    );
}
