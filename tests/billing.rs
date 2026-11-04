//! Money: what Stripe says reaching the mirror, and what the mirror allows.

mod support;

use jiff::Timestamp;
use support::{
    Harness, WEBHOOK_SECRET, billed_harness, client_for, harness, person, seat, stripe::FakeStripe,
};
use tablo::testing::{TestClient, body_string, form_body};
use tablo_saas::{
    accounts::User,
    billing::{
        BillingAccount, BillingStatus, PlanId, StripeEvent,
        jobs::{ReconcileBilling, SyncSeats},
        stripe::sign,
    },
    jobs::{JobRow, JobState, enqueue},
    teams::{MemberResource, Team, TeamRole, actions::RemoveMember},
};
use topcoat::router::{Body, StatusCode, header::LOCATION};

const CSRF: &str = "3f0f6d3a-8f43-4f4d-9c59-2d3b3b1f4a11";

/// A row as `ensure_account` leaves it, without a trip through Checkout.
async fn account(harness: &Harness, team: &Team, customer: &str) -> BillingAccount {
    toasty::create!(BillingAccount {
        team_id: team.tenant(),
        customer_id: customer.to_string(),
        plan: PlanId::Free,
        status: BillingStatus::None,
        cancel_at_period_end: false,
    })
    .exec(&mut harness.db.clone())
    .await
    .expect("write the account")
}

async fn mirror(harness: &Harness, customer: &str) -> BillingAccount {
    BillingAccount::filter(BillingAccount::fields().customer_id().eq(customer))
        .first()
        .exec(&mut harness.db.clone())
        .await
        .expect("query")
        .expect("the account")
}

fn event(id: &str, kind: &str, customer: &str) -> String {
    serde_json::json!({
        "id": id,
        "type": kind,
        "created": 1_800_000_000,
        "data": { "object": { "customer": customer } },
    })
    .to_string()
}

async fn deliver(harness: &Harness, body: &str, signature: Option<String>) -> StatusCode {
    let mut request = http::Request::builder()
        .method("POST")
        .uri("/webhooks/stripe");
    if let Some(signature) = signature {
        request = request.header("stripe-signature", signature);
    }
    let request = request
        .body(Body::new(body.to_string()))
        .expect("a request");
    harness.app.handle(request).await.status()
}

async fn signed(harness: &Harness, body: &str) -> StatusCode {
    let signature = sign(
        body.as_bytes(),
        WEBHOOK_SECRET,
        Timestamp::now().as_second(),
    );
    deliver(harness, body, Some(signature)).await
}

async fn queued(harness: &Harness) -> usize {
    JobRow::filter(JobRow::fields().state().eq(JobState::Queued))
        .exec(&mut harness.db.clone())
        .await
        .expect("query")
        .len()
}

