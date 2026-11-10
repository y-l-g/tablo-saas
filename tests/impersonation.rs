//! Staff acting as a customer from the app panel's support page, and the back office's log of it.

mod support;

use support::{Harness, client_for, harness, person, staff_client};
use tablo::{
    AuditEntry,
    auth::{AuthFactor, FactorKind, IMPERSONATE_FIELD},
    testing::{TestClient, body_string, form_body, impersonate_buttons, impersonation_banner},
};
use tablo_saas::{
    accounts::{Accounts, StaffRole, User},
    backoffice::ImpersonationResource,
};
use topcoat::router::StatusCode;

/// A member of staff signed in to the app panel, with a second factor there when `factor` says.
async fn staff_in_app<'a>(
    harness: &'a Harness,
    email: &str,
    role: StaffRole,
    factor: bool,
) -> (User, TestClient<'a>) {
    let (user, _) = person(&harness.db, "Sam Support", email).await;
    let mut db = harness.db.clone();
    User::filter_by_id(user.id)
        .update()
        .staff_role(Some(role))
        .exec(&mut db)
        .await
        .expect("make staff");
    if factor {
        toasty::create!(AuthFactor {
            user_id: user.id.to_string(),
            panel: "/app".to_string(),
            kind: FactorKind::EmailCode,
            secret: String::new(),
            confirmed_at: Some(jiff::Timestamp::now()),
            last_step: None,
            failures: 0,
            retry_at: None,
            created_at: jiff::Timestamp::now(),
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

async fn search(client: &TestClient<'_>, term: &str) -> (StatusCode, String) {
    let page = client.get(&format!("/app/support?q={term}")).await;
    (page.status(), body_string(page).await)
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
    let (_, client) = staff_in_app(&harness, "sam@example.com", StaffRole::Support, true).await;

    let (status, html) = search(&client, "ada").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(impersonate_buttons(&html), vec![ada.id.to_string()]);
    assert!(!html.contains("data-support-notice"), "{html}");

    let started = client.submit("/app/impersonate", &act_as(&ada)).await;
    assert_eq!(started.status(), StatusCode::SEE_OTHER);
    let shown = banner(&client).await.expect("the banner shows");
    assert_eq!(shown.user_id, ada.id.to_string());
    assert!(shown.text.contains("Ada Lovelace"), "{}", shown.text);
    assert_eq!(
        client.get("/app/support").await.status(),
        StatusCode::FORBIDDEN,
        "acting as a customer, the page is the customer's to see, and they may not"
    );

    let stopped = client.submit("/app/impersonate/stop", "").await;
    assert_eq!(stopped.status(), StatusCode::SEE_OTHER);
    assert!(banner(&client).await.is_none());

    let events: Vec<String> = AuditEntry::all()
        .exec(&mut harness.db.clone())
        .await
        .expect("query")
        .into_iter()
        .filter(|entry| entry.panel == "/app")
        .map(|entry| entry.event)
        .collect();
    assert_eq!(events.len(), 2, "{events:?}");
    assert!(events.contains(&"impersonate".to_string()));
    assert!(events.contains(&"stop_impersonating".to_string()));
}

#[tokio::test]
async fn an_administrator_reads_who_acted_as_whom() {
    let harness = harness().await;
    let (ada, _) = person(&harness.db, "Ada Lovelace", "ada@example.com").await;
    let (_, client) = staff_in_app(&harness, "sam@example.com", StaffRole::Support, true).await;
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
        events.iter().any(|text| text.contains("Started")),
        "{events:?}"
    );
    assert!(
        events.iter().any(|text| text.contains("Stopped")),
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

#[tokio::test]
async fn staff_without_a_factor_on_the_app_panel_act_as_nobody() {
    let harness = harness().await;
    let (ada, _) = person(&harness.db, "Ada Lovelace", "ada@example.com").await;
    let (_, client) =
        staff_in_app(&harness, "sam@example.com", StaffRole::Administrator, false).await;

    let (status, html) = search(&client, "ada").await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("data-support-notice"), "{html}");
    assert!(html.contains("data-customer=\"ada@example.com\""), "{html}");
    assert_eq!(impersonate_buttons(&html), Vec::<String>::new());

    let refused = client.submit("/app/impersonate", &act_as(&ada)).await;
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);
    assert!(banner(&client).await.is_none());
}

#[tokio::test]
async fn staff_never_act_as_staff() {
    let harness = harness().await;
    let (other, _) =
        staff_in_app(&harness, "root@example.com", StaffRole::Administrator, true).await;
    let (_, client) = staff_in_app(&harness, "sam@example.com", StaffRole::Support, true).await;

    let (_, html) = search(&client, "root").await;
    assert!(
        !html.contains("data-customer="),
        "the search lists customers only"
    );
    let refused = client.submit("/app/impersonate", &act_as(&other)).await;
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_customer_neither_sees_the_page_nor_acts_as_anyone() {
    let harness = harness().await;
    let (ada, team) = person(&harness.db, "Ada Lovelace", "ada@example.com").await;
    let (grace, _) = person(&harness.db, "Grace Hopper", "grace@example.com").await;
    let client = client_for(&harness, &ada, &team).await;

    assert_eq!(
        client.get("/app/support").await.status(),
        StatusCode::FORBIDDEN
    );
    let refused = client.submit("/app/impersonate", &act_as(&grace)).await;
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn billing_stays_the_customers_while_staff_act_as_them() {
    let harness = harness().await;
    let (ada, _) = person(&harness.db, "Ada Lovelace", "ada@example.com").await;
    let (_, client) = staff_in_app(&harness, "sam@example.com", StaffRole::Support, true).await;
    client.submit("/app/impersonate", &act_as(&ada)).await;

    assert_eq!(
        client.get("/app/settings/billing").await.status(),
        StatusCode::OK,
        "the page reads Ada's team, which she owns"
    );
    let checkout = client
        .submit(
            "/app/settings/billing/checkout",
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
