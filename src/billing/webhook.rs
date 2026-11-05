//! Stripe's webhook: prove it, store it, answer.

use jiff::Timestamp;
use topcoat::{
    Result,
    context::{Cx, context},
    router::{
        Body, StatusCode,
        error::{bad_request, not_found},
        request, route, to_bytes,
    },
};

use super::{
    Billing, StripeEvent,
    jobs::ProcessStripeEvent,
    stripe::{Event, verify_signature},
};
use crate::jobs::enqueue;

/// The largest event Stripe sends is far below this.
const BODY_LIMIT: usize = 1024 * 1024;

/// The events that can change what a team has or should hear. Subscribe the endpoint to these.
const HANDLED: &[&str] = &[
    "checkout.session.completed",
    "customer.subscription.created",
    "customer.subscription.updated",
    "customer.subscription.deleted",
    "customer.subscription.paused",
    "customer.subscription.resumed",
    "customer.subscription.trial_will_end",
    "invoice.paid",
    "invoice.payment_failed",
];

fn invalid() -> topcoat::Error {
    bad_request("invalid webhook").into()
}

/// Inserting the event and queueing its job commit together, so a crash between them cannot lose
/// one, and a redelivery finds the row and queues nothing. Slow work happens in the job: Stripe
/// wants an answer within seconds, and retries what it does not get.
#[route(POST "/webhooks/stripe")]
async fn stripe_webhook(cx: &Cx, body: Body) -> Result<StatusCode> {
    let billing = context::<Billing>(cx);
    let Some(secret) = billing.webhook_secret() else {
        return Err(not_found().into());
    };
    let bytes = to_bytes(body, BODY_LIMIT).await?;
    let signature = request::headers(cx)
        .get("stripe-signature")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(invalid)?;
    if let Err(error) = verify_signature(signature, &bytes, secret, Timestamp::now().as_second()) {
        tracing::warn!(%error, "refused a Stripe webhook");
        return Err(invalid());
    }
    let event: Event = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if !HANDLED.contains(&event.kind.as_str()) {
        return Ok(StatusCode::OK);
    }
    let Some(customer) = event.data.object.get("customer").and_then(|id| id.as_str()) else {
        return Ok(StatusCode::OK);
    };

    let mut handle = tablo::db::db(cx)?;
    let mut tx = handle.transaction().await?;
    let known = StripeEvent::filter_by_id(&event.id)
        .first()
        .exec(&mut tx)
        .await?;
    if known.is_none() {
        StripeEvent::upsert_by_id(&event.id)
            .on_create(|create| {
                create
                    .kind(event.kind.clone())
                    .customer_id(customer.to_string())
            })
            .or_ignore()
            .exec(&mut tx)
            .await?;
        enqueue(&ProcessStripeEvent {
            event: event.id.clone(),
            customer: customer.to_string(),
        })
        .dedupe(event.id.clone())
        .exec(&mut tx)
        .await
        .map_err(|error| topcoat::Error::msg(format!("{error:#}")))?;
    }
    tx.commit().await?;
    Ok(StatusCode::OK)
}