#[tokio::test]
async fn a_webhook_without_a_good_signature_changes_nothing() {
    let stripe = FakeStripe::default();
    let harness = billed_harness(&stripe).await;
    let body = event("evt_1", "invoice.paid", "cus_1");

    assert_eq!(
        deliver(&harness, &body, None).await,
        StatusCode::BAD_REQUEST
    );
    let forged = sign(body.as_bytes(), "whsec_wrong", Timestamp::now().as_second());
    assert_eq!(
        deliver(&harness, &body, Some(forged)).await,
        StatusCode::BAD_REQUEST
    );
    let stale = sign(body.as_bytes(), WEBHOOK_SECRET, 1_000_000_000);
    assert_eq!(
        deliver(&harness, &body, Some(stale)).await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(queued(&harness).await, 0);
}

#[tokio::test]
async fn the_webhook_is_absent_while_billing_is_off() {
    let harness = harness().await;
    let body = event("evt_1", "invoice.paid", "cus_1");
    assert_eq!(signed(&harness, &body).await, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn an_event_refreshes_the_mirror_from_stripe_not_from_its_body() {
    let stripe = FakeStripe::default();
    let mut harness = billed_harness(&stripe).await;
    let (_, team) = person(&harness.db, "Ada", "ada@example.com").await;
    account(&harness, &team, "cus_1").await;
    stripe.subscribe("cus_1", "active", "pro_month", 4);

    let body = event("evt_1", "customer.subscription.updated", "cus_1");
    assert_eq!(signed(&harness, &body).await, StatusCode::OK);
    harness.run_jobs().await;

    let row = mirror(&harness, "cus_1").await;
    assert_eq!(row.plan, PlanId::Pro);
    assert_eq!(row.status, BillingStatus::Active);
    assert_eq!(row.seats, Some(4));
    assert_eq!(row.subscription_id.as_deref(), Some("sub_cus_1"));
    assert!(row.synced_at.is_some());
}

#[tokio::test]
async fn a_redelivered_event_runs_once() {
    let stripe = FakeStripe::default();
    let mut harness = billed_harness(&stripe).await;
    let (_, team) = person(&harness.db, "Ada", "ada@example.com").await;
    account(&harness, &team, "cus_1").await;
    stripe.subscribe("cus_1", "active", "pro_month", 1);

    let body = event("evt_1", "invoice.paid", "cus_1");
    signed(&harness, &body).await;
    signed(&harness, &body).await;
    assert_eq!(queued(&harness).await, 1);
    harness.run_jobs().await;
    signed(&harness, &body).await;
    harness.run_jobs().await;

    assert_eq!(stripe.with(|state| state.refreshes), 1);
    let events = StripeEvent::all()
        .exec(&mut harness.db.clone())
        .await
        .expect("query");
    assert_eq!(events.len(), 1);
    assert!(events[0].processed_at.is_some());
}

#[tokio::test]
async fn events_out_of_order_end_on_what_stripe_says_now() {
    let stripe = FakeStripe::default();
    let mut harness = billed_harness(&stripe).await;
    let (_, team) = person(&harness.db, "Ada", "ada@example.com").await;
    account(&harness, &team, "cus_1").await;
    // Cancelled by now, and the older `created` event arrives after the `deleted` one.
    stripe.subscribe("cus_1", "canceled", "pro_month", 1);
    signed(
        &harness,
        &event("evt_2", "customer.subscription.deleted", "cus_1"),
    )
    .await;
    signed(
        &harness,
        &event("evt_1", "customer.subscription.created", "cus_1"),
    )
    .await;
    harness.run_jobs().await;

    assert_eq!(
        mirror(&harness, "cus_1").await.status,
        BillingStatus::Canceled
    );
}

#[tokio::test]
async fn an_unhandled_event_is_acknowledged_and_ignored() {
    let stripe = FakeStripe::default();
    let harness = billed_harness(&stripe).await;
    let body = event("evt_1", "payment_method.attached", "cus_1");
    assert_eq!(signed(&harness, &body).await, StatusCode::OK);
    assert_eq!(queued(&harness).await, 0);
}

#[tokio::test]
async fn a_failed_stripe_call_retries_the_event_rather_than_losing_it() {
    let stripe = FakeStripe::default();
    let mut harness = billed_harness(&stripe).await;
    let (_, team) = person(&harness.db, "Ada", "ada@example.com").await;
    account(&harness, &team, "cus_1").await;
    stripe.with(|state| state.down = true);
    signed(&harness, &event("evt_1", "invoice.paid", "cus_1")).await;
    harness.run_jobs().await;

    let jobs = JobRow::all()
        .exec(&mut harness.db.clone())
        .await
        .expect("query");
    let job = jobs
        .iter()
        .find(|job| job.kind == "process-stripe-event")
        .expect("the job");
    assert_eq!(job.state, JobState::Queued, "backing off, not dead");
    assert_eq!(job.attempts, 1);
}

#[tokio::test]
async fn a_failed_payment_mails_the_owner_and_starts_the_grace_period() {
    let stripe = FakeStripe::default();
    let mut harness = billed_harness(&stripe).await;
    let (_, team) = person(&harness.db, "Ada", "ada@example.com").await;
    account(&harness, &team, "cus_1").await;
    stripe.subscribe("cus_1", "past_due", "pro_month", 1);

    signed(&harness, &event("evt_1", "invoice.payment_failed", "cus_1")).await;
    harness.run_jobs().await;

    let row = mirror(&harness, "cus_1").await;
    assert_eq!(row.status, BillingStatus::PastDue);
    assert!(row.past_due_since.is_some());
    let sent = harness.mail(1).await;
    assert_eq!(sent[0].to()[0].address(), "ada@example.com");
    assert_eq!(sent[0].subject(), "Your payment failed");

    stripe.subscribe("cus_1", "active", "pro_month", 1);
    signed(&harness, &event("evt_2", "invoice.paid", "cus_1")).await;
    harness.run_jobs().await;
    assert!(mirror(&harness, "cus_1").await.past_due_since.is_none());
}

#[tokio::test]
async fn an_owner_leaves_for_checkout_with_the_team_seat_count() {
    let stripe = FakeStripe::default();
    let harness = billed_harness(&stripe).await;
    let (ada, team) = person(&harness.db, "Ada", "ada@example.com").await;
    let (grace, _) = person(&harness.db, "Grace", "grace@example.com").await;
    seat(&harness.db, &grace, &team, TeamRole::Member).await;

    let client = client_for(&harness, &ada, &team).await;
    let response = client
        .csrf(CSRF)
        .post_form(
            "/app/billing/checkout",
            form_body(&[("plan", "pro"), ("interval", "month"), ("csrf_token", CSRF)]),
        )
        .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers()[LOCATION],
        "https://checkout.stripe.test/session"
    );
    let checkouts = stripe.with(|state| std::mem::take(&mut state.checkouts));
    assert_eq!(checkouts[0].seats, 2);
    assert_eq!(checkouts[0].price, "price_pro_month");
    assert_eq!(checkouts[0].team, team.id.to_string());
}

#[tokio::test]
async fn only_the_owner_reaches_checkout_and_never_without_the_token() {
    let stripe = FakeStripe::default();
    let harness = billed_harness(&stripe).await;
    let (ada, team) = person(&harness.db, "Ada", "ada@example.com").await;
    let (grace, _) = person(&harness.db, "Grace", "grace@example.com").await;
    seat(&harness.db, &grace, &team, TeamRole::Admin).await;
    let form = |token| {
        form_body(&[
            ("plan", "pro"),
            ("interval", "month"),
            ("csrf_token", token),
        ])
    };

    let admin = client_for(&harness, &grace, &team).await;
    let response = admin
        .csrf(CSRF)
        .post_form("/app/billing/checkout", form(CSRF))
        .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let owner = client_for(&harness, &ada, &team).await;
    let response = owner
        .csrf(CSRF)
        .post_form(
            "/app/billing/checkout",
            form("00000000-0000-4000-8000-000000000000"),
        )
        .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(stripe.with(|state| state.checkouts.len()), 0);

    let stranger = TestClient::new(&harness.app);
    let response = stranger
        .csrf(CSRF)
        .post_form("/app/billing/checkout", form(CSRF))
        .await;
    assert_ne!(response.status(), StatusCode::OK);
    assert_eq!(stripe.with(|state| state.checkouts.len()), 0);
}

#[tokio::test]
async fn an_unverified_account_does_not_reach_checkout() {
    let stripe = FakeStripe::default();
    let harness = billed_harness(&stripe).await;
    let (ada, team) = person(&harness.db, "Ada", "ada@example.com").await;
    User::filter_by_id(ada.id)
        .update()
        .email_verified_at(None)
        .exec(&mut harness.db.clone())
        .await
        .expect("unverify");
    let unverified = User::get_by_id(&mut harness.db.clone(), ada.id)
        .await
        .expect("query");

    let client = client_for(&harness, &unverified, &team).await;
    let response = client
        .csrf(CSRF)
        .post_form(
            "/app/billing/checkout",
            form_body(&[("plan", "pro"), ("interval", "month"), ("csrf_token", CSRF)]),
        )
        .await;
    assert_ne!(
        response
            .headers()
            .get(LOCATION)
            .map(|value| value.to_str().ok()),
        Some(Some("https://checkout.stripe.test/session"))
    );
    assert_eq!(stripe.with(|state| state.checkouts.len()), 0);
}

#[tokio::test]
async fn a_running_subscription_is_not_bought_twice() {
    let stripe = FakeStripe::default();
    let mut harness = billed_harness(&stripe).await;
    let (ada, team) = person(&harness.db, "Ada", "ada@example.com").await;
    account(&harness, &team, "cus_1").await;
    stripe.subscribe("cus_1", "active", "pro_month", 1);
    signed(&harness, &event("evt_1", "invoice.paid", "cus_1")).await;
    harness.run_jobs().await;

    let client = client_for(&harness, &ada, &team).await;
    let response = client
        .csrf(CSRF)
        .post_form(
            "/app/billing/checkout",
            form_body(&[
                ("plan", "business"),
                ("interval", "year"),
                ("csrf_token", CSRF),
            ]),
        )
        .await;
    assert_eq!(response.headers()[LOCATION], "/app/billing");
    assert_eq!(stripe.with(|state| state.checkouts.len()), 0);
}

#[tokio::test]
async fn the_billing_page_shows_the_plan_after_the_return_from_checkout() {
    let stripe = FakeStripe::default();
    let harness = billed_harness(&stripe).await;
    let (ada, team) = person(&harness.db, "Ada", "ada@example.com").await;
    account(&harness, &team, "cus_1").await;
    stripe.subscribe("cus_1", "active", "business_month", 1);

    let client = client_for(&harness, &ada, &team).await;
    let page = client.get("/app/billing?checkout=success").await;
    assert_eq!(page.status(), StatusCode::OK);
    let html = body_string(page).await;
    assert!(html.contains("Business"), "{html}");
    assert_eq!(mirror(&harness, "cus_1").await.plan, PlanId::Business);
}

#[tokio::test]
async fn a_plain_member_does_not_open_the_billing_page() {
    let stripe = FakeStripe::default();
    let harness = billed_harness(&stripe).await;
    let (_, team) = person(&harness.db, "Ada", "ada@example.com").await;
    let (grace, _) = person(&harness.db, "Grace", "grace@example.com").await;
    seat(&harness.db, &grace, &team, TeamRole::Member).await;
    let client = client_for(&harness, &grace, &team).await;
    assert_eq!(
        client.get("/app/billing").await.status(),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn removing_a_member_updates_the_subscription_quantity() {
    let stripe = FakeStripe::default();
    let mut harness = billed_harness(&stripe).await;
    let (ada, team) = person(&harness.db, "Ada", "ada@example.com").await;
    let (grace, _) = person(&harness.db, "Grace", "grace@example.com").await;
    let grace_seat = seat(&harness.db, &grace, &team, TeamRole::Member).await;
    account(&harness, &team, "cus_1").await;
    stripe.subscribe("cus_1", "active", "pro_month", 2);
    signed(&harness, &event("evt_1", "invoice.paid", "cus_1")).await;
    harness.run_jobs().await;

    let client = client_for(&harness, &ada, &team).await;
    let reply = client
        .resource::<MemberResource>()
        .action::<RemoveMember>(&grace_seat)
        .submit()
        .await;
    assert!(reply.status().is_redirection(), "{}", reply.status());
    assert!(
        queued(&harness).await >= 1,
        "the change queues, debounced, in the removal's transaction"
    );

    enqueue(&SyncSeats { team: team.id })
        .exec(&mut harness.db.clone())
        .await
        .expect("enqueue");
    harness.run_jobs().await;
    let changes = stripe.with(|state| std::mem::take(&mut state.seat_changes));
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].seats, 1);
    assert_eq!(mirror(&harness, "cus_1").await.seats, Some(1));
}

#[tokio::test]
async fn reconciliation_catches_an_event_that_never_arrived() {
    let stripe = FakeStripe::default();
    let mut harness = billed_harness(&stripe).await;
    let (_, team) = person(&harness.db, "Ada", "ada@example.com").await;
    account(&harness, &team, "cus_1").await;
    stripe.subscribe("cus_1", "active", "pro_month", 1);
    signed(&harness, &event("evt_1", "invoice.paid", "cus_1")).await;
    harness.run_jobs().await;

    stripe.subscribe("cus_1", "canceled", "pro_month", 1);
    enqueue(&ReconcileBilling)
        .exec(&mut harness.db.clone())
        .await
        .expect("enqueue");
    harness.run_jobs().await;
    assert_eq!(
        mirror(&harness, "cus_1").await.status,
        BillingStatus::Canceled
    );
}

#[tokio::test]
async fn an_invitation_stops_at_the_free_plan_seat_limit() {
    let stripe = FakeStripe::default();
    let harness = billed_harness(&stripe).await;
    let (ada, team) = person(&harness.db, "Ada", "ada@example.com").await;
    let (grace, _) = person(&harness.db, "Grace", "grace@example.com").await;
    seat(&harness.db, &grace, &team, TeamRole::Member).await;
    let client = client_for(&harness, &ada, &team).await;
    for email in ["a@example.com", "b@example.com"] {
        client
            .csrf(CSRF)
            .post_form(
                "/app/members/-/actions/invite",
                form_body(&[("email", email), ("role", "member"), ("csrf_token", CSRF)]),
            )
            .await;
    }
    let invitations = tablo_saas::teams::Invitation::all()
        .exec(&mut harness.db.clone())
        .await
        .expect("query");
    assert_eq!(
        invitations.len(),
        1,
        "two members and one pending fill three seats"
    );
}

#[tokio::test]
async fn the_panel_policy_lets_the_billing_forms_land_on_stripe() {
    let stripe = FakeStripe::default();
    let harness = billed_harness(&stripe).await;
    let (ada, team) = person(&harness.db, "Ada", "ada@example.com").await;
    let client = client_for(&harness, &ada, &team).await;
    let response = client.get("/app/billing").await;
    let policy = response.headers()["content-security-policy"]
        .to_str()
        .expect("ascii");
    assert!(policy.contains("https://checkout.stripe.com"), "{policy}");
    assert!(policy.contains("https://billing.stripe.com"), "{policy}");
}
