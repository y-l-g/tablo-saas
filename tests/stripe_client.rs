//! The HTTP client against `stripe-mock`, which checks each request against Stripe's OpenAPI spec:
//! a parameter Stripe would refuse fails here. It runs when `STRIPE_MOCK_URL` is set, as in CI.

use tablo_saas::billing::{
    BillingInterval, Plan, PlanId, lookup_key,
    stripe::{
        Gateway, NewCheckout, NewCustomer, NewPrice, PortalProduct, SeatChange, StripeClient,
    },
};

fn client() -> Option<StripeClient> {
    let base = std::env::var("STRIPE_MOCK_URL").ok()?;
    Some(StripeClient::new("sk_test_123", &base).expect("a client"))
}

#[tokio::test]
async fn the_catalog_calls_are_ones_stripe_accepts() {
    let Some(stripe) = client() else { return };
    let pro = Plan::of(PlanId::Pro);
    assert!(
        stripe
            .product_exists("prod_x".to_string())
            .await
            .expect("get")
    );
    stripe
        .create_product("prod_x".to_string(), "Pro".to_string())
        .await
        .expect("create a product");
    stripe
        .price_by_lookup_key("pro_month".to_string())
        .await
        .expect("find a price");
    let price = stripe
        .create_price(NewPrice {
            product: "prod_x".to_string(),
            lookup_key: lookup_key(&pro, BillingInterval::Month),
            amount: pro.monthly_amount,
            currency: "eur",
            interval: BillingInterval::Month,
        })
        .await
        .expect("create a price");
    stripe
        .configure_portal(vec![PortalProduct {
            product: "prod_x".to_string(),
            prices: vec![price.id],
        }])
        .await
        .expect("configure the portal");
}

#[tokio::test]
async fn the_subscription_calls_are_ones_stripe_accepts() {
    let Some(stripe) = client() else { return };
    let customer = stripe
        .create_customer(NewCustomer {
            team: "team".to_string(),
            name: "Team".to_string(),
            email: "owner@example.com".to_string(),
        })
        .await
        .expect("create a customer");
    let url = stripe
        .create_checkout(NewCheckout {
            customer: customer.clone(),
            team: "team".to_string(),
            price: "price_x".to_string(),
            seats: 3,
            trial_days: Some(14),
            automatic_tax: true,
            success_url: "https://example.com/ok".to_string(),
            cancel_url: "https://example.com/no".to_string(),
        })
        .await
        .expect("open a checkout");
    assert!(url.starts_with("http"), "{url}");
    stripe
        .create_portal_session(customer.clone(), "https://example.com".to_string())
        .await
        .expect("open the portal");

    let subscriptions = stripe
        .customer_subscriptions(customer.clone())
        .await
        .expect("list subscriptions");
    let subscription = stripe
        .subscription("sub_x".to_string())
        .await
        .expect("read a subscription");
    assert!(!subscription.items.data.is_empty() || subscriptions.is_empty());
    stripe
        .set_seats(SeatChange {
            subscription: "sub_x".to_string(),
            item: "si_x".to_string(),
            seats: 4,
        })
        .await
        .expect("change the seats");
    stripe
        .cancel_subscription("sub_x".to_string())
        .await
        .expect("cancel");
    stripe.invoices(customer).await.expect("list invoices");
}
